use crate::ego_codec::{EgoRecord, compact};
use crate::ego_compact_contract::EgoCreation;
use crate::{Cva, CvaReconcileError, EgoAnchorId, EgoAnchorPriority};
use std::fs;
use std::path::PathBuf;

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(".test-tmp")
            .join(format!(
                "cva-reconcile-ego-compact-{}",
                uuid::Uuid::new_v4()
            ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn file(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn divergent_reconciliation_rejects_valid_compact_ego_and_creates_no_output() {
    let scratch = Scratch::new();
    let base = scratch.file("base.rel");
    drop(Cva::create_project(&base).unwrap());
    let left = scratch.file("left.rel");
    let right = scratch.file("right.rel");
    let output = scratch.file("merged.rel");
    fs::copy(&base, &left).unwrap();
    fs::copy(&base, &right).unwrap();

    let mut left_owner = Cva::open(&left).unwrap();
    let ego = EgoRecord::Anchor {
        ego_version: 1,
        id: EgoAnchorId([0x51; 16]),
        revision: 1,
        deleted: false,
        priority: EgoAnchorPriority::High,
        text: "  valid compact Ego anchor  ".into(),
    };
    let ego_frame = compact::encode_creation(&EgoCreation::new(ego).unwrap()).unwrap();
    left_owner.container.append(&ego_frame).unwrap();
    left_owner.sync().unwrap();
    drop(left_owner);

    let mut right_owner = Cva::open(&right).unwrap();
    right_owner
        .append_node(
            "right-only-node".into(),
            "right-only-conversation".into(),
            None,
            "user".into(),
            1,
            "right branch",
        )
        .unwrap();
    right_owner.sync().unwrap();
    drop(right_owner);

    assert_eq!(
        Cva::compare(&left, &right).unwrap().relation,
        crate::CvaRelation::Diverged
    );
    assert!(matches!(
        Cva::reconcile(&left, &right, &output),
        Err(CvaReconcileError::UnsupportedSemanticOwner(
            "Ego state in divergent reconciliation"
        ))
    ));
    assert!(
        !output.exists(),
        "rejected reconciliation must publish no output"
    );
}
