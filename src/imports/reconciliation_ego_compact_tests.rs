use crate::ego_codec::{self, EgoRecord, compact};
use crate::ego_compact_contract::{EgoCreation, EgoDelta, EgoTerminal};
use crate::{Cva, CvaReconcileError, EgoAnchorId, EgoAnchorPriority, EgoIdentityId};
use std::fs;
use std::path::PathBuf;

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(".test-tmp")
            .join(format!(
                "reconciliation-ego-compact-{}",
                uuid::Uuid::new_v4()
            ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn file(&self) -> PathBuf {
        self.0.join("owner.rel")
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn identity(
    version: u64,
    revision: u64,
    deleted: bool,
    activate: bool,
    name: &str,
    text: &str,
) -> EgoRecord {
    EgoRecord::Identity {
        ego_version: version,
        id: EgoIdentityId([0x11; 16]),
        revision,
        deleted,
        activate,
        name: name.into(),
        text: text.into(),
    }
}

fn anchor(
    version: u64,
    revision: u64,
    deleted: bool,
    priority: EgoAnchorPriority,
    text: &str,
) -> EgoRecord {
    EgoRecord::Anchor {
        ego_version: version,
        id: EgoAnchorId([0x22; 16]),
        revision,
        deleted,
        priority,
        text: text.into(),
    }
}

fn assert_unhandled_interval(cva: &mut Cva, payload: &[u8]) {
    let boundary = cva.container.chunks().unwrap().len();
    cva.container.append(payload).unwrap();
    assert!(matches!(
        super::validate_replay_interval(cva, boundary),
        Err(CvaReconcileError::UnsupportedSemanticOwner(
            "unhandled owner payload in import interval"
        ))
    ));
}

#[test]
fn import_interval_explicitly_rejects_all_ego_compact_publication_kinds_and_activation() {
    let scratch = Scratch::new();
    let mut cva = Cva::create(scratch.file()).unwrap();

    let identity_first = identity(1, 1, false, true, "Default", "first identity");
    let anchor_first = anchor(1, 1, false, EgoAnchorPriority::High, "first anchor");
    let personality_first = EgoRecord::Personality {
        ego_version: 1,
        revision: 1,
        source_memory_version: 0,
        text: "first personality".into(),
    };
    let synthesis_first = EgoRecord::Synthesis {
        ego_version: 1,
        revision: 1,
        source_memory_version: 0,
        text: "first synthesis".into(),
    };

    for record in [
        identity_first.clone(),
        anchor_first.clone(),
        personality_first.clone(),
        synthesis_first.clone(),
    ] {
        let bytes = compact::encode_creation(&EgoCreation::new(record).unwrap()).unwrap();
        assert_unhandled_interval(&mut cva, &bytes);
    }

    let successors = [
        identity(2, 2, false, true, "Default", "revised identity"),
        anchor(2, 2, false, EgoAnchorPriority::Low, "revised anchor"),
        EgoRecord::Personality {
            ego_version: 2,
            revision: 2,
            source_memory_version: 0,
            text: "revised personality".into(),
        },
        EgoRecord::Synthesis {
            ego_version: 2,
            revision: 2,
            source_memory_version: 0,
            text: "revised synthesis".into(),
        },
    ];
    for (first, next) in [
        (identity_first, successors[0].clone()),
        (anchor_first, successors[1].clone()),
        (personality_first, successors[2].clone()),
        (synthesis_first, successors[3].clone()),
    ] {
        let bytes = compact::encode_delta(&EgoDelta::between(&first, &next).unwrap()).unwrap();
        assert_unhandled_interval(&mut cva, &bytes);
    }

    for record in [
        identity(3, 3, true, false, "", ""),
        anchor(3, 3, true, EgoAnchorPriority::Normal, ""),
    ] {
        let bytes = compact::encode_terminal(&EgoTerminal::new(record).unwrap()).unwrap();
        assert_unhandled_interval(&mut cva, &bytes);
    }

    let activation = ego_codec::encode(&EgoRecord::IdentityActive {
        ego_version: 1,
        id: EgoIdentityId([0x33; 16]),
    })
    .unwrap();
    assert_unhandled_interval(&mut cva, &activation);
}

#[test]
fn import_interval_does_not_silently_ignore_unknown_or_truncated_ego_frames() {
    let scratch = Scratch::new();
    let mut cva = Cva::create(scratch.file()).unwrap();
    for payload in [
        b"CVAECR99".as_slice(),
        b"CVAEDL99".as_slice(),
        b"CVAETM99".as_slice(),
        b"CVAECR01".as_slice(),
        b"CVAEDL01".as_slice(),
        b"CVAETM01".as_slice(),
    ] {
        assert_unhandled_interval(&mut cva, payload);
    }
}
