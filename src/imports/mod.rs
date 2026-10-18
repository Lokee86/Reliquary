//! Versioned native historical import. Adapters normalize; Archive remains authoritative.
mod commit;
mod completion;
mod contract;
mod epoch_codec;
mod epoch_scope;
mod epoch_store;
mod hermes;
pub(crate) mod legacy_epoch;
mod prepare;
mod providers;
mod readiness;
mod reconciliation;
mod reconciliation_interval;
mod reconciliation_replay;
pub use contract::*;
pub use hermes::normalize_hermes_jsonl;
pub use legacy_epoch::{LegacyImportEpoch, LegacyImportEpochOrigin, LegacyImportEpochStatus};
pub(crate) use reconciliation::reconcile_import_epochs;
#[cfg(test)]
mod reconcile_tests;
#[cfg(test)]
mod sequential_completion_tests;
#[cfg(test)]
pub(crate) mod tests;
pub(crate) use epoch_scope::ImportEpochScope;
pub(crate) use epoch_store::ImportEpochStore;
pub(crate) use prepare::prepare;
mod chatgpt;
mod claude;
pub use chatgpt::normalize_chatgpt;
pub use claude::normalize_claude;
pub use providers::normalize_agent;
