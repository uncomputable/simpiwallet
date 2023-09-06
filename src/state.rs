use std::collections::{HashMap, HashSet};
use std::fs::{File, OpenOptions};
use std::io::{BufReader, BufWriter};
use std::path::Path;

use elements::bitcoin;
use elements::secp256k1_zkp;
use elements_miniscript as miniscript;
use miniscript::{elements, Descriptor, DescriptorPublicKey};
use serde::{Deserialize, Serialize};

use crate::descriptor;
use crate::error::Error;
use crate::key::DescriptorSecretKey;
use crate::network::Network;
use crate::rpc::Connection;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct State {
    keymap: HashMap<DescriptorPublicKey, DescriptorSecretKey>,
    descriptor: Descriptor<DescriptorPublicKey>,
    next_index: u32,
    seen_cmrs: HashSet<[u8; 32]>,
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
            seen_cmrs: HashSet::new(),
            fee: bitcoin::Amount::from_sat(1000),
            rpc: Connection::default(),
            network: Network::Testnet,
        }
    }

    pub fn keymap(&self) -> &HashMap<DescriptorPublicKey, DescriptorSecretKey> {
        &self.keymap
    }

    pub fn descriptor(&self) -> &Descriptor<DescriptorPublicKey> {
        &self.descriptor
    }

    pub fn next_index(&mut self) -> Result<u32, Error> {
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

    pub fn max_child_index(&self) -> u32 {
        self.next_index
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

    pub fn seen_cmr(&self, cmr: &simplicity::Cmr) -> bool {
        self.seen_cmrs.contains(cmr.as_ref())
    }

    pub fn add_cmr(&mut self, cmr: simplicity::Cmr) {
        self.seen_cmrs.insert(cmr.to_byte_array());
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
    pub index: u32,
    pub amount: bitcoin::amount::Amount,
    pub outpoint: elements::OutPoint,
}

#[derive(Clone, Debug)]
pub struct UtxoSet(pub Vec<Utxo>);
