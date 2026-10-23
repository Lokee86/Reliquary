//! Immutable owner-derived manifest data consumed by native Import planning.
use crate::freshness::FreshnessRecord;
use crate::history::import_admission::HistoricalMemoryInput;
use crate::imports::historical_settlement_plan::PropagationInputs;
use crate::{MemoryId, freshness::historical_planner::HistoricalTimeline};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone)]
pub(crate) struct Manifest {
    pub(crate) propagation: PropagationInputs,
    pub(crate) input_digest: [u8; 32],
    pub(crate) input_bytes: u64,
    pub(crate) memory_inputs: BTreeMap<MemoryId, HistoricalMemoryInput>,
    pub(crate) timeline: HistoricalTimeline,
    pub(crate) digest: [u8; 32],
    pub(crate) corpus_digest: [u8; 32],
    pub(crate) import_id: [u8; 32],
    pub(crate) initial: BTreeMap<MemoryId, FreshnessRecord>,
    pub(crate) superseded: BTreeSet<String>,
    /// Whether this proof is bound to one authenticated import epoch.
    pub(crate) epoch: bool,
    /// Born and provisionally affected recipients; legacy retains the owner set.
    pub(crate) cohort: BTreeSet<MemoryId>,
    /// Pre-start activity watermark for an epoch.
    pub(crate) base_cut: u64,
    /// Digest of the durable ImportStart evidence for an epoch.
    pub(crate) start_evidence_digest: [u8; 32],
    #[cfg(test)]
    pub(crate) value: crate::history::manifest_encoding::Value,
}
