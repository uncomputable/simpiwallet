use std::borrow::Borrow;
use std::cell::RefCell;
use std::collections::HashMap;
use std::ops::Deref;
use std::rc::Rc;
use std::str::FromStr;

use elements::bitcoin;
use elements::secp256k1_zkp;
use elements_miniscript as miniscript;
use miniscript::{
    elements, DefiniteDescriptorKey, Descriptor, DescriptorPublicKey, MiniscriptKey, Preimage32,
    Satisfier, ToPublicKey,
};

use crate::error::Error;
use crate::key::DescriptorSecretKey;
use crate::state::{State, UtxoSet};

pub const BITCOIN_ASSET_ID: &str =
    "b2e15d0d7a0c94e4e2ce0fe6e8691b9e451377f6e46e8045a86f7c4b5d4f0f23";
pub const ELEMENTS_REGTEST_GENESIS_BLOCK_HASH: &str =
    "209577bda6bf4b5804bd46f8621580dd6d4e8bfa2d190e1c50e932492baca07d";

pub fn get_balance(state: &State) -> Result<bitcoin::Amount, Error> {
    let parent_descriptor = state.descriptor();
    let utxo_set = state
        .rpc()
        .scan(parent_descriptor, state.max_child_index())?;
    dbg!(&utxo_set);
    Ok(utxo_set.total_amount())
}

pub fn send_to_address(state: &mut State, send_to: Payment) -> Result<elements::Txid, Error> {
    let change_index = state.next_index()?;
    let parent_descriptor = state.descriptor();
    let change_descriptor = parent_descriptor
        .at_derivation_index(change_index)
        .expect("valid child index");

    let utxo_set = state
        .rpc()
        .scan(parent_descriptor, state.max_child_index())?;
    let (selection, available) = utxo_set
        .select_coins(send_to.amount + state.fee())
        .ok_or(Error::NotEnoughFunds)?;

    let change = Payment {
        amount: available - send_to.amount - state.fee(), // available >= send_to.amount + fee
        address: change_descriptor
            .address(&elements::AddressParams::ELEMENTS)
            .expect("taproot address"),
    };

    let mut builder = TransactionBuilder::default();

    for input in selection.into_inputs(parent_descriptor) {
        builder.add_input(input);
    }

    builder.add_output(send_to.to_output());
    builder.add_output(change.to_output());
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
    pub fn to_output(&self) -> elements::TxOut {
        elements::TxOut {
            asset: elements::confidential::Asset::Explicit(
                elements::AssetId::from_str(BITCOIN_ASSET_ID).expect("const"),
            ),
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

    pub fn into_inputs(self, parent_descriptor: &Descriptor<DescriptorPublicKey>) -> Vec<Input> {
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
                asset: elements::confidential::Asset::Explicit(
                    elements::AssetId::from_str(BITCOIN_ASSET_ID).expect("const"),
                ),
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

#[derive(Default)]
struct TransactionBuilder {
    inputs: Vec<elements::TxIn>,
    desc_indices: Vec<u32>,
    prevouts: Vec<elements::TxOut>,
    outputs: Vec<elements::TxOut>,
}

impl TransactionBuilder {
    pub fn add_input(&mut self, input: Input) {
        self.inputs.push(input.input);
        self.desc_indices.push(input.index);
        self.prevouts.push(input.prevout);
    }

    pub fn add_output(&mut self, output: elements::TxOut) {
        self.outputs.push(output);
    }

    pub fn add_fee(&mut self, amount: bitcoin::Amount) {
        let output = elements::TxOut::new_fee(
            amount.to_sat(),
            elements::AssetId::from_str(BITCOIN_ASSET_ID).expect("const"),
        );
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
            let child_descriptor = parent_descriptor.at_derivation_index(desc_index).expect("valid child index");
            let (script_cmr, control_block) = get_cmr_control_block(&child_descriptor)?;

            let satisfier = DynamicSigner {
                keymap,
                max_key_index,
                input_index: txin_index,
                prevouts: elements::sighash::Prevouts::All(&self.prevouts),
                locktime: tx.lock_time,
                sequence: tx.input[txin_index].sequence,
                script_cmr,
                control_block,
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

        for (index, witness) in witnesses.into_iter().enumerate() {
            tx.input[index].witness = witness;
        }

        Some(tx)
    }
}

fn get_cmr_control_block(
    descriptor: &Descriptor<DefiniteDescriptorKey>,
) -> Option<(simplicity::Cmr, elements::taproot::ControlBlock)> {
    if let Descriptor::Tr(tr) = descriptor {
        let policy = tr.get_simplicity()?;
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
                let child_public_key = desc_pk.clone().at_derivation_index(index).expect("valid child index");
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
                elements::BlockHash::from_str(ELEMENTS_REGTEST_GENESIS_BLOCK_HASH).expect("const"),
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