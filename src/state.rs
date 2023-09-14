use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{BufReader, BufWriter};
use std::path::Path;

use bitcoin::key::PublicKey;
use elements::{bitcoin, secp256k1_zkp};
use elements_miniscript as miniscript;
use elements_miniscript::TranslatePk;
use miniscript::{elements, Descriptor, DescriptorPublicKey};
use serde::{Deserialize, Serialize};

use crate::descriptor;
use crate::descriptor::AssemblySet;
use crate::error::Error;
use crate::key::{DescriptorSecretKey, ToEvenY};
use crate::network::Network;
use crate::rpc::Connection;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct State {
    keymap: HashMap<DescriptorPublicKey, DescriptorSecretKey>,
    descriptor: Descriptor<DescriptorPublicKey>,
    next_index: u32,
    assembly: AssemblySet,
    #[serde(with = "bitcoin::amount::serde::as_sat")]
    fee: bitcoin::Amount,
    rpc: Connection,
    network: Network,
}

impl State {
    pub fn new(xpriv: DescriptorSecretKey) -> Self {
        let xpub = xpriv.0.to_public(secp256k1_zkp::SECP256K1).expect("xpriv");
        let descriptor = descriptor::simplicity_pk(xpub.clone());
        let mut keymap = HashMap::new();
        keymap.insert(xpub, xpriv);

        Self {
            keymap,
            descriptor,
            next_index: 0,
            assembly: AssemblySet::default(),
            fee: bitcoin::Amount::from_sat(1000),
            rpc: Connection::default(),
            network: Network::Testnet,
        }
    }

    fn next_index(&mut self) -> Result<u32, Error> {
        if self.next_index & (1 << 31) == 0 {
            let index = self.next_index;
            self.next_index += 1;
            Ok(index)
        } else {
            Err(Error::Bip32(bitcoin::bip32::Error::InvalidChildNumber(
                self.next_index,
            )))
        }
    }

    pub fn next_child_descriptor(&mut self) -> Result<Descriptor<PublicKey>, Error> {
        let i = self.next_index()?;
        Ok(self
            .descriptor
            .derived_descriptor(secp256k1_zkp::SECP256K1, i)
            .expect("good xpub")
            .translate_pk(&mut ToEvenY)
            .expect("never fails"))
    }

    pub fn child_descriptors(&self) -> impl Iterator<Item = Descriptor<PublicKey>> + '_ {
        (0..self.next_index).map(|i| {
            self.descriptor
                .derived_descriptor(secp256k1_zkp::SECP256K1, i)
                .expect("good xpub")
                .translate_pk(&mut ToEvenY)
                .expect("never fails")
        })
    }

    pub fn get_keypair(&self, key: &PublicKey) -> Option<elements::schnorr::KeyPair> {
        for parent_sk in self.keymap.values() {
            // TODO: Update once there is support for multiple descriptors
            for index in 0..self.next_index {
                let child_sk = parent_sk
                    .clone()
                    .at_derivation_index(index)
                    .ok()?
                    .to_private_key()
                    .inner;
                if child_sk.public_key(secp256k1_zkp::SECP256K1) == key.inner {
                    let keypair = elements::schnorr::KeyPair::from_secret_key(
                        secp256k1_zkp::SECP256K1,
                        &child_sk,
                    );
                    return Some(keypair);
                }
                // Case where public key P with odd y-coordinate was converted
                // into public key -P with even y-coordinate:
                // P = xG and -P = (-x)G for the generator G
                if child_sk.negate().public_key(secp256k1_zkp::SECP256K1) == key.inner {
                    let keypair = elements::schnorr::KeyPair::from_secret_key(
                        secp256k1_zkp::SECP256K1,
                        &child_sk.negate(),
                    );
                    return Some(keypair);
                }
            }
        }

        None
    }

    pub fn next_address(&mut self) -> Result<elements::Address, Error> {
        let index = self.next_index()?;
        let child = self
            .descriptor
            .at_derivation_index(index)
            .expect("valid child index");
        let address = child
            .address(self.network.address_params())
            .expect("taproot address");
        Ok(address)
    }

    pub fn assembly(&self) -> &AssemblySet {
        &self.assembly
    }

    pub fn assembly_mut(&mut self) -> &mut AssemblySet {
        &mut self.assembly
    }

    pub fn fee(&self) -> bitcoin::Amount {
        self.fee
    }

    pub fn set_fee(&mut self, fee: bitcoin::Amount) {
        self.fee = fee;
    }

    pub fn rpc(&self) -> &Connection {
        &self.rpc
    }

    pub fn set_rpc(&mut self, rpc: Connection) {
        self.rpc = rpc;
    }

    pub fn network(&self) -> Network {
        self.network
    }

    pub fn set_network(&mut self, network: Network) {
        self.network = network;
    }

    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self, Error> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        let state = serde_json::from_reader(reader)?;
        Ok(state)
    }

    pub fn save<P: AsRef<Path>>(&self, path: P, init: bool) -> Result<(), Error> {
        let file = OpenOptions::new()
            .write(true)
            .truncate(true)
            .create_new(init)
            .open(path)?;
        let writer = BufWriter::new(file);
        serde_json::to_writer_pretty(writer, self)?;
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct Utxo {
    pub descriptor: Descriptor<PublicKey>,
    pub amount: bitcoin::amount::Amount,
    pub outpoint: elements::OutPoint,
}

#[derive(Clone, Debug)]
pub struct UtxoSet(pub Vec<Utxo>);
