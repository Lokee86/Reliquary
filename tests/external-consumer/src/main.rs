use reliquary_memory::{Gateway, OwnerInfo, OwnerScope, Phylactery, Rel};
use std::path::Path;

fn inspect(rel: &Rel, phy: &Phylactery) -> (OwnerInfo, OwnerInfo) {
    (rel.info().expect("REL info"), phy.info().expect("PHY info"))
}

fn create_rel(path: &Path) -> Result<Rel, reliquary_memory::ProductError> {
    Rel::create(path, OwnerScope::Project)
}

fn create_phy(path: &Path) -> Result<Phylactery, reliquary_memory::ProductError> {
    Phylactery::create(path)
}

fn main() {
    let _public_constructor = Gateway::new;
    let _rel_constructor = create_rel;
    let _phy_constructor = create_phy;
    let _scope = OwnerScope::Project;
    let _inspect = inspect;
}
