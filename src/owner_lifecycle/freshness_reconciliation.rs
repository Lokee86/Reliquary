//! Owner-local Freshness replay writes with explicit destination store borrows.

use crate::freshness::FreshnessRecord;
use crate::freshness_dream_journal::DreamFreshnessJournal;
use crate::freshness_storage::{FreshnessReplayEntry, FreshnessStore};
use crate::owner_lifecycle::OwnerError;
use crate::{Archive, Container, MemoryId};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RefreshDue {
    pub(crate) memories: Vec<MemoryId>,
    pub(crate) turn: u64,
}

pub(crate) fn replay_entry(
    container: &mut Container,
    archive: &Archive,
    memories: &crate::memory_store::MemoryStore,
    owner_uuid: Option<[u8; 16]>,
    freshness: &mut FreshnessStore,
    mut entry: FreshnessReplayEntry,
    destination_turn: u64,
) -> Result<Option<RefreshDue>, OwnerError> {
    if let FreshnessReplayEntry::Event {
        provenance,
        semantic,
        ..
    } = &mut entry
    {
        if let Some(provenance) = provenance {
            provenance.accepted_turn = destination_turn;
        }
        if let Some(evidence) = semantic {
            evidence.accepted_turn = destination_turn;
        }
    }
    match entry {
        FreshnessReplayEntry::RewriteBaseline { baseline } => {
            if baseline.destination_cut != destination_turn {
                return Err(OwnerError::Freshness(
                    "rewrite baseline activity prefix changed".into(),
                ));
            }
            let payload = baseline.encode().map_err(OwnerError::Freshness)?;
            if !freshness
                .rewrite_baselines
                .contains_key(&baseline.memory_id)
            {
                container.append(&payload)?;
                freshness
                    .ingest_rewrite_baseline(&payload)
                    .map_err(OwnerError::Freshness)?;
            } else {
                freshness
                    .ingest_rewrite_baseline(&payload)
                    .map_err(OwnerError::Freshness)?;
            }
            Ok(None)
        }
        FreshnessReplayEntry::Initialize {
            memory_id,
            admitted_at_birth,
            ..
        } => replay_initialize(
            container,
            archive,
            memories,
            owner_uuid,
            freshness,
            memory_id,
            destination_turn,
            admitted_at_birth,
        )
        .map(|()| None),
        FreshnessReplayEntry::Admit { memory_id, .. } => replay_admit(
            container,
            archive,
            memories,
            owner_uuid,
            freshness,
            memory_id,
            destination_turn,
        ),
        FreshnessReplayEntry::Event {
            event_id,
            effects,
            proof: _,
            provenance: _,
            semantic: Some(evidence),
            ..
        } => replay_semantic_event(
            container, archive, owner_uuid, freshness, event_id, evidence, effects,
        ),
        FreshnessReplayEntry::Event {
            event_id,
            effects,
            proof,
            provenance,
            semantic: None,
            ..
        } => replay_event(
            container,
            archive,
            memories,
            owner_uuid,
            freshness,
            event_id,
            destination_turn,
            proof,
            effects,
            provenance,
        ),
        FreshnessReplayEntry::AcceptedUseReceipt {
            use_id,
            sources,
            policy_version,
            ..
        } => replay_accepted_use_receipt(
            container,
            owner_uuid,
            freshness,
            use_id,
            destination_turn,
            sources,
            policy_version,
        )
        .map(|()| None),
    }
}

pub(crate) fn admit(
    container: &mut Container,
    archive: &Archive,
    memories: &crate::memory_store::MemoryStore,
    owner_uuid: Option<[u8; 16]>,
    freshness: &mut FreshnessStore,
    id: MemoryId,
    turn: u64,
) -> Result<Option<RefreshDue>, OwnerError> {
    require_memory(memories, id)?;
    if turn > archive.rel_turn_count() {
        return Err(OwnerError::Freshness(
            "admission turn exceeds accepted owner REL activity".into(),
        ));
    }
    let Some(current) = freshness.records().get(&id).copied() else {
        return Err(OwnerError::Freshness(
            "Memory must be explicitly enrolled before admission".into(),
        ));
    };
    let mut admitted = current;
    if !admitted
        .admit(turn)
        .map_err(|error| OwnerError::Freshness(error.to_string()))?
    {
        return Ok(None);
    }
    let owner = owner_uuid.ok_or_else(|| {
        OwnerError::Freshness("legacy owner requires explicit migration before admission".into())
    })?;
    freshness
        .persist_policy_if_needed(container, owner)
        .map_err(OwnerError::Freshness)?;
    let payload = freshness.encode_admit(owner, id, admitted);
    container.append(&payload)?;
    container.sync()?;
    freshness
        .ingest_appended(&payload)
        .map_err(OwnerError::Freshness)?;
    Ok(Some(RefreshDue {
        memories: vec![id],
        turn,
    }))
}

fn replay_initialize(
    container: &mut Container,
    archive: &Archive,
    memories: &crate::memory_store::MemoryStore,
    owner_uuid: Option<[u8; 16]>,
    freshness: &mut FreshnessStore,
    id: MemoryId,
    destination_turn: u64,
    admitted_at_birth: bool,
) -> Result<(), OwnerError> {
    require_memory(memories, id)?;
    if freshness.records().contains_key(&id) {
        return Ok(());
    }
    if destination_turn > archive.rel_turn_count() {
        return Err(OwnerError::Freshness(
            "replay initialization turn exceeds destination REL activity".into(),
        ));
    }
    let owner = owner_uuid.ok_or_else(|| {
        OwnerError::Freshness("legacy owner cannot replay Freshness history".into())
    })?;
    freshness
        .persist_policy_if_needed(container, owner)
        .map_err(OwnerError::Freshness)?;
    let mut record = FreshnessRecord::created();
    if admitted_at_birth {
        record
            .admit(destination_turn)
            .map_err(|error| OwnerError::Freshness(error.to_string()))?;
    }
    let payload = freshness.encode_initialize(owner, id, destination_turn, record);
    container.append(&payload)?;
    container.sync()?;
    freshness
        .ingest_appended(&payload)
        .map_err(OwnerError::Freshness)
}

fn replay_admit(
    container: &mut Container,
    archive: &Archive,
    memories: &crate::memory_store::MemoryStore,
    owner_uuid: Option<[u8; 16]>,
    freshness: &mut FreshnessStore,
    id: MemoryId,
    destination_turn: u64,
) -> Result<Option<RefreshDue>, OwnerError> {
    admit(
        container,
        archive,
        memories,
        owner_uuid,
        freshness,
        id,
        destination_turn,
    )
}

fn replay_event(
    container: &mut Container,
    archive: &Archive,
    memories: &crate::memory_store::MemoryStore,
    owner_uuid: Option<[u8; 16]>,
    freshness: &mut FreshnessStore,
    event_id: String,
    destination_turn: u64,
    proof: crate::FreshnessEventProof,
    effects: BTreeMap<MemoryId, i16>,
    provenance: Option<crate::freshness_storage::FreshnessEventProvenance>,
) -> Result<Option<RefreshDue>, OwnerError> {
    if destination_turn > archive.rel_turn_count() {
        return Err(OwnerError::Freshness(
            "replay event turn exceeds destination REL activity".into(),
        ));
    }
    if freshness
        .contains_event(&event_id)
        .map_err(OwnerError::Freshness)?
    {
        return if freshness
            .event_matches_with_provenance(
                &event_id,
                destination_turn,
                proof,
                &effects,
                provenance.as_ref(),
            )
            .map_err(OwnerError::Freshness)?
        {
            Ok(None)
        } else {
            Err(OwnerError::Freshness(
                "replayed event identity collides with different payload".into(),
            ))
        };
    }
    let owner = owner_uuid.ok_or_else(|| {
        OwnerError::Freshness("legacy owner cannot replay Freshness history".into())
    })?;
    for (id, amount) in &effects {
        require_memory(memories, *id)?;
        if *amount <= 0 || *amount > freshness.policy().linkage_principal {
            return Err(OwnerError::Freshness(
                "replayed event effect exceeds principal bound".into(),
            ));
        }
        if !freshness.records().contains_key(id) {
            return Err(OwnerError::Freshness(format!(
                "Memory {:?} has no Freshness enrollment",
                id
            )));
        }
    }
    let postimages = freshness
        .prepare_event_postimages_for(&event_id, destination_turn, &effects)
        .map_err(OwnerError::Freshness)?;
    freshness
        .validate_event_payload(&event_id, destination_turn, proof, &effects, &postimages)
        .map_err(OwnerError::Freshness)?;
    let payload = freshness
        .encode_event_with_provenance(
            owner,
            event_id,
            destination_turn,
            proof,
            effects.clone(),
            postimages,
            provenance,
        )
        .map_err(OwnerError::Freshness)?;
    container.append(&payload)?;
    container.sync()?;
    freshness
        .ingest_appended(&payload)
        .map_err(OwnerError::Freshness)?;
    Ok(Some(RefreshDue {
        memories: effects.keys().copied().collect(),
        turn: destination_turn,
    }))
}

fn replay_semantic_event(
    container: &mut Container,
    archive: &Archive,
    owner_uuid: Option<[u8; 16]>,
    freshness: &mut FreshnessStore,
    event_id: String,
    evidence: crate::freshness_storage::SemanticEventEvidence,
    effects: BTreeMap<MemoryId, i16>,
) -> Result<Option<RefreshDue>, OwnerError> {
    let owner = owner_uuid.ok_or(OwnerError::InvalidContainerIdentity(
        "semantic replay requires owner",
    ))?;
    if evidence.owner_uuid != owner {
        return Err(OwnerError::Freshness(
            "unsupported or conflicting semantic replay evidence".into(),
        ));
    }
    if semantic_evidence_position(archive, &evidence)? != evidence.accepted_turn {
        return Err(OwnerError::Freshness(
            "unsupported or conflicting semantic replay evidence".into(),
        ));
    }
    let turn = evidence.accepted_turn;
    let memories = effects.keys().copied().collect();
    if crate::freshness::reconciliation::replay_semantic_event(
        container, owner, freshness, event_id, evidence, effects,
    )
    .map_err(|error| match error {
        crate::freshness::reconciliation::ReplayError::Freshness(error) => {
            OwnerError::Freshness(error)
        }
        crate::freshness::reconciliation::ReplayError::Container(error) => {
            OwnerError::Container(error)
        }
    })? {
        Ok(Some(RefreshDue { memories, turn }))
    } else {
        Ok(None)
    }
}

fn replay_accepted_use_receipt(
    container: &mut Container,
    owner_uuid: Option<[u8; 16]>,
    freshness: &mut FreshnessStore,
    use_id: String,
    turn: u64,
    sources: Vec<MemoryId>,
    policy_version: u64,
) -> Result<(), OwnerError> {
    if freshness
        .resolve_accepted_use(&use_id, turn, &sources)
        .map_err(OwnerError::Freshness)?
        .is_some()
    {
        return Ok(());
    }
    let owner = owner_uuid
        .ok_or_else(|| OwnerError::Freshness("accepted-use replay lacks owner".into()))?;
    let payload =
        freshness.encode_accepted_use_receipt(owner, use_id, turn, sources, policy_version);
    container.append(&payload)?;
    container.sync()?;
    freshness
        .ingest_appended(&payload)
        .map_err(OwnerError::Freshness)
}

fn require_memory(
    memories: &crate::memory_store::MemoryStore,
    id: MemoryId,
) -> Result<(), OwnerError> {
    if memories.contains_memory(id) {
        Ok(())
    } else {
        Err(OwnerError::Freshness(format!(
            "unknown owner-local Memory {:?}",
            id
        )))
    }
}

fn semantic_evidence_position(
    archive: &Archive,
    evidence: &crate::freshness_storage::SemanticEventEvidence,
) -> Result<u64, OwnerError> {
    if let Some(fragment) = &evidence.fragment {
        let record = archive
            .fragments
            .get(fragment.fragment_id)
            .ok_or_else(|| OwnerError::Freshness("missing semantic fragment".into()))?;
        let mut sources: Vec<_> = archive
            .fragment_nodes_for(record)?
            .into_iter()
            .map(|node| (node.conversation_id, node.id))
            .collect();
        sources.sort();
        if sources != fragment.sources {
            return Err(OwnerError::Freshness(
                "conflicting semantic fragment sources".into(),
            ));
        }
        sources
            .iter()
            .filter_map(|(conversation, node)| {
                archive.activity_position_for_turn(conversation, node)
            })
            .max()
            .ok_or_else(|| OwnerError::Freshness("fragment has no accepted activity".into()))
    } else {
        archive
            .activity_position_for_turn(&evidence.conversation_id, &evidence.node_id)
            .ok_or_else(|| OwnerError::Freshness("missing semantic source".into()))
    }
}
