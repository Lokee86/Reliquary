use crate::{Branch, Cva};
use std::fs;

#[test]
fn current_branch_inventory_exposes_latest_revision_after_reopen() {
    let unique = uuid::Uuid::new_v4();
    let dir = std::env::temp_dir().join(format!("continuity-branch-inventory-{unique}"));
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("inventory.cva");
    let mut cva = Cva::create(&path).unwrap();
    for (id, parent) in [("n1", None), ("n2", Some("n1"))] {
        cva.append_node(
            id.into(),
            "conversation".into(),
            parent.map(str::to_owned),
            "user".into(),
            1,
            id,
        )
        .unwrap();
    }
    cva.append_branch(Branch {
        id: "main".into(),
        conversation_id: "conversation".into(),
        leaf_node_id: "n1".into(),
        canonical: true,
    })
    .unwrap();
    cva.append_branch(Branch {
        id: "main".into(),
        conversation_id: "conversation".into(),
        leaf_node_id: "n2".into(),
        canonical: true,
    })
    .unwrap();
    cva.sync().unwrap();
    drop(cva);

    let branches = Cva::open(path).unwrap().branches();
    assert_eq!(branches.len(), 1);
    assert_eq!(branches[0].id, "main");
    assert_eq!(branches[0].leaf_node_id, "n2");
}
