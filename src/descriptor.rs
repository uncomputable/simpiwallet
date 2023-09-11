use std::collections::HashMap;
use std::fmt;
use std::str::FromStr;
use std::sync::Arc;

use bitcoin::key::XOnlyPublicKey;
use elements::bitcoin;
use elements_miniscript as miniscript;
use elements_miniscript::{DescriptorPublicKey, ToPublicKey};
use miniscript::descriptor::TapTree;
use miniscript::elements;
use miniscript::{Descriptor, MiniscriptKey};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::key::UnspendableKey;

pub fn simplicity_pk<Pk: MiniscriptKey + UnspendableKey>(key: Pk) -> Descriptor<Pk> {
    let internal_key = Pk::unspendable();
    let policy = simplicity::Policy::Key(key);
    let tree = TapTree::SimplicityLeaf(Arc::new(policy));
    Descriptor::new_tr(internal_key, Some(tree)).expect("single leaf is within bounds")
}

pub fn simplicity_asm<Pk: MiniscriptKey + UnspendableKey>(cmr: simplicity::Cmr) -> Descriptor<Pk> {
    let internal_key = Pk::unspendable();
    let policy = simplicity::Policy::Assembly(cmr);
    let tree = TapTree::SimplicityLeaf(Arc::new(policy));
    Descriptor::new_tr(internal_key, Some(tree)).expect("single leaf is within bounds")
}

pub fn get_cmr<Pk: ToPublicKey>(descriptor: &Descriptor<Pk>) -> Option<simplicity::Cmr> {
    match descriptor {
        Descriptor::Tr(tr) => match tr.taptree() {
            Some(TapTree::SimplicityLeaf(policy)) => Some(policy.cmr()),
            _ => None,
        },
        _ => None,
    }
}

pub fn get_control_block<Pk: ToPublicKey>(
    descriptor: &Descriptor<Pk>,
) -> Option<elements::taproot::ControlBlock> {
    match descriptor {
        Descriptor::Tr(tr) => match tr.taptree() {
            Some(TapTree::SimplicityLeaf(policy)) => {
                let cmr = policy.cmr();
                let script = elements::Script::from(cmr.as_ref().to_vec());
                let script_ver = (script, simplicity::leaf_version());
                let control_block = tr
                    .spend_info()
                    .control_block(&script_ver)
                    .expect("Control block must exist in script map for every known leaf");
                Some(control_block)
            }
            _ => None,
        },
        _ => None,
    }
}

pub fn child_script_pubkeys(
    parent_descriptor: &Descriptor<DescriptorPublicKey>,
    max_child_index: u32,
) -> impl Iterator<Item = elements::Script> + '_ {
    (0..max_child_index).map(|i| {
        parent_descriptor
            .at_derivation_index(i)
            .expect("valid child index")
            .script_pubkey()
    })
}

#[derive(Serialize, Deserialize, Default, Debug, Clone)]
pub struct AssemblySet {
    descriptors: Vec<Descriptor<XOnlyPublicKey>>,
    satisfactions: HashMap<simplicity::Cmr, SerdeWitnessNode<simplicity::jet::Elements>>,
}

impl AssemblySet {
    pub fn iter(&self) -> impl Iterator<Item = simplicity::Cmr> + '_ {
        self.descriptors.iter().filter_map(get_cmr)
    }

    pub fn contains(&self, cmr: simplicity::Cmr) -> bool {
        self.iter().any(|c| c == cmr)
    }

    pub fn insert(&mut self, cmr: simplicity::Cmr) -> bool {
        if self.contains(cmr) {
            false
        } else {
            self.descriptors.push(simplicity_asm(cmr));
            true
        }
    }

    pub fn get_address(
        &self,
        cmr: &simplicity::Cmr,
        params: &'static elements::AddressParams,
    ) -> Option<elements::Address> {
        self.descriptors
            .iter()
            .filter_map(|d| get_cmr(d).filter(|c| c == cmr).map(|_| d))
            .next()
            .map(|d| d.address(params).expect("taproot address"))
    }

    pub fn insert_satisfaction(
        &mut self,
        program: &simplicity::WitnessNode<simplicity::jet::Elements>,
    ) -> Result<Option<SerdeWitnessNode<simplicity::jet::Elements>>, simplicity::Error> {
        let finalized = program.finalize()?;
        let maybe_replaced = self
            .satisfactions
            .insert(program.cmr(), SerdeWitnessNode::new_unchecked(finalized));
        Ok(maybe_replaced)
    }
}

#[derive(Clone, Debug)]
pub struct SerdeWitnessNode<J: simplicity::jet::Jet>(Arc<simplicity::RedeemNode<J>>);

impl<J: simplicity::jet::Jet> SerdeWitnessNode<J> {
    pub fn new(program: Arc<simplicity::WitnessNode<J>>) -> Result<Self, simplicity::Error> {
        Ok(Self(program.finalize()?))
    }

    pub fn new_unchecked(program: Arc<simplicity::RedeemNode<J>>) -> Self {
        Self(program)
    }

    pub fn unwrap(&self) -> Arc<simplicity::WitnessNode<J>> {
        self.0.to_witness_node()
    }
}

impl<J: simplicity::jet::Jet> fmt::Display for SerdeWitnessNode<J> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let bytes = self.0.encode_to_vec();
        let s = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, bytes);
        f.write_str(s.as_str())
    }
}

impl<J: simplicity::jet::Jet> FromStr for SerdeWitnessNode<J> {
    type Err = crate::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let bytes = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, s)
            .map_err(|e| crate::Error::CouldNotParse(e.to_string()))?;
        let mut iter = simplicity::BitIter::from(bytes.into_iter());
        let program = simplicity::RedeemNode::decode(&mut iter)?;
        Ok(Self(program))
    }
}

impl<J: simplicity::jet::Jet> Serialize for SerdeWitnessNode<J> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.collect_str(self)
    }
}

impl<'de, J: simplicity::jet::Jet> Deserialize<'de> for SerdeWitnessNode<J> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        SerdeWitnessNode::from_str(&s).map_err(serde::de::Error::custom)
    }
}
