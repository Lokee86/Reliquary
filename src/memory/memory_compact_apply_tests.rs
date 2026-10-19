use super::*;
use crate::{
    EpisodeId, MemoryBodyId, MemoryId, MemorySourceRef, MemoryTemporalInference,
    TemporalIndicationKind, TemporalInference, TemporalInferenceResolution,
};

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

fn all_fields_successor(previous: &MemoryRecord) -> MemoryRecord {
    let mut next = successor(previous);
    next.category = "belief".into();
    next.memory_type = "user".into();
    next.authority_kind = "correction".into();
    next.temporal_status = "historical".into();
    next.scope = "global".into();
    next.lifecycle_state = "superseded".into();
    next.archived = true;
    next.superseded_by = Some(MemoryId([3; 32]));
    next.parent_id = Some(MemoryId([4; 32]));
    next.source_node_id = Some("source-node".into());
    next.content_source_conversation_id = Some("content-conversation".into());
    next.content_source_node_id = Some("content-node".into());
    next.grounding_source_conversation_id = Some("grounding-conversation".into());
    next.grounding_source_node_id = Some("grounding-node".into());
    next.source_episode_id = Some(EpisodeId([5; 32]));
    next.source_time_ns = Some(-123);
    next.source_ref = Some(MemorySourceRef {
        owner_id: "owner".into(),
        principal_id: Some("principal".into()),
        source_episode_id: EpisodeId([5; 32]),
        source_node_id: "source-node".into(),
        content_source_conversation_id: Some("content-conversation".into()),
        content_source_node_id: Some("content-node".into()),
        grounding_source_conversation_id: Some("grounding-conversation".into()),
        grounding_source_node_id: Some("grounding-node".into()),
    });
    next.temporal_inference = Some(MemoryTemporalInference {
        body_id: next.body_id,
        source_time_ns: next.source_time_ns,
        inference: TemporalInference {
            model: "test-model".into(),
            contract_version: "contract-1".into(),
            resolutions: vec![TemporalInferenceResolution {
                start_byte: 0,
                end_byte: 4,
                kind: TemporalIndicationKind::Relative,
                evidence: "yesterday".into(),
                canonical_expression: "2026-10-18".into(),
            }],
        },
    });
    next.created_at_ns = -50;
    next.updated_at_ns = -20;
    next
}

fn assert_rejected_without_mutation(previous: &MemoryRecord, delta: MemoryDelta) {
    let before = previous.clone();
    assert!(apply_delta(previous, &delta).is_err());
    assert_eq!(*previous, before);
}

#[test]
fn applies_every_typed_field_and_preserves_the_immutable_body_reference() {
    let previous = initial();
    let expected = all_fields_successor(&previous);
    let delta = MemoryDelta::between(&previous, &expected).unwrap();
    assert_eq!(delta.fields.mask(), (1 << 20) - 1);

    let before = previous.clone();
    let actual = apply_delta(&previous, &delta).unwrap();
    assert_eq!(actual, expected);
    assert_eq!(actual.id, previous.id);
    assert_eq!(actual.body_id, previous.body_id);
    assert_eq!(previous, before);
}

#[test]
fn explicit_optional_clears_are_applied_as_set_none() {
    let populated = all_fields_successor(&initial());
    let mut cleared = successor(&populated);
    cleared.superseded_by = None;
    cleared.parent_id = None;
    cleared.source_node_id = None;
    cleared.content_source_conversation_id = None;
    cleared.content_source_node_id = None;
    cleared.grounding_source_conversation_id = None;
    cleared.grounding_source_node_id = None;
    cleared.source_episode_id = None;
    cleared.source_time_ns = None;
    cleared.source_ref = None;
    cleared.temporal_inference = None;
    let delta = MemoryDelta::between(&populated, &cleared).unwrap();

    assert_eq!(delta.fields.superseded_by, MemoryFieldChange::Set(None));
    assert_eq!(delta.fields.source_ref, MemoryFieldChange::Set(None));
    assert_eq!(
        delta.fields.temporal_inference,
        MemoryFieldChange::Set(None)
    );
    assert_eq!(apply_delta(&populated, &delta).unwrap(), cleared);
}

#[test]
fn revision_with_no_changed_metadata_is_a_valid_empty_mask_publication() {
    let previous = initial();
    let next = successor(&previous);
    let delta = MemoryDelta::between(&previous, &next).unwrap();
    assert_eq!(delta.fields.mask(), 0);
    assert_eq!(apply_delta(&previous, &delta).unwrap(), next);
}

#[test]
fn rejects_bad_identity_base_revision_and_publication_envelope_without_mutation() {
    let previous = initial();
    let next = successor(&previous);
    let base = MemoryDelta::between(&previous, &next).unwrap();

    let mut wrong_id = base.clone();
    wrong_id.id = MemoryId([9; 32]);
    assert_rejected_without_mutation(&previous, wrong_id);

    let mut wrong_base = base.clone();
    wrong_base.previous_revision += 1;
    assert_rejected_without_mutation(&previous, wrong_base);

    let mut wrong_revision = base.clone();
    wrong_revision.revision += 1;
    assert_rejected_without_mutation(&previous, wrong_revision);

    let mut zero_global = base.clone();
    zero_global.global_version = 0;
    assert_rejected_without_mutation(&previous, zero_global);

    let mut stale_memory = base.clone();
    stale_memory.memory_version = previous.memory_version;
    assert_rejected_without_mutation(&previous, stale_memory);

    let mut empty_mutation = base.clone();
    empty_mutation.mutation_id.clear();
    assert_rejected_without_mutation(&previous, empty_mutation);

    let mut repeated_mutation = base;
    repeated_mutation.mutation_id = previous.mutation_id.clone();
    assert_rejected_without_mutation(&previous, repeated_mutation);
}

#[test]
fn rejects_redundant_set_values_without_mutating_the_previous_record() {
    let previous = initial();
    let next = successor(&previous);
    let mut delta = MemoryDelta::between(&previous, &next).unwrap();
    delta.fields.category = MemoryFieldChange::Set(previous.category.clone());

    assert_rejected_without_mutation(&previous, delta);
}
