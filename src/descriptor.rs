use std::sync::Arc;

use elements_miniscript as miniscript;
use miniscript::descriptor::TapTree;
use miniscript::{Descriptor, MiniscriptKey};

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
