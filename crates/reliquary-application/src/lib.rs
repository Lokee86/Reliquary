//! Typed application workflows over the domain owners.
//!
//! Native ingest preparation validates a caller-accepted Episode and exact
//! source evidence before producing routed Memory proposals. Durable owner
//! publication is deliberately not exposed while owner sessions are read-only.

use reliquary_archive::{Episode, EpisodeId, ResolvedTurn};
use reliquary_insomnia::{
    EvidenceTurn, InsomniaCandidate, InsomniaExtraction, RawCandidate, validate_candidates,
};
use reliquary_owner_session::{OwnerSessionError, RelSession};
use std::{collections::HashSet, fmt};

mod dream_workflow;
mod entity_workflow;
mod external_source_workflow;
mod history_workflow;
mod maintenance_workflow;
mod retrieval_workflow;
mod vector_workflow;
pub use dream_workflow::{
    DreamOrganizationDraft, DreamWorkflowError, MAX_DREAM_CANDIDATES, draft_dream_organization,
};
pub use entity_workflow::{
    EntityMemoryMention, EntityOwnerSession, EntityResolutionProposal, EntityWorkflowError,
    resolve_memory_mention,
};
pub use external_source_workflow::{
    ExternalSourceError, VerifiedExternalSource, verify_project_attachment,
};
pub use history_workflow::{
    MemoryHistoryLookup, MemoryHistoryRead, MemoryHistoryStatus, MemoryHistoryWorkflowError,
    read_owner_memory_history,
};
pub use maintenance_workflow::{
    MaintenanceBlocker, MaintenanceWorkflowError, MemoryMaintenancePreparation,
    MemoryPruneCandidate, prepare_tombstoned_memory_maintenance,
    recheck_tombstoned_memory_maintenance,
};
pub use retrieval_workflow::{
    OwnerMemoryEvidence, OwnerMemorySelection, RetrievalWorkflowError, SelectedMemory,
    select_owner_memories,
};
pub use vector_workflow::{
    MemoryEmbeddingBatch, MemoryEmbeddingSource, VectorWorkflowError, prepare_memory_embeddings,
};

pub const MAX_INGEST_EVIDENCE_TURNS: usize = 64;
pub const MAX_INGEST_EVIDENCE_BYTES: usize = 128 << 10;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum IngestError {
    EvidenceTurnLimit,
    EvidenceByteLimit,
    InvalidEvidence(&'static str),
    EpisodeUnavailable,
    EvidenceNotInArchive,
    OwnerSession(String),
    Insomnia(String),
}

impl fmt::Display for IngestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EvidenceTurnLimit => f.write_str("ingest evidence turn limit exceeded"),
            Self::EvidenceByteLimit => f.write_str("ingest evidence byte limit exceeded"),
            Self::InvalidEvidence(reason) => write!(f, "invalid ingest evidence: {reason}"),
            Self::EpisodeUnavailable => f.write_str("accepted Archive Episode is unavailable"),
            Self::EvidenceNotInArchive => {
                f.write_str("ingest evidence does not match an accepted Archive turn")
            }
            Self::OwnerSession(reason) => write!(f, "REL owner read failed: {reason}"),
            Self::Insomnia(reason) => write!(f, "Insomnia rejected ingest: {reason}"),
        }
    }
}

impl std::error::Error for IngestError {}

impl From<OwnerSessionError> for IngestError {
    fn from(error: OwnerSessionError) -> Self {
        Self::OwnerSession(error.to_string())
    }
}

/// A validated, retry-stable proposal batch bound to its accepted Episode.
///
/// `episode_id` is the stable Archive identity. Candidates retain verbatim
/// source references from Insomnia; owner writes still require a future
/// transactional publication workflow.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreparedIngest {
    pub episode_id: EpisodeId,
    pub conversation_id: String,
    pub candidates: Vec<InsomniaCandidate>,
    pub rejected: Vec<reliquary_insomnia::InsomniaRejection>,
}

impl PreparedIngest {
    pub fn user_memories(&self) -> impl Iterator<Item = &InsomniaCandidate> {
        self.candidates
            .iter()
            .filter(|candidate| candidate.ownership == reliquary_insomnia::InsomniaOwnership::User)
    }

    pub fn project_memories(&self) -> impl Iterator<Item = &InsomniaCandidate> {
        self.candidates.iter().filter(|candidate| {
            candidate.ownership == reliquary_insomnia::InsomniaOwnership::Project
        })
    }
}

/// Validate source identity and bounded evidence, then prepare only
/// source-grounded Insomnia candidates for a later owner-local commit.
fn prepare_native_ingest(
    episode: &Episode,
    accepted_turns: &[ResolvedTurn],
    evidence: &[EvidenceTurn],
    proposals: Vec<RawCandidate>,
) -> Result<PreparedIngest, IngestError> {
    validate_evidence(evidence)?;
    let InsomniaExtraction {
        candidates,
        rejected,
    } = validate_candidates(episode, accepted_turns, evidence, proposals)
        .map_err(IngestError::Insomnia)?;
    Ok(PreparedIngest {
        episode_id: episode.id,
        conversation_id: episode.conversation_id.clone(),
        candidates,
        rejected,
    })
}

/// Hydrate accepted turns and verify every supplied evidence turn against the
/// REL owner's Archive before preparing source-grounded proposals.
///
/// The caller is responsible for obtaining this session through the product's
/// authorization boundary. A `RelSession` proves owner-local data provenance,
/// not principal access rights.
pub fn prepare_native_ingest_from_archive(
    rel: &RelSession,
    episode_id: EpisodeId,
    evidence: &[EvidenceTurn],
    proposals: Vec<RawCandidate>,
) -> Result<PreparedIngest, IngestError> {
    // Enforce the request bounds before any owner reads. Besides avoiding
    // unnecessary per-turn Archive lookups, this keeps the cost of rejected
    // ingest requests bounded by the evidence contract.
    validate_evidence(evidence)?;
    let episode = rel
        .episode(episode_id)
        .cloned()
        .ok_or(IngestError::EpisodeUnavailable)?;
    let turns = rel
        .episode_turns(episode_id)?
        .ok_or(IngestError::EpisodeUnavailable)?;
    for source in evidence {
        let archived = rel
            .resolved_archive_turn(&source.conversation_id, &source.node_id)?
            .ok_or(IngestError::EvidenceNotInArchive)?;
        if archived.role != source.role
            || archived.timestamp_ns != source.timestamp_ns
            || archived.content != source.content
        {
            return Err(IngestError::EvidenceNotInArchive);
        }
    }
    prepare_native_ingest(&episode, &turns, evidence, proposals)
}

fn validate_evidence(evidence: &[EvidenceTurn]) -> Result<(), IngestError> {
    if evidence.len() > MAX_INGEST_EVIDENCE_TURNS {
        return Err(IngestError::EvidenceTurnLimit);
    }
    let mut identities = HashSet::with_capacity(evidence.len());
    let mut bytes = 0usize;
    for turn in evidence {
        if turn.conversation_id.trim().is_empty()
            || turn.node_id.trim().is_empty()
            || turn.role.trim().is_empty()
        {
            return Err(IngestError::InvalidEvidence("missing source identity"));
        }
        if !identities.insert((&turn.conversation_id, &turn.node_id)) {
            return Err(IngestError::InvalidEvidence("duplicate source identity"));
        }
        bytes = bytes
            .checked_add(turn.content.len())
            .ok_or(IngestError::EvidenceByteLimit)?;
    }
    if bytes > MAX_INGEST_EVIDENCE_BYTES {
        return Err(IngestError::EvidenceByteLimit);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use reliquary_archive::{
        Archive, AttachmentInput, EpisodeBoundary, EpisodeOrigin, ProjectAttachmentInput, TurnInput,
    };
    use reliquary_container::{Container, ContainerIdentity, ReliquaryScope};
    use reliquary_insomnia::{InsomniaOwnership, RawCandidate};
    use reliquary_owner_session::GraphAdmission;
    use reliquary_vector_store::{
        ArchiveVectorStore, CompatibilityProfileStore, MemoryVectorStore, PackedVectorStore,
        VectorGenerationStore,
    };
    use std::{
        fs,
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
    };

    static NEXT: AtomicU64 = AtomicU64::new(1);
    struct Temp(PathBuf);
    impl Temp {
        fn new() -> Self {
            let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".test-tmp");
            fs::create_dir_all(&root).unwrap();
            Self(root.join(format!(
                "native-ingest-{}-{}.rel",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            )))
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = fs::remove_file(&self.0);
        }
    }

    fn candidate(quote: &str) -> RawCandidate {
        RawCandidate {
            authority_kind: "direct".into(),
            category: "preference".into(),
            memory_type: "project".into(),
            temporal_status: "current".into(),
            ownership: InsomniaOwnership::Project,
            title: "Private project notes".into(),
            content: "The user wants project notes kept private.".into(),
            source_node_id: "user-turn".into(),
            source_quote: quote.into(),
            authority_source: None,
            grounding_source: None,
            routing_mentions: vec![],
        }
    }

    fn accepted_rel() -> (Temp, RelSession, EpisodeId) {
        let path = Temp::new();
        let owner = [71; 16];
        let mut container = Container::create(
            &path.0,
            ContainerIdentity::reliquary(ReliquaryScope::Project),
            owner,
        )
        .unwrap();
        PackedVectorStore::default()
            .initialize(&mut container)
            .unwrap();
        CompatibilityProfileStore::default()
            .initialize(&mut container)
            .unwrap();
        ArchiveVectorStore::default()
            .initialize(&mut container)
            .unwrap();
        MemoryVectorStore::default()
            .initialize(&mut container)
            .unwrap();
        VectorGenerationStore::empty()
            .initialize(&mut container)
            .unwrap();
        let mut archive = Archive::open(&container).unwrap();
        archive
            .accept_turn(
                &mut container,
                TurnInput {
                    id: "user-turn".into(),
                    conversation_id: "conversation".into(),
                    parent_id: None,
                    role: "user".into(),
                    principal_id: None,
                    timestamp_ns: 1,
                    content: "Keep my project notes private.".into(),
                    attachments: Vec::<AttachmentInput>::new(),
                    project_attachments: Vec::<ProjectAttachmentInput>::new(),
                },
            )
            .unwrap();
        archive
            .accept_turn(
                &mut container,
                TurnInput {
                    id: "assistant-turn".into(),
                    conversation_id: "conversation".into(),
                    parent_id: Some("user-turn".into()),
                    role: "assistant".into(),
                    principal_id: None,
                    timestamp_ns: 2,
                    content: "Understood.".into(),
                    attachments: vec![],
                    project_attachments: vec![],
                },
            )
            .unwrap();
        let episode_id = archive
            .put_episode(
                &mut container,
                "conversation",
                "user-turn",
                "assistant-turn",
                EpisodeOrigin::Live,
                EpisodeBoundary::Explicit,
                2,
            )
            .unwrap();
        container.sync().unwrap();
        drop(archive);
        drop(container);
        let admission = GraphAdmission::for_replay(owner, [], [], [], [], []);
        let rel = RelSession::open_path(&path.0, &admission, [], []).unwrap();
        (path, rel, episode_id)
    }

    #[test]
    fn prepares_routed_proposals_only_from_hydrated_archive_episode() {
        let (_path, rel, episode_id) = accepted_rel();
        let prepared = prepare_native_ingest_from_archive(
            &rel,
            episode_id,
            &[],
            vec![candidate("Keep my project notes private.")],
        )
        .unwrap();
        assert_eq!(prepared.episode_id, episode_id);
        assert_eq!(prepared.user_memories().count(), 0);
        assert_eq!(prepared.project_memories().count(), 1);

        let fabricated = prepare_native_ingest_from_archive(
            &rel,
            episode_id,
            &[],
            vec![candidate("Keep everything secret.")],
        )
        .unwrap();
        assert!(fabricated.candidates.is_empty());
        assert_eq!(fabricated.rejected.len(), 1);
    }

    #[test]
    fn supplied_supporting_evidence_must_match_archive_turn_bytes_and_identity() {
        let (_path, rel, episode_id) = accepted_rel();
        let evidence = EvidenceTurn {
            conversation_id: "conversation".into(),
            node_id: "assistant-turn".into(),
            role: "assistant".into(),
            timestamp_ns: 2,
            content: "Understood.".into(),
        };
        assert!(
            prepare_native_ingest_from_archive(
                &rel,
                episode_id,
                std::slice::from_ref(&evidence),
                vec![],
            )
            .is_ok()
        );
        let mut altered = evidence;
        altered.content = "I will keep them private.".into();
        assert_eq!(
            prepare_native_ingest_from_archive(&rel, episode_id, &[altered], vec![]),
            Err(IngestError::EvidenceNotInArchive)
        );
        let foreign_turn = EvidenceTurn {
            conversation_id: "foreign-conversation".into(),
            node_id: "assistant-turn".into(),
            role: "assistant".into(),
            timestamp_ns: 2,
            content: "Understood.".into(),
        };
        assert_eq!(
            prepare_native_ingest_from_archive(&rel, episode_id, &[foreign_turn], vec![]),
            Err(IngestError::EvidenceNotInArchive)
        );
        assert_eq!(
            prepare_native_ingest_from_archive(&rel, EpisodeId([255; 32]), &[], vec![]),
            Err(IngestError::EpisodeUnavailable)
        );
    }

    #[test]
    fn rejects_oversized_or_ambiguous_evidence_before_insomnia() {
        let too_many = vec![
            EvidenceTurn {
                conversation_id: "c".into(),
                node_id: "n".into(),
                role: "user".into(),
                timestamp_ns: 0,
                content: String::new(),
            };
            MAX_INGEST_EVIDENCE_TURNS + 1
        ];
        assert_eq!(
            validate_evidence(&too_many),
            Err(IngestError::EvidenceTurnLimit)
        );
        let duplicate = EvidenceTurn {
            conversation_id: "c".into(),
            node_id: "n".into(),
            role: "user".into(),
            timestamp_ns: 0,
            content: String::new(),
        };
        assert!(matches!(
            validate_evidence(&[duplicate.clone(), duplicate]),
            Err(IngestError::InvalidEvidence(_))
        ));
    }

    #[test]
    fn rejects_oversized_evidence_before_episode_lookup() {
        let (_path, rel, _) = accepted_rel();
        let too_many = vec![
            EvidenceTurn {
                conversation_id: "conversation".into(),
                node_id: "user-turn".into(),
                role: "user".into(),
                timestamp_ns: 1,
                content: String::new(),
            };
            MAX_INGEST_EVIDENCE_TURNS + 1
        ];
        assert_eq!(
            prepare_native_ingest_from_archive(
                &rel,
                EpisodeId([255; 32]),
                &too_many,
                vec![],
            ),
            Err(IngestError::EvidenceTurnLimit)
        );
    }
}
