use crate::{
    Cva, EmbeddingEndpoint, EmbeddingMode, MemoryDraft, MemoryId, MemoryRetrievalConfig,
    MemoryRetrievalMode, PackedVectors, ScalarType, SimulatedEmbeddingEndpoint,
    VectorNormalization, VectorSchema,
};
use std::{fs, path::PathBuf};

fn path(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/test-tmp");
    fs::create_dir_all(&dir).unwrap();
    dir.join(format!("candidate-{}-{name}", uuid::Uuid::new_v4()))
}

fn draft(key: &str, title: &str, content: &str, archived: bool) -> MemoryDraft {
    MemoryDraft {
        category: "fact".into(),
        memory_type: "project".into(),
        authority_kind: "direct".into(),
        temporal_status: "current".into(),
        title: title.into(),
        content: content.into(),
        scope: "test".into(),
        lifecycle_state: "knowledge".into(),
        archived,
        superseded_by: None,
        parent_id: None,
        source_node_id: None,
        content_source_conversation_id: None,
        content_source_node_id: None,
        grounding_source_conversation_id: None,
        grounding_source_node_id: None,
        source_episode_id: None,
        source_time_ns: None,
        mutation_id: key.into(),
        created_at_ns: 1,
        updated_at_ns: 1,
    }
}

fn add(rel: &mut Cva, key: &str, title: &str, content: &str, archived: bool) -> MemoryId {
    rel.publish_memory(None, 0, draft(key, title, content, archived))
        .unwrap()
        .0
        .id
}

fn setup() -> (
    Cva,
    SimulatedEmbeddingEndpoint,
    crate::CompatibilityProfileId,
) {
    let rel = Cva::create_project(path(".rel")).unwrap();
    let endpoint = SimulatedEmbeddingEndpoint::new(8, VectorNormalization::L2, 417);
    let mut rel = rel;
    let profile = rel.establish_compatibility_profile(&endpoint).unwrap();
    (rel, endpoint, profile.id)
}

#[test]
fn candidate_pool_keeps_complete_union_beyond_ten_and_matches_legacy_exact_scores() {
    let (mut rel, endpoint, profile) = setup();
    let mut expected = Vec::new();
    for i in 0..16 {
        expected.push(add(
            &mut rel,
            &format!("needle-{i}"),
            &format!("Needle record {i}"),
            &format!("needle distinct evidence item {i}"),
            false,
        ));
    }
    let archived = add(
        &mut rel,
        "needle-archived",
        "Needle archived",
        "needle old evidence",
        true,
    );
    rel.build_missing_memory_vectors(profile, &endpoint)
        .unwrap();
    let query = endpoint
        .embed(
            EmbeddingMode::Document,
            &["needle distinct evidence item 3".into()],
        )
        .unwrap()
        .remove(0);
    let pool = rel
        .collect_memory_candidate_pool("needle", &query, profile)
        .unwrap();
    expected.sort();
    assert_eq!(pool.matched_memory_ids().collect::<Vec<_>>(), expected);
    assert_eq!(pool.candidates.len(), 16);
    assert!(!pool.matched_memory_ids().any(|id| id == archived));
    assert!(pool.candidates.iter().all(|candidate| {
        candidate.evidence.lexical_score.is_some() && candidate.evidence.semantic_score.is_some()
    }));

    let legacy = rel
        .retrieve_memories(
            profile,
            &query,
            MemoryRetrievalConfig {
                mode: MemoryRetrievalMode::GlobalExact,
                seed_limit: 64,
                ..MemoryRetrievalConfig::default()
            },
        )
        .unwrap();
    let pool_scores = pool
        .candidates
        .iter()
        .map(|candidate| {
            (
                candidate.memory_id,
                candidate.evidence.semantic_score.unwrap(),
            )
        })
        .collect::<std::collections::BTreeMap<_, _>>();
    assert_eq!(legacy.seeds.len(), pool_scores.len());
    for hit in legacy.seeds {
        assert_eq!(pool_scores.get(&hit.memory_id), Some(&hit.score));
    }
}

#[test]
fn candidate_pool_preserves_lane_disagreement_and_overlap_evidence() {
    let (mut rel, endpoint, profile) = setup();
    let lexical = add(
        &mut rel,
        "lexical",
        "Needle lexical",
        "needle ordinary facts",
        false,
    );
    let semantic = add(
        &mut rel,
        "semantic",
        "Semantic only",
        "rare phrase semantic target",
        false,
    );
    rel.build_missing_memory_vectors(profile, &endpoint)
        .unwrap();
    let query = endpoint
        .embed(
            EmbeddingMode::Document,
            &["Semantic only\n\nrare phrase semantic target".into()],
        )
        .unwrap()
        .remove(0);
    let pool = rel
        .collect_memory_candidate_pool("needle", &query, profile)
        .unwrap();
    let lexical_candidate = pool
        .candidates
        .iter()
        .find(|candidate| candidate.memory_id == lexical)
        .unwrap();
    let semantic_candidate = pool
        .candidates
        .iter()
        .find(|candidate| candidate.memory_id == semantic)
        .unwrap();
    assert!(lexical_candidate.evidence.lexical_score.is_some());
    assert!(lexical_candidate.evidence.semantic_score.is_some());
    assert_eq!(semantic_candidate.evidence.lexical_score, None);
    assert!((semantic_candidate.evidence.semantic_score.unwrap() - 1.0).abs() < 1e-6);
    assert!(
        lexical_candidate.evidence.semantic_score.unwrap()
            < semantic_candidate.evidence.semantic_score.unwrap()
    );
}

#[test]
fn absent_profile_or_vectors_keeps_lexical_matches() {
    let (mut rel, _, profile) = setup();
    let lexical = add(
        &mut rel,
        "lexical-only",
        "Cedar needle",
        "needle without a vector",
        false,
    );
    let missing_vectors = rel
        .collect_memory_candidate_pool("needle", &[1.0; 8], profile)
        .unwrap();
    assert_eq!(
        missing_vectors.matched_memory_ids().collect::<Vec<_>>(),
        vec![lexical]
    );
    assert_eq!(missing_vectors.candidates[0].evidence.semantic_score, None);

    let absent_profile = crate::CompatibilityProfileId([0xA5; 32]);
    let absent = rel
        .collect_memory_candidate_pool("needle", &[1.0; 8], absent_profile)
        .unwrap();
    assert_eq!(absent.compatibility_profile_id, absent_profile);
    assert_eq!(
        absent.matched_memory_ids().collect::<Vec<_>>(),
        vec![lexical]
    );
    assert_eq!(absent.candidates[0].evidence.semantic_score, None);
}

#[test]
fn invalid_query_and_invalid_stored_vectors_fail_closed() {
    let (mut rel, _, profile) = setup();
    let id = add(
        &mut rel,
        "invalid-vector",
        "Needle invalid",
        "needle vector",
        false,
    );
    let body_id = rel.memory_body_id(id).unwrap();
    assert!(
        rel.collect_memory_candidate_pool("needle", &[], profile)
            .is_err()
    );
    assert!(
        rel.collect_memory_candidate_pool("needle", &[0.0; 8], profile)
            .is_err()
    );
    assert!(
        rel.collect_memory_candidate_pool("needle", &[1.0; 7], profile)
            .is_err()
    );
    let wrong_width_schema = VectorSchema::new(7, ScalarType::F32).unwrap();
    let wrong_width = rel
        .put_packed_vectors(PackedVectors::from_bytes(wrong_width_schema, vec![0; 7 * 4]).unwrap())
        .unwrap();
    assert!(
        rel.put_memory_vectors(profile, wrong_width, vec![body_id])
            .is_err()
    );
    let mut non_finite = [1.0; 8];
    non_finite[3] = f32::NAN;
    assert!(
        rel.collect_memory_candidate_pool("needle", &non_finite, profile)
            .is_err()
    );

    let dimensions = rel.compatibility_profile(profile).unwrap().dimensions as usize;
    let schema = VectorSchema::new(dimensions as u32, ScalarType::F32).unwrap();
    let bytes = vec![0_u8; dimensions * 4];
    let packed = rel
        .put_packed_vectors(PackedVectors::from_bytes(schema, bytes).unwrap())
        .unwrap();
    rel.put_memory_vectors(profile, packed, vec![body_id])
        .unwrap();
    assert!(
        rel.collect_memory_candidate_pool("needle", &[1.0; 8], profile)
            .is_err()
    );
}

#[test]
fn non_finite_stored_vector_is_rejected_during_packed_decode() {
    let (mut rel, _, profile) = setup();
    let id = add(
        &mut rel,
        "invalid-nan-vector",
        "Needle NaN",
        "needle non-finite vector",
        false,
    );
    let body_id = rel.memory_body_id(id).unwrap();
    let dimensions = rel.compatibility_profile(profile).unwrap().dimensions as usize;
    let schema = VectorSchema::new(dimensions as u32, ScalarType::F32).unwrap();
    let mut bytes = vec![0_u8; dimensions * 4];
    bytes[0..4].copy_from_slice(&f32::NAN.to_le_bytes());
    let packed = rel
        .put_packed_vectors(PackedVectors::from_bytes(schema, bytes).unwrap())
        .unwrap();
    rel.put_memory_vectors(profile, packed, vec![body_id])
        .unwrap();
    assert!(matches!(
        rel.collect_memory_candidate_pool("needle", &[1.0; 8], profile),
        Err(crate::MemoryRetrievalError::CorruptVector(
            "non-finite vector"
        ))
    ));
}

#[test]
fn equal_lane_scores_have_deterministic_memory_id_order() {
    let (mut rel, endpoint, profile) = setup();
    let first = add(
        &mut rel,
        "tie-a",
        "Shared needle",
        "needle same body",
        false,
    );
    let second = add(
        &mut rel,
        "tie-b",
        "Shared needle",
        "needle same body",
        false,
    );
    rel.build_missing_memory_vectors(profile, &endpoint)
        .unwrap();
    let query = endpoint
        .embed(EmbeddingMode::Document, &["needle same body".into()])
        .unwrap()
        .remove(0);
    let pool = rel
        .collect_memory_candidate_pool("needle", &query, profile)
        .unwrap();
    let mut expected = vec![first, second];
    expected.sort();
    assert_eq!(pool.matched_memory_ids().collect::<Vec<_>>(), expected);
    assert_eq!(
        pool.candidates[0].evidence.lexical_score,
        pool.candidates[1].evidence.lexical_score
    );
    assert_eq!(
        pool.candidates[0].evidence.semantic_score,
        pool.candidates[1].evidence.semantic_score
    );
}
