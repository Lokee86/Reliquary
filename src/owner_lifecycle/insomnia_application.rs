//! Canonical grouped Insomnia/Memory/Freshness publication; no provider or facade callback.
use crate::freshness_storage::FreshnessStore;
use crate::insomnia::completion::{
    InsomniaCompletion, InsomniaCompletionBody, decode_completion_body_spans, encode_completion,
};
use crate::insomnia::store::InsomniaStore;
use crate::insomnia::{PreparedApplication, UserPublication};
use crate::memory_store::MemoryStore;
use crate::{
    Container, INSOMNIA_EXTRACTOR_CONTRACT_VERSION, InsomniaProcessError, InsomniaProcessResult,
    InsomniaWork,
};

#[allow(clippy::too_many_arguments)]
pub(crate) fn commit_application(
    container: &mut Container,
    memories: &mut MemoryStore,
    insomnia: &mut InsomniaStore,
    freshness: &mut FreshnessStore,
    dream_freshness: &mut crate::freshness_dream_journal::DreamFreshnessJournal,
    owner_uuid: Option<[u8; 16]>,
    accepted_rel_turn: u64,
    claim: &InsomniaWork,
    prepared: PreparedApplication,
    user_publication: Option<UserPublication>,
    started_at_ns: i64,
    completed_at_ns: i64,
) -> Result<InsomniaProcessResult, InsomniaProcessError> {
    let token = claim
        .lease_token
        .ok_or(InsomniaProcessError::InvalidClaim)?;
    insomnia.active_claim(claim.episode_id, token, completed_at_ns)?;
    if !prepared.user_drafts.is_empty() && user_publication.is_none() {
        return Err(InsomniaProcessError::UserRoutingRequired);
    }
    let user_publication = user_publication.unwrap_or(UserPublication {
        created: Vec::new(),
        existing: Vec::new(),
        refs: Vec::new(),
    });
    let project_drafts = prepared
        .project_drafts
        .into_iter()
        .map(|prepared| {
            debug_assert_eq!(
                prepared.temporal.analysis.source_timestamp_ns,
                prepared.draft.source_time_ns
            );
            let temporal_inference = prepared.bound_temporal_inference();
            (
                prepared.draft,
                temporal_inference,
                prepared.routing_metadata,
            )
        })
        .collect();
    let batch = memories.stage_grouped_publication(container, project_drafts)?;
    let existing = batch.existing;
    let mut memory_ids: Vec<_> = batch.records.iter().map(|record| record.id).collect();
    memory_ids.extend(existing.iter().map(|memory| memory.id));
    let transaction_time_ns =
        Container::transaction_time_now_ns().map_err(crate::InsomniaError::from)?;
    let freshness_birth_memory_ids = if owner_uuid.is_some() {
        batch
            .records
            .iter()
            .filter(|record| record.revision == 1)
            .map(|record| record.id)
            .collect()
    } else {
        Vec::new()
    };
    let mut freshness_after = freshness.clone();
    if let Some(owner) = owner_uuid {
        for record in batch.records.iter().filter(|record| record.revision == 1) {
            freshness_after
                .derive_birth(owner, record.id, accepted_rel_turn, false)
                .map_err(|error| {
                    crate::InsomniaProcessError::Memory(crate::MemoryError::Freshness(error))
                })?;
        }
    }
    let completion = InsomniaCompletion {
        episode_id: claim.episode_id,
        attempt: claim.attempt_count,
        started_at_ns,
        completed_at_ns,
        transaction_time_ns,
        extractor_model: prepared.model,
        extractor_version: INSOMNIA_EXTRACTOR_CONTRACT_VERSION.into(),
        rejected_count: prepared.rejected.len() as u32,
        memory_ids,
        external_memory_refs: user_publication.refs.clone(),
        global_version_start: batch.global_version_start,
        bodies: batch
            .bodies
            .into_iter()
            .map(|(id, bytes)| InsomniaCompletionBody { id, bytes })
            .collect(),
        records: batch.records,
        routing_metadata: batch.routing_metadata,
        freshness_birth_turn: owner_uuid.map(|_| accepted_rel_turn),
        freshness_policy_version: owner_uuid
            .map(|_| crate::freshness::FRESHNESS_POLICY_VERSION as u64),
        freshness_birth_memory_ids,
    };
    let payload = encode_completion(&completion)
        .map_err(|_| crate::InsomniaError::InvalidField("completion record"))?;
    let completion_chunk = container
        .append(&payload)
        .map_err(crate::InsomniaError::from)?;
    container
        .commit_embedded_version_range_at(
            completion.global_version_start,
            completion.records.len(),
            Some(completion.transaction_time_ns),
        )
        .map_err(crate::InsomniaError::from)?;
    container.sync().map_err(crate::InsomniaError::from)?;
    let spans = decode_completion_body_spans(&payload)
        .map_err(crate::MemoryError::CorruptRecord)?
        .ok_or(crate::MemoryError::CorruptRecord(
            "missing completion body spans",
        ))?;
    memories.apply_grouped_spans(completion_chunk, &payload, &spans)?;
    let mut created = memories.apply_grouped_records(container, &completion.records)?;
    memories.apply_grouped_routing_metadata(&completion.routing_metadata)?;
    for memory in &mut created {
        *memory = memories.memory(container, memory.id)?;
    }
    insomnia.apply_completion(
        &crate::owner_lifecycle::insomnia_replay::completion_receipt(&completion),
    )?;
    *freshness = freshness_after;
    if owner_uuid.is_some() {
        for record in completion
            .records
            .iter()
            .filter(|record| record.revision == 1)
        {
            dream_freshness.derive_publication_birth(record.id);
        }
    }
    Ok(InsomniaProcessResult {
        created,
        existing,
        user_created: user_publication.created,
        user_existing: user_publication.existing,
        rejected: prepared.rejected,
    })
}
