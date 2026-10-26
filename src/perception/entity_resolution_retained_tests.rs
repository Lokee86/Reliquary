use crate::entity_resolution_codec::*;
use std::{fs, path::PathBuf};
fn scratch() -> PathBuf {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(".test-tmp")
        .join(format!("retained-resolution-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&p).unwrap();
    p
}
#[test]
fn retained_literal_evidence_opens_in_both_current_owner_kinds_without_projection() {
    let root = scratch();
    let encoded = retained::encode(b"CVAERS02exact inert evidence").unwrap();
    for phy in [false, true] {
        let path = root.join(format!("current-{phy}"));
        if phy {
            let mut owner = crate::Phylactery::create(&path).unwrap();
            owner.container.append(&encoded).unwrap();
            owner.sync().unwrap();
        } else {
            let mut owner = crate::Cva::create(&path).unwrap();
            owner.container.append(&encoded).unwrap();
            owner.sync().unwrap();
        }
        let before = fs::read(&path).unwrap();
        if phy {
            let owner = crate::Phylactery::open(&path).unwrap();
            assert!(owner.entity_resolution_records().is_empty());
        } else {
            let owner = crate::Cva::open(&path).unwrap();
            assert!(owner.entity_resolution_records().is_empty());
        }
        assert_eq!(fs::read(&path).unwrap(), before);
    }
}
