//! Existing import/reconciliation coordinate mapping above the Dream journal.
use crate::freshness_dream_journal::DreamFreshnessJournal;

pub(crate) fn rebase_payload(
    journal: &DreamFreshnessJournal,
    payload: &[u8],
    turn: impl FnMut(u64) -> Result<u64, String>,
    graph: impl FnMut(u64) -> Result<u64, String>,
    proof: impl FnMut(crate::FreshnessEventProof) -> Result<crate::FreshnessEventProof, String>,
) -> Result<Vec<u8>, String> {
    journal.rebase_replay_payload(payload, turn, graph, proof)
}
