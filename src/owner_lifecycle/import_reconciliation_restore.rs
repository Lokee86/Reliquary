//! Concrete owner-state restoration for the Import reconciliation coordinator.

use crate::owner_lifecycle::ReliquaryOwner;

pub(crate) fn freshness_policies_match(left: &ReliquaryOwner, right: &ReliquaryOwner) -> bool {
    left.freshness.policy() == right.freshness.policy()
}

pub(crate) fn publish_community_snapshot(
    owner: &mut ReliquaryOwner,
    owner_uuid: [u8; 16],
    snapshot: crate::CommunitySnapshot,
) -> Result<(), crate::CommunityError> {
    owner
        .communities
        .publish(&mut owner.container, &owner.graph, owner_uuid, snapshot)
}

pub(crate) fn ingest_materialized_dream_begin(
    owner: &mut ReliquaryOwner,
    owner_uuid: [u8; 16],
    pass: crate::InitialDreamPass,
) -> Result<(), String> {
    owner
        .dream_freshness
        .ingest_materialized_begin(owner_uuid, pass)
}

pub(crate) fn ingest_rebased_dream_payload(
    owner: &mut ReliquaryOwner,
    payload: &[u8],
) -> Result<(), String> {
    owner.dream_freshness.ingest(payload)
}

pub(crate) fn persist_entity_resolution(
    owner: &mut ReliquaryOwner,
    change: crate::entity_resolution_codec::DecodedEntityResolution,
) -> Result<(), crate::MemoryError> {
    owner
        .entity_resolutions
        .persist_change(&mut owner.container, change)
}

#[derive(Clone, Debug)]
pub(crate) struct HistoricalManifestOwnerFacts {
    pub(crate) graph_version: u64,
    pub(crate) memory_graph_version: u64,
    pub(crate) vector_version: u64,
    pub(crate) community_snapshot: Option<crate::CommunitySnapshot>,
    pub(crate) insomnia: crate::InsomniaStats,
}

pub(crate) fn import_watermarks(owner: &ReliquaryOwner) -> crate::imports::ImportWatermarks {
    crate::imports::ImportWatermarks {
        global_version: owner.latest_global_version(),
        archive_version: owner.archive.archive_version(),
        memory_version: owner.memories.memory_version(),
        entity_version: owner.entities.entity_version(),
        relationship_version: owner.relationships.relationship_version(),
        graph_version: owner.graph.graph_version(),
        memory_graph_version: owner.graph.memory_graph_version(),
        vector_version: owner.vector_generations.vector_version(),
    }
}

pub(crate) fn historical_manifest_owner_facts(
    owner: &ReliquaryOwner,
) -> HistoricalManifestOwnerFacts {
    HistoricalManifestOwnerFacts {
        graph_version: owner.graph.graph_version(),
        memory_graph_version: owner.graph.memory_graph_version(),
        vector_version: owner.vector_generations.vector_version(),
        community_snapshot: owner
            .communities
            .latest_if_live_members(&owner.graph)
            .cloned(),
        insomnia: owner.insomnia.stats(),
    }
}

pub(crate) fn reconciliation_requires_vector_rebuild(owner: &ReliquaryOwner) -> bool {
    owner.archive_vectors.stats().objects > 0
        || owner.packed_vectors.stats().objects > 0
        || owner.memory_vectors.stats().objects > 0
}

pub(crate) fn queue_replayed_episode_if_missing(
    owner: &mut ReliquaryOwner,
    episode: &crate::Episode,
    priority: crate::InsomniaPriority,
) -> Result<(), crate::InsomniaError> {
    if owner.insomnia.work(episode.id).is_none() {
        let source = crate::owner_lifecycle::insomnia_replay::episode_source(episode, priority);
        owner
            .insomnia
            .queue(&mut owner.container, &source, episode.finalized_at_ns)?;
    }
    Ok(())
}
