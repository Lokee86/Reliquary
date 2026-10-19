use super::*;
use crate::MemoryBodyId;

fn initial() -> MemoryRecord {
    MemoryRecord {
        id: MemoryId([1; 32]),
        revision: 1,
        body_id: MemoryBodyId([2; 32]),
        category: "fact".into(),
        memory_type: "project".into(),
        authority_kind: "direct".into(),
        temporal_status: "current".into(),
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
        source_ref: None,
        temporal_inference: None,
        mutation_id: "creation".into(),
        created_at_ns: 100,
        updated_at_ns: 100,
        global_version: 4,
        memory_version: 2,
    }
}
fn successor(previous: &MemoryRecord) -> MemoryRecord {
    let mut next = previous.clone();
    next.revision += 1;
    next.global_version += 3;
    next.memory_version += 2;
    next.mutation_id = format!("revision-{}", next.revision);
    next
}

#[test]
fn optional_clear_set_and_unchanged_are_distinct_for_every_transition() {
    for (previous, next, expected) in [
        (None, None, MemoryFieldChange::Unchanged),
        (None, Some("a"), MemoryFieldChange::Set(Some("a"))),
        (Some("a"), None, MemoryFieldChange::Set(None)),
        (Some("a"), Some("a"), MemoryFieldChange::Unchanged),
        (Some("a"), Some("b"), MemoryFieldChange::Set(Some("b"))),
    ] {
        assert_eq!(MemoryFieldChange::between(&previous, &next), expected);
    }
    let mut previous = initial();
    previous.source_node_id = Some("node".into());
    let mut next = successor(&previous);
    next.source_node_id = None;
    next.parent_id = Some(MemoryId([3; 32]));
    let delta = MemoryDelta::between(&previous, &next).unwrap();
    assert_eq!(delta.fields.source_node_id, MemoryFieldChange::Set(None));
    assert_eq!(
        delta.fields.parent_id,
        MemoryFieldChange::Set(Some(MemoryId([3; 32])))
    );
    assert_eq!(delta.fields.source_episode_id, MemoryFieldChange::Unchanged);
    assert_eq!(delta.fields.mask(), (1 << 8) | (1 << 9));
}

#[test]
fn structured_proof_collections_use_ordered_whole_value_replacement() {
    let mut previous = initial();
    let resolution = |start_byte| crate::TemporalInferenceResolution {
        start_byte,
        end_byte: start_byte + 3,
        kind: crate::TemporalIndicationKind::Relative,
        evidence: "now".into(),
        canonical_expression: "2026-10-19".into(),
    };
    previous.temporal_inference = Some(MemoryTemporalInference {
        body_id: previous.body_id,
        source_time_ns: None,
        inference: crate::TemporalInference {
            model: "test".into(),
            contract_version: "1".into(),
            resolutions: vec![resolution(0), resolution(4)],
        },
    });
    let mut next = successor(&previous);
    next.temporal_inference
        .as_mut()
        .unwrap()
        .inference
        .resolutions
        .reverse();
    let delta = MemoryDelta::between(&previous, &next).unwrap();
    assert_eq!(
        delta.fields.temporal_inference,
        MemoryFieldChange::Set(next.temporal_inference.clone())
    );
    assert_eq!(delta.fields.mask(), 1 << 17);
    next.temporal_inference = None;
    assert_eq!(
        MemoryDelta::between(&previous, &next)
            .unwrap()
            .fields
            .temporal_inference,
        MemoryFieldChange::Set(None)
    );
    next.temporal_inference = previous.temporal_inference.clone();
    assert_eq!(
        MemoryDelta::between(&previous, &next)
            .unwrap()
            .fields
            .mask(),
        0
    );
}

#[test]
fn creation_retains_exact_initial_record_and_requires_first_revision_coordinates() {
    let record = initial();
    assert_eq!(MemoryCreation::new(record.clone()).unwrap().record, record);
    let mut bad = record.clone();
    bad.revision = 2;
    assert!(MemoryCreation::new(bad).is_err());
    for owner_clock in [false, true] {
        let mut bad = record.clone();
        if owner_clock {
            bad.memory_version = 0;
        } else {
            bad.global_version = 0;
        }
        assert!(MemoryCreation::new(bad).is_err());
    }
}

#[test]
fn contracts_accept_retained_v1_creation_and_revision_without_reinterpreting_fields() {
    let directory = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(".test-tmp")
        .join(format!("memory-contract-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("owner.rel");
    let mut rel = crate::Cva::create(&path).unwrap();
    let (memory, changed) = rel
        .memories
        .publish_legacy_fixture(
            &mut rel.container,
            None,
            0,
            crate::entity_candidate_test_support::memory_draft(
                "contract-v1-birth",
                "Server",
                "The server accepts requests.",
            ),
        )
        .unwrap();
    assert!(changed);
    let id = memory.id;
    let mut later = crate::entity_candidate_test_support::memory_draft(
        "contract-v1-later",
        "Server",
        "The server accepts requests.",
    );
    later.temporal_status = "historical".into();
    later.updated_at_ns = 2;
    rel.memories
        .publish_legacy_fixture(&mut rel.container, Some(id), 1, later)
        .unwrap();
    let accepted = rel.memories.records().to_vec();
    drop(rel);
    let mut bytes = std::fs::read(&path).unwrap();
    bytes[8..10].copy_from_slice(&1_u16.to_le_bytes());
    std::fs::write(&path, &bytes).unwrap();
    let v1 = crate::Cva::open_v1_read_only(&path).unwrap();
    assert_eq!(v1.memories.records(), accepted);
    assert_eq!(
        MemoryCreation::new(accepted[0].clone()).unwrap().record,
        accepted[0]
    );
    let delta = MemoryDelta::between(&accepted[0], &accepted[1]).unwrap();
    assert_eq!(
        delta.fields.temporal_status,
        MemoryFieldChange::Set("historical".into())
    );
    assert_eq!(delta.id, id);
    assert_eq!(delta.global_version, accepted[1].global_version);
    assert_eq!(delta.memory_version, accepted[1].memory_version);
    drop(v1);
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn delta_retains_envelope_and_allows_revision_with_no_metadata_changes() {
    let previous = initial();
    let next = successor(&previous);
    let delta = MemoryDelta::between(&previous, &next).unwrap();
    assert_eq!(delta.fields.mask(), 0);
    assert_eq!(
        (delta.id, delta.previous_revision, delta.revision),
        (previous.id, 1, 2)
    );
    assert_eq!((delta.global_version, delta.memory_version), (7, 4));
    assert_eq!(delta.mutation_id, next.mutation_id);
    // V1 can revise created_at_ns; preserve it instead of imposing a new rule.
    let mut changed = next;
    changed.created_at_ns = -50;
    assert_eq!(
        MemoryDelta::between(&previous, &changed)
            .unwrap()
            .fields
            .created_at_ns,
        MemoryFieldChange::Set(-50)
    );
}

#[test]
fn immutable_identity_body_revision_and_clock_failures_reject() {
    let previous = initial();
    for case in 0..7 {
        let mut next = successor(&previous);
        match case {
            0 => next.id = MemoryId([9; 32]),
            1 => next.body_id = MemoryBodyId([9; 32]),
            2 => next.revision += 1,
            3 => next.global_version = previous.global_version,
            4 => next.memory_version = previous.memory_version,
            5 => next.mutation_id.clear(),
            _ => next.mutation_id = previous.mutation_id.clone(),
        }
        assert!(
            MemoryDelta::between(&previous, &next).is_err(),
            "case {case}"
        );
    }
}

#[test]
fn masks_have_fixed_order_and_reject_every_reserved_bit() {
    assert_eq!(
        MEMORY_DELTA_FIELD_ORDER,
        &[
            "category",
            "memory_type",
            "authority_kind",
            "temporal_status",
            "scope",
            "lifecycle_state",
            "archived",
            "superseded_by",
            "parent_id",
            "source_node_id",
            "content_source_conversation_id",
            "content_source_node_id",
            "grounding_source_conversation_id",
            "grounding_source_node_id",
            "source_episode_id",
            "source_time_ns",
            "source_ref",
            "temporal_inference",
            "created_at_ns",
            "updated_at_ns"
        ]
    );
    assert_eq!(MEMORY_DELTA_KNOWN_MASK, (1 << 20) - 1);
    for bit in 0..64 {
        assert_eq!(MemoryDelta::validate_mask(1 << bit).is_ok(), bit < 20);
    }
    let previous = initial();
    let mut a = successor(&previous);
    let mut b = a.clone();
    a.archived = true;
    a.scope = "personal".into();
    b.scope = "personal".into();
    b.archived = true;
    assert_eq!(
        MemoryDelta::between(&previous, &a).unwrap(),
        MemoryDelta::between(&previous, &b).unwrap()
    );
    assert_eq!(
        MemoryDelta::between(&previous, &a).unwrap().fields.mask(),
        (1 << 4) | (1 << 6)
    );
}
