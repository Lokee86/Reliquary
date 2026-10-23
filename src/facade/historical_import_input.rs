//! Facade read-part gathering for the canonical History admission proof.
pub(super) use crate::history::import_admission::{AdmissionProof, HistoricalMemoryInput};
pub(super) use crate::history::source_proof::{node_binding, required_nodes, source_key};
use crate::{Cva, CvaError, MemoryId, freshness::FreshnessRecord};
use std::collections::{BTreeMap, BTreeSet};
fn parts(cva: &Cva) -> crate::history::import_admission::ImportAdmissionReadParts<'_> {
    crate::history::import_admission::ImportAdmissionReadParts {
        access: crate::owner_lifecycle::HistoryReadAccess::new(&cva.container),
        archive: &cva.archive,
        memories: &cva.memories,
        freshness: &cva.freshness,
        dream: &cva.dream_freshness,
    }
}
pub(super) fn corpus_digest(cva: &mut Cva, through: u64) -> Result<[u8; 32], CvaError> {
    crate::history::import_admission::corpus_digest(&parts(cva), through)
}
pub(super) fn scoped_corpus_digest(
    cva: &mut Cva,
    source_nodes: &BTreeSet<(String, String)>,
) -> Result<[u8; 32], CvaError> {
    crate::history::import_admission::scoped_corpus_digest(&parts(cva), source_nodes)
}
pub(super) fn admission_proof(cva: &mut Cva) -> Result<AdmissionProof, CvaError> {
    crate::history::import_admission::admission_proof(&parts(cva))
}
pub(super) fn admission_proof_scoped(
    cva: &mut Cva,
    scope: Option<&crate::imports::ImportEpochScope>,
) -> Result<AdmissionProof, CvaError> {
    crate::history::import_admission::admission_proof_scoped(&parts(cva), scope)
}
