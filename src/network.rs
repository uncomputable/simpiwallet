use std::fmt;
use std::str::FromStr;

use elements_miniscript as miniscript;
use miniscript::bitcoin::hashes::{sha256, Hash};
use miniscript::elements;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
pub enum Network {
    Regtest,
    Testnet,
}

const REGTEST_BITCOIN_ID: [u8; 32] = [
    0x23, 0x0f, 0x4f, 0x5d, 0x4b, 0x7c, 0x6f, 0xa8, 0x45, 0x80, 0x6e, 0xe4, 0xf6, 0x77, 0x13, 0x45,
    0x9e, 0x1b, 0x69, 0xe8, 0xe6, 0x0f, 0xce, 0xe2, 0xe4, 0x94, 0x0c, 0x7a, 0x0d, 0x5d, 0xe1, 0xb2,
];

const REGTEST_GENESIS_HASH: [u8; 32] = [
    0x7d, 0xa0, 0xac, 0x2b, 0x49, 0x32, 0xe9, 0x50, 0x1c, 0x0e, 0x19, 0x2d, 0xfa, 0x8b, 0x4e, 0x6d,
    0xdd, 0x80, 0x15, 0x62, 0xf8, 0x46, 0xbd, 0x04, 0x58, 0x4b, 0xbf, 0xa6, 0xbd, 0x77, 0x95, 0x20,
];

const TESTNET_ADDRESS_PARAMS: elements::AddressParams = elements::AddressParams {
    p2pkh_prefix: 235,
    p2sh_prefix: 75,
    blinded_prefix: 4,
    bech_hrp: "tex",
    blech_hrp: "tlq",
};

const TESTNET_BITCOIN_ID: [u8; 32] = [
    0xe8, 0x0b, 0x7c, 0x83, 0x59, 0xd7, 0xbd, 0x72, 0x2e, 0xd7, 0xfc, 0x91, 0x35, 0x5a, 0x63, 0x46,
    0x2f, 0x17, 0xc6, 0x90, 0x4e, 0xac, 0x8f, 0x1b, 0x3a, 0xcc, 0x1b, 0xe1, 0x01, 0x95, 0x28, 0xd0,
];

const TESTNET_GENESIS_HASH: [u8; 32] = [
    0x40, 0x8d, 0x4c, 0x48, 0xac, 0x8c, 0x9c, 0x88, 0xaa, 0x16, 0x5b, 0x68, 0x46, 0x04, 0xf7, 0x5c,
    0x7e, 0x5b, 0xde, 0xde, 0xb8, 0x23, 0xad, 0xc7, 0xf8, 0x43, 0x0e, 0x4b, 0x01, 0x4a, 0xdb, 0xfb,
];

impl Network {
    pub fn address_params(self) -> &'static elements::AddressParams {
        match self {
            Network::Regtest => &elements::AddressParams::ELEMENTS,
            Network::Testnet => &TESTNET_ADDRESS_PARAMS,
        }
    }

    /// Output of `elements-cli getsidechaininfo | jq --raw-output '.pegged_asset'`
    ///
    /// The command prints the hex string in **reversed byte order**!
    /// This matches the behavior of `fmt::Display` implementations of `hashes::Midstate`.
    /// The raw byte array must be written in **original order** in the Rust code!
    pub fn bitcoin_id(self) -> elements::AssetId {
        let bytes = match self {
            Network::Regtest => REGTEST_BITCOIN_ID,
            Network::Testnet => TESTNET_BITCOIN_ID,
        };
        elements::AssetId::from_inner(sha256::Midstate(bytes))
    }

    /// Output of `elements-cli getblockhash 0`
    pub fn genesis_hash(self) -> elements::BlockHash {
        let bytes = match self {
            Network::Regtest => REGTEST_GENESIS_HASH,
            Network::Testnet => TESTNET_GENESIS_HASH,
        };
        elements::BlockHash::from_byte_array(bytes)
    }
}

impl FromStr for Network {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "regtest" => Ok(Self::Regtest),
            "testnet" => Ok(Self::Testnet),
            _ => Err("Unknown network"),
        }
    }
}

impl fmt::Display for Network {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Network::Regtest => f.write_str("regtest"),
            Network::Testnet => f.write_str("testnet"),
        }
    }
}
