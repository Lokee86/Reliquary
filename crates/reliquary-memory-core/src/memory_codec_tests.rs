use crate::memory_codec::{decode_record, encode_record};
use crate::memory_model::MemoryRecord;
use crate::{
    EpisodeId, MemoryBodyId, MemoryId, MemorySourceRef, MemoryTemporalInference,
    TemporalIndicationKind, TemporalInference, TemporalInferenceResolution,
};

#[test]
fn memory_record_v8_round_trips_source_reference_principal_and_temporal_status() {
    let mut record = record("adoption");
    record.source_time_ns = Some(42);
    record.source_ref = Some(MemorySourceRef {
        owner_id: "rel-00000000-0000-0000-0000-000000000001".into(),
        principal_id: Some("phy-00000000-0000-0000-0000-000000000002".into()),
        source_episode_id: EpisodeId([7; 32]),
        source_node_id: "u1".into(),
        content_source_conversation_id: Some("c1".into()),
        content_source_node_id: Some("a0".into()),
        grounding_source_conversation_id: None,
        grounding_source_node_id: None,
    });
    record.temporal_inference = Some(MemoryTemporalInference {
        body_id: record.body_id,
        source_time_ns: record.source_time_ns,
        inference: (TemporalInference {
            model: "chronos-test".into(),
            contract_version: "chronos-temporal-inference-v1".into(),
            resolutions: vec![TemporalInferenceResolution {
                start_byte: 0,
                end_byte: 8,
                kind: TemporalIndicationKind::Recurrence,
                evidence: "biweekly".into(),
                canonical_expression: "every two weeks".into(),
            }],
        })
        .into(),
    });
    let decoded = decode_record(&encode_record(&record).unwrap())
        .unwrap()
        .unwrap();
    assert_eq!(decoded.authority_kind, "adoption");
    assert_eq!(decoded.source_time_ns, Some(42));
    assert_eq!(decoded.source_ref, record.source_ref);
    assert_eq!(decoded.temporal_status, "current");
    assert_eq!(decoded.temporal_inference, record.temporal_inference);
    assert_eq!(decoded.category, record.category);
    assert_eq!(decoded.memory_type, record.memory_type);
}

fn record(authority_kind: &str) -> MemoryRecord {
    MemoryRecord {
        id: MemoryId([1; 32]),
        revision: 1,
        body_id: MemoryBodyId([2; 32]),
        category: "decision".into(),
        memory_type: "project".into(),
        authority_kind: authority_kind.into(),
        temporal_status: "current".into(),
        scope: "private".into(),
        lifecycle_state: "extracted".into(),
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
        mutation_id: "codec-test".into(),
        created_at_ns: 1,
        updated_at_ns: 2,
        global_version: 3,
        memory_version: 4,
    }
}
