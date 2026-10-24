use crate::community_store::CommunityStore;
use crate::freshness::FreshnessPolicy;
use crate::graph_store::GraphStore;
use crate::{FreshnessEventProof, MemoryId, PropagationEvent};

pub(crate) fn retry_exists(
    store: &crate::freshness_storage::FreshnessStore,
    use_id: &str,
    accepted_turn: u64,
    memories: &[MemoryId],
) -> Result<bool, String> {
    store
        .resolve_accepted_use(use_id, accepted_turn, memories)
        .map(|receipt| receipt.is_some())
}

pub(crate) fn policy(store: &crate::freshness_storage::FreshnessStore) -> FreshnessPolicy {
    store.policy()
}

pub(crate) fn ego_grades(
    store: &crate::freshness_storage::FreshnessStore,
    memory_ids: &[MemoryId],
    as_of_turn: u64,
    activity_head: u64,
) -> Result<Vec<(MemoryId, i16, crate::FreshnessState)>, String> {
    crate::ego_grade_read::EgoGradeRead::new(store).grades(memory_ids, as_of_turn, activity_head)
}

pub(crate) fn set_policy_before_enrollment(
    store: &mut crate::freshness_storage::FreshnessStore,
    policy: FreshnessPolicy,
) -> Result<(), String> {
    store.set_policy_before_enrollment(policy)
}

pub(crate) fn has_records(store: &crate::freshness_storage::FreshnessStore) -> bool {
    !store.records().is_empty()
}

pub(crate) fn initialize_payload(
    store: &crate::freshness_storage::FreshnessStore,
    owner: [u8; 16],
    id: MemoryId,
    turn: u64,
    record: crate::FreshnessRecord,
) -> Vec<u8> {
    store.encode_initialize(owner, id, turn, record)
}

pub(crate) fn has_record(store: &crate::freshness_storage::FreshnessStore, id: MemoryId) -> bool {
    store.records().contains_key(&id)
}

pub(crate) fn current_record(
    store: &crate::freshness_storage::FreshnessStore,
    id: MemoryId,
    turn: u64,
) -> Option<crate::FreshnessRecord> {
    store.current_rewrite_record(id, turn)
}

pub(crate) fn graph_proof(
    graph: &GraphStore,
    communities: &CommunityStore,
) -> (u64, Option<crate::CommunitySnapshot>, FreshnessEventProof) {
    let graph_version = graph.memory_graph_version();
    let snapshot = communities
        .latest_if_live_members(graph)
        .filter(|snapshot| snapshot.derived_graph_version == graph_version)
        .cloned();
    let proof = FreshnessEventProof {
        graph_version,
        community_generation: snapshot.as_ref().map(|snapshot| snapshot.generation),
        community_graph_version: snapshot
            .as_ref()
            .map(|snapshot| snapshot.derived_graph_version),
    };
    (graph_version, snapshot, proof)
}

pub(crate) fn proof_available(
    graph: &GraphStore,
    communities: &CommunityStore,
    proof: FreshnessEventProof,
) -> bool {
    proof.graph_version <= graph.memory_graph_version()
        && match (proof.community_generation, proof.community_graph_version) {
            (Some(generation), Some(version)) => {
                version == proof.graph_version && communities.contains_snapshot(generation, version)
            }
            (None, None) => true,
            _ => false,
        }
}

pub(crate) fn propagation(
    events: &[PropagationEvent],
    adjacency: &crate::freshness::MemoryAdjacency,
    memberships: &std::collections::BTreeMap<MemoryId, crate::CommunityId>,
    workers: usize,
    policy: &FreshnessPolicy,
) -> Result<Vec<std::collections::BTreeMap<MemoryId, i16>>, crate::freshness::PropagationError> {
    crate::freshness::propagate_event_candidates_with_policy(
        events,
        adjacency,
        memberships,
        workers,
        policy,
    )
}

pub(crate) fn prepare_payload(
    store: &crate::freshness_storage::FreshnessStore,
    owner_uuid: [u8; 16],
    use_id: String,
    accepted_turn: u64,
    memories: Vec<MemoryId>,
    proof: FreshnessEventProof,
    outcomes: Vec<std::collections::BTreeMap<MemoryId, i16>>,
) -> Result<Vec<u8>, String> {
    store.prepare_accepted_use_batch(owner_uuid, use_id, accepted_turn, memories, proof, outcomes)
}

pub(crate) fn ingest_appended(
    store: &mut crate::freshness_storage::FreshnessStore,
    payload: &[u8],
) -> Result<(), String> {
    store.ingest_appended(payload)
}
