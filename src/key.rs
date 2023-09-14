use std::str::FromStr;

use elements::bitcoin;
use elements::secp256k1_zkp;
use elements::secp256k1_zkp::rand::RngCore;
use elements_miniscript as miniscript;
use elements_miniscript::ToPublicKey;
use miniscript::descriptor::{
    ConversionError, DescriptorSecretKey as MSDescriptorSecretKey, Wildcard,
};
use miniscript::elements;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

const UNSPENDABLE_PUBLIC_KEY: [u8; 32] = [
    0x50, 0x92, 0x9b, 0x74, 0xc1, 0xa0, 0x49, 0x54, 0xb7, 0x8b, 0x4b, 0x60, 0x35, 0xe9, 0x7a, 0x5e,
    0x07, 0x8a, 0x5a, 0x0f, 0x28, 0xec, 0x96, 0xd5, 0x47, 0xbf, 0xee, 0x9a, 0xce, 0x80, 0x3a, 0xc0,
];

pub trait UnspendableKey {
    fn unspendable() -> Self;
}

impl UnspendableKey for bitcoin::key::XOnlyPublicKey {
    fn unspendable() -> Self {
        bitcoin::key::XOnlyPublicKey::from_slice(&UNSPENDABLE_PUBLIC_KEY)
            .expect("unspendable pubkey is valid")
    }
}

impl UnspendableKey for bitcoin::key::PublicKey {
    fn unspendable() -> Self {
        bitcoin::key::XOnlyPublicKey::unspendable()
            .to_public_key()
            .to_public_key()
    }
}

impl UnspendableKey for miniscript::DescriptorPublicKey {
    fn unspendable() -> Self {
        Self::Single(miniscript::descriptor::SinglePub {
            origin: None,
            key: miniscript::descriptor::SinglePubKey::XOnly(
                bitcoin::key::XOnlyPublicKey::unspendable(),
            ),
        })
    }
}

#[derive(Clone, Debug)]
pub struct DescriptorSecretKey(pub MSDescriptorSecretKey);

impl Serialize for DescriptorSecretKey {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.0.to_string())
    }
}

impl<'de> Deserialize<'de> for DescriptorSecretKey {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        let descriptor_secret_key =
            MSDescriptorSecretKey::from_str(&s).map_err(serde::de::Error::custom)?;
        Ok(DescriptorSecretKey(descriptor_secret_key))
    }
}

impl DescriptorSecretKey {
    pub fn random() -> Result<Self, bitcoin::bip32::Error> {
        let mut seed = [0u8; 32];
        secp256k1_zkp::rand::rngs::OsRng.fill_bytes(&mut seed);
        Self::from_seed(&seed)
    }

    pub fn from_seed(seed: &[u8]) -> Result<Self, bitcoin::bip32::Error> {
        let xpriv = bitcoin::bip32::ExtendedPrivKey::new_master(bitcoin::Network::Regtest, seed)?;
        let descriptor_xpriv =
            MSDescriptorSecretKey::XPrv(miniscript::descriptor::DescriptorXKey {
                origin: None,
                xkey: xpriv,
                derivation_path: bitcoin::bip32::DerivationPath::from_str("m/84'/0'/0'")?,
                wildcard: Wildcard::Unhardened,
            });
        Ok(Self(descriptor_xpriv))
    }

    pub fn at_derivation_index(self, index: u32) -> Result<Self, ConversionError> {
        match self.0 {
            MSDescriptorSecretKey::Single(..) => Ok(self),
            MSDescriptorSecretKey::XPrv(xpriv) => {
                let derivation_path = match xpriv.wildcard {
                    Wildcard::None => xpriv.derivation_path,
                    Wildcard::Unhardened => xpriv.derivation_path.into_child(
                        bitcoin::bip32::ChildNumber::from_normal_idx(index)
                            .map_err(|_| ConversionError::HardenedChild)?,
                    ),
                    Wildcard::Hardened => xpriv.derivation_path.into_child(
                        bitcoin::bip32::ChildNumber::from_hardened_idx(index)
                            .map_err(|_| ConversionError::HardenedChild)?,
                    ),
                };
                let descriptor_secret_key =
                    MSDescriptorSecretKey::XPrv(miniscript::descriptor::DescriptorXKey {
                        origin: None,
                        xkey: xpriv.xkey,
                        derivation_path,
                        wildcard: xpriv.wildcard,
                    });
                Ok(Self(descriptor_secret_key))
            }
            MSDescriptorSecretKey::MultiXPrv(..) => Err(ConversionError::MultiKey),
        }
    }

    pub fn to_private_key(&self) -> bitcoin::PrivateKey {
        match &self.0 {
            MSDescriptorSecretKey::Single(single) => single.key,
            MSDescriptorSecretKey::XPrv(xpriv) => xpriv
                .xkey
                .derive_priv(secp256k1_zkp::SECP256K1, &xpriv.derivation_path)
                .expect("never fails")
                .to_priv(),
            MSDescriptorSecretKey::MultiXPrv(..) => {
                unimplemented!("no support for multi-path xprivs")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use miniscript::ToPublicKey;

    #[test]
    fn descriptor_secret_key_at_derivation_index() {
        let parent_xpriv = DescriptorSecretKey::from_seed(&[0; 32]).expect("const");
        let parent_xpub = parent_xpriv
            .0
            .to_public(secp256k1_zkp::SECP256K1)
            .expect("const");

        for index in 0..10 {
            let private_key = parent_xpriv
                .clone()
                .at_derivation_index(index)
                .expect("valid child index")
                .to_private_key();
            let public_key_from_private_key = private_key.public_key(secp256k1_zkp::SECP256K1);
            let public_key = parent_xpub
                .clone()
                .at_derivation_index(index)
                .expect("valid child index")
                .to_public_key();
            assert_eq!(public_key_from_private_key, public_key);
        }
    }
}
