//! Explicit owner maintenance operations and their concrete request values.

mod prune_selection;

pub use prune_selection::{HistoryPruneReport, HistoryPruneRequest};
pub(crate) use prune_selection::{
    PhylacteryPruneTargets, ReliquaryPruneTargets, select_phylactery_targets,
    select_reliquary_targets,
};

mod phylactery_prune;
pub(crate) use phylactery_prune::{PhylacteryPruneParts, prune_phylactery};

pub(crate) mod reclamation;
pub(crate) mod reclamation_scan;

mod phylactery_copy;
pub(crate) mod phylactery_reclamation;

pub(crate) mod entity_resolution;
pub(crate) mod tail_repair;

pub(crate) mod compactions;
