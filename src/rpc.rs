use std::fmt;

use elements::bitcoin;
use elements_miniscript as miniscript;
use jsonrpc::simple_http::SimpleHttpTransport;
use jsonrpc::{simple_http, Client};
use miniscript::elements::hex::ToHex;
use miniscript::{elements, DefiniteDescriptorKey, Descriptor, DescriptorPublicKey};

use crate::error::Error;
use crate::state::{Utxo, UtxoSet};

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct Connection {
    pub url: String,
    pub user: String,
    pub pass: Option<String>,
}

impl fmt::Display for Connection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}@{}", self.user, self.url)?;
        if let Some(pass) = &self.pass {
            write!(f, " with password {}", "*".repeat(pass.len()))?;
        }
        Ok(())
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
struct ScanTxOutResult {
    pub bestblock: elements::BlockHash,
    pub height: u64,
    pub success: bool,
    #[serde(with = "bitcoin::amount::serde::as_btc")]
    pub total_unblinded_bitcoin_amount: bitcoin::amount::Amount,
    pub txouts: u64,
    pub unspents: Vec<Unspents>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
struct Unspents {
    #[serde(with = "bitcoin::amount::serde::as_btc")]
    pub amount: bitcoin::amount::Amount,
    pub asset: elements::AssetId,
    pub desc: String,
    pub height: u64,
    pub script_pub_key: elements::Script,
    pub txid: elements::Txid,
    pub vout: u32,
}

impl Default for Connection {
    fn default() -> Self {
        Self {
            url: "localhost:18443".to_string(),
            user: "user".to_string(),
            pass: Some("pass".to_string()),
        }
    }
}

impl Connection {
    fn client(&self) -> Result<Client, simple_http::Error> {
        let t = SimpleHttpTransport::builder()
            .url(&self.url)?
            .auth(&self.user, self.pass.as_ref())
            .build();

        Ok(Client::with_transport(t))
    }

    fn scantxoutset(
        &self,
        child_descriptors: &[Descriptor<DefiniteDescriptorKey>],
        address_params: &'static elements::AddressParams,
    ) -> Result<ScanTxOutResult, Error> {
        let action = serde_json::Value::String("start".to_string());

        let addresses: Vec<_> = child_descriptors
            .iter()
            .map(|d| d.address(address_params).expect("taproot address"))
            .map(|a| format!("addr({})", a))
            .map(serde_json::Value::String)
            .collect();
        let addresses = serde_json::Value::Array(addresses);

        let parameters = [jsonrpc::arg(action), jsonrpc::arg(addresses)];

        let client = self.client()?;
        let request = client.build_request("scantxoutset", &parameters);
        let response = client.send_request(request)?;

        response.result().map_err(|e| e.into())
    }

    pub fn scan(
        &self,
        parent_descriptor: &Descriptor<DescriptorPublicKey>,
        max_child_index: u32,
        address_params: &'static elements::AddressParams,
    ) -> Result<UtxoSet, Error> {
        let child_descriptors: Vec<_> = (0..max_child_index)
            .map(|i| {
                parent_descriptor
                    .at_derivation_index(i)
                    .expect("valid child index")
            })
            .collect();
        let result = self.scantxoutset(&child_descriptors, address_params)?;
        let mut utxos = Vec::new();

        for unspent in result.unspents {
            let index = child_descriptors
                .iter()
                .position(|d| d.script_pubkey() == unspent.script_pub_key)
                .expect("valid child index");
            let utxo = Utxo {
                index: index as u32, // safe cast because there are only u32 many child descriptors
                amount: unspent.amount,
                outpoint: elements::OutPoint {
                    txid: unspent.txid,
                    vout: unspent.vout,
                },
            };
            utxos.push(utxo);
        }

        Ok(UtxoSet(utxos))
    }

    pub fn sendrawtransaction(&self, tx: &elements::Transaction) -> Result<elements::Txid, Error> {
        let hex =
            serde_json::Value::String(elements::pset::serialize::Serialize::serialize(tx).to_hex());
        let parameters = [jsonrpc::arg(hex)];

        let client = self.client()?;
        let request = client.build_request("sendrawtransaction", &parameters);
        let response = client.send_request(request)?;

        response.result().map_err(|e| e.into())
    }
}
