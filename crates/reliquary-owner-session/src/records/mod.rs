//! Passive owner-scoped durability records. Semantic truth and foreign-owner
//! admission remain with their owning stores and coordinators.
pub mod dream;
pub(crate) mod dream_batch;
pub(crate) mod dream_frontier;
pub use dream_frontier::InitialDreamFrontier;
pub(crate) mod dream_empty;
pub(crate) mod dream_group;
pub mod dream_no_link;
pub(crate) mod dream_supersession;
pub mod external_index;
pub(crate) mod external_memory;
pub(crate) mod external_reference;
pub mod import;
mod import_model;
pub(crate) mod import_settlement;
pub mod insomnia_completion;
pub mod maintenance;
pub(crate) mod memory_prune_proof;
pub(crate) mod user_memory;
pub mod verified_copy;
pub use dream::{DreamBeginAdmission, DreamEvidenceStore};
pub use dream_group::DreamRelationCut;
pub use dream_no_link::{DreamMemoryCut, DreamNoLinkCut};
pub use external_memory::{ExternalMemoryDraft, ExternalMemoryProvenance, ExternalMemoryReceipt};
pub use external_reference::{ExternalReferenceSnapshotManifest, ExternalReferenceSnapshotReceipt};
pub(crate) use import::{IMPORT_PROCESSING_MAGIC, ImportProcessingBinding, processing_marker_id};
pub use import::{ImportEpochStore, import_id};
pub use import_model::{ImportComplete, ImportEpochStatus, ImportStart, ImportWatermarks};
pub use import_settlement::{ImportFreshnessAuthority, ImportFreshnessSettlement, hex_digest};
pub use insomnia_completion::{
    GroupedExternalMemoryRef, GroupedInsomniaCompletion, GroupedMemoryBody,
    encode_insomnia_memory_creation,
};
pub use maintenance::{
    HistoryPruneManifest, HistoryRetentionDescriptor, OwnerMaintenanceStore, PrunedPayload,
    RetentionDomain, RetentionTarget,
};
pub use verified_copy::{VerifiedOwnerCopyPlan, VerifiedOwnerCopyReceipt};

#[cfg(test)]
mod tests;
