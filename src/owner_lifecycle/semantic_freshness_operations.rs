//! Concrete REL owner reads and durable writes for live semantic Freshness.

use crate::owner_lifecycle::ReliquaryOwner;
use crate::owner_lifecycle::semantic_archive_read::SemanticArchiveRead;
use crate::owner_lifecycle::semantic_source::{
    AcceptedSemanticSource, SemanticSourceReadParts, SemanticSourceReadiness,
};
use crate::{
    CompatibilityProfileId, FragmentId, MemoryBodyId, MemoryId, VectorGeneration,
    VectorGenerationId,
};

pub(crate) fn archive_read(owner: &ReliquaryOwner) -> SemanticArchiveRead<'_> {
    SemanticArchiveRead::new(SemanticSourceReadParts {
        container: &owner.container,
        archive: &owner.archive,
        freshness: &owner.freshness,
        generations: &owner.vector_generations,
        compatibility_profiles: &owner.compatibility_profiles,
        archive_vectors: &owner.archive_vectors,
        packed_vectors: &owner.packed_vectors,
    })
}

pub(crate) fn prepare_fragment(
    owner: &ReliquaryOwner,
    id: FragmentId,
    generation_id: VectorGenerationId,
) -> Result<SemanticSourceReadiness, super::OwnerError> {
    crate::owner_lifecycle::semantic_source::prepare_semantic_fragment(
        SemanticSourceReadParts {
            container: &owner.container,
            archive: &owner.archive,
            freshness: &owner.freshness,
            generations: &owner.vector_generations,
            compatibility_profiles: &owner.compatibility_profiles,
            archive_vectors: &owner.archive_vectors,
            packed_vectors: &owner.packed_vectors,
        },
        id,
        generation_id,
    )
}

pub(crate) fn fragment_sources(
    owner: &ReliquaryOwner,
    id: FragmentId,
) -> Result<Vec<(String, String)>, super::OwnerError> {
    crate::owner_lifecycle::semantic_source::fragment_sources(&owner.archive, id)
}

pub(crate) fn fragment_position(
    owner: &ReliquaryOwner,
    id: FragmentId,
) -> Result<Option<u64>, super::OwnerError> {
    crate::owner_lifecycle::semantic_source::fragment_position(&owner.archive, id)
}

pub(crate) fn source_excludes_memory(
    owner: &ReliquaryOwner,
    source: &AcceptedSemanticSource,
    memory: &crate::Memory,
) -> Result<bool, super::OwnerError> {
    let episode_id = memory.source_episode_id.or_else(|| {
        memory
            .source_ref
            .as_ref()
            .map(|reference| reference.source_episode_id)
    });
    let Some(episode_id) = episode_id else {
        return Ok(false);
    };
    let episode = owner
        .archive
        .episode(episode_id)
        .ok_or_else(|| super::OwnerError::Freshness("missing semantic target Episode".into()))?;
    let sources = source
        .fragment
        .as_ref()
        .map(|fragment| fragment.sources.clone())
        .unwrap_or_else(|| vec![(source.node.conversation_id.clone(), source.node.id.clone())]);
    for (conversation, node) in &sources {
        if conversation == &episode.conversation_id
            && owner.archive.episode_contains_node(episode_id, node)?
        {
            return Ok(true);
        }
    }
    Ok(false)
}

pub(crate) fn vector_generation(
    owner: &ReliquaryOwner,
    id: VectorGenerationId,
) -> Option<VectorGeneration> {
    owner.vector_generations.generation(id)
}

pub(crate) fn latest_generation(owner: &ReliquaryOwner) -> Option<VectorGeneration> {
    owner
        .vector_generations
        .generations()
        .iter()
        .max_by_key(|generation| generation.vector_version)
        .copied()
}

pub(crate) fn fragments(owner: &ReliquaryOwner) -> Vec<crate::Fragment> {
    owner.archive.fragments()
}

pub(crate) fn accepted_node(owner: &ReliquaryOwner, node: &crate::Node) -> bool {
    owner.archive.nodes.get(&node.conversation_id, &node.id) == Some(node)
}

pub(crate) fn semantic_memory_targets(
    owner: &ReliquaryOwner,
    profile: CompatibilityProfileId,
) -> Result<
    Vec<crate::owner_lifecycle::public_consumer_operations::SemanticMemoryTarget>,
    crate::MemoryError,
> {
    crate::owner_lifecycle::public_consumer_operations::semantic_memory_targets(
        &owner.memories,
        &owner.memory_vectors,
        profile,
    )
}

/// Hydrates exactly one current Memory body for the caller's bounded pass.
pub(crate) fn memory(
    owner: &ReliquaryOwner,
    id: MemoryId,
) -> Result<crate::Memory, crate::MemoryError> {
    owner.memories.memory(&owner.container, id)
}

pub(crate) fn target_timing(owner: &ReliquaryOwner, id: MemoryId) -> (Option<u64>, Option<u64>) {
    (
        owner
            .freshness
            .records()
            .get(&id)
            .and_then(|record| record.admitted_at_turn),
        owner.freshness.birth_turn(id),
    )
}

pub(crate) fn event_seen(owner: &ReliquaryOwner, id: &str) -> Result<bool, String> {
    owner.freshness.contains_event(id)
}

pub(crate) fn vector_location(
    owner: &ReliquaryOwner,
    profile: CompatibilityProfileId,
    body: MemoryBodyId,
) -> Option<crate::MemoryVectorLocation> {
    owner.memory_vectors.location(profile, body)
}

pub(crate) fn memory_vector_set(
    owner: &ReliquaryOwner,
    id: crate::MemoryVectorId,
) -> Result<crate::MemoryVectorSet, crate::MemoryVectorError> {
    owner.memory_vectors.get(&owner.container, id)
}

pub(crate) fn packed_vectors(
    owner: &ReliquaryOwner,
    id: crate::PackedVectorId,
) -> Result<lodestone_packed::PackedVectors, crate::PackedVectorError> {
    owner.packed_vectors.get(&owner.container, id)
}

pub(crate) fn encode_semantic_event(
    owner: &ReliquaryOwner,
    owner_uuid: [u8; 16],
    evidence: crate::freshness_storage::SemanticEventEvidence,
    effects: std::collections::BTreeMap<MemoryId, i16>,
) -> Result<Vec<u8>, String> {
    owner
        .freshness
        .encode_semantic_event(owner_uuid, evidence, effects)
}

pub(crate) fn validate_semantic_preappend(
    owner: &ReliquaryOwner,
    owner_uuid: [u8; 16],
    payload: &[u8],
) -> Result<(), String> {
    owner
        .freshness
        .validate_semantic_preappend(owner_uuid, payload)
}

pub(crate) fn append_semantic_event(
    owner: &mut ReliquaryOwner,
    owner_uuid: [u8; 16],
    payload: &[u8],
) -> Result<(), crate::CvaError> {
    crate::owner_lifecycle::freshness_operations::persist_policy_if_needed(owner, owner_uuid)
        .map_err(crate::CvaError::Freshness)?;
    crate::owner_lifecycle::freshness_operations::append_and_sync(&mut owner.container, payload)
        .map_err(|error| crate::CvaError::Freshness(error.to_string()))
}

pub(crate) fn ingest_appended_semantic_event(
    owner: &mut ReliquaryOwner,
    payload: &[u8],
) -> Result<(), String> {
    owner.freshness.ingest_appended(payload)
}
