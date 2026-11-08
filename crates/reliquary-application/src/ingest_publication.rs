//! Same-owner REL publication for validated native Insomnia results.

use crate::PreparedIngest;
use reliquary_insomnia::InsomniaOwnership;
use reliquary_owner_session::model::{
    FRESHNESS_POLICY_VERSION, InsomniaEpisodeId, InsomniaWork, MemoryBody, MemoryDraft,
    MemoryRecord, MemorySourceRef, PayloadState, ResolvedTurn, memory_body_id, memory_id,
};
use reliquary_owner_session::{
    CompletionCheck, PhyWriter, RelSession, RelWriter, RoutedUserMemoryProof,
    RoutedUserMemoryReceipt,
    records::{GroupedInsomniaCompletion, GroupedMemoryBody, encode_insomnia_memory_creation},
};
use sha2::Digest;
use std::{collections::HashSet, fmt};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum IngestPublicationError {
    OwnerMismatch,
    EpisodeMismatch,
    RoutedOwnerUnsupported,
    MissingLease,
    MissingSourceTurn,
    InvalidUserRoute,
    InvalidMetadata(&'static str),
    OwnerSession(String),
}

impl fmt::Display for IngestPublicationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OwnerMismatch => f.write_str("prepared ingest belongs to another REL owner"),
            Self::EpisodeMismatch => f.write_str("Insomnia lease does not match prepared Episode"),
            Self::RoutedOwnerUnsupported => {
                f.write_str("ingest batch requires a separately authorized PHY route")
            }
            Self::MissingLease => f.write_str("Insomnia work has no active lease token"),
            Self::MissingSourceTurn => f.write_str("candidate source turn is unavailable"),
            Self::InvalidUserRoute => f.write_str("user Memory route lacks source principal proof"),
            Self::InvalidMetadata(reason) => {
                write!(f, "invalid ingest publication metadata: {reason}")
            }
            Self::OwnerSession(reason) => write!(f, "REL grouped commit failed: {reason}"),
        }
    }
}

impl std::error::Error for IngestPublicationError {}

/// Build a privacy-preserving PHY draft from one sealed User-owned candidate.
/// The caller must separately authorize both owners and verify the supplied
/// resolved source turn against the REL Archive before publishing the draft.
fn prepare_routed_user_memory(
    rel: &RelSession,
    prepared: &PreparedIngest,
    candidate: &reliquary_insomnia::InsomniaCandidate,
    source_turn: &ResolvedTurn,
    principal_id: &str,
) -> Result<MemoryDraft, IngestPublicationError> {
    if !prepared.candidates.contains(candidate)
        || candidate.ownership != InsomniaOwnership::User
        || source_turn.node_id != candidate.source_node_id
        || source_turn.principal_id.as_deref() != Some(principal_id)
        || principal_id.trim().is_empty()
    {
        return Err(IngestPublicationError::InvalidUserRoute);
    }
    let attribution = candidate.authority_source.as_ref();
    let grounding = candidate.grounding_source.as_ref();
    let mut key = Vec::new();
    for field in [
        candidate.source_node_id.as_str(),
        candidate.category.as_str(),
        candidate.memory_type.as_str(),
        candidate.authority_kind.as_str(),
        candidate.temporal_status.as_str(),
        candidate.source_quote.as_str(),
        attribution.map_or("", |source| &source.conversation_id),
        attribution.map_or("", |source| &source.node_id),
        grounding.map_or("", |source| &source.conversation_id),
        grounding.map_or("", |source| &source.node_id),
    ] {
        key.extend_from_slice(&(field.len() as u64).to_le_bytes());
        key.extend_from_slice(field.as_bytes());
    }
    let key = hex(&sha2::Sha256::digest(&key));
    let mutation_id = format!(
        "insomnia-user:{}:{}:{key}",
        hex(&prepared.owner_id),
        hex(&prepared.episode_id.0)
    );
    let source_ref = MemorySourceRef {
        owner_id: hex(&prepared.owner_id),
        principal_id: Some(principal_id.to_owned()),
        source_episode_id: prepared.episode_id.0,
        source_node_id: candidate.source_node_id.clone(),
        content_source_conversation_id: attribution.map(|source| source.conversation_id.clone()),
        content_source_node_id: attribution.map(|source| source.node_id.clone()),
        grounding_source_conversation_id: grounding.map(|source| source.conversation_id.clone()),
        grounding_source_node_id: grounding.map(|source| source.node_id.clone()),
    };
    let draft = MemoryDraft {
        title: candidate.title.clone(),
        content: candidate.content.clone(),
        category: candidate.category.clone(),
        memory_type: candidate.memory_type.clone(),
        authority_kind: candidate.authority_kind.clone(),
        temporal_status: candidate.temporal_status.clone(),
        scope: "user".into(),
        lifecycle_state: "extracted".into(),
        archived: false,
        parent_id: None,
        superseded_by: None,
        source_node_id: None,
        content_source_conversation_id: None,
        content_source_node_id: None,
        grounding_source_conversation_id: None,
        grounding_source_node_id: None,
        source_episode_id: None,
        source_ref: Some(source_ref),
        source_time_ns: Some(candidate_source_time(rel, candidate, source_turn)?),
        temporal_inference: None,
        mutation_id,
        created_at_ns: prepared.source_through_ns,
        updated_at_ns: prepared.source_through_ns,
    };
    Ok(draft)
}

fn candidate_source_time(
    rel: &RelSession,
    candidate: &reliquary_insomnia::InsomniaCandidate,
    user_turn: &ResolvedTurn,
) -> Result<i64, IngestPublicationError> {
    let Some(attribution) = &candidate.authority_source else {
        return Ok(user_turn.timestamp_ns);
    };
    let turn = rel
        .archive_evidence_turn(&attribution.conversation_id, &attribution.node_id)
        .map_err(|error| IngestPublicationError::OwnerSession(error.to_string()))?
        .filter(|turn| {
            turn.role == "assistant"
                && turn.timestamp_ns <= user_turn.timestamp_ns
                && turn.content.contains(&attribution.quote)
        })
        .ok_or(IngestPublicationError::MissingSourceTurn)?;
    Ok(turn.timestamp_ns)
}

/// Validate the source Episode and seal every routed draft before the PHY writer
/// receives any of them. Gateway authorizes both mounts and the principal.
pub fn prepare_routed_user_proofs(
    rel: &RelSession,
    destination: &PhyWriter,
    prepared: &PreparedIngest,
    principal: &str,
) -> Result<Vec<RoutedUserMemoryProof>, IngestPublicationError> {
    validate_prepared_episode(rel, prepared)?;
    let mut ids = HashSet::new();
    prepared
        .user_memories()
        .map(|candidate| {
            let turn = rel
                .archive_evidence_turn(prepared.conversation_id(), &candidate.source_node_id)
                .map_err(|error| IngestPublicationError::OwnerSession(error.to_string()))?
                .ok_or(IngestPublicationError::MissingSourceTurn)?;
            let draft = prepare_routed_user_memory(rel, prepared, candidate, &turn, principal)?;
            if !ids.insert(memory_id(&draft.mutation_id)) {
                return Err(IngestPublicationError::InvalidMetadata(
                    "duplicate routed Memory identity",
                ));
            }
            rel.prove_routed_user_memory(
                destination,
                &draft,
                &candidate.source_quote,
                candidate
                    .authority_source
                    .as_ref()
                    .map(|source| source.quote.as_str()),
            )
            .map_err(|error| IngestPublicationError::OwnerSession(error.to_string()))
        })
        .collect()
}

fn validate_prepared_episode(
    rel: &RelSession,
    prepared: &PreparedIngest,
) -> Result<(), IngestPublicationError> {
    if rel.owner_id() != prepared.owner_id {
        return Err(IngestPublicationError::OwnerMismatch);
    }
    let episode = rel
        .episode(prepared.episode_id)
        .ok_or(IngestPublicationError::EpisodeMismatch)?;
    if episode.conversation_id != prepared.conversation_id {
        return Err(IngestPublicationError::EpisodeMismatch);
    }
    if let Some((claimed_episode, claimed_turns)) = &prepared.claim_snapshot {
        let archived_turns = rel
            .archive_evidence_episode(prepared.episode_id)
            .map_err(|error| IngestPublicationError::OwnerSession(error.to_string()))?;
        if episode != claimed_episode || archived_turns.as_ref() != Some(claimed_turns) {
            return Err(IngestPublicationError::EpisodeMismatch);
        }
    }
    Ok(())
}

/// Publish Project-owned candidates from one prepared Episode to the same REL
/// owner. User-owned candidates require a separately authorized PHY lease, so
/// mixed or cross-owner batches fail closed instead of partially committing.
/// All generated identities are stable for `(episode, attempt, candidate index)`;
/// replaying the same leased attempt produces the same grouped completion.
pub fn commit_prepared_native_ingest(
    writer: &mut RelWriter,
    prepared: &PreparedIngest,
    work: &InsomniaWork,
    extractor_model: &str,
    extractor_version: &str,
    completed_at_ns: i64,
) -> Result<CompletionCheck, IngestPublicationError> {
    commit_prepared_native_ingest_with_external_refs(
        writer,
        prepared,
        work,
        extractor_model,
        extractor_version,
        completed_at_ns,
        &[],
    )
}

/// Commit one prepared Episode after the Gateway has durably published and
/// revalidated every user-owned Memory in its authorized PHY owner.
pub fn commit_prepared_native_ingest_with_external_refs(
    writer: &mut RelWriter,
    prepared: &PreparedIngest,
    work: &InsomniaWork,
    extractor_model: &str,
    extractor_version: &str,
    completed_at_ns: i64,
    external_memory_refs: &[reliquary_owner_session::records::GroupedExternalMemoryRef],
) -> Result<CompletionCheck, IngestPublicationError> {
    if !external_memory_refs.is_empty() {
        // A caller-supplied reference is not evidence of an authorized PHY publication.
        return Err(IngestPublicationError::RoutedOwnerUnsupported);
    }
    commit_prepared_native_ingest_with_routed_receipts(
        writer,
        prepared,
        work,
        extractor_model,
        extractor_version,
        completed_at_ns,
        &[],
    )
}

pub fn commit_prepared_native_ingest_with_routed_receipts(
    writer: &mut RelWriter,
    prepared: &PreparedIngest,
    work: &InsomniaWork,
    extractor_model: &str,
    extractor_version: &str,
    completed_at_ns: i64,
    routed: &[RoutedUserMemoryReceipt],
) -> Result<CompletionCheck, IngestPublicationError> {
    commit_prepared_scoped_ingest(
        writer,
        prepared,
        work,
        extractor_model,
        extractor_version,
        completed_at_ns,
        routed,
        None,
    )
}

/// Publish real extraction results under an explicitly admitted native import.
/// The owner validates the import epoch, source Episode, lease and routed proofs.
pub fn commit_prepared_native_import_with_routed_receipts(
    writer: &mut RelWriter,
    import_id: &str,
    prepared: &PreparedIngest,
    work: &InsomniaWork,
    extractor_model: &str,
    extractor_version: &str,
    completed_at_ns: i64,
    routed: &[RoutedUserMemoryReceipt],
) -> Result<CompletionCheck, IngestPublicationError> {
    commit_prepared_scoped_ingest(
        writer,
        prepared,
        work,
        extractor_model,
        extractor_version,
        completed_at_ns,
        routed,
        Some(import_id),
    )
}

fn commit_prepared_scoped_ingest(
    writer: &mut RelWriter,
    prepared: &PreparedIngest,
    work: &InsomniaWork,
    extractor_model: &str,
    extractor_version: &str,
    completed_at_ns: i64,
    routed: &[RoutedUserMemoryReceipt],
    import_id: Option<&str>,
) -> Result<CompletionCheck, IngestPublicationError> {
    if let Some(import_id) = import_id {
        writer
            .validate_import_episode_identity(import_id, prepared.episode_id())
            .map_err(|error| IngestPublicationError::OwnerSession(error.to_string()))?;
    }
    let external_memory_refs: Vec<_> = routed
        .iter()
        .map(RoutedUserMemoryReceipt::external_ref)
        .collect();
    validate_prepared_episode(writer.read_session(), prepared)?;
    if work.episode_id.0 != prepared.episode_id.0 {
        return Err(IngestPublicationError::EpisodeMismatch);
    }
    let token = work
        .lease_token
        .ok_or(IngestPublicationError::MissingLease)?;
    if work.attempt_count == 0 || completed_at_ns < work.updated_at_ns {
        return Err(IngestPublicationError::InvalidMetadata(
            "attempt or completion time",
        ));
    }
    if extractor_model.trim().is_empty() || extractor_version.trim().is_empty() {
        return Err(IngestPublicationError::InvalidMetadata(
            "extractor identity",
        ));
    }
    let rejected_count = u32::try_from(prepared.rejected.len())
        .map_err(|_| IngestPublicationError::InvalidMetadata("rejection count"))?;
    let user_candidate_count = prepared
        .candidates
        .iter()
        .filter(|candidate| candidate.ownership == InsomniaOwnership::User)
        .count();
    if user_candidate_count != external_memory_refs.len() {
        return Err(IngestPublicationError::RoutedOwnerUnsupported);
    }
    let mut external_ids = HashSet::with_capacity(external_memory_refs.len());
    for ((candidate, receipt), reference) in prepared
        .user_memories()
        .zip(routed)
        .zip(&external_memory_refs)
    {
        let turn = writer
            .read_session()
            .archive_evidence_turn(prepared.conversation_id(), &candidate.source_node_id)
            .map_err(|error| IngestPublicationError::OwnerSession(error.to_string()))?
            .ok_or(IngestPublicationError::MissingSourceTurn)?;
        let principal = turn
            .principal_id
            .as_deref()
            .ok_or(IngestPublicationError::InvalidUserRoute)?;
        let draft = prepare_routed_user_memory(
            writer.read_session(),
            prepared,
            candidate,
            &turn,
            principal,
        )?;
        if !external_ids.insert(reference.memory_id) || !receipt.matches_draft(&draft) {
            return Err(IngestPublicationError::InvalidMetadata(
                "routed receipt differs from candidate",
            ));
        }
    }

    let owner_id = hex(&prepared.owner_id);
    let mut bodies = Vec::with_capacity(prepared.candidates.len());
    let mut records = Vec::with_capacity(prepared.candidates.len());
    let mut expected_records = Vec::with_capacity(prepared.candidates.len());
    let project_count = prepared.candidates.len() - user_candidate_count;
    let mut ids = Vec::with_capacity(project_count + user_candidate_count);
    let mut body_ids = HashSet::with_capacity(prepared.candidates.len());
    let global_version_start = writer
        .next_global_version()
        .map_err(|error| IngestPublicationError::OwnerSession(error.to_string()))?;
    let memory_version_start = writer
        .read_session()
        .memory_version()
        .checked_add(1)
        .ok_or(IngestPublicationError::InvalidMetadata(
            "Memory version exhausted",
        ))?;
    for (index, candidate) in prepared
        .candidates
        .iter()
        .filter(|candidate| candidate.ownership == InsomniaOwnership::Project)
        .enumerate()
    {
        let source = writer
            .read_session()
            .archive_evidence_turn(&prepared.conversation_id, &candidate.source_node_id)
            .map_err(|error| IngestPublicationError::OwnerSession(error.to_string()))?
            .ok_or(IngestPublicationError::MissingSourceTurn)?;
        let body_id = memory_body_id(&candidate.title, &candidate.content);
        let mutation_id = format!(
            "insomnia:{}:{}:{index}",
            hex(&prepared.episode_id.0),
            work.attempt_count
        );
        let id = memory_id(&mutation_id);
        let index = u64::try_from(index)
            .map_err(|_| IngestPublicationError::InvalidMetadata("candidate count"))?;
        let memory_version = memory_version_start
            .checked_add(index)
            .ok_or(IngestPublicationError::InvalidMetadata("version overflow"))?;
        let grounding = candidate.grounding_source.as_ref();
        let (content_conversation, content_node) = candidate.authority_source.as_ref().map_or(
            (
                prepared.conversation_id.as_str(),
                candidate.source_node_id.as_str(),
            ),
            |attribution| {
                (
                    attribution.conversation_id.as_str(),
                    attribution.node_id.as_str(),
                )
            },
        );
        let record = MemoryRecord {
            id,
            revision: 1,
            body_id,
            category: candidate.category.clone(),
            memory_type: candidate.memory_type.clone(),
            authority_kind: candidate.authority_kind.clone(),
            temporal_status: candidate.temporal_status.clone(),
            scope: "project".into(),
            lifecycle_state: "extracted".into(),
            archived: false,
            parent_id: None,
            superseded_by: None,
            source_node_id: Some(candidate.source_node_id.clone()),
            content_source_conversation_id: Some(content_conversation.to_owned()),
            content_source_node_id: Some(content_node.to_owned()),
            grounding_source_conversation_id: grounding
                .map(|source| source.conversation_id.clone()),
            grounding_source_node_id: grounding.map(|source| source.node_id.clone()),
            source_episode_id: Some(prepared.episode_id.0),
            source_ref: Some(MemorySourceRef {
                owner_id: owner_id.clone(),
                principal_id: None,
                source_episode_id: prepared.episode_id.0,
                source_node_id: candidate.source_node_id.clone(),
                content_source_conversation_id: Some(content_conversation.to_owned()),
                content_source_node_id: Some(content_node.to_owned()),
                grounding_source_conversation_id: grounding
                    .map(|source| source.conversation_id.clone()),
                grounding_source_node_id: grounding.map(|source| source.node_id.clone()),
            }),
            source_time_ns: Some(candidate_source_time(
                writer.read_session(),
                candidate,
                &source,
            )?),
            temporal_inference: None,
            mutation_id,
            created_at_ns: completed_at_ns,
            updated_at_ns: completed_at_ns,
            memory_version,
        };
        if body_ids.insert(body_id) {
            bodies.push(GroupedMemoryBody::from_content(
                &candidate.title,
                &candidate.content,
            ));
        }
        ids.push(id.0);
        records.push(
            encode_insomnia_memory_creation(&record)
                .map_err(|error| IngestPublicationError::OwnerSession(error.to_string()))?,
        );
        expected_records.push(record);
    }

    if let Some(receipt) = writer
        .read_session()
        .insomnia_receipt(InsomniaEpisodeId(prepared.episode_id.0), work.attempt_count)
    {
        let receipt_matches = receipt.started_at_ns == work.updated_at_ns
            && receipt.completed_at_ns == completed_at_ns
            && receipt.extractor_model == extractor_model
            && receipt.extractor_version == extractor_version
            && receipt.rejected_count == rejected_count
            && receipt.memory_ids.iter().map(|id| id.0).collect::<Vec<_>>() == ids
            && receipt
                .external_memory_refs
                .iter()
                .map(|reference| (reference.owner_id.as_str(), reference.memory_id.0))
                .collect::<Vec<_>>()
                == external_memory_refs
                    .iter()
                    .map(|reference| (reference.owner_id.as_str(), reference.memory_id))
                    .collect::<Vec<_>>();
        let records_match = expected_records
            .iter()
            .zip(prepared.project_memories())
            .all(|(expected, candidate)| {
                writer
                    .read_session()
                    .memory_history(expected.id)
                    .ok()
                    .flatten()
                    .and_then(|history| {
                        history
                            .revisions
                            .into_iter()
                            .find(|revision| revision.record.revision == 1)
                    })
                    .is_some_and(|revision| {
                        let mut expected = expected.clone();
                        expected.memory_version = revision.record.memory_version;
                        revision.record == expected
                            && revision.payload
                                == PayloadState::Available(MemoryBody {
                                    title: candidate.title.clone(),
                                    content: candidate.content.clone(),
                                })
                    })
            });
        if receipt_matches && records_match {
            return Ok(CompletionCheck::AlreadyAccepted);
        }
        return Err(IngestPublicationError::OwnerSession(
            "completion attempt already has a conflicting receipt".into(),
        ));
    }

    let record_count = records.len();
    let completion = GroupedInsomniaCompletion {
        episode_id: prepared.episode_id.0,
        attempt: work.attempt_count,
        started_at_ns: work.updated_at_ns,
        completed_at_ns,
        transaction_time_ns: completed_at_ns,
        extractor_model: extractor_model.to_owned(),
        extractor_version: extractor_version.to_owned(),
        rejected_count,
        memory_ids: ids.clone(),
        external_memory_refs,
        global_version_start,
        bodies,
        memory_records: records,
        routing_metadata: vec![],
        freshness_birth_turn: (record_count > 0).then(|| writer.read_session().rel_turn_count()),
        freshness_policy_version: (record_count > 0).then_some(u64::from(FRESHNESS_POLICY_VERSION)),
        freshness_birth_memory_ids: ids,
    };
    let result = match import_id {
        Some(import_id) => writer.commit_native_import_completion_with_routed_receipts(
            import_id,
            token,
            completion,
            completed_at_ns,
            routed,
        ),
        None => writer.commit_grouped_insomnia_completion_with_routed_receipts(
            token,
            completion,
            completed_at_ns,
            routed,
        ),
    };
    result.map_err(|error| IngestPublicationError::OwnerSession(error.to_string()))
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(DIGITS[usize::from(byte >> 4)] as char);
        output.push(DIGITS[usize::from(byte & 0x0f)] as char);
    }
    output
}
