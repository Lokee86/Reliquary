//! Existing import/reconciliation coordinate mapping above the Dream journal.
use crate::freshness_dream_journal::{DreamEntry, DreamFreshnessJournal};

pub(crate) fn rebase_payload(
    journal: &DreamFreshnessJournal,
    payload: &[u8],
    mut turn: impl FnMut(u64) -> Result<u64, String>,
    mut graph: impl FnMut(u64) -> Result<u64, String>,
    mut proof: impl FnMut(crate::FreshnessEventProof) -> Result<crate::FreshnessEventProof, String>,
) -> Result<Vec<u8>, String> {
    let Some(entry) = journal.replay_entry(payload)? else {
        return Ok(Vec::new());
    };
    let mapped = match entry {
        DreamEntry::Accept(id, cut) => DreamEntry::Accept(id, turn(cut)?),
        DreamEntry::AcceptGraph(id, cut, version, pinned) => {
            DreamEntry::AcceptGraph(id, turn(cut)?, graph(version)?, proof(pinned)?)
        }
        DreamEntry::Intent(id, mutation, cut, admitted) => {
            DreamEntry::Intent(id, mutation, turn(cut)?, admitted)
        }
        other => other,
    };
    journal.encode_replay_entry(mapped)
}
