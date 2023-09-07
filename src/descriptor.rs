use std::sync::Arc;

use bitcoin::key::XOnlyPublicKey;
use elements::bitcoin;
use elements_miniscript as miniscript;
use elements_miniscript::ToPublicKey;
use miniscript::descriptor::TapTree;
use miniscript::elements;
use miniscript::{Descriptor, MiniscriptKey};
use serde::{Deserialize, Serialize};

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

#[derive(Serialize, Deserialize, Default, Debug, Clone)]
pub struct AssemblySet {
    descriptors: Vec<Descriptor<XOnlyPublicKey>>,
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
}
