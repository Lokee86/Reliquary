//! Passive owner-scoped durability records. Semantic truth and foreign-owner
//! admission remain with their owning stores and coordinators.
pub mod dream;
pub(crate) mod dream_group;
pub mod dream_no_link;
pub mod external_index;
pub mod import;
mod import_model;
pub mod insomnia_completion;
pub mod maintenance;
pub mod verified_copy;
pub use dream::{DreamBeginAdmission, DreamEvidenceStore};
pub use dream_group::DreamRelationCut;
pub use dream_no_link::{DreamMemoryCut, DreamNoLinkCut};
pub use import::{ImportEpochStore, import_id};
pub use import_model::{ImportComplete, ImportEpochStatus, ImportStart, ImportWatermarks};
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
