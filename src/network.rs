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
    0x8c, 0xd3, 0xf1, 0xbb, 0x67, 0xf2, 0x1a, 0x94, 0xaa, 0x7c, 0xc0, 0xef, 0xd6, 0x5a, 0x3e, 0xb0,
    0x8d, 0x74, 0xdf, 0x81, 0x08, 0xbb, 0x4c, 0xc4, 0x25, 0x65, 0x66, 0x69, 0xf0, 0x78, 0x79, 0x64,
];

const REGTEST_GENESIS_HASH: [u8; 32] = [
    0xdd, 0x7a, 0xa4, 0xca, 0x86, 0xfb, 0x64, 0x70, 0x85, 0x09, 0x18, 0xed, 0x28, 0xb4, 0x71, 0xe8,
    0xdf, 0x46, 0x36, 0x1d, 0x0b, 0xa5, 0x8d, 0x47, 0x97, 0xc7, 0xe7, 0x3d, 0x7e, 0x82, 0x29, 0x51,
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
