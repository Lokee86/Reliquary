use crate::{Cva, Phylactery};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

fn test_path(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(".test-tmp")
        .join(format!("storage-format-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&dir).unwrap();
    dir.join(name)
}

fn downgrade_header_to_v1(path: &PathBuf) {
    let mut bytes = fs::read(path).unwrap();
    bytes[8..10].copy_from_slice(&1_u16.to_le_bytes());
    bytes[10..12].copy_from_slice(&0_u16.to_le_bytes());
    fs::write(path, bytes).unwrap();
}

fn append_incomplete_tail(path: &PathBuf) {
    let mut file = OpenOptions::new().append(true).open(path).unwrap();
    file.write_all(&[0x41, 0x42, 0x43]).unwrap();
    file.sync_all().unwrap();
}

fn copy_file(source: &PathBuf, destination: &PathBuf) {
    let mut input = fs::File::open(source).unwrap();
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination)
        .unwrap();
    std::io::copy(&mut input, &mut output).unwrap();
    output.sync_all().unwrap();
}

#[test]
fn new_rel_and_phy_owners_use_v2_and_reopen() {
    let rel_path = test_path("new.rel");
    let rel = Cva::create(&rel_path).unwrap();
    assert_eq!(
        rel.container.version(),
        crate::FormatVersion { major: 2, minor: 0 }
    );
    drop(rel);
    assert_eq!(
        u16::from_le_bytes(fs::read(&rel_path).unwrap()[8..10].try_into().unwrap()),
        2
    );
    assert_eq!(
        Cva::open(&rel_path).unwrap().container.version(),
        crate::FormatVersion { major: 2, minor: 0 }
    );

    let phy_path = test_path("new.phy");
    let phy = Phylactery::create(&phy_path).unwrap();
    assert_eq!(
        phy.container.version(),
        crate::FormatVersion { major: 2, minor: 0 }
    );
    drop(phy);
    assert_eq!(
        u16::from_le_bytes(fs::read(&phy_path).unwrap()[8..10].try_into().unwrap()),
        2
    );
    assert_eq!(
        Phylactery::open(&phy_path).unwrap().container.version(),
        crate::FormatVersion { major: 2, minor: 0 }
    );
}

#[test]
fn v1_owner_open_and_write_rejection_leave_rel_bytes_unchanged() {
    let path = test_path("legacy.rel");
    drop(Cva::create(&path).unwrap());
    downgrade_header_to_v1(&path);
    let before = fs::read(&path).unwrap();

    let mut rel = Cva::open(&path).unwrap();
    assert_eq!(
        rel.container.version(),
        crate::FormatVersion { major: 1, minor: 0 }
    );
    assert_eq!(
        rel.rel_metadata(),
        crate::RelMetadata {
            type_label: None,
            dependencies: vec![]
        }
    );
    assert!(matches!(
        rel.set_rel_metadata(None, Vec::new()),
        Err(crate::CvaError::Container(
            crate::ContainerError::StorageFormatUpgradeRequired
        ))
    ));
    assert_eq!(
        rel.rel_metadata(),
        crate::RelMetadata {
            type_label: None,
            dependencies: vec![]
        }
    );
    assert!(matches!(
        rel.set_rel_metadata(Some("legacy write".into()), Vec::new()),
        Err(crate::CvaError::Container(
            crate::ContainerError::StorageFormatUpgradeRequired
        ))
    ));
    assert!(matches!(
        rel.set_rel_metadata(Some("retry".into()), Vec::new()),
        Err(crate::CvaError::Container(
            crate::ContainerError::StorageFormatUpgradeRequired
        ))
    ));
    drop(rel);
    assert_eq!(fs::read(&path).unwrap(), before);
}

#[test]
fn v1_phylactery_write_rejection_is_typed_and_preserves_bytes() {
    let path = test_path("legacy.phy");
    drop(Phylactery::create(&path).unwrap());
    downgrade_header_to_v1(&path);
    let before = fs::read(&path).unwrap();

    let mut phy = Phylactery::open_v1_read_only(&path).unwrap();
    assert!(matches!(
        phy.set_profile(None),
        Err(crate::PhylacteryError::Container(
            crate::ContainerError::StorageFormatUpgradeRequired
        ))
    ));
    assert_eq!(phy.profile().display_name, None);
    assert!(matches!(
        phy.set_profile(Some("legacy write".into())),
        Err(crate::PhylacteryError::Container(
            crate::ContainerError::StorageFormatUpgradeRequired
        ))
    ));
    assert!(matches!(
        phy.set_profile(Some("retry".into())),
        Err(crate::PhylacteryError::Container(
            crate::ContainerError::StorageFormatUpgradeRequired
        ))
    ));
    drop(phy);
    assert_eq!(fs::read(&path).unwrap(), before);
}

#[test]
fn v1_tail_open_fails_without_repair_but_v2_open_keeps_recovery_behavior() {
    let legacy_path = test_path("legacy-tail.rel");
    drop(Cva::create(&legacy_path).unwrap());
    downgrade_header_to_v1(&legacy_path);
    append_incomplete_tail(&legacy_path);
    let legacy_bytes = fs::read(&legacy_path).unwrap();
    assert!(matches!(
        Cva::open_v1_read_only(&legacy_path),
        Err(crate::CvaError::Container(
            crate::ContainerError::TruncatedChunk(_)
        ))
    ));
    assert_eq!(fs::read(&legacy_path).unwrap(), legacy_bytes);

    let current_path = test_path("current-tail.rel");
    drop(Cva::create(&current_path).unwrap());
    append_incomplete_tail(&current_path);
    let damaged_len = fs::metadata(&current_path).unwrap().len();
    let _opened = Cva::open(&current_path).unwrap();
    assert!(fs::metadata(&current_path).unwrap().len() < damaged_len);
}

#[test]
fn v1_read_only_api_rejects_v2_and_raw_container_scope_cannot_bypass_upgrade_gate() {
    let path = test_path("v1-gate.rel");
    drop(Cva::create(&path).unwrap());
    downgrade_header_to_v1(&path);
    let before = fs::read(&path).unwrap();
    let mut container = crate::Container::open_v1_read_only(&path).unwrap();
    container.set_owner_write_scope(true);
    assert!(matches!(
        container.append(b"must not write"),
        Err(crate::ContainerError::StorageFormatUpgradeRequired)
    ));
    let output = path.with_file_name("must-not-be-created.compact");
    assert!(matches!(
        container.create_empty_like(&output),
        Err(crate::ContainerError::StorageFormatUpgradeRequired)
    ));
    assert!(!output.exists());
    drop(container);
    assert_eq!(fs::read(&path).unwrap(), before);

    let v2_rel = test_path("v2.rel");
    drop(Cva::create(&v2_rel).unwrap());
    assert!(matches!(
        Cva::open_v1_read_only(&v2_rel),
        Err(crate::CvaError::Container(
            crate::ContainerError::UnsupportedVersion(crate::FormatVersion { major: 2, minor: 0 })
        ))
    ));
    let v2_phy = test_path("v2.phy");
    drop(Phylactery::create(&v2_phy).unwrap());
    assert!(matches!(
        Phylactery::open_v1_read_only(&v2_phy),
        Err(crate::PhylacteryError::Container(
            crate::ContainerError::UnsupportedVersion(crate::FormatVersion { major: 2, minor: 0 })
        ))
    ));
}

#[test]
fn v1_open_projects_duplicate_compactions_without_reclaiming_durable_frames() {
    let source = test_path("duplicate-compactions-source.rel");
    let mut rel = Cva::create(&source).unwrap();
    rel.put_conversation_compaction(
        "conversation-x".into(),
        "through-1".into(),
        "older duplicate summary".into(),
        None,
    )
    .unwrap();
    rel.put_conversation_compaction(
        "conversation-x".into(),
        "through-1".into(),
        "newer duplicate summary".into(),
        None,
    )
    .unwrap();
    drop(rel);

    let legacy = test_path("duplicate-compactions-v1.rel");
    copy_file(&source, &legacy);
    downgrade_header_to_v1(&legacy);
    let before = fs::read(&legacy).unwrap();
    let legacy_open = Cva::open_v1_read_only(&legacy).unwrap();
    let records = legacy_open.conversation_compactions("conversation-x");
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].summary, "newer duplicate summary");
    drop(legacy_open);
    assert_eq!(fs::read(&legacy).unwrap(), before);

    let current = test_path("duplicate-compactions-v2.rel");
    copy_file(&source, &current);
    let before_recovery = fs::read(&current).unwrap();
    let current_open = Cva::open(&current).unwrap();
    let records = current_open.conversation_compactions("conversation-x");
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].summary, "newer duplicate summary");
    drop(current_open);
    assert_ne!(fs::read(&current).unwrap(), before_recovery);
}

#[test]
fn v1_open_preserves_retired_entity_resolution_revision_and_reference() {
    let source = test_path("stale-retired-resolution-source.rel");
    let mut rel = Cva::create(&source).unwrap();
    let survivor = rel
        .entities
        .publish_legacy_fixture(
            &mut rel.container,
            None,
            0,
            crate::entity_candidate_test_support::entity_draft(
                "Canonical Server",
                &[],
                "storage-boundary-survivor",
                1,
            ),
            None,
        )
        .unwrap()
        .0;
    let retired = rel
        .entities
        .publish_legacy_fixture(
            &mut rel.container,
            None,
            0,
            crate::entity_candidate_test_support::entity_draft(
                "Retired Server",
                &[],
                "storage-boundary-retired",
                1,
            ),
            None,
        )
        .unwrap()
        .0;
    let (memory, changed) = rel
        .memories
        .publish_legacy_fixture(
            &mut rel.container,
            None,
            0,
            crate::entity_candidate_test_support::memory_draft(
                "storage-boundary-memory",
                "Server",
                "The Retired Server handles requests.",
            ),
        )
        .unwrap();
    assert!(changed);
    let memory_id = memory.id;
    let key = crate::entity_candidate_test_support::install_rel_mention(
        &mut rel,
        memory_id,
        "Retired Server",
    );
    rel.put_entity_resolved(
        key,
        0,
        retired.id,
        crate::EntityResolutionReason::ContextMatch,
        20,
    )
    .unwrap();
    // Construct the legacy retirement and its canonical resolution explicitly:
    // ordinary merge also writes a current-format survivor metadata revision.
    rel.entities
        .tombstone(
            &mut rel.container,
            retired.id,
            retired.revision,
            survivor.id,
        )
        .unwrap();
    let canonical = crate::MemoryEntityResolution {
        key,
        revision: 2,
        updated_at_ns: 21,
        status: crate::MemoryEntityResolutionStatus::Resolved {
            entity_id: survivor.id,
            reason: crate::EntityResolutionReason::ContextMatch,
        },
    };
    rel.container
        .append(&crate::entity_resolution_codec::encode_resolution(&canonical).unwrap())
        .unwrap();

    // Reproduce a historically accepted stale reference at a later durable revision. V2 open
    // repairs this canonical-owner drift; V1 inspection must retain exactly what was stored.
    let stale = crate::MemoryEntityResolution {
        key,
        revision: 3,
        updated_at_ns: 22,
        status: crate::MemoryEntityResolutionStatus::Resolved {
            entity_id: retired.id,
            reason: crate::EntityResolutionReason::ContextMatch,
        },
    };
    rel.container
        .append(&crate::entity_resolution_codec::encode_resolution(&stale).unwrap())
        .unwrap();
    rel.sync().unwrap();
    drop(rel);

    let legacy = test_path("stale-retired-resolution-v1.rel");
    copy_file(&source, &legacy);
    downgrade_header_to_v1(&legacy);
    let before = fs::read(&legacy).unwrap();
    let mut inspected = Cva::open_v1_read_only(&legacy).unwrap();
    let stored = inspected.entity_resolution(key).unwrap();
    assert_eq!(stored.revision, 3);
    assert!(matches!(
        stored.status,
        crate::MemoryEntityResolutionStatus::Resolved { entity_id, .. } if entity_id == retired.id
    ));
    assert_eq!(
        inspected.retired_entity_replacement(retired.id),
        Some(survivor.id)
    );
    drop(inspected);
    assert_eq!(fs::read(&legacy).unwrap(), before);

    let current = test_path("stale-retired-resolution-v2.rel");
    copy_file(&source, &current);
    let current_len = fs::metadata(&current).unwrap().len();
    let repaired = Cva::open(&current).unwrap();
    let resolution = repaired.entity_resolution(key).unwrap();
    assert_eq!(resolution.revision, 4);
    assert!(matches!(
        resolution.status,
        crate::MemoryEntityResolutionStatus::Resolved { entity_id, .. } if entity_id == survivor.id
    ));
    drop(repaired);
    assert!(fs::metadata(&current).unwrap().len() > current_len);

    // Failed proof admission must precede the same otherwise-valid recovery.
    let invalid = test_path("stale-resolution-unauthorized-proof.rel");
    copy_file(&source, &invalid);
    let mut raw = crate::Container::open(&invalid).unwrap();
    let record = crate::entity_model::EntityRecord {
        id: retired.id,
        global_id: retired.global_id,
        revision: 1,
        canonical_name: "[pruned]".into(),
        aliases: vec![],
        kind: retired.kind.clone(),
        summary: String::new(),
        mutation_id: "unauthorized-prune-proof".into(),
        created_at_ns: 1,
        updated_at_ns: 1,
        global_version: 1,
        entity_version: 1,
    };
    let creation = crate::entity_compact_contract::EntityCreation::new_pruned(
        record,
        crate::entity_compact_contract::EntityDescriptionProof {
            original_carrier_digest: [1; 32],
            manifest_digest: [2; 32],
        },
    )
    .unwrap();
    raw.append(&crate::entity_compact_codec::encode_creation(&creation).unwrap())
        .unwrap();
    raw.sync().unwrap();
    drop(raw);
    let before = fs::read(&invalid).unwrap();
    assert!(Cva::open(&invalid).is_err());
    assert_eq!(fs::read(&invalid).unwrap(), before);
}

#[test]
fn v1_rejects_exact_memory_retry_and_empty_graph_batch_before_state_change() {
    let source = test_path("memory-graph-source.rel");
    let draft = crate::MemoryDraft {
        category: "fact".into(),
        memory_type: "project".into(),
        authority_kind: "direct".into(),
        temporal_status: "current".into(),
        title: "Persisted title".into(),
        content: "Persisted body".into(),
        scope: "project".into(),
        lifecycle_state: "knowledge".into(),
        archived: false,
        superseded_by: None,
        parent_id: None,
        source_node_id: None,
        content_source_conversation_id: None,
        content_source_node_id: None,
        grounding_source_conversation_id: None,
        grounding_source_node_id: None,
        source_episode_id: None,
        source_time_ns: None,
        mutation_id: "storage-format-memory-retry".into(),
        created_at_ns: 10,
        updated_at_ns: 10,
    };
    let mut rel = Cva::create(&source).unwrap();
    let (memory, changed) = rel
        .memories
        .publish_legacy_fixture(&mut rel.container, None, 0, draft.clone())
        .unwrap();
    assert!(changed);
    drop(rel);

    let legacy = test_path("memory-graph-v1.rel");
    copy_file(&source, &legacy);
    downgrade_header_to_v1(&legacy);
    let before = fs::read(&legacy).unwrap();
    let mut rel = Cva::open_v1_read_only(&legacy).unwrap();
    assert!(matches!(
        rel.publish_memory(Some(memory.id), memory.revision, draft),
        Err(crate::MemoryError::Container(
            crate::ContainerError::StorageFormatUpgradeRequired
        ))
    ));
    assert_eq!(rel.memory(memory.id).unwrap(), memory);

    let graph_version = rel.graph_version();
    let empty = rel.set_memory_relations(&[], graph_version);
    assert!(matches!(
        empty,
        Err(crate::GraphError::Container(
            crate::ContainerError::StorageFormatUpgradeRequired
        ))
    ));
    assert!(rel.graph_relations().is_empty());
    drop(rel);
    assert_eq!(fs::read(&legacy).unwrap(), before);
}

#[test]
fn v1_rejects_exact_ego_anchor_update_and_v2_empty_clone_preserves_identity() {
    let source = test_path("ego-source.rel");
    let mut rel = Cva::create(&source).unwrap();
    let anchor = rel
        .create_ego_anchor(crate::EgoAnchorPriority::Normal, "Stable anchor".into())
        .unwrap();
    let owner_uuid = rel.owner_uuid();
    let identity = rel.container.identity();
    let clone_path = test_path("empty-clone.rel");
    let clone = rel.container.create_empty_like(&clone_path).unwrap();
    assert_eq!(clone.version(), crate::FormatVersion { major: 2, minor: 0 });
    assert_eq!(clone.identity(), identity);
    assert_eq!(clone.owner_uuid(), owner_uuid);
    drop(clone);
    drop(rel);

    let legacy = test_path("ego-v1.rel");
    copy_file(&source, &legacy);
    downgrade_header_to_v1(&legacy);
    let before = fs::read(&legacy).unwrap();
    let mut rel = Cva::open_v1_read_only(&legacy).unwrap();
    assert!(matches!(
        rel.update_ego_anchor(
            anchor.id,
            anchor.revision,
            anchor.priority,
            anchor.text.clone(),
        ),
        Err(crate::CvaError::Container(
            crate::ContainerError::StorageFormatUpgradeRequired
        ))
    ));
    assert_eq!(rel.ego_anchor(anchor.id), Some(anchor));
    drop(rel);
    assert_eq!(fs::read(&legacy).unwrap(), before);
}

#[cfg(feature = "history-oracle-fixtures")]
#[test]
fn authoritative_v1_owners_open_directly_without_changing_accepted_source_hashes() {
    use sha2::{Digest, Sha256};
    use std::io::Read;
    let baseline: serde_json::Value = serde_json::from_str(include_str!(
        "../../benchmarks/history-fixture-baseline-2026-10-19.json"
    ))
    .unwrap();
    let root = std::env::var_os("RELIQUARY_HISTORY_FIXTURES")
        .map(PathBuf::from)
        .expect("set RELIQUARY_HISTORY_FIXTURES to the authoritative fixture directory");
    fn hash(path: &std::path::Path) -> String {
        let mut file = fs::File::open(path).unwrap();
        let mut digest = Sha256::new();
        let mut buffer = [0_u8; 65536];
        loop {
            let count = file.read(&mut buffer).unwrap();
            if count == 0 {
                break;
            }
            digest.update(&buffer[..count]);
        }
        format!("{:x}", digest.finalize())
    }
    let fixtures = baseline["fixtures"].as_object().unwrap();
    assert_eq!(fixtures.len(), 6);
    for (relative, accepted) in fixtures {
        let path = root.join(relative);
        let before = fs::metadata(&path).unwrap();
        assert_eq!(before.len(), accepted["bytes"].as_u64().unwrap());
        assert_eq!(hash(&path), accepted["sha256"].as_str().unwrap());
        match accepted["kind"].as_str().unwrap() {
            "rel" => {
                let owner = Cva::open_v1_read_only(&path).unwrap();
                assert_eq!(owner.container.version().major, 1);
            }
            "phy" => {
                let owner = Phylactery::open_v1_read_only(&path).unwrap();
                assert_eq!(owner.container.version().major, 1);
            }
            other => panic!("unexpected fixture kind {other}"),
        }
        assert_eq!(
            fs::metadata(&path).unwrap().modified().unwrap(),
            before.modified().unwrap()
        );
        assert_eq!(hash(&path), accepted["sha256"].as_str().unwrap());
    }
}
