//! Existing REL composition of accepted Archive/Memory facts and Insomnia state.
use crate::insomnia::completion::InsomniaCompletion;
use crate::insomnia::store::{InsomniaEpisodeSource, InsomniaStore};
use crate::{
    Archive, Episode, EpisodeBoundary, EpisodeOrigin, InsomniaAttempt, InsomniaError,
    InsomniaPriority, InsomniaWorkState,
};

pub(crate) fn episode_source(
    episode: &Episode,
    priority: InsomniaPriority,
) -> InsomniaEpisodeSource {
    InsomniaEpisodeSource {
        episode_id: episode.id,
        priority,
        source_through_ns: episode.source_through_ns,
        conversation_id: episode.conversation_id.clone(),
        start_node_id: episode.start_node_id.clone(),
        finalized_at_ns: episode.finalized_at_ns,
    }
}

pub(crate) fn completion_receipt(completion: &InsomniaCompletion) -> InsomniaAttempt {
    InsomniaAttempt {
        episode_id: completion.episode_id,
        attempt: completion.attempt,
        state: InsomniaWorkState::Complete,
        started_at_ns: completion.started_at_ns,
        completed_at_ns: completion.completed_at_ns,
        extractor_model: completion.extractor_model.clone(),
        extractor_version: completion.extractor_version.clone(),
        memory_ids: completion.memory_ids.clone(),
        external_memory_refs: completion.external_memory_refs.clone(),
        rejected_count: completion.rejected_count,
        error: None,
    }
}

pub(crate) fn ingest_frame(
    state: &mut crate::insomnia::rebuild::InsomniaOpenState,
    chunk: crate::ObjectRef,
    payload: &[u8],
) -> Result<(), InsomniaError> {
    if let Some(completion) = crate::insomnia::completion::decode_completion(payload)
        .map_err(InsomniaError::CorruptRecord)?
    {
        return state.ingest_completion(&completion_receipt(&completion));
    }
    state.ingest(chunk, payload)
}

pub(crate) fn validate_references(
    store: &InsomniaStore,
    archive: &Archive,
    memories: &crate::memory_store::MemoryStore,
) -> Result<(), InsomniaError> {
    if !store
        .referenced_episodes()
        .all(|id| archive.historical_episode(id).is_some())
    {
        return Err(InsomniaError::MissingEpisode);
    }
    if !store
        .completed_memory_ids()
        .all(|id| memories.contains_historical_memory(id))
    {
        return Err(InsomniaError::InvalidTransition);
    }
    Ok(())
}

pub(crate) fn rebuild_schedule(
    store: &mut InsomniaStore,
    archive: &Archive,
) -> Result<(), InsomniaError> {
    let sources: Vec<_> = archive
        .episodes
        .iter()
        .map(|episode| {
            let priority = if matches!(
                episode.boundary,
                EpisodeBoundary::CreateMemory | EpisodeBoundary::Explicit
            ) {
                InsomniaPriority::ImmediateLive
            } else {
                match episode.origin {
                    EpisodeOrigin::Live => InsomniaPriority::Live,
                    EpisodeOrigin::Import => InsomniaPriority::Import,
                }
            };
            episode_source(episode, priority)
        })
        .collect();
    store.rebuild_schedule(&sources)
}
