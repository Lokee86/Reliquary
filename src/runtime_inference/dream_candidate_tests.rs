use crate::dream_candidate_test_support::{install_vectors, memory, test_path};
use crate::dream_owner_candidates::dream_memory_context as dream_memory_context_from_parts;
use crate::memory_source_time::memory_source_timestamp_ns;
use crate::{Cva, DreamCandidateConfig, GraphRelationKind, MemoryDraft};

#[test]
fn semantic_lane_finds_the_known_nearest_memory_without_embedding_calls() {
    let mut cva = Cva::create(test_path("semantic.cva")).unwrap();
    let source = memory(
        &mut cva,
        "source",
        "Wall system",
        "Use cedar siding.",
        100,
        false,
    );
    let near = memory(
        &mut cva,
        "near",
        "West wall",
        "Cedar siding is selected.",
        90,
        false,
    );
    let far = memory(
        &mut cva,
        "far",
        "Payroll",
        "Payroll closes Friday.",
        80,
        false,
    );
    let profile = install_vectors(
        &mut cva,
        &[source, near, far],
        &[&[1.0, 0.0, 0.0], &[0.99, 0.1, 0.0], &[0.0, 1.0, 0.0]],
    );
    let config = DreamCandidateConfig {
        limit: 2,
        semantic_limit: 2,
        prior_semantic_quota: 0,
        lexical_limit: 0,
        temporal_limit: 0,
    };
    let result = cva.dream_candidates(profile, source, config).unwrap();
    let repeated = cva.dream_candidates(profile, source, config).unwrap();
    assert_eq!(result, repeated);
    assert_eq!(result.candidates[0].context.memory.id, near);
    assert_eq!(result.candidates[0].semantic_rank, Some(1));
    assert!(result.candidates[0].semantic_score.unwrap() > 0.99);
}

#[test]
fn prior_semantic_quota_uses_source_timestamps_and_survives_top_n_pressure() {
    let mut cva = Cva::create(test_path("prior.cva")).unwrap();
    let source = memory(
        &mut cva,
        "source",
        "Current",
        "Current wall choice.",
        100,
        false,
    );
    let newer_a = memory(
        &mut cva,
        "new-a",
        "Recent A",
        "Recent choice A.",
        120,
        false,
    );
    let newer_b = memory(
        &mut cva,
        "new-b",
        "Recent B",
        "Recent choice B.",
        110,
        false,
    );
    let old = memory(&mut cva, "old", "Old", "Old wall choice.", 10, false);
    let profile = install_vectors(
        &mut cva,
        &[source, newer_a, newer_b, old],
        &[&[1.0, 0.0], &[0.999, 0.01], &[0.998, 0.02], &[0.70, 0.70]],
    );
    let result = cva
        .dream_candidates(
            profile,
            source,
            DreamCandidateConfig {
                limit: 2,
                semantic_limit: 2,
                prior_semantic_quota: 1,
                lexical_limit: 0,
                temporal_limit: 0,
            },
        )
        .unwrap();
    assert!(
        result
            .candidates
            .iter()
            .any(|candidate| candidate.context.memory.id == old)
    );
    let old_candidate = result
        .candidates
        .iter()
        .find(|candidate| candidate.context.memory.id == old)
        .unwrap();
    assert_eq!(old_candidate.prior_semantic_rank, Some(1));
    assert_eq!(old_candidate.context.source_timestamp_ns, Some(10));
    assert_eq!(result.source.source_timestamp_ns, Some(100));
}

#[test]
fn lexical_lane_can_recover_an_unembedded_candidate_and_excludes_archived_memories() {
    let mut cva = Cva::create(test_path("lexical.cva")).unwrap();
    let source = memory(
        &mut cva,
        "source",
        "Exterior wall",
        "Use cedar siding on west elevation.",
        100,
        false,
    );
    let lexical = memory(
        &mut cva,
        "lexical",
        "West elevation siding",
        "Cedar siding installation details.",
        90,
        false,
    );
    let archived = memory(
        &mut cva,
        "archived",
        "West elevation siding",
        "Cedar siding on west elevation.",
        80,
        true,
    );
    let profile = install_vectors(&mut cva, &[source], &[&[1.0, 0.0]]);
    let result = cva
        .dream_candidates(
            profile,
            source,
            DreamCandidateConfig {
                limit: 4,
                semantic_limit: 0,
                prior_semantic_quota: 0,
                lexical_limit: 4,
                temporal_limit: 0,
            },
        )
        .unwrap();
    assert_eq!(result.candidates.len(), 1);
    assert_eq!(result.candidates[0].context.memory.id, lexical);
    assert_eq!(result.candidates[0].semantic_score, None);
    assert!(result.candidates[0].lexical_score > 0.0);
    assert!(
        !result
            .candidates
            .iter()
            .any(|candidate| candidate.context.memory.id == archived)
    );
}

#[test]
fn direct_memory_context_matches_candidate_context_builder() {
    let mut cva = Cva::create(test_path("direct-context.cva")).unwrap();
    let source = memory(
        &mut cva,
        "source",
        "Source",
        "Shared wall fact.",
        100,
        false,
    );
    let candidate = memory(
        &mut cva,
        "candidate",
        "Candidate",
        "Shared wall fact.",
        90,
        false,
    );
    let profile = install_vectors(
        &mut cva,
        &[source, candidate],
        &[&[1.0, 0.0], &[0.99, 0.01]],
    );
    let direct = cva.dream_memory_context(candidate).unwrap();
    let set = cva
        .dream_candidates(
            profile,
            source,
            DreamCandidateConfig {
                limit: 1,
                semantic_limit: 1,
                prior_semantic_quota: 0,
                lexical_limit: 0,
                temporal_limit: 0,
            },
        )
        .unwrap();
    assert_eq!(direct, set.candidates[0].context);
}

#[test]
fn source_context_keeps_graph_state_while_connected_pair_is_excluded() {
    let mut cva = Cva::create(test_path("graph.cva")).unwrap();
    let source = memory(
        &mut cva,
        "source",
        "Source",
        "Shared wall fact.",
        100,
        false,
    );
    let candidate = memory(
        &mut cva,
        "candidate",
        "Candidate",
        "Shared wall fact.",
        90,
        false,
    );
    let other = memory(&mut cva, "other", "Other", "Different detail.", 80, false);
    let profile = install_vectors(
        &mut cva,
        &[source, candidate, other],
        &[&[1.0, 0.0], &[0.99, 0.01], &[0.95, 0.05]],
    );
    cva.set_memory_relation(source, candidate, GraphRelationKind::Factual, true, 0)
        .unwrap();
    cva.set_memory_relation(candidate, other, GraphRelationKind::References, true, 1)
        .unwrap();

    let candidate_context = cva.dream_memory_context(candidate).unwrap();
    assert_eq!(candidate_context.graph_relations.len(), 2);

    let result = cva
        .dream_candidates(
            profile,
            source,
            DreamCandidateConfig {
                limit: 1,
                semantic_limit: 1,
                prior_semantic_quota: 0,
                lexical_limit: 0,
                temporal_limit: 0,
            },
        )
        .unwrap();
    assert_eq!(result.source.graph_relations.len(), 1);
    assert_eq!(result.candidates[0].context.memory.id, other);
    assert_eq!(result.candidates[0].context.graph_relations.len(), 1);
    assert!(
        !result
            .candidates
            .iter()
            .any(|item| item.context.memory.id == candidate)
    );
}

#[test]
fn previously_evaluated_pair_does_not_consume_candidate_capacity() {
    let mut cva = Cva::create(test_path("evaluated-pair.cva")).unwrap();
    let source = memory(
        &mut cva,
        "source",
        "Source",
        "Shared wall fact.",
        100,
        false,
    );
    let already_evaluated = memory(
        &mut cva,
        "already-evaluated",
        "Nearest",
        "Shared wall fact.",
        90,
        false,
    );
    let unexplored = memory(
        &mut cva,
        "unexplored",
        "Second nearest",
        "Shared wall detail.",
        80,
        false,
    );
    let profile = install_vectors(
        &mut cva,
        &[source, already_evaluated, unexplored],
        &[&[1.0, 0.0], &[0.999, 0.001], &[0.98, 0.02]],
    );
    cva.mark_dream_pair_evaluated(source, already_evaluated)
        .unwrap();

    let result = cva
        .dream_candidates(
            profile,
            source,
            DreamCandidateConfig {
                limit: 1,
                semantic_limit: 2,
                prior_semantic_quota: 0,
                lexical_limit: 0,
                temporal_limit: 0,
            },
        )
        .unwrap();
    assert_eq!(result.candidates.len(), 1);
    assert_eq!(result.candidates[0].context.memory.id, unexplored);
}

#[test]
fn owner_candidate_core_accepts_persisted_memory_source_time() {
    let mut cva = Cva::create(test_path("owner-core.cva")).unwrap();
    let (memory, _) = cva
        .publish_memory(
            None,
            0,
            MemoryDraft {
                category: "preference".into(),
                memory_type: "user".into(),
                authority_kind: "direct".into(),
                temporal_status: "unknown".into(),
                title: "Answer style".into(),
                content: "Prefer concise answers.".into(),
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
                source_time_ns: Some(123),
                mutation_id: "owner-core-source-time".into(),
                created_at_ns: 999,
                updated_at_ns: 999,
            },
        )
        .unwrap();
    let source_time = |memory: &crate::Memory| memory_source_timestamp_ns(memory);
    let context = dream_memory_context_from_parts(
        &mut cva.container,
        &cva.memories,
        &cva.graph,
        memory.id,
        &source_time,
    )
    .unwrap();
    assert_eq!(context.source_timestamp_ns, Some(123));
    assert!(context.graph_relations.is_empty());
}
