//! Accepted grouped Import replay admission over concrete owner constituents.
use super::OwnerError;
use crate::insomnia::completion::{
    InsomniaCompletion, InsomniaCompletionBody, decode_completion_body_spans, encode_completion,
};
use crate::memory_store::MemoryStore;
pub(crate) struct GroupedImportDestination<'a> {
    pub(crate) container: &'a mut crate::Container,
    pub(crate) archive: &'a crate::Archive,
    pub(crate) memories: &'a mut MemoryStore,
    pub(crate) insomnia: &'a mut crate::insomnia::store::InsomniaStore,
    pub(crate) freshness: &'a mut crate::freshness_storage::FreshnessStore,
    pub(crate) dream_freshness: &'a mut crate::freshness_dream_journal::DreamFreshnessJournal,
}
pub(crate) fn hydrate_grouped_import_completion_bodies(
    container: &mut crate::Container,
    memories: &crate::memory_store::MemoryStore,
    completion: &mut InsomniaCompletion,
) -> Result<(), crate::MemoryError> {
    let mut present = completion
        .bodies
        .iter()
        .map(|body| body.id)
        .collect::<std::collections::HashSet<_>>();
    if present.len() != completion.bodies.len() {
        return Err(crate::MemoryError::CorruptRecord(
            "duplicate grouped completion body identity",
        )
        .into());
    }
    for body in &completion.bodies {
        MemoryStore::validate_body_bytes(body.id, &body.bytes)?;
    }
    for record in &completion.records {
        if present.insert(record.body_id) {
            let bytes = memories.retained_body_bytes(container, record.body_id)?;
            MemoryStore::validate_body_bytes(record.body_id, &bytes)?;
            completion.bodies.push(InsomniaCompletionBody {
                id: record.body_id,
                bytes,
            });
        }
    }
    Ok(())
}

/// Re-publish one accepted grouped Insomnia first publication into a REL
/// import interval. The source completion is the retained owner proof; only
/// destination global/local version coordinates and the destination birth cut
/// are recomputed.
pub(crate) fn replay_grouped_import_completion(
    parts: GroupedImportDestination<'_>,
    mut completion: InsomniaCompletion,
) -> Result<bool, OwnerError> {
    if !completion.external_memory_refs.is_empty() {
        return Err(OwnerError::Freshness(
            "grouped import replay requires a shared REL owner and first-publication records"
                .into(),
        ));
    }
    parts
        .archive
        .episode(completion.episode_id)
        .ok_or_else(|| {
            OwnerError::Freshness("destination grouped completion Episode is missing".into())
        })?;
    if completion.records.iter().any(|record| {
        record.revision != 1 || record.source_episode_id != Some(completion.episode_id)
    }) {
        return Err(OwnerError::Freshness(
            "grouped import completion does not prove same-Episode first publications".into(),
        ));
    }
    let birth_ids: std::collections::BTreeSet<_> =
        completion.records.iter().map(|record| record.id).collect();
    let recorded_births: std::collections::BTreeSet<_> = completion
        .freshness_birth_memory_ids
        .iter()
        .copied()
        .collect();
    let has_owner = parts.container.owner_uuid().is_some();
    if (has_owner
        && (completion.freshness_birth_turn.is_none()
            || completion.freshness_policy_version
                != Some(crate::freshness::FRESHNESS_POLICY_VERSION as u64)
            || recorded_births != birth_ids))
        || (!has_owner
            && (completion.freshness_birth_turn.is_some()
                || completion.freshness_policy_version.is_some()
                || !recorded_births.is_empty()))
        || completion.freshness_birth_memory_ids.len() != recorded_births.len()
        || completion
            .memory_ids
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            != completion.memory_ids.len()
        || !birth_ids
            .iter()
            .all(|id| completion.memory_ids.contains(id))
    {
        return Err(OwnerError::Freshness(
            "grouped import completion lacks exact first-publication birth evidence".into(),
        ));
    }
    if completion
        .memory_ids
        .iter()
        .filter(|id| !birth_ids.contains(id))
        .any(|id| !parts.memories.contains_memory(*id))
    {
        return Err(OwnerError::Memories(crate::MemoryError::MutationConflict));
    }
    if completion.attempt == 0
        || parts
            .insomnia
            .work(completion.episode_id)
            .is_none_or(|work| {
                work.state == crate::InsomniaWorkState::Complete
                    || completion.attempt < work.attempt_count
            })
    {
        return Err(OwnerError::Insomnia(
            crate::InsomniaError::InvalidTransition,
        ));
    }

    completion.global_version_start = parts
        .memories
        .rebase_grouped_replay_versions(
            parts.container,
            &mut completion.records,
            &completion
                .bodies
                .iter()
                .map(|body| body.id)
                .collect::<Vec<_>>(),
        )
        .map_err(OwnerError::Memories)?;

    let accepted_turn = parts.archive.rel_turn_count();
    let owner = parts.container.owner_uuid();
    completion.freshness_birth_turn = owner.map(|_| accepted_turn);
    let mut freshness_after = parts.freshness.clone();
    if let Some(owner) = owner {
        for record in &completion.records {
            freshness_after
                .derive_birth(owner, record.id, accepted_turn, false)
                .map_err(OwnerError::Freshness)?;
        }
    }

    let payload = encode_completion(&completion).map_err(|_| {
        OwnerError::Insomnia(crate::InsomniaError::InvalidField("completion record"))
    })?;
    let completion_chunk = parts.container.append(&payload)?;
    parts.container.commit_embedded_version_range_at(
        completion.global_version_start,
        completion.records.len(),
        Some(completion.transaction_time_ns),
    )?;
    parts.container.sync()?;

    let spans = decode_completion_body_spans(&payload)
        .map_err(crate::MemoryError::CorruptRecord)?
        .ok_or(crate::MemoryError::CorruptRecord(
            "missing completion body spans",
        ))?;
    parts
        .memories
        .apply_grouped_spans(completion_chunk, &payload, &spans)?;
    parts
        .memories
        .apply_grouped_records(parts.container, &completion.records)
        .map_err(OwnerError::Memories)?;
    parts
        .memories
        .apply_grouped_routing_metadata(&completion.routing_metadata)
        .map_err(OwnerError::Memories)?;
    parts
        .insomnia
        .apply_completion(&super::insomnia_replay::completion_receipt(&completion))
        .map_err(OwnerError::Insomnia)?;
    *parts.freshness = freshness_after;
    if owner.is_some() {
        for record in &completion.records {
            parts.dream_freshness.derive_publication_birth(record.id);
        }
    }
    Ok(true)
}
