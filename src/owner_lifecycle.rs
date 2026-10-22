//! Validated owner lifecycle boundary and read-only historical source capability.

#[path = "owner_lifecycle/history_source.rs"]
mod history_source;
pub(crate) use history_source::{HistoryFrame, HistoryReadFailure, HistorySource};

#[path = "owner_lifecycle/vocabulary.rs"]
mod vocabulary;
pub use vocabulary::{
    HistoricalPayload, LifecycleError, LifecycleState, LifecycleTarget, PayloadUnavailable,
    TombstoneCoordinates, TombstoneOutcome, TombstonePolicy,
};

#[path = "owner_lifecycle/error.rs"]
mod error;
pub use error::OwnerError;
