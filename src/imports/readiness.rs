use super::{
    ImportReadinessBlocker, ImportReadinessOwner as Owner, ImportReadinessReport,
    ImportReadinessStatus as Status, ImportReceipt, ImportStart, ImportVectorReadiness,
};
use crate::dream_readiness::DreamReadiness;
use crate::freshness_storage::HistoricalImportFreshnessSettlement;
use crate::insomnia::import_readiness::ImportReadiness as InsomniaReadiness;
use crate::memory_store::MemoryStore;
use crate::owner_lifecycle::HistoryReadAccess;
use crate::perception_import_readiness::ImportReadiness as PerceptionReadiness;
use crate::{Archive, EpisodeId, EpisodeOrigin, InsomniaWorkState};

pub(crate) fn blocker(
    owner: Owner,
    identity: impl Into<String>,
    status: Status,
) -> ImportReadinessBlocker {
    ImportReadinessBlocker {
        owner,
        identity: identity.into(),
        status,
    }
}

/// Resolve the public readiness request before any receipt or scope reads.
pub(crate) fn requested_start(
    epochs: &super::ImportEpochStore,
    import_id: &str,
) -> Result<ImportStart, crate::owner_lifecycle::OwnerError> {
    match epochs.status(import_id) {
        Some(super::ImportEpochStatus::Open(start)) => Ok(start),
        Some(super::ImportEpochStatus::Completed { .. }) => Err(super::ImportError::Epoch(
            "readiness is available only for an open native import",
        )
        .into()),
        None => Err(super::ImportError::Epoch(
            "readiness identity is not the current open native import",
        )
        .into()),
    }
}

pub(crate) fn missing_receipt_report(import_id: String) -> ImportReadinessReport {
    ImportReadinessReport {
        import_id,
        blocker: Some(blocker(Owner::Archive, "import-receipt", Status::Missing)),
        vectors: ImportVectorReadiness::SupportedUnavailable,
    }
}

/// Construct the Perception-owned live readiness view at the Import policy
/// boundary, where its bounded Memory and resolution inputs are assembled.
pub(crate) fn perception_readiness(
    owner: &crate::owner_lifecycle::ReliquaryOwner,
) -> PerceptionReadiness<'_> {
    PerceptionReadiness::new(&owner.memories, &owner.entity_resolutions)
}

/// Finish the read-only report after exact owner readiness checks. Vector
/// availability does not create work and never certifies complete coverage.
pub(crate) fn readiness_report(
    start: ImportStart,
    settlement: Option<&HistoricalImportFreshnessSettlement>,
    scope: &super::ImportEpochScope,
    mut readiness: Option<ImportReadinessBlocker>,
) -> ImportReadinessReport {
    if readiness.is_none() && settlement.is_none() {
        readiness = Some(blocker(
            Owner::Freshness,
            start.import_id.clone(),
            Status::Missing,
        ));
    }
    let has_outputs = scope
        .outputs
        .values()
        .any(|output| !output.memory_ids.is_empty());
    let vectors = if scope.source_nodes.is_empty() && !has_outputs {
        ImportVectorReadiness::NotRequired
    } else {
        ImportVectorReadiness::SupportedUnavailable
    };
    ImportReadinessReport {
        import_id: start.import_id,
        blocker: readiness,
        vectors,
    }
}

/// Derive readiness from exact owner-owned import evidence. Reads that can fail
/// remain lazy and occur at their prior policy checkpoint through the bounded
/// lifecycle access value; the policy itself contains no Cva or Container.
#[allow(clippy::too_many_arguments)]
pub(crate) fn derive_import_readiness(
    start: &ImportStart,
    receipt: &ImportReceipt,
    settlement: Option<&HistoricalImportFreshnessSettlement>,
    scope: &super::ImportEpochScope,
    archive: &Archive,
    memories: &MemoryStore,
    insomnia: &InsomniaReadiness<'_>,
    dream: &DreamReadiness<'_>,
    perception: &PerceptionReadiness<'_>,
    read_access: &HistoryReadAccess<'_>,
    current_turn: u64,
    current_watermarks: super::ImportWatermarks,
) -> Result<Option<ImportReadinessBlocker>, crate::owner_lifecycle::OwnerError> {
    if scope.start != *start
        || receipt.import_key != start.import_key
        || receipt.source_namespace != start.source_namespace
        || receipt.source_format != start.source_format
    {
        return Ok(Some(blocker(
            Owner::Semantic,
            start.import_id.clone(),
            Status::ConflictingEvidence,
        )));
    }

    // A newly imported source path without a complete Import Episode has
    // not established that its import work was exhausted. Reused exact
    // source Nodes from an earlier accepted import do not create new work.
    if scope.episodes.is_empty() && !scope.source_nodes.is_empty() {
        let mut uncovered = scope.source_nodes.clone();
        let source = read_access
            .snapshot()
            .map_err(|error| crate::owner_lifecycle::OwnerError::Container(error.0))?;
        let prior_epochs = crate::history::completion_epochs(&source, &archive.import_receipts)?;
        let statuses = prior_epochs.statuses();
        for (key, prior) in &archive.import_receipts {
            if key == &start.import_key || uncovered.is_empty() {
                continue;
            }
            let was_completed = statuses.iter().any(|status| match status {
                super::ImportEpochStatus::Completed {
                    start: prior_start, ..
                } => prior_start.import_key == *key,
                super::ImportEpochStatus::Open(_) => false,
            });
            if !was_completed {
                continue;
            }
            for conversation in &prior.conversations {
                for message in &conversation.messages {
                    if let Some(node_id) = &message.node_id {
                        uncovered.remove(&(conversation.conversation_id.clone(), node_id.clone()));
                    }
                }
            }
        }
        if !uncovered.is_empty() {
            return Ok(Some(blocker(
                Owner::Insomnia,
                "import-episode-set",
                Status::Missing,
            )));
        }
    }

    let mut perception_candidates = std::collections::BTreeSet::new();

    // Native import schedules only its Import-origin Episodes through
    // Insomnia. Require exact durable completion for those Episodes.
    for raw_id in &scope.episodes {
        let id = EpisodeId(*raw_id);
        let Some(episode) = archive.historical_episode(id) else {
            return Ok(Some(blocker(Owner::Insomnia, hex(raw_id), Status::Missing)));
        };
        if episode.origin != EpisodeOrigin::Import {
            return Ok(Some(blocker(
                Owner::Insomnia,
                hex(raw_id),
                Status::ConflictingEvidence,
            )));
        }
        let Some(work_state) = insomnia.state(id) else {
            return Ok(Some(blocker(Owner::Insomnia, hex(raw_id), Status::Missing)));
        };
        match work_state {
            InsomniaWorkState::Complete => {}
            InsomniaWorkState::Pending | InsomniaWorkState::Processing => {
                return Ok(Some(blocker(Owner::Insomnia, hex(raw_id), Status::Pending)));
            }
            InsomniaWorkState::Failed | InsomniaWorkState::Terminal => {
                return Ok(Some(blocker(Owner::Insomnia, hex(raw_id), Status::Failed)));
            }
        }
        let Some(output) = scope.outputs.get(raw_id) else {
            return Ok(Some(blocker(Owner::Insomnia, hex(raw_id), Status::Missing)));
        };
        if output.episode_id != *raw_id {
            return Ok(Some(blocker(
                Owner::Insomnia,
                hex(raw_id),
                Status::ConflictingEvidence,
            )));
        }

        // External Memory publications belong to another owner and cannot
        // be authenticated by this REL's MemoryStore.
        if let Some(reference) = output.external_memory_refs.iter().next() {
            return Ok(Some(blocker(
                Owner::Memory,
                format!("{}:{}", reference.owner_id, hex(&reference.memory_id.0)),
                Status::Unsupported,
            )));
        }
        if output
            .records
            .keys()
            .any(|id| !output.memory_ids.contains(id))
        {
            return Ok(Some(blocker(
                Owner::Memory,
                hex(raw_id),
                Status::ConflictingEvidence,
            )));
        }

        // Completion memory_ids can include an exact same-mutation
        // idempotent result. That is not fresh import publication evidence:
        // every nonzero output ID needs a retained first-publication record
        // from this epoch and this Episode. Caller-supplied IDs or Memory
        // provenance fields alone cannot prove accepted grouped publication.
        for memory_id in &output.memory_ids {
            if !memories.contains_historical_memory(*memory_id) {
                return Ok(Some(blocker(
                    Owner::Memory,
                    hex(&memory_id.0),
                    Status::Missing,
                )));
            }
            let Some(record) = output.records.get(memory_id) else {
                return Ok(Some(blocker(
                    Owner::Memory,
                    hex(&memory_id.0),
                    Status::ConflictingEvidence,
                )));
            };
            if record.global_version <= start.pre.global_version
                || record.revision != 1
                || record.source_episode_id != Some(id)
                || !scope.cohort.contains(memory_id)
                || memories.first_mutation_id(*memory_id) != Some(record.mutation_id.as_str())
            {
                return Ok(Some(blocker(
                    Owner::Memory,
                    hex(&memory_id.0),
                    Status::ConflictingEvidence,
                )));
            }
            // Terminal Memories retain this first-publication proof; only
            // current Memories can contribute downstream Perception work.
            if memories.contains_memory(*memory_id) {
                let current = read_access.current_memory(memories, *memory_id)?;
                if current.revision < record.revision {
                    return Ok(Some(blocker(
                        Owner::Memory,
                        hex(&memory_id.0),
                        Status::ConflictingEvidence,
                    )));
                }
                if current.source_episode_id != Some(id) {
                    return Ok(Some(blocker(
                        Owner::Memory,
                        hex(&memory_id.0),
                        Status::ConflictingEvidence,
                    )));
                }
                if !current.archived && current.superseded_by.is_none() {
                    perception_candidates.insert(*memory_id);
                }
            }
        }
    }

    // The scoped CVAFRS07 settlement is the semantic work receipt. Its
    // authenticated source/cohort scope and store validator bind import
    // events, exact source identities and sparse affected recipients.
    // Settlement.cohort intentionally may also include existing affected
    // recipients; only epoch.born_memories is compared to the birth cohort.

    // Each first-publication Memory has a deterministic initial Dream pass
    // identity. The Dream owner records its accepted no-link or linked result.
    for id in &scope.cohort {
        // A pass begun before archival remains owner work. The Dream owner
        // resumes such a pass before applying its new-pass eligibility rules.
        if let Some(pass_id) = dream.pending_pass_id_for_source(*id) {
            return Ok(Some(blocker(Owner::Dream, pass_id, Status::Pending)));
        }
        if !memories.contains_memory(*id) {
            continue;
        }
        let current = read_access.current_memory(memories, *id)?;
        // Dream intentionally does not start an initial pass for archived
        // publication births. Superseded Memories remain eligible in the
        // Dream owner, so their deterministic pass is still required.
        if current.archived {
            continue;
        }
        let pass_id = dream.initial_pass_id(start.owner_uuid, *id);
        if !dream.is_settled(&pass_id) {
            return Ok(Some(blocker(Owner::Dream, pass_id, Status::Pending)));
        }
    }

    // Perception schedules mention resolution only after initial Dream
    // processing. Check exact imported publications and require durable
    // terminal outcomes; unrelated Memories do not participate.
    for id in &perception_candidates {
        if scope.cohort.contains(id) && !dream.was_processed(*id) {
            return Ok(Some(blocker(
                Owner::Dream,
                hex(&id.0),
                Status::ConflictingEvidence,
            )));
        }
        if !dream.was_processed(*id) {
            continue;
        }
        for key in perception.mention_keys(*id) {
            let Some(resolution) = perception.resolution(key) else {
                return Ok(Some(blocker(
                    Owner::Perception,
                    mention_identity(key),
                    Status::Pending,
                )));
            };
            match resolution.status {
                crate::MemoryEntityResolutionStatus::Resolved { .. }
                | crate::MemoryEntityResolutionStatus::Rejected { .. } => {}
                crate::MemoryEntityResolutionStatus::Pending(_)
                | crate::MemoryEntityResolutionStatus::Dormant(_) => {
                    return Ok(Some(blocker(
                        Owner::Perception,
                        mention_identity(key),
                        Status::Pending,
                    )));
                }
            }
        }
    }

    if let Some(settlement) = settlement {
        let Some(epoch) = settlement.scope_identity() else {
            return Ok(Some(blocker(
                Owner::Freshness,
                start.import_id.clone(),
                Status::ConflictingEvidence,
            )));
        };
        if settlement.owner != start.owner_uuid
            || settlement
                .import_id
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>()
                != start.import_id
            || epoch.owner != start.owner_uuid
            || epoch.import_id != settlement.import_id
            || epoch.start_evidence_digest != scope.start_digest
            || epoch.base_cut != scope.base_cut
            || epoch.source_nodes != scope.source_nodes
            || epoch.born_memories != scope.cohort
        {
            return Ok(Some(blocker(
                Owner::Freshness,
                start.import_id.clone(),
                Status::ConflictingEvidence,
            )));
        }
        if receipt.conversations.is_empty()
            && (!epoch.source_nodes.is_empty()
                || !epoch.born_memories.is_empty()
                || !settlement.cohort.is_empty()
                || !settlement.records.is_empty()
                || !settlement.superseded.is_empty()
                || settlement.base_cut != settlement.settlement_cut)
        {
            return Ok(Some(blocker(
                Owner::Freshness,
                start.import_id.clone(),
                Status::ConflictingEvidence,
            )));
        }
        if !settlement_is_current(settlement, current_turn, current_watermarks) {
            return Ok(Some(blocker(
                Owner::Freshness,
                start.import_id.clone(),
                Status::Pending,
            )));
        }
    }

    // Native import schedules no Ego mutations and enqueues no vector job.
    // Missing vector rows remain supported-unavailable; they are not work.
    Ok(None)
}

fn mention_identity(key: crate::MemoryEntityMentionKey) -> String {
    format!(
        "{}:{:?}:{}:{}",
        hex(&key.memory_id.0),
        key.field,
        key.start_byte,
        key.end_byte
    )
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub(crate) fn settlement_is_current(
    settlement: &HistoricalImportFreshnessSettlement,
    current_turn: u64,
    post: super::ImportWatermarks,
) -> bool {
    settlement.settlement_cut == current_turn
        && settlement.global_version == post.global_version
        && settlement.archive_version == post.archive_version
        && settlement.graph_version == post.graph_version
        && settlement.memory_graph_version == post.memory_graph_version
}
