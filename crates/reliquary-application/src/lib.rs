//! Typed application workflows over the domain owners.
//!
//! Native ingest preparation validates a caller-accepted Episode and exact
//! source evidence before producing routed Memory proposals and named durable
//! owner publication workflows.

use reliquary_insomnia::{
    EvidenceTurn, InsomniaCandidate, InsomniaExtraction, RawCandidate, validate_candidates,
};
use reliquary_owner_session::model::{Episode, EpisodeId, ResolvedTurn};
use reliquary_owner_session::{OwnerSessionError, RelSession};
use std::{collections::HashSet, fmt};

mod context_workflow;
mod conversation_workflow;
pub use context_workflow::{
    ContextMemoryEvidence, ContextMemoryScope, ContextRepositoryEvidence, ContextWorkflowError,
    MAX_CONTEXT_PROJECT_SOURCES, MAX_CONTEXT_SOURCE_BYTES, MAX_CONTEXT_SOURCE_TURNS,
    PreparedContextWorkflow, finalize_context_workflow, prepare_context_workflow,
    read_context_repository_source,
};
pub use reliquary_context_engine::{
    CompactionBudget, CompactionCheckpoint, CompactionPlan, ContextMessage, ContextRequest,
    ContextRole, ContextTokenCounter, ContextTurn, ContextView,
};
mod dream_candidates;
mod dream_empty_publication;
mod dream_publication;
mod dream_supersession_publication;
mod dream_temporal;
mod dream_workflow;
pub use conversation_workflow::{
    NativeConversationCommit, PublishedNativeTurn, commit_native_conversation_turn,
    finalize_native_episode, validate_native_conversation_commit,
};
mod entity_workflow;
mod external_source_workflow;
mod freshness_due_workflow;
mod history_workflow;
pub mod import_entity_work;
mod ingest_publication;
mod maintenance_workflow;
mod management_workflow;
mod perception_lifecycle_workflow;
pub use management_workflow::inspection;
pub use management_workflow::{
    CommunityRenameRequest, CommunityRenameResult, rename_community_at_cut,
};
pub use management_workflow::{
    ConversationInspection, CreateMemoryReceipt, CreateMemoryRequest, MAX_MANAGEMENT_PAGE_BYTES,
    MAX_MANAGEMENT_PAGE_ITEMS, ManagedBranch, ManagementError, ManagementPage, ManagementVersions,
    MemoryProvenance, MemoryStatistics, MemorySummary, RelManagementHealth,
    SemanticOwnerStatistics, TranscriptTurn, create_direct_memory, create_requested_memory,
    create_requested_memory_at_cut, management_conversations, management_health,
    management_memories, management_memory, management_provenance, management_statistics,
    management_transcript, set_management_title, user_mutation_id,
};
pub use management_workflow::{
    CorrectMemoryReceipt, CorrectMemoryRequest, RetireMemoryReceipt, correct_memory_at_cut,
    retire_memory_at_cut,
};
pub mod native_import_source;
pub mod native_import_workflow;
pub use native_import_workflow::settle_native_import_initial_dream;
pub mod live_processing;
mod native_insomnia;
mod native_insomnia_evidence;
pub mod processing_plan;
mod project_correlation_workflow;
mod rel_reconciliation_workflow;
mod rewrite_workflow;
pub use native_insomnia::{
    INSOMNIA_EXTRACTOR_CONTRACT_VERSION, NativeInsomniaExtraction, extract_native_insomnia,
    extract_native_insomnia_with_ownership,
};
pub use native_insomnia_evidence::{
    ResolvedNativeInsomniaEvidence, resolve_import_insomnia_evidence,
    resolve_native_insomnia_evidence,
};
pub use project_correlation_workflow::{
    MAX_CORRELATED_PROJECT_FILE_BYTES, PreparedProjectCorrelation, ProjectCorrelatedNativeCommit,
    ProjectCorrelationError, ProjectCorrelationRequest, commit_project_correlated_native_turn,
    prepare_project_correlation, validate_project_correlated_native_commit,
    validate_project_correlation_retry,
};
mod retrieval_workflow;
mod vector_publication;
pub use vector_publication::evaluate_archive_vectors;
mod vector_workflow;
pub use reliquary_vectors::{
    EmbeddingEndpoint, EmbeddingEndpointError, SwitchboardEmbeddingEndpoint, VectorNormalization,
};
pub use vector_publication::{
    PreparedMemoryVectors, commit_native_import_memory_vectors, commit_phy_memory_vectors,
    commit_rel_memory_vectors, evaluate_memory_vectors, prepare_compatibility_profile,
    verify_compatibility_profile,
};
mod verified_copy_workflow;
pub use dream_candidates::{
    DreamCandidateReadReport, draft_ranked_dream_organization, ranked_owner_dream_candidates,
    ranked_owner_dream_candidates_with_report,
};
pub use dream_empty_publication::{commit_empty_dream_cycle, settle_initial_dream_cycle};
pub use dream_publication::{DreamOwnerWriter, commit_dream_relation, commit_dream_zero_link};
pub use dream_supersession_publication::commit_dream_supersession;
pub use dream_workflow::{
    DreamOrganizationDraft, DreamWorkflowError, MAX_DREAM_CANDIDATES, draft_dream_organization,
};
pub use entity_workflow::{
    EntityMemoryMention, EntityOwnerSession, EntityResolutionProposal, EntityWorkflowError,
    PreparedEntityMention, prepare_memory_mention, prepare_phy_writer_mention,
    resolve_memory_mention,
};
pub use external_source_workflow::{
    ExternalSourceError, PreparedProjectAttachment, VerifiedExternalSource,
    prepare_project_attachment, verify_project_attachment,
};
pub use freshness_due_workflow::{
    FreshnessLifecycleState, FreshnessTransition, due_freshness_transitions,
};
pub use history_workflow::{
    MemoryHistoryLookup, MemoryHistoryRead, MemoryHistoryStatus, MemoryHistoryWorkflowError,
    read_owner_memory_history, read_owner_memory_history_at_cut,
};
pub use ingest_publication::{
    IngestPublicationError, commit_prepared_native_import_with_routed_receipts,
    commit_prepared_native_ingest, commit_prepared_native_ingest_with_external_refs,
    commit_prepared_native_ingest_with_routed_receipts, prepare_routed_user_proofs,
};
pub use maintenance_workflow::{
    MaintenanceBlocker, MaintenanceWorkflowError, MemoryMaintenancePreparation,
    MemoryPruneCandidate, prepare_tombstoned_memory_maintenance, promote_verified_memory_copy,
    prune_verified_memory_copy, recheck_tombstoned_memory_maintenance,
};
pub use perception_lifecycle_workflow::PerceptionLifecycleWriter;
pub use rel_reconciliation_workflow::{
    ReconciledRelOwner, RelReconciliationEvidence, reconcile_rel_owner,
};
pub use reliquary_dream::{DreamClassifier, DreamVerificationPolicy, DreamVerifier};
pub use reliquary_entity_resolution::TextField;
pub use reliquary_owner_session::{
    ArchivePruneReport, EntityPruneReport, ProfileRepairError, ProfileRepairPlan,
    ProfileRepairReport, RelationshipPruneReport, RetiredProfilePruneReport,
    StorageReclamationReport, execute_profile_repair, plan_profile_repair,
    prune_retired_profile_vectors_copy, prune_terminal_archive_copy,
    prune_terminal_entity_descriptions_copy, prune_terminal_relationship_descriptions_copy,
    reclaim_storage_copy,
};
pub use retrieval_workflow::{
    OwnerMemoryEvidence, OwnerMemorySelection, RetrievalWorkflowError, SelectedMemory,
    select_owner_memories,
};
pub use rewrite_workflow::{
    ArchiveRewriteMode, DerivedStateRewriteMode, FreshnessRewriteMode, MemoryRewriteFilter,
    OwnerRewriteKind, RewriteError, RewriteIdentityMode, RewritePlan, RewritePolicy,
    RewriteSelection, execute_memory_rewrite, plan_phy_rewrite, plan_rel_rewrite,
    verify_rewrite_source,
};
pub use vector_workflow::{
    MemoryEmbeddingBatch, MemoryEmbeddingSource, VectorWorkflowError, prepare_memory_embeddings,
};
pub use verified_copy_workflow::{
    VerifiedPhyOwnerCopy, VerifiedRelOwnerCopy, copy_and_reopen_phy_owner,
    copy_and_reopen_rel_owner, reconcile_phy_prefix, reconcile_rel_prefix,
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
    owner_id: [u8; 16],
    episode_id: EpisodeId,
    conversation_id: String,
    source_through_ns: i64,
    claim_snapshot: Option<(Episode, Vec<ResolvedTurn>)>,
    candidates: Vec<InsomniaCandidate>,
    rejected: Vec<reliquary_insomnia::InsomniaRejection>,
}

impl PreparedIngest {
    pub fn owner_id(&self) -> [u8; 16] {
        self.owner_id
    }

    pub fn episode_id(&self) -> EpisodeId {
        self.episode_id
    }

    pub fn conversation_id(&self) -> &str {
        &self.conversation_id
    }

    pub fn source_through_ns(&self) -> i64 {
        self.source_through_ns
    }

    pub fn candidates(&self) -> &[InsomniaCandidate] {
        &self.candidates
    }

    pub fn rejected(&self) -> &[reliquary_insomnia::InsomniaRejection] {
        &self.rejected
    }

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
    owner_id: [u8; 16],
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
        owner_id,
        episode_id: episode.id,
        conversation_id: episode.conversation_id.clone(),
        source_through_ns: episode.source_through_ns,
        claim_snapshot: None,
        candidates,
        rejected,
    })
}

/// Revalidate a model extraction against the owner-validated Episode snapshot
/// returned by the runtime claim API. The claim's lease and owner identity stay
/// bound through later publication.
pub fn prepare_native_ingest_from_claim(
    claim: &reliquary_owner_session::RelInsomniaClaimInput,
    extraction: InsomniaExtraction,
) -> Result<PreparedIngest, IngestError> {
    let proposals = raw_insomnia_candidates(extraction.candidates);
    let mut prepared =
        prepare_native_ingest(claim.owner_id, &claim.episode, &claim.turns, &[], proposals)?;
    prepared.claim_snapshot = Some((claim.episode.clone(), claim.turns.clone()));
    prepared.rejected.extend(extraction.rejected);
    Ok(prepared)
}

fn raw_insomnia_candidates(candidates: Vec<InsomniaCandidate>) -> Vec<RawCandidate> {
    candidates
        .into_iter()
        .map(|candidate| RawCandidate {
            authority_kind: candidate.authority_kind,
            category: candidate.category,
            memory_type: candidate.memory_type,
            temporal_status: candidate.temporal_status,
            ownership: candidate.ownership,
            title: candidate.title,
            content: candidate.content,
            source_node_id: candidate.source_node_id,
            source_quote: candidate.source_quote,
            authority_source: candidate.authority_source,
            grounding_source: candidate.grounding_source,
            routing_mentions: candidate.routing_mentions,
        })
        .collect()
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
        .archive_evidence_episode(episode_id)?
        .ok_or(IngestError::EpisodeUnavailable)?;
    for source in evidence {
        let archived = rel
            .archive_evidence_turn(&source.conversation_id, &source.node_id)?
            .ok_or(IngestError::EvidenceNotInArchive)?;
        if archived.role != source.role
            || archived.timestamp_ns != source.timestamp_ns
            || archived.content != source.content
        {
            return Err(IngestError::EvidenceNotInArchive);
        }
    }
    prepare_native_ingest(rel.owner_id(), &episode, &turns, evidence, proposals)
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
    use reliquary_freshness_store::FreshnessStore;
    use reliquary_insomnia::{InsomniaOwnership, RawCandidate};
    use reliquary_insomnia_store::AcceptedEpisodeOrder;
    use reliquary_memory_core::MemoryStore;
    use reliquary_owner_session::{GraphAdmission, RelWriter};
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
        MemoryStore::create(&mut container).unwrap();
        FreshnessStore::default()
            .initialize(&mut container)
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

    fn accepted_writer() -> (Temp, RelWriter, EpisodeId) {
        let (path, rel, episode_id) = accepted_rel();
        let owner = rel.owner_id();
        let episode = rel.episode(episode_id).unwrap();
        let order = AcceptedEpisodeOrder {
            episode_id: reliquary_insomnia_store::EpisodeId(episode_id.0),
            source_through_ns: episode.source_through_ns,
            conversation_id: episode.conversation_id.clone(),
            start_node_id: episode.start_node_id.clone(),
        };
        drop(rel);
        let admission = GraphAdmission::for_replay(owner, [], [], [], [], []);
        let writer = RelWriter::open_path(&path.0, admission, [order], []).unwrap();
        (path, writer, episode_id)
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
            prepare_native_ingest_from_archive(&rel, EpisodeId([255; 32]), &too_many, vec![],),
            Err(IngestError::EvidenceTurnLimit)
        );
    }

    #[test]
    fn publication_rejects_forged_claim_source_text() {
        let (_path, mut writer, episode_id) = accepted_writer();
        let work = writer
            .queue_and_claim_insomnia_episode(episode_id, "ingest-test", 10, 1_000)
            .unwrap();
        let rel = writer.read_session();
        let mut claim = reliquary_owner_session::RelInsomniaClaimInput {
            owner_id: rel.owner_id(),
            work,
            episode: rel.episode(episode_id).unwrap().clone(),
            turns: rel.episode_turns(episode_id).unwrap().unwrap(),
            started_at_ns: 10,
        };
        let quote = "Keep my project plans private.";
        claim.turns[0].content = quote.into();
        let extraction =
            validate_candidates(&claim.episode, &claim.turns, &[], vec![candidate(quote)]).unwrap();
        let prepared = prepare_native_ingest_from_claim(&claim, extraction).unwrap();
        assert_eq!(prepared.candidates().len(), 1);
        let version = writer.next_global_version().unwrap();
        assert!(matches!(
            commit_prepared_native_ingest(
                &mut writer,
                &prepared,
                &claim.work,
                "fixture-model",
                "v1",
                20
            ),
            Err(IngestPublicationError::EpisodeMismatch)
        ));
        assert_eq!(writer.next_global_version().unwrap(), version);
    }

    #[test]
    fn rel_unresolved_mention_preserves_revision_history_on_reopen() {
        use reliquary_owner_session::{
            EntityMentionCommit,
            model::{
                EntityResolutionReason, MemoryEntityMentionKey, MemoryEntityResolutionStatus,
                MemoryId, MemoryTextField, memory_body_id,
            },
        };
        let (path, mut writer, episode_id) = accepted_writer();
        let prepared = prepare_native_ingest_from_archive(
            writer.read_session(),
            episode_id,
            &[],
            vec![candidate("Keep my project notes private.")],
        )
        .unwrap();
        let work = writer
            .queue_and_claim_insomnia_episode(episode_id, "mention-test", 10, 1000)
            .unwrap();
        commit_prepared_native_ingest(&mut writer, &prepared, &work, "fixture", "v1", 20).unwrap();
        let receipt = writer
            .read_session()
            .insomnia_receipt(
                reliquary_insomnia_store::EpisodeId(episode_id.0),
                work.attempt_count,
            )
            .unwrap();
        let memory = writer
            .read_session()
            .current_memory(MemoryId(receipt.memory_ids[0].0))
            .unwrap();
        let owner_id = writer.owner_id();
        let key = MemoryEntityMentionKey {
            memory_id: memory.record.id.0,
            field: MemoryTextField::Title,
            start_byte: 0,
            end_byte: 7,
        };
        let mut command = EntityMentionCommit {
            owner_id,
            memory_revision: memory.record.revision,
            body_id: memory_body_id(
                "Private project notes",
                "The user wants project notes kept private.",
            ),
            key,
            surface: "Private".into(),
            expected_revision: 0,
            updated_at_ns: 21,
            status: MemoryEntityResolutionStatus::Unresolved {
                reason: EntityResolutionReason::InsufficientEvidence,
            },
            target_revisions: vec![],
            new_entity: None,
        };
        let mut wrong_body = command.clone();
        wrong_body.body_id = memory_body_id("different", "body");
        assert!(writer.commit_entity_mention(wrong_body).is_err());
        let first = writer.commit_entity_mention(command.clone()).unwrap();
        command.expected_revision = first.revision;
        command.updated_at_ns = 22;
        command.status = MemoryEntityResolutionStatus::Unresolved {
            reason: EntityResolutionReason::RecurrenceRequired,
        };
        let second = writer.commit_entity_mention(command).unwrap();
        drop(writer);
        let admission = GraphAdmission::for_replay(owner_id, [], [], [], [], []);
        let rel = RelSession::open_path(&path.0, &admission, [], []).unwrap();
        assert_eq!(rel.mention(key), Some(&second));
        assert_eq!(rel.mention_revision(key, first.revision), Some(&first));
    }

    #[test]
    fn commits_prepared_project_memory_through_owner_lease_idempotently() {
        let (_path, mut writer, episode_id) = accepted_writer();
        let prepared = prepare_native_ingest_from_archive(
            writer.read_session(),
            episode_id,
            &[],
            vec![candidate("Keep my project notes private.")],
        )
        .unwrap();
        let work = writer
            .queue_and_claim_insomnia_episode(episode_id, "ingest-test", 10, 1_000)
            .unwrap();
        let first =
            commit_prepared_native_ingest(&mut writer, &prepared, &work, "fixture-model", "v1", 20)
                .unwrap();
        assert_eq!(first, reliquary_owner_session::CompletionCheck::Apply);
        assert_eq!(
            commit_prepared_native_ingest(
                &mut writer,
                &prepared,
                &work,
                "fixture-model",
                "v1",
                20,
            )
            .unwrap(),
            reliquary_owner_session::CompletionCheck::AlreadyAccepted
        );
        let receipt = writer
            .read_session()
            .insomnia_receipt(
                reliquary_insomnia_store::EpisodeId(episode_id.0),
                work.attempt_count,
            )
            .unwrap();
        assert_eq!(receipt.memory_ids.len(), 1);
        let history = writer
            .read_session()
            .memory_history(reliquary_memory_core::MemoryId(receipt.memory_ids[0].0))
            .unwrap()
            .unwrap();
        assert!(matches!(
            &history.revisions[0].payload,
            reliquary_memory_core::PayloadState::Available(body)
                if body.title == "Private project notes"
                    && body.content == "The user wants project notes kept private."
        ));
    }

    #[test]
    fn same_owner_publication_fails_closed_for_user_owned_route() {
        let (_path, mut writer, episode_id) = accepted_writer();
        let mut raw = candidate("Keep my project notes private.");
        raw.ownership = InsomniaOwnership::User;
        let prepared =
            prepare_native_ingest_from_archive(writer.read_session(), episode_id, &[], vec![raw])
                .unwrap();
        let work = writer
            .queue_and_claim_insomnia_episode(episode_id, "ingest-test", 10, 1_000)
            .unwrap();
        assert_eq!(
            commit_prepared_native_ingest(&mut writer, &prepared, &work, "fixture-model", "v1", 20,),
            Err(IngestPublicationError::RoutedOwnerUnsupported)
        );
        let forged = reliquary_owner_session::records::GroupedExternalMemoryRef {
            owner_id: "fabricated-owner".to_owned(),
            memory_id: [42; 32],
        };
        assert_eq!(
            commit_prepared_native_ingest_with_external_refs(
                &mut writer,
                &prepared,
                &work,
                "fixture-model",
                "v1",
                20,
                &[forged],
            ),
            Err(IngestPublicationError::RoutedOwnerUnsupported)
        );
        assert!(
            writer
                .read_session()
                .insomnia_receipt(
                    reliquary_insomnia_store::EpisodeId(episode_id.0),
                    work.attempt_count,
                )
                .is_none()
        );
    }
    #[test]
    fn new_owner_native_turn_publishes_memory_and_recovers_idempotently() {
        let path = Temp::new();
        let owner = [72; 16];
        let mut writer = RelWriter::create_path(&path.0, ReliquaryScope::Project, owner).unwrap();
        commit_native_conversation_turn(
            &mut writer,
            NativeConversationCommit {
                node_id: "user-turn".into(),
                conversation_id: "conversation".into(),
                parent_node_id: None,
                branch_id: "main".into(),
                principal_id: "principal".into(),
                committed_timestamp_ns: 1,
                content: "Keep my project notes private.".into(),
            },
        )
        .unwrap();
        let episode = finalize_native_episode(
            &mut writer,
            "conversation",
            "user-turn",
            "user-turn",
            EpisodeBoundary::Explicit,
            2,
        )
        .unwrap();
        let claim = writer
            .claim_next_insomnia_input("worker", 10, 1000, 1)
            .unwrap()
            .unwrap();
        let prepared = prepare_native_ingest_from_archive(
            writer.read_session(),
            episode,
            &[],
            vec![candidate("Keep my project notes private.")],
        )
        .unwrap();
        assert_eq!(
            commit_prepared_native_ingest(
                &mut writer,
                &prepared,
                &claim.work,
                "fixture-model",
                "v1",
                20
            )
            .unwrap(),
            reliquary_owner_session::CompletionCheck::Apply
        );
        let receipt = writer
            .read_session()
            .insomnia_receipt(
                reliquary_insomnia_store::EpisodeId(episode.0),
                claim.work.attempt_count,
            )
            .unwrap()
            .clone();
        assert_eq!(receipt.memory_ids.len(), 1);
        drop(writer);
        let source_id = reliquary_memory_core::MemoryId(receipt.memory_ids[0].0);
        let dream_admission = reliquary_owner_session::DreamBeginAdmission {
            owner_uuid: owner,
            pass_id: "native-first-cycle".into(),
            source_id,
            turn: 1,
            graph_version_before: 0,
            preexisting_ids: Default::default(),
            memory_cut: 1,
        };
        let begin = reliquary_owner_session::records::dream::dream_begin_codec::BeginRecord {
            owner_uuid: owner, pass_id: dream_admission.pass_id.clone(), source_id, turn: 1,
            graph_version_before: 0, population: reliquary_owner_session::records::dream::dream_begin_codec::BeginPopulation::CompactCut {
                memory_cut: 1, digest: reliquary_owner_session::records::dream::population_digest(&Default::default()),
            },
        };
        let mut container = Container::open_writer(&path.0).unwrap();
        container
            .append(
                &reliquary_owner_session::records::dream::dream_begin_codec::encode(&begin)
                    .unwrap(),
            )
            .unwrap();
        container.sync().unwrap();
        drop(container);
        let admission = GraphAdmission::for_replay(owner, [], [], [], [], []);
        let mut reopened =
            RelWriter::open_path(&path.0, admission, [], [dream_admission.clone()]).unwrap();
        assert_eq!(
            reopened.read_session().dream_begin("native-first-cycle"),
            Some(&begin)
        );
        let history = reopened
            .read_session()
            .memory_history(reliquary_memory_core::MemoryId(receipt.memory_ids[0].0))
            .unwrap()
            .unwrap();
        assert_eq!(
            history.revisions[0].record.source_episode_id,
            Some(episode.0)
        );
        assert!(
            matches!(&history.revisions[0].payload, reliquary_memory_core::PayloadState::Available(body) if body.content == "The user wants project notes kept private.")
        );
        assert_eq!(
            commit_prepared_native_ingest(
                &mut reopened,
                &prepared,
                &claim.work,
                "fixture-model",
                "v1",
                20
            )
            .unwrap(),
            reliquary_owner_session::CompletionCheck::AlreadyAccepted
        );
        // A later metadata revision must not make an accepted completion retry
        // conflict with the original revision it actually published.
        let original = history.revisions[0].record.clone();
        drop(reopened);
        let mut container = Container::open_writer(&path.0).unwrap();
        let mut memories = MemoryStore::open_with_grouped_completion(&container, |payload| {
            let completion =
                reliquary_owner_session::records::GroupedInsomniaCompletion::decode(payload)
                    .map_err(|_| {
                        reliquary_memory_core::MemoryError::Corrupt("grouped completion")
                    })?;
            Ok(Some((
                completion
                    .bodies
                    .into_iter()
                    .map(|body| (body.id, body.bytes))
                    .collect(),
                completion.memory_records,
            )))
        })
        .unwrap();
        let revision = reliquary_memory_core::MemoryDraft {
            title: "Private project notes".into(),
            content: "The user wants project notes kept private.".into(),
            category: "revised".into(),
            memory_type: original.memory_type.clone(),
            authority_kind: original.authority_kind.clone(),
            temporal_status: original.temporal_status.clone(),
            scope: original.scope.clone(),
            lifecycle_state: original.lifecycle_state.clone(),
            archived: original.archived,
            parent_id: original.parent_id,
            superseded_by: original.superseded_by,
            source_node_id: original.source_node_id.clone(),
            content_source_conversation_id: original.content_source_conversation_id.clone(),
            content_source_node_id: original.content_source_node_id.clone(),
            grounding_source_conversation_id: original.grounding_source_conversation_id.clone(),
            grounding_source_node_id: original.grounding_source_node_id.clone(),
            source_episode_id: original.source_episode_id,
            source_ref: original.source_ref.clone(),
            source_time_ns: original.source_time_ns,
            temporal_inference: original.temporal_inference.clone(),
            mutation_id: "later-revision".into(),
            created_at_ns: original.created_at_ns,
            updated_at_ns: 21,
        };
        memories
            .publish(&mut container, Some(original.id), 1, revision)
            .unwrap();
        container.sync().unwrap();
        drop(memories);
        drop(container);
        let admission = GraphAdmission::for_replay(owner, [], [], [], [], []);
        let mut reopened = RelWriter::open_path(&path.0, admission, [], [dream_admission]).unwrap();
        assert_eq!(
            commit_prepared_native_ingest(
                &mut reopened,
                &prepared,
                &claim.work,
                "fixture-model",
                "v1",
                20
            )
            .unwrap(),
            reliquary_owner_session::CompletionCheck::AlreadyAccepted
        );
    }
}

mod dream_batch_workflow;
pub use dream_batch_workflow::{commit_dream_batch, draft_dream_batch_organization};

mod dream_frontier_workflow;
pub use dream_frontier_workflow::{
    PreparedDreamFrontier, begin_prepared_dream_frontier, prepare_explicit_dream_frontier,
};

mod external_hydration;
pub use external_hydration::{
    HydratedExternalChunk, HydratedExternalMessage, validate_external_hydration_request,
    verify_external_hydration,
};

mod external_lexical_index;
pub use external_lexical_index::ExternalLexicalIndex;

mod external_change_feed;
pub use external_change_feed::{
    ExternalChangeBatch, ExternalChangeKind, ExternalConversationChange,
    apply_external_change_batch,
};

mod external_processing;
pub use external_processing::{
    ExternalMemoryProposal, external_window_id, prepare_external_memory,
};

mod agentic_retrieval;
pub use agentic_retrieval::{
    AgenticMemory, RetrievalAction, RetrievalAnchorReport, RetrievalDirection, RetrievalEvaluation,
    RetrievalRelationEvidence, evaluate_retrieval,
};
pub use reliquary_retrieval::RetrievalConfig;
pub use reliquary_retrieval::agentic::RetrievalTurn;
pub use reliquary_retrieval::memory_retrieval::{MemoryRetrievalConfig, MemoryRetrievalResult};

mod owner_retrieval;
pub use owner_retrieval::{agentic_memories, rank_owner_snapshot, route_owner_snapshot};
