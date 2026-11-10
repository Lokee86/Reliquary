//! Typed composition and mutation authority for the current REL and PHY owners.
//!
//! The aggregate keeps individual stores private. Replayed graph endpoints,
//! accepted Episodes are derived from durable owners; Dream first-cycle
//! populations still require externally supplied admission evidence.

pub use reliquary_graph::CommunityId;

pub use reliquary_graph::GraphAdmission;
pub use reliquary_insomnia_store::AcceptedEpisodeOrder;
pub use reliquary_memory_core::MemoryId;
pub mod model;
pub mod records;
pub use records::{
    DreamBeginAdmission, DreamMemoryCut, DreamNoLinkCut, DreamRelationCut,
    ExternalReferenceSnapshotManifest, ExternalReferenceSnapshotReceipt, InitialDreamFrontier,
    VerifiedOwnerCopyPlan, VerifiedOwnerCopyReceipt,
};
pub use records::{ExternalMemoryDraft, ExternalMemoryProvenance, ExternalMemoryReceipt};
mod session;
mod staged_copy;
pub use reliquary_freshness_store::{FreshnessEventProof, FreshnessMemoryId, FreshnessUseReceipt};
/// Bound for durable Insomnia inference checkpoints, owned by the session store.
pub use reliquary_insomnia_store::MAX_HELD_PAYLOAD_BYTES as MAX_INSOMNIA_CHECKPOINT_BYTES;
pub use reliquary_insomnia_store::{CompletionCheck, InsomniaLeaseToken, InsomniaWorkState};
pub use session::{CompletedImportExtraction, InsomniaEpisodeStatus, LiveExtractionOutput};
pub use session::{
    DreamBatchCut, DreamCandidatePool, DreamLifecyclePublication, DreamPairPublication,
    InitialDreamSnapshot, NativeImportFreshnessInput, NativeImportHistoricalEvent,
    NativeImportMemoryAvailability, NativeImportRetainedEvent, OwnerFailureKind, OwnerScope,
    OwnerSessionError, PhySession, PhyWriter, PrunedOwnerCopyReport, RelInsomniaClaimInput,
    RelRetrievalMemory, RelRetrievalSnapshot, RelSession, RelWriter, RoutedUserMemoryProof,
    RoutedUserMemoryReceipt, promote_terminal_memory_copy, prune_terminal_memory_copy,
};
pub use session::{
    FreshHistoryArchive, FreshHistoryFreshness, FreshHistoryIdentity, FreshHistoryLineage,
    FreshHistoryRequest, FreshHistoryRewriteReport, fresh_history_lineage,
    owner_history_generation, rewrite_memory_history_copy,
};
pub use session::{
    HistoricalOwnerCut, HistoricalOwnerProof, HistoricalProofError, RelHistoricalCut,
};
pub use session::{
    ManagementRead, RelConversationSummary, RelImportedSource, RelSourceTurn, RelTranscriptTurn,
};
pub use session::{
    RelHistoryComparison, RelHistoryRelation, RelReconciliationError, RelReconciliationReport,
    compare_rel_histories, reconcile_divergent_rel_copy,
};
pub use session::{
    SemanticGraphCommunity, SemanticGraphCommunityMemberPage, SemanticGraphCommunityPage,
    SemanticGraphCut, SemanticGraphMemory, SemanticGraphMemoryPage, SemanticGraphRelationPage,
    SemanticGraphSnapshot,
};

pub use reliquary_vector_store::VectorGeneration;
pub use session::EntityMentionCommit;
pub use session::MemoryAssociationCut;
pub use session::{ArchivePruneReport, prune_terminal_archive_copy};
pub use session::{
    ArchiveVectorInput, ArchiveVectorRow, MAX_ARCHIVE_VECTOR_BATCH_ROWS,
    MAX_ARCHIVE_VECTOR_RESPONSE_BYTES,
};
pub use session::{
    CreateRelationshipRequest, EntityRetirementRequest, MemoryRetirementRequest,
    RelationshipParticipantEvidence, RelationshipRetirementRequest, WithdrawEntityMentionRequest,
};
pub use session::{EntityPruneReport, prune_terminal_entity_descriptions_copy};
pub use session::{MemoryVectorInput, MemoryVectorRow};
pub use session::{
    ProfileRepairError, ProfileRepairPlan, ProfileRepairReport, execute_profile_repair,
    plan_profile_repair,
};
pub use session::{RelationshipPruneReport, prune_terminal_relationship_descriptions_copy};
pub use session::{RetiredProfilePruneReport, prune_retired_profile_vectors_copy};
pub use session::{StorageReclamationReport, reclaim_storage_copy};
pub use session::{UserMemoryCorrection, UserMemoryCorrectionResult};

pub use session::{EntityCandidateMemory, EntityMentionInputs};
