//! Facade read-part gathering for the canonical History admission proof.
pub(super) use crate::history::import_admission::{AdmissionProof, HistoricalMemoryInput};
pub(super) use crate::history::source_proof::{node_binding, required_nodes, source_key};
use crate::{Cva, CvaError, MemoryId, freshness::FreshnessRecord};
use std::collections::{BTreeMap, BTreeSet};
pub(super) fn corpus_digest(cva: &Cva, through: u64) -> Result<[u8; 32], CvaError> {
    cva.runtime_historical_corpus_digest(through)
}
pub(super) fn scoped_corpus_digest(
    cva: &Cva,
    source_nodes: &BTreeSet<(String, String)>,
) -> Result<[u8; 32], CvaError> {
    cva.runtime_historical_scoped_corpus_digest(source_nodes)
}
pub(super) fn admission_proof(cva: &Cva) -> Result<AdmissionProof, CvaError> {
    cva.runtime_historical_admission_proof()
}
pub(super) fn admission_proof_scoped(
    cva: &Cva,
    scope: Option<&crate::imports::ImportEpochScope>,
) -> Result<AdmissionProof, CvaError> {
    cva.runtime_historical_admission_proof_scoped(scope)
}
