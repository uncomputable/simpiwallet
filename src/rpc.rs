use std::fmt;

use bitcoin::key::PublicKey;
use elements::bitcoin;
use elements_miniscript as miniscript;
use jsonrpc::simple_http::SimpleHttpTransport;
use jsonrpc::{simple_http, Client};
use miniscript::elements::hex::ToHex;
use miniscript::{elements, Descriptor};

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
        descriptors: &[Descriptor<PublicKey>],
    ) -> Result<ScanTxOutResult, Error> {
        let action = serde_json::Value::String("start".to_string());

        let descriptors: Vec<_> = descriptors
            .iter()
            .map(|desc| desc.script_pubkey().as_bytes().to_hex())
            .map(|hex| format!("raw({})", hex))
            .map(serde_json::Value::String)
            .collect();
        let descriptors = serde_json::Value::Array(descriptors);

        let parameters = [jsonrpc::arg(action), jsonrpc::arg(descriptors)];

        let client = self.client()?;
        let request = client.build_request("scantxoutset", &parameters);
        let response = client.send_request(request)?;

        response.result().map_err(|e| e.into())
    }

    pub fn scan(&self, descriptors: &[Descriptor<PublicKey>]) -> Result<UtxoSet, Error> {
        let result = self.scantxoutset(descriptors)?;
        let mut utxos = Vec::new();

        for unspent in result.unspents {
            let descriptor = descriptors
                .iter()
                .find(|desc| desc.script_pubkey() == unspent.script_pub_key)
                .expect("Output script_pubkey was queried for")
                .clone();
            let utxo = Utxo {
                descriptor,
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
