//! Current CVAINSC7/CVAINSC8 codec acceptance, independent of retired input formats.
use super::{
    InsomniaCompletion, InsomniaCompletionBody, decode_completion,
    decode_completion_with_body_spans, decode_embedded_version_range, encode_completion,
};
use crate::memory_model::MemoryRecord;
use crate::{
    CHRONOS_INFERENCE_CONTRACT_VERSION, EpisodeId, MemoryBodyId, MemoryEntityMention, MemoryId,
    MemoryRef, MemoryRoutingMetadata, MemorySourceRef, MemoryTemporalInference, MemoryTextField,
    TemporalIndicationKind, TemporalInference, TemporalInferenceResolution,
};
const GLOBAL_START: u64 = 5;
const TRANSACTION_TIME_NS: i64 = 900;
fn push_string(out: &mut Vec<u8>, value: &str) {
    out.extend_from_slice(&(value.len() as u32).to_le_bytes());
    out.extend_from_slice(value.as_bytes());
}

fn push_memory_ids(out: &mut Vec<u8>, ids: &[MemoryId]) {
    out.extend_from_slice(&(ids.len() as u32).to_le_bytes());
    for id in ids {
        out.extend_from_slice(&id.0);
    }
}

fn fixture_record(index: u8, global_version: u64, memory_version: u64) -> MemoryRecord {
    let id = MemoryId([index; 32]);
    let body_id = MemoryBodyId([index + 20; 32]);
    MemoryRecord {
        id,
        revision: 1,
        body_id,
        category: "decision".into(),
        memory_type: "project".into(),
        authority_kind: "direct".into(),
        temporal_status: "current".into(),
        scope: "private".into(),
        lifecycle_state: "extracted".into(),
        archived: index % 2 == 0,
        superseded_by: None,
        parent_id: Some(MemoryId([99; 32])),
        source_node_id: Some(format!("assistant-{index}")),
        content_source_conversation_id: Some("retained-conversation".into()),
        content_source_node_id: Some(format!("user-{index}")),
        grounding_source_conversation_id: Some("grounding-conversation".into()),
        grounding_source_node_id: Some(format!("grounding-{index}")),
        source_episode_id: Some(reliquary_memory_core::SourceEpisodeId(
            (EpisodeId([index + 40; 32])).0,
        )),
        source_time_ns: Some(700 + index as i64),
        source_ref: Some(MemorySourceRef {
            owner_id: "rel-retained-fixture".into(),
            principal_id: Some("phy-retained-principal".into()),
            source_episode_id: reliquary_memory_core::SourceEpisodeId(
                (EpisodeId([index + 40; 32])).0,
            ),
            source_node_id: format!("source-{index}"),
            content_source_conversation_id: Some("content-conversation".into()),
            content_source_node_id: Some(format!("content-{index}")),
            grounding_source_conversation_id: Some("grounding-conversation".into()),
            grounding_source_node_id: Some(format!("grounding-{index}")),
        }),
        temporal_inference: Some(MemoryTemporalInference {
            body_id,
            source_time_ns: Some(700 + index as i64),
            inference: crate::memory_temporal_conversion::to_memory_inference(TemporalInference {
                model: "chronos-retained-test".into(),
                contract_version: CHRONOS_INFERENCE_CONTRACT_VERSION.into(),
                resolutions: vec![TemporalInferenceResolution {
                    start_byte: 0,
                    end_byte: 8,
                    kind: TemporalIndicationKind::Recurrence,
                    evidence: "biweekly".into(),
                    canonical_expression: "every two weeks".into(),
                }],
            }),
        }),
        mutation_id: format!("retained-record-{index}"),
        created_at_ns: 100 + index as i64,
        updated_at_ns: 200 + index as i64,
        global_version,
        memory_version,
    }
}

fn fixture_completion(version: u8) -> InsomniaCompletion {
    let globals = if version == 1 {
        [GLOBAL_START, GLOBAL_START + 6]
    } else {
        [GLOBAL_START, GLOBAL_START + 1]
    };
    let records = vec![
        fixture_record(1, globals[0], 8),
        fixture_record(2, globals[1], 13),
    ];
    let memory_ids = records.iter().map(|record| record.id).collect::<Vec<_>>();
    let bodies = if version >= 2 {
        records
            .iter()
            .map(|record| InsomniaCompletionBody {
                id: record.body_id,
                bytes: format!("body-for-{}", record.id.0[0]).into_bytes(),
            })
            .collect()
    } else {
        Vec::new()
    };
    let external_memory_refs = if version >= 3 {
        vec![MemoryRef {
            owner_id: "phy-external-retained-fixture".into(),
            memory_id: MemoryId([71; 32]),
        }]
    } else {
        Vec::new()
    };
    let routing_metadata = if version >= 5 {
        records
            .iter()
            .map(|record| MemoryRoutingMetadata {
                memory_id: record.id,
                body_id: record.body_id,
                entity_mentions: vec![MemoryEntityMention {
                    field: MemoryTextField::Title,
                    start_byte: 0,
                    end_byte: 8,
                    text: "Decision".into(),
                }],
            })
            .collect()
    } else {
        Vec::new()
    };
    let has_birth_proof = version == 6;
    InsomniaCompletion {
        episode_id: EpisodeId([17; 32]),
        attempt: 3,
        started_at_ns: 100,
        completed_at_ns: 110,
        transaction_time_ns: if version >= 4 { TRANSACTION_TIME_NS } else { 0 },
        extractor_model: "retained-model".into(),
        extractor_version: "retained-version".into(),
        rejected_count: 2,
        memory_ids: memory_ids.clone(),
        external_memory_refs,
        global_version_start: GLOBAL_START,
        bodies,
        records,
        routing_metadata,
        freshness_birth_turn: has_birth_proof.then_some(77),
        freshness_policy_version: has_birth_proof
            .then_some(crate::freshness::FRESHNESS_POLICY_VERSION as u64),
        freshness_birth_memory_ids: if has_birth_proof {
            memory_ids
        } else {
            Vec::new()
        },
    }
}

fn assert_completion_fields_equal(left: &InsomniaCompletion, right: &InsomniaCompletion) {
    assert_eq!(right.episode_id, left.episode_id);
    assert_eq!(right.attempt, left.attempt);
    assert_eq!(right.started_at_ns, left.started_at_ns);
    assert_eq!(right.completed_at_ns, left.completed_at_ns);
    assert_eq!(right.transaction_time_ns, left.transaction_time_ns);
    assert_eq!(right.extractor_model, left.extractor_model);
    assert_eq!(right.extractor_version, left.extractor_version);
    assert_eq!(right.rejected_count, left.rejected_count);
    assert_eq!(right.memory_ids, left.memory_ids);
    assert_eq!(right.external_memory_refs, left.external_memory_refs);
    assert_eq!(right.global_version_start, left.global_version_start);
    assert_eq!(right.bodies.len(), left.bodies.len());
    for (actual, expected) in right.bodies.iter().zip(&left.bodies) {
        assert_eq!(actual.id, expected.id);
        assert_eq!(actual.bytes, expected.bytes);
    }
    assert_eq!(right.records, left.records);
    assert_eq!(right.routing_metadata, left.routing_metadata);
    assert_eq!(right.freshness_birth_turn, left.freshness_birth_turn);
    assert_eq!(
        right.freshness_policy_version,
        left.freshness_policy_version
    );
    assert_eq!(
        right.freshness_birth_memory_ids,
        left.freshness_birth_memory_ids
    );
}

fn retained_frame(layout: u8, value: &InsomniaCompletion) -> Vec<u8> {
    let mut out = b"CVAINSC8".to_vec();
    out.push(layout);
    out.extend(value.episode_id.0);
    out.extend(value.attempt.to_le_bytes());
    out.extend(value.started_at_ns.to_le_bytes());
    out.extend(value.completed_at_ns.to_le_bytes());
    out.extend(value.rejected_count.to_le_bytes());
    if layout >= 2 {
        out.extend(value.global_version_start.to_le_bytes());
        out.extend((value.records.len() as u32).to_le_bytes());
    }
    if layout >= 4 {
        out.extend(value.transaction_time_ns.to_le_bytes());
    }
    push_string(&mut out, &value.extractor_model);
    push_string(&mut out, &value.extractor_version);
    push_memory_ids(&mut out, &value.memory_ids);
    if layout >= 3 {
        out.extend((value.external_memory_refs.len() as u32).to_le_bytes());
        for reference in &value.external_memory_refs {
            push_string(&mut out, &reference.owner_id);
            out.extend(reference.memory_id.0);
        }
    }
    if layout >= 2 {
        out.extend((value.bodies.len() as u32).to_le_bytes());
        for body in &value.bodies {
            out.extend(body.id.0);
            out.extend((body.bytes.len() as u32).to_le_bytes());
            out.extend(&body.bytes);
        }
    }
    out.extend((value.records.len() as u32).to_le_bytes());
    for record in &value.records {
        let creation = crate::memory_compact_contract::MemoryCreation::new(record.clone()).unwrap();
        let bytes = crate::memory_compact_codec::encode_creation(&creation).unwrap();
        out.extend((bytes.len() as u32).to_le_bytes());
        out.extend(bytes);
    }
    if layout >= 5 {
        out.extend((value.routing_metadata.len() as u32).to_le_bytes());
        for routing in &value.routing_metadata {
            let bytes = crate::memory_routing_codec::encode(routing).unwrap();
            out.extend((bytes.len() as u32).to_le_bytes());
            out.extend(bytes);
        }
    }
    if layout == 6 {
        out.extend(value.freshness_birth_turn.unwrap().to_le_bytes());
        out.extend(value.freshness_policy_version.unwrap().to_le_bytes());
        push_memory_ids(&mut out, &value.freshness_birth_memory_ids);
    }
    out
}
#[test]
fn current_retained_layouts_preserve_all_fields_and_clock_observability() {
    for layout in 1..=6 {
        let expected = fixture_completion(layout);
        let bytes = retained_frame(layout, &expected);
        let actual = decode_completion(&bytes).unwrap().unwrap();
        assert_completion_fields_equal(&expected, &actual);
        let range = decode_embedded_version_range(&bytes).unwrap();
        if layout == 1 {
            assert_eq!(range, None);
        } else {
            assert_eq!(
                range,
                Some((
                    GLOBAL_START,
                    2,
                    (layout >= 4).then_some(TRANSACTION_TIME_NS)
                ))
            );
        }
        assert!(decode_completion(&bytes[..bytes.len() - 1]).is_err());
    }
}

#[test]
fn grouped_completion_body_spans_are_absolute_for_all_supported_layouts() {
    for layout in 1..=6 {
        let bytes = retained_frame(layout, &fixture_completion(layout));
        let (decoded, spans) = decode_completion_with_body_spans(&bytes).unwrap().unwrap();
        assert_eq!(spans.len(), decoded.bodies.len());
        for (span, body) in spans.iter().zip(&decoded.bodies) {
            assert_eq!(span.id, body.id);
            assert_eq!(&bytes[span.range.clone()], body.bytes.as_slice());
            assert_eq!(
                span.encoding,
                reliquary_memory_core::MemoryGroupedBodyEncoding::Raw
            );
        }
    }
}
#[test]
fn current_atomic_writer_and_retained_timed_layout_keep_identical_creation_facts() {
    for layout in [5, 6] {
        let value = fixture_completion(layout);
        let current = encode_completion(&value).unwrap();
        let retained = retained_frame(layout, &value);
        assert_completion_fields_equal(
            &decode_completion(&current).unwrap().unwrap(),
            &decode_completion(&retained).unwrap().unwrap(),
        );
    }
}
#[test]
fn retained_layout_bounds_and_invalid_range_fail_before_allocating_record_lists() {
    let mut bytes = retained_frame(6, &fixture_completion(6));
    bytes[8] = 7;
    assert!(decode_completion(&bytes).is_err());
    assert!(decode_embedded_version_range(&bytes).is_err());
    let mut bytes = retained_frame(3, &fixture_completion(3));
    bytes[65..73].fill(0);
    assert!(decode_embedded_version_range(&bytes).is_err());
    for end in [0, 8, 9, 64, 76, 84] {
        let bytes = retained_frame(6, &fixture_completion(6));
        let value = decode_completion(&bytes[..end]);
        if end >= 8 {
            assert!(value.is_err());
        } else {
            assert!(value.unwrap().is_none());
        }
    }
}
