//! Passive owner-scoped durability records. Semantic truth and foreign-owner
//! admission remain with their owning stores and coordinators.
pub mod dream;
pub mod external_index;
pub mod import;
mod import_model;
pub mod insomnia_completion;
pub mod maintenance;
pub use dream::{DreamBeginAdmission, DreamEvidenceStore};
pub use import::{ImportEpochStore, import_id};
pub use import_model::{ImportComplete, ImportEpochStatus, ImportStart, ImportWatermarks};
pub use insomnia_completion::{
    GroupedExternalMemoryRef, GroupedInsomniaCompletion, GroupedMemoryBody,
};
pub use maintenance::{
    HistoryPruneManifest, HistoryRetentionDescriptor, OwnerMaintenanceStore, PrunedPayload,
    RetentionDomain, RetentionTarget,
};

#[cfg(test)]
mod tests;
