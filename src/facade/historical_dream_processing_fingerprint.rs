//! Test-only fingerprint of the owners' current Dream processing-lane state.
//!
//! Dream processing epochs have no independent history clock. This projection
//! therefore names a clock-neutral current snapshot and includes only the
//! canonical decoded processing-epoch records owned by REL or PHY.

use crate::historical_fingerprint::{HistoricalFingerprint, OwnerProjection};
use crate::processing_epoch_model::{ProcessingEpochState, ProcessingLaneId};
use crate::{Cva, MemoryId, Phylactery};
use serde_json::json;

pub(crate) struct DreamProcessingCut {
    pub(crate) fingerprint: HistoricalFingerprint,
    pub(crate) records: Vec<(MemoryId, ProcessingLaneId, ProcessingEpochState)>,
}

pub(crate) fn reliquary_dream_processing(cva: &Cva) -> Result<DreamProcessingCut, String> {
    dream_processing(
        cva.owner_uuid()
            .ok_or("Dream processing oracle needs owner UUID")?,
        cva.processing_epochs.records(),
    )
}

pub(crate) fn phylactery_dream_processing(phy: &Phylactery) -> Result<DreamProcessingCut, String> {
    dream_processing(
        phy.owner_uuid()
            .ok_or("Dream processing oracle needs owner UUID")?,
        phy.processing_epochs.records(),
    )
}

fn dream_processing(
    owner: [u8; 16],
    records: impl IntoIterator<Item = (MemoryId, ProcessingLaneId, ProcessingEpochState)>,
) -> Result<DreamProcessingCut, String> {
    let mut projection = OwnerProjection::new(owner, "dream_processing", 0, None)?;
    let mut records: Vec<_> = records.into_iter().collect();
    records.sort_by_key(|(memory, lane, _)| (memory.0, lane.0));
    for (memory, lane, state) in &records {
        let mut key = Vec::with_capacity(34);
        key.extend_from_slice(&memory.0);
        key.extend_from_slice(&lane.0.to_le_bytes());
        projection.insert(
            "processing_epoch",
            key,
            json!({
                "memory_id": memory.0,
                "lane_id": lane.0,
                "satisfied_through_epoch": state.satisfied_through_epoch,
                "cadence_version": state.cadence_version,
                "checkpoint_at_ns": state.checkpoint_at_ns,
            }),
        )?;
    }
    Ok(DreamProcessingCut {
        fingerprint: projection.fingerprint()?,
        records,
    })
}

#[cfg(test)]
#[path = "historical_dream_processing_fingerprint_tests.rs"]
mod tests;
