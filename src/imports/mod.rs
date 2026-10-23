//! Versioned native historical import. Adapters normalize; Archive remains authoritative.
pub(crate) mod completion_coordinator;
mod contract;
pub(crate) mod epoch_codec;
pub(crate) mod epoch_model;
pub(crate) mod epoch_store;
mod hermes;
pub(crate) mod native_commit;
mod prepare;
mod providers;
pub(crate) mod readiness;
mod reconciliation;
mod reconciliation_copy;
mod reconciliation_epoch_replay;
mod reconciliation_interval;
mod reconciliation_replay;
pub use contract::*;
pub use hermes::normalize_hermes_jsonl;
pub(crate) use reconciliation::reconcile_import_epochs;
#[cfg(test)]
mod reconcile_tests;
#[cfg(test)]
mod sequential_completion_tests;
#[cfg(test)]
pub(crate) mod tests;
pub(crate) use crate::history::ImportEpochScope;
pub(crate) use epoch_store::ImportEpochStore;
pub(crate) use prepare::prepare;
mod chatgpt;
mod claude;
pub use chatgpt::normalize_chatgpt;
pub use claude::normalize_claude;
pub use providers::normalize_agent;

pub(crate) mod historical_event_plan;
pub(crate) mod historical_events;
pub(crate) mod historical_manifest_model;

pub(crate) mod historical_embedding_model;
pub(crate) mod historical_manifest_plan;
pub(crate) mod historical_reconstruction_model;
pub(crate) mod historical_settlement_plan;

pub(crate) mod historical_freeze;
