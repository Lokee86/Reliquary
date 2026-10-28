use std::collections::{HashMap, HashSet};

use reliquary_graph::{COMMUNITY_ALGORITHM_VERSION, Community, CommunityId, CommunitySnapshot};
use reliquary_memory_core::MemoryId;
use reliquary_retrieval::memory_retrieval::{
    MemoryRetrievalConfig, MemoryRetrievalIndex, MemoryRetrievalSnapshot, RetrievalPolicyError,
};
use reliquary_vectors::CompatibilityProfileId;

fn id(byte: u8) -> MemoryId {
    MemoryId([byte; 32])
}

fn community_snapshot(members: &[MemoryId], graph_version: u64) -> CommunitySnapshot {
    CommunitySnapshot {
        generation: 7,
        derived_graph_version: graph_version,
        algorithm_version: COMMUNITY_ALGORITHM_VERSION,
        seed: 1,
        resolution: 1.0,
        quality: 1.0,
        communities: vec![Community {
            id: CommunityId([42; 32]),
            members: members.iter().map(|memory_id| memory_id.0).collect(),
        }],
    }
}

fn build_index(
    vectors: HashMap<MemoryId, Vec<f32>>,
    searchable: HashSet<MemoryId>,
) -> MemoryRetrievalIndex {
    let members = [id(1), id(2)];
    MemoryRetrievalIndex::build(MemoryRetrievalSnapshot {
        compatibility_profile_id: CompatibilityProfileId([9; 32]),
        memory_version: 3,
        memory_graph_version: 5,
        community_generation: Some(7),
        vector_bindings: vectors.len(),
        subcentroids_per_community: 4,
        community_snapshot: Some(community_snapshot(&members, 5)),
        vectors,
        searchable,
    })
    .unwrap()
}

#[test]
fn routed_candidate_union_keeps_unclustered_vectors_and_excludes_search_only_memories() {
    let vectors = HashMap::from([
        (id(1), vec![1.0, 0.0]),
        (id(2), vec![0.9, 0.1]),
        (id(3), vec![1.0, 0.0]),
    ]);
    let searchable = HashSet::from([id(1), id(2), id(3), id(4)]);
    let index = build_index(vectors, searchable);

    let plan = index
        .prepare_retrieval(
            &[1.0, 0.0],
            MemoryRetrievalConfig::default(),
            3,
            5,
            Some(7),
            3,
        )
        .unwrap();
    assert_eq!(plan.candidate_ids, vec![id(1), id(2), id(3)]);
    assert_eq!(plan.seeds[0].memory_id, id(1));
    assert_eq!(plan.global_memory_vectors, 3);
    assert_eq!(plan.routing_vectors_scored, 2);
}

#[test]
fn global_fallback_and_watermark_validation_are_pure() {
    let vectors = HashMap::from([(id(1), vec![1.0, 0.0]), (id(2), vec![0.0, 1.0])]);
    let index = MemoryRetrievalIndex::build(MemoryRetrievalSnapshot {
        compatibility_profile_id: CompatibilityProfileId([9; 32]),
        memory_version: 3,
        memory_graph_version: 5,
        community_generation: None,
        vector_bindings: vectors.len(),
        subcentroids_per_community: 4,
        community_snapshot: None,
        vectors,
        searchable: HashSet::from([id(1), id(2)]),
    })
    .unwrap();
    let config = MemoryRetrievalConfig::default();
    let fallback = index
        .prepare_retrieval(&[1.0, 0.0], config, 3, 5, None, 2)
        .unwrap();
    assert!(fallback.used_global_fallback);
    assert_eq!(
        fallback.mode_used,
        reliquary_retrieval::memory_retrieval::MemoryRetrievalMode::GlobalExact
    );
    assert_eq!(fallback.candidate_ids, vec![id(1), id(2)]);
    assert_eq!(
        index
            .prepare_retrieval(&[1.0, 0.0], config, 3, 5, None, 1)
            .unwrap_err(),
        RetrievalPolicyError::StaleIndex
    );
    assert_eq!(
        index
            .prepare_retrieval(&[1.0, 0.0], config, 3, 6, None, 2)
            .unwrap_err(),
        RetrievalPolicyError::StaleIndex
    );
}
