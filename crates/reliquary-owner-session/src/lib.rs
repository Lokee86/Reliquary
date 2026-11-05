//! Typed composition and mutation authority for the current REL and PHY owners.
//!
//! The aggregate keeps individual stores private. Replayed graph endpoints,
//! accepted Episodes are derived from durable owners; Dream first-cycle
//! populations still require externally supplied admission evidence.

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
pub use reliquary_freshness_store::{FreshnessEventProof, FreshnessMemoryId, FreshnessUseReceipt};
/// Bound for durable Insomnia inference checkpoints, owned by the session store.
pub use reliquary_insomnia_store::MAX_HELD_PAYLOAD_BYTES as MAX_INSOMNIA_CHECKPOINT_BYTES;
pub use reliquary_insomnia_store::{CompletionCheck, InsomniaLeaseToken};
pub use session::{
    DreamBatchCut, DreamCandidatePool, DreamLifecyclePublication, DreamPairPublication,
    InitialDreamSnapshot, OwnerSessionError, PhySession, PhyWriter, RelInsomniaClaimInput,
    RelRetrievalMemory, RelRetrievalSnapshot, RelSession, RelWriter, RoutedUserMemoryProof,
    RoutedUserMemoryReceipt,
};

pub use reliquary_vector_store::VectorGeneration;
pub use session::EntityMentionCommit;
pub use session::{
    ArchiveVectorInput, ArchiveVectorRow, MAX_ARCHIVE_VECTOR_BATCH_ROWS,
    MAX_ARCHIVE_VECTOR_RESPONSE_BYTES,
};
pub use session::{MemoryVectorInput, MemoryVectorRow};

pub use session::{EntityCandidateMemory, EntityMentionInputs};
