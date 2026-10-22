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

#[path = "owner_lifecycle/memory_frames.rs"]
pub(crate) mod memory_frames;

#[path = "owner_lifecycle/insomnia_replay.rs"]
pub(crate) mod insomnia_replay;

#[path = "owner_lifecycle/open_route.rs"]
pub(crate) mod open_route;

#[path = "owner_lifecycle/reliquary_replay.rs"]
mod reliquary_replay;
pub(crate) use reliquary_replay::ReliquaryReplay;

#[path = "owner_lifecycle/phylactery_error.rs"]
mod phylactery_error;
pub use phylactery_error::OwnerPhylacteryError;

#[path = "owner_lifecycle/phy_profile/mod.rs"]
mod phy_profile;
pub(crate) use phy_profile::PhylacteryProfileStore;
pub use phy_profile::{MAX_PHYLACTERY_PROFILE_NAME_BYTES, PhylacteryProfile};

#[path = "owner_lifecycle/read_validation.rs"]
mod read_validation;
pub(crate) use read_validation::{
    validate_current_graph_node, validate_current_relationship_references, validate_dream_pairs,
    validate_entity_resolution_references, validate_phylactery_draft_provenance,
    validate_phylactery_global_versions, validate_phylactery_provenance,
    validate_principal_graph_kind, validate_rebuilt_graph, validate_relationship_entities,
};

#[path = "owner_lifecycle/phylactery.rs"]
mod phylactery;
pub(crate) use phylactery::{
    PhylacteryOwner, initialize_phylactery, open_phylactery_from_container,
};

#[path = "owner_lifecycle/global_versions.rs"]
pub(crate) mod global_versions;
#[path = "owner_lifecycle/memory_provenance.rs"]
pub(crate) mod memory_provenance;
