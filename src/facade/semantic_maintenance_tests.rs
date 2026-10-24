use crate::entity_resolution_codec::DecodedEntityResolution;
use crate::entity_resolution_test_support::{phy_with_three_mentions, rel_with_three_mentions};
use crate::imports::{ImportStart, ImportWatermarks};
use crate::{
    ContainerError, Cva, EntityId, EntityResolutionMaintenance, EntityResolutionReason,
    MemoryEntityResolutionStatus, Phylactery, TailRepairOutcome,
};
use sha2::Digest;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

fn test_path(name: &str) -> PathBuf {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".test-tmp");
    fs::create_dir_all(&root).unwrap();
    root.join(format!("{name}-{}.db", uuid::Uuid::new_v4()))
}

fn append_incomplete_header(path: &PathBuf) {
    let mut file = OpenOptions::new().append(true).open(path).unwrap();
    file.write_all(&[1, 2, 3, 4]).unwrap();
    file.sync_all().unwrap();
}

#[test]
fn rel_repair_refuses_complete_unaccepted_owner_frame_with_or_without_torn_tail() {
    for torn_tail in [false, true] {
        let path = test_path("rel-repair-unaccepted-frame");
        let mut owner = Cva::create(&path).unwrap();
        owner
            .container
            .append_publication_envelope(b"unknown-owner-body")
            .unwrap();
        owner.sync().unwrap();
        drop(owner);
        if torn_tail {
            append_incomplete_header(&path);
        }
        let before = fs::read(&path).unwrap();
        assert!(Cva::open(&path).is_err());
        assert_eq!(fs::read(&path).unwrap(), before);
        assert!(Cva::repair_incomplete_tail(&path).is_err());
        assert_eq!(fs::read(&path).unwrap(), before);
    }
}

#[test]
fn phy_repair_refuses_complete_unaccepted_owner_frame_with_or_without_torn_tail() {
    for torn_tail in [false, true] {
        let path = test_path("phy-repair-unaccepted-frame");
        let mut owner = Phylactery::create(&path).unwrap();
        owner
            .container
            .append_publication_envelope(b"unknown-owner-body")
            .unwrap();
        owner.sync().unwrap();
        drop(owner);
        if torn_tail {
            append_incomplete_header(&path);
        }
        let before = fs::read(&path).unwrap();
        assert!(Phylactery::open(&path).is_err());
        assert_eq!(fs::read(&path).unwrap(), before);
        assert!(Phylactery::repair_incomplete_tail(&path).is_err());
        assert_eq!(fs::read(&path).unwrap(), before);
    }
}

#[test]
fn rel_tail_repair_requires_explicit_semantic_validation_and_keeps_one_source_lease() {
    let path = test_path("rel-tail-repair");
    let owner = Cva::create_project(&path).unwrap();
    drop(owner);
    let original_len = fs::metadata(&path).unwrap().len();
    append_incomplete_header(&path);
    let damaged = fs::read(&path).unwrap();

    assert!(Cva::open_read_only(&path).is_err());
    assert_eq!(fs::read(&path).unwrap(), damaged);
    assert_eq!(
        Cva::repair_incomplete_tail(&path).unwrap(),
        TailRepairOutcome::Repaired
    );
    assert_eq!(fs::metadata(&path).unwrap().len(), original_len);
    assert!(Cva::open_read_only(&path).is_ok());
    assert_eq!(
        Cva::repair_incomplete_tail(&path).unwrap(),
        TailRepairOutcome::NoIncompleteTail
    );
}

#[test]
fn phy_tail_repair_requires_explicit_semantic_validation_and_keeps_one_source_lease() {
    let path = test_path("phy-tail-repair");
    let owner = Phylactery::create(&path).unwrap();
    drop(owner);
    let original_len = fs::metadata(&path).unwrap().len();
    append_incomplete_header(&path);
    let damaged = fs::read(&path).unwrap();

    assert!(Phylactery::open_read_only(&path).is_err());
    assert_eq!(fs::read(&path).unwrap(), damaged);
    assert_eq!(
        Phylactery::repair_incomplete_tail(&path).unwrap(),
        TailRepairOutcome::Repaired
    );
    assert_eq!(fs::metadata(&path).unwrap().len(), original_len);
    assert!(Phylactery::open_read_only(&path).is_ok());
    assert_eq!(
        Phylactery::repair_incomplete_tail(&path).unwrap(),
        TailRepairOutcome::NoIncompleteTail
    );
}

#[test]
fn rel_tail_repair_refuses_open_import_epoch_without_mutating_source() {
    let path = test_path("rel-tail-open-import");
    let mut owner = Cva::create_project(&path).unwrap();
    let owner_uuid = owner.owner_uuid().unwrap();
    let import_key = "a".repeat(64);
    let import_id = format!(
        "{:x}",
        sha2::Sha256::digest([owner_uuid.as_slice(), import_key.as_bytes()].concat())
    );
    owner
        .owner
        .import_epochs
        .append_start(
            &mut owner.owner.container,
            ImportStart {
                owner_uuid,
                import_id,
                import_key,
                source_namespace: "maintenance-test".into(),
                source_format: "test".into(),
                started_at_ns: 1,
                pre: ImportWatermarks::default(),
            },
        )
        .unwrap();
    owner.sync().unwrap();
    drop(owner);

    append_incomplete_header(&path);
    let before = fs::read(&path).unwrap();
    assert!(matches!(
        Cva::repair_incomplete_tail(&path),
        Err(crate::CvaError::Container(
            ContainerError::WriteAdmissionClosed
        ))
    ));
    assert_eq!(fs::read(&path).unwrap(), before);
}

#[test]
fn rel_open_preserves_retired_resolution_evidence_until_explicit_repair() {
    let (mut owner, _, keys, path) = rel_with_three_mentions();
    let survivor = EntityId([0; 32]);
    let retired = EntityId([1; 32]);
    owner.merge_entities(survivor, retired, 10).unwrap();
    let change = DecodedEntityResolution {
        key: keys[0],
        revision: 1,
        updated_at_ns: 11,
        status: Some(MemoryEntityResolutionStatus::Resolved {
            entity_id: retired,
            reason: EntityResolutionReason::ContextMatch,
        }),
    };
    owner
        .container
        .append(&crate::entity_resolution_codec::delta::encode(&change, None).unwrap())
        .unwrap();
    owner.sync().unwrap();
    let before = owner.container.physical_digest().unwrap();
    drop(owner);

    let mut reopened = Cva::open(&path).unwrap();
    assert_eq!(reopened.container.physical_digest().unwrap(), before);
    assert_eq!(
        reopened.entity_resolution_maintenance().unwrap(),
        EntityResolutionMaintenance::RetiredEntityReferences { revisions: 1 }
    );
    drop(reopened);
    let mut reopened = Cva::open(&path).unwrap();
    assert_eq!(reopened.repair_retired_entity_references().unwrap(), 1);
    drop(reopened);
    let reopened = Cva::open(&path).unwrap();
    assert_eq!(
        reopened.entity_resolution(keys[0]).unwrap().status,
        MemoryEntityResolutionStatus::Resolved {
            entity_id: survivor,
            reason: EntityResolutionReason::ContextMatch,
        }
    );
}

#[test]
fn phy_open_preserves_retired_resolution_evidence_until_explicit_repair() {
    let (mut owner, _, keys, path) = phy_with_three_mentions();
    let survivor = EntityId([0; 32]);
    let retired = EntityId([1; 32]);
    owner.merge_entities(survivor, retired, 10).unwrap();
    let change = DecodedEntityResolution {
        key: keys[0],
        revision: 1,
        updated_at_ns: 11,
        status: Some(MemoryEntityResolutionStatus::Resolved {
            entity_id: retired,
            reason: EntityResolutionReason::ContextMatch,
        }),
    };
    owner
        .container
        .append(&crate::entity_resolution_codec::delta::encode(&change, None).unwrap())
        .unwrap();
    owner.sync().unwrap();
    let before = owner.container.physical_digest().unwrap();
    drop(owner);

    let mut reopened = Phylactery::open(&path).unwrap();
    assert_eq!(reopened.container.physical_digest().unwrap(), before);
    assert_eq!(
        reopened.entity_resolution_maintenance().unwrap(),
        EntityResolutionMaintenance::RetiredEntityReferences { revisions: 1 }
    );
    drop(reopened);
    let mut reopened = Phylactery::open(&path).unwrap();
    assert_eq!(reopened.repair_retired_entity_references().unwrap(), 1);
    drop(reopened);
    let reopened = Phylactery::open(&path).unwrap();
    assert_eq!(
        reopened.entity_resolution(keys[0]).unwrap().status,
        MemoryEntityResolutionStatus::Resolved {
            entity_id: survivor,
            reason: EntityResolutionReason::ContextMatch,
        }
    );
}
