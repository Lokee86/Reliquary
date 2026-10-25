//! Existing import/reconciliation coordinate mapping above the Dream journal.
use crate::freshness_dream_journal::DreamFreshnessJournal;
use crate::freshness_dream_journal::DreamReplayCoordinateRequests;
use crate::freshness_dream_journal::DreamReplayCoordinates;

pub(crate) struct PreparedDreamReplay {
    pub(crate) coordinates: DreamReplayCoordinates,
    pub(crate) requests: DreamReplayCoordinateRequests,
}

/// Dream validates the journal record and returns its finite coordinate needs
/// with an initialized passive plan for Import to fill.
pub(crate) fn prepare_rebase(
    journal: &DreamFreshnessJournal,
    payload: &[u8],
) -> Result<PreparedDreamReplay, String> {
    let requests = journal
        .replay_coordinate_requests(payload)?
        .unwrap_or_default();
    let mut coordinates = DreamReplayCoordinates::default();
    coordinates.turns.insert(0, 0);
    Ok(PreparedDreamReplay {
        coordinates,
        requests,
    })
}

pub(crate) fn rebase_payload(
    journal: &DreamFreshnessJournal,
    payload: &[u8],
    coordinates: &DreamReplayCoordinates,
) -> Result<Vec<u8>, String> {
    journal.rebase_replay_payload(payload, coordinates)
}
