use std::borrow::Borrow;
use std::cell::RefCell;
use std::collections::HashMap;
use std::ops::Deref;
use std::rc::Rc;

use elements::bitcoin;
use elements::secp256k1_zkp;
use elements_miniscript as miniscript;
use elements_miniscript::descriptor::TapTree;
use miniscript::{
    elements, DefiniteDescriptorKey, Descriptor, DescriptorPublicKey, MiniscriptKey, Preimage32,
    Satisfier, ToPublicKey,
};

use crate::error::Error;
use crate::key::DescriptorSecretKey;
use crate::network::Network;
use crate::state::{State, UtxoSet};

pub fn get_balance(state: &State) -> Result<bitcoin::Amount, Error> {
    let parent_descriptor = state.descriptor();
    let utxo_set = state.rpc().scan(
        parent_descriptor,
        state.max_child_index(),
        state.network().address_params(),
    )?;
    dbg!(&utxo_set);
    Ok(utxo_set.total_amount())
}

pub fn send_to_address(state: &mut State, send_to: Payment) -> Result<elements::Txid, Error> {
    let change_index = state.next_index()?;
    let parent_descriptor = state.descriptor();
    let change_descriptor = parent_descriptor
        .at_derivation_index(change_index)
        .expect("valid child index");

    let utxo_set = state.rpc().scan(
        parent_descriptor,
        state.max_child_index(),
        state.network().address_params(),
    )?;
    let (selection, available) = utxo_set
        .select_coins(send_to.amount + state.fee())
        .ok_or(Error::NotEnoughFunds)?;

    let change = Payment {
        amount: available - send_to.amount - state.fee(), // available >= send_to.amount + fee
        address: change_descriptor
            .address(state.network().address_params())
            .expect("taproot address"),
    };

    let mut builder = TransactionBuilder::new(state.network());

    for input in selection.into_inputs(parent_descriptor, state.network().bitcoin_id()) {
        builder.add_input(input);
    }

    builder.add_output(send_to.to_output(state.network().bitcoin_id()));
    builder.add_output(change.to_output(state.network().bitcoin_id()));
    builder.add_fee(state.fee());

    let tx = builder
        .sign(parent_descriptor, state.keymap(), state.max_child_index())
        .ok_or(Error::CouldNotSatisfy)?;
    let txid = state.rpc().sendrawtransaction(&tx)?;
    Ok(txid)
}

#[derive(Clone, Debug)]
pub struct Payment {
    pub amount: bitcoin::Amount,
    pub address: elements::Address,
}

impl Payment {
    pub fn to_output(&self, bitcoin_id: elements::AssetId) -> elements::TxOut {
        elements::TxOut {
            asset: elements::confidential::Asset::Explicit(bitcoin_id),
            value: elements::confidential::Value::Explicit(self.amount.to_sat()),
            nonce: elements::confidential::Nonce::Null,
            script_pubkey: self.address.script_pubkey(),
            witness: elements::TxOutWitness::default(),
        }
    }
}

impl UtxoSet {
    pub fn select_coins(&self, amount: bitcoin::Amount) -> Option<(Self, bitcoin::Amount)> {
        let mut selected_amount = bitcoin::Amount::ZERO;
        let mut selected_utxos = vec![];

        for utxo in &self.0 {
            if selected_amount >= amount {
                break;
            }

            selected_utxos.push(utxo.clone());
            selected_amount += utxo.amount;
        }

        if selected_amount < amount {
            None
        } else {
            Some((Self(selected_utxos), selected_amount))
        }
    }

    pub fn total_amount(&self) -> bitcoin::Amount {
        self.0.iter().map(|u| u.amount).sum()
    }

    pub fn into_inputs(
        self,
        parent_descriptor: &Descriptor<DescriptorPublicKey>,
        bitcoin_id: elements::AssetId,
    ) -> Vec<Input> {
        let mut inputs = Vec::with_capacity(self.0.len());

        for utxo in self.0 {
            let input = elements::TxIn {
                previous_output: utxo.outpoint,
                is_pegin: false,
                script_sig: elements::Script::new(),
                sequence: elements::Sequence::MAX,
                asset_issuance: elements::AssetIssuance::default(),
                witness: elements::TxInWitness::default(),
            };
            let child_descriptor = parent_descriptor
                .at_derivation_index(utxo.index)
                .expect("xpub with wildcard");
            let prevout = elements::TxOut {
                asset: elements::confidential::Asset::Explicit(bitcoin_id),
                value: elements::confidential::Value::Explicit(utxo.amount.to_sat()),
                nonce: elements::confidential::Nonce::Null,
                script_pubkey: child_descriptor.script_pubkey(),
                witness: elements::TxOutWitness::default(),
            };
            inputs.push(Input {
                index: utxo.index,
                input,
                prevout,
            });
        }

        inputs
    }
}

#[derive(Clone, Debug)]
pub struct Input {
    pub index: u32,
    pub input: elements::TxIn,
    pub prevout: elements::TxOut,
}

struct TransactionBuilder {
    inputs: Vec<elements::TxIn>,
    desc_indices: Vec<u32>,
    prevouts: Vec<elements::TxOut>,
    outputs: Vec<elements::TxOut>,
    network: Network,
}

impl TransactionBuilder {
    pub fn new(network: Network) -> Self {
        Self {
            inputs: vec![],
            desc_indices: vec![],
            prevouts: vec![],
            outputs: vec![],
            network,
        }
    }

    pub fn add_input(&mut self, input: Input) {
        self.inputs.push(input.input);
        self.desc_indices.push(input.index);
        self.prevouts.push(input.prevout);
    }

    pub fn add_output(&mut self, output: elements::TxOut) {
        self.outputs.push(output);
    }

    pub fn add_fee(&mut self, amount: bitcoin::Amount) {
        let output = elements::TxOut::new_fee(amount.to_sat(), self.network.bitcoin_id());
        self.outputs.push(output);
    }

    fn to_transaction(&self) -> elements::Transaction {
        elements::Transaction {
            version: 2,
            lock_time: elements::LockTime::ZERO,
            input: self.inputs.clone(),
            output: self.outputs.clone(),
        }
    }

    pub fn sign(
        &self,
        parent_descriptor: &Descriptor<DescriptorPublicKey>,
        keymap: &HashMap<DescriptorPublicKey, DescriptorSecretKey>,
        max_key_index: u32,
    ) -> Option<elements::Transaction> {
        let mut tx = self.to_transaction();
        let cache = Rc::new(RefCell::new(simplicity::sighash::SighashCache::new(&tx)));
        let mut witnesses = Vec::with_capacity(self.inputs.len());

        for (txin_index, desc_index) in self.desc_indices.iter().copied().enumerate() {
            let child_descriptor = parent_descriptor
                .at_derivation_index(desc_index)
                .expect("valid child index");
            let (script_cmr, control_block) = get_taproot_info(&child_descriptor)?;

            let satisfier = DynamicSigner {
                keymap,
                max_key_index,
                input_index: txin_index,
                prevouts: elements::sighash::Prevouts::All(&self.prevouts),
                locktime: tx.lock_time,
                sequence: tx.input[txin_index].sequence,
                script_cmr,
                control_block,
                genesis_hash: self.network.genesis_hash(),
                cache: cache.clone(),
            };

            let (script_witness, script_sig) = child_descriptor.get_satisfaction(satisfier).ok()?;
            assert!(
                script_sig.is_empty(),
                "No support for pre-segwit descriptors"
            );
            witnesses.push(elements::TxInWitness {
                amount_rangeproof: None,
                inflation_keys_rangeproof: None,
                script_witness,
                pegin_witness: vec![],
            });
        }

        // In the first loop we could not mutate tx because it is borrowed by the sighash cache
        // Add the witness to each input in a second loop
        for (txin_index, witness) in witnesses.into_iter().enumerate() {
            tx.input[txin_index].witness = witness;
        }

        Some(tx)
    }
}

fn get_taproot_info(
    descriptor: &Descriptor<DefiniteDescriptorKey>,
) -> Option<(simplicity::Cmr, elements::taproot::ControlBlock)> {
    if let Descriptor::Tr(tr) = descriptor {
        if let TapTree::SimplicityLeaf(policy) = tr.taptree().as_ref()? {
            let commit = policy.serialize_no_witness();
            let cmr = commit.cmr();

            let script = elements::Script::from(cmr.as_ref().to_vec());
            let script_ver = (script, simplicity::leaf_version());
            let control_block = tr
                .spend_info()
                .control_block(&script_ver)
                .expect("Control block must exist in script map for every known leaf");

            Some((cmr, control_block))
        } else {
            None
        }
    } else {
        None
    }
}

struct DynamicSigner<'a, T, O>
where
    T: Deref<Target = elements::Transaction> + Clone,
    O: Borrow<elements::TxOut>,
{
    // Key variables
    keymap: &'a HashMap<DescriptorPublicKey, DescriptorSecretKey>,
    max_key_index: u32,
    // Transaction variables
    input_index: usize,
    prevouts: elements::sighash::Prevouts<'a, O>,
    locktime: elements::LockTime,
    sequence: elements::Sequence,
    // Taproot variables
    script_cmr: simplicity::Cmr,
    control_block: elements::taproot::ControlBlock,
    genesis_hash: elements::BlockHash,
    // Use Rc<RefCell<_>> because Satisfier methods take &self while we need internal mutability
    cache: Rc<RefCell<simplicity::sighash::SighashCache<T>>>,
}

impl<'a, T, O> DynamicSigner<'a, T, O>
where
    T: Deref<Target = elements::Transaction> + Clone,
    O: Borrow<elements::TxOut>,
{
    fn get_keypair(&self, pk: bitcoin::PublicKey) -> Option<elements::schnorr::KeyPair> {
        for (desc_pk, desc_sk) in self.keymap {
            // TODO: Update once there is support for multiple descriptors
            for index in 0..self.max_key_index {
                let child_public_key = desc_pk
                    .clone()
                    .at_derivation_index(index)
                    .expect("valid child index");
                if child_public_key.to_public_key() == pk {
                    let child_secret_key = desc_sk.clone().at_derivation_index(index).ok()?;
                    let keypair = elements::schnorr::KeyPair::from_secret_key(
                        secp256k1_zkp::SECP256K1,
                        &child_secret_key.to_private_key().inner,
                    );
                    return Some(keypair);
                }
            }
        }

        None
    }

    fn get_signature(
        &self,
        sighash: &[u8],
        keypair: &elements::schnorr::KeyPair,
    ) -> elements::SchnorrSig {
        let msg = secp256k1_zkp::Message::from_slice(sighash).expect("32-byte sighash");
        let sig = keypair.sign_schnorr(msg);

        elements::SchnorrSig {
            sig,
            hash_ty: elements::sighash::SchnorrSigHashType::All,
        }
    }
}

impl<'a, Pk, T, O> Satisfier<Pk> for DynamicSigner<'a, T, O>
where
    Pk: MiniscriptKey + ToPublicKey,
    T: Deref<Target = elements::Transaction> + Clone,
    O: Borrow<elements::TxOut>,
{
    fn lookup_tap_key_spend_sig(&self) -> Option<elements::SchnorrSig> {
        let internal_key = self.control_block.internal_key;
        let keypair = self.get_keypair(internal_key.to_public_key())?;
        let sighash = self
            .cache
            .borrow_mut()
            .taproot_key_spend_signature_hash(
                self.input_index,
                &self.prevouts,
                elements::sighash::SchnorrSigHashType::All,
                self.genesis_hash,
            )
            .ok()?;

        let signature = self.get_signature(sighash.as_ref(), &keypair);
        Some(signature)
    }

    fn lookup_tap_leaf_script_sig(
        &self,
        pk: &Pk,
        _leaf_hash: &elements::taproot::TapLeafHash,
    ) -> Option<elements::SchnorrSig> {
        let keypair = self.get_keypair(pk.to_public_key())?;
        let sighash = self
            .cache
            .borrow_mut()
            .simplicity_spend_signature_hash(
                self.input_index,
                &self.prevouts,
                self.script_cmr,
                self.control_block.clone(),
                self.genesis_hash,
            )
            .ok()?;

        let signature = self.get_signature(sighash.as_ref(), &keypair);
        Some(signature)
    }

    fn lookup_sha256(&self, _image: &Pk::Sha256) -> Option<Preimage32> {
        None
    }

    fn check_older(&self, sequence: elements::Sequence) -> bool {
        Satisfier::<Pk>::check_older(&self.sequence, sequence)
    }

    fn check_after(&self, locktime: elements::LockTime) -> bool {
        Satisfier::<Pk>::check_after(&self.locktime, locktime)
    }
}
