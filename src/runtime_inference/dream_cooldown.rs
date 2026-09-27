use crate::chronos_processing_epoch::{
    ProcessingCadence, cadence_elapsed, epoch_has_advanced, processing_epoch,
};
use crate::memory_source_time::{memory_source_timestamp_ns, reliquary_memory_source_timestamp_ns};
use crate::processing_epoch_model::{ProcessingEpochState, ProcessingLaneId};
use crate::{Cva, Memory, MemoryError, MemoryId, Phylactery};
use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) const DREAM_PROCESSING_LANE: ProcessingLaneId = ProcessingLaneId(1);
pub(crate) const DREAM_CADENCE_VERSION: u32 = 1;
pub const DEFAULT_DREAM_REPROCESS_COOLDOWN_NS: i64 = 30 * 24 * 60 * 60 * 1_000_000_000;

fn dream_cadence() -> ProcessingCadence {
    ProcessingCadence::new(DEFAULT_DREAM_REPROCESS_COOLDOWN_NS)
        .expect("Dream reprocess cadence must be positive")
}

fn dream_processing_state(epoch: u64, processed_at_ns: i64) -> ProcessingEpochState {
    ProcessingEpochState {
        satisfied_through_epoch: epoch,
        cadence_version: DREAM_CADENCE_VERSION,
        checkpoint_at_ns: Some(processed_at_ns),
    }
}

pub(crate) fn dream_epoch(source_time_ns: i64, now_ns: i64) -> u64 {
    processing_epoch(source_time_ns, now_ns, dream_cadence())
}

fn eligible_dream_epoch(
    memory: &Memory,
    source_time_ns: Option<i64>,
    last_processed: Option<ProcessingEpochState>,
    now_ns: i64,
) -> Result<Option<u64>, MemoryError> {
    if memory.archived {
        return Ok(None);
    }
    if memory.lifecycle_state == "extracted" {
        return Ok(Some(
            source_time_ns
                .map(|source_time| dream_epoch(source_time, now_ns))
                .unwrap_or(0),
        ));
    }
    if last_processed.is_some_and(|state| state.cadence_version != DREAM_CADENCE_VERSION) {
        return Err(MemoryError::InvalidField("Dream cadence version"));
    }

    if let Some(source_time_ns) = source_time_ns {
        let satisfied_epoch = last_processed
            .map(|state| state.satisfied_through_epoch)
            .unwrap_or_else(|| dream_epoch(source_time_ns, memory.updated_at_ns));
        return Ok(epoch_has_advanced(
            source_time_ns,
            satisfied_epoch,
            now_ns,
            dream_cadence(),
        ));
    }

    let last_processed_ns = last_processed
        .and_then(|state| state.checkpoint_at_ns)
        .unwrap_or(memory.updated_at_ns);
    Ok(cadence_elapsed(last_processed_ns, now_ns, dream_cadence()).then_some(0))
}

pub(crate) fn unix_now_ns() -> i64 {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    nanos.min(i64::MAX as u128) as i64
}

impl Cva {
    pub(crate) fn dream_eligible_epoch(
        &mut self,
        id: MemoryId,
        now_ns: i64,
    ) -> Result<Option<u64>, MemoryError> {
        let memory = self.memory(id)?;
        let source_time = reliquary_memory_source_timestamp_ns(&self.archive, &memory);
        eligible_dream_epoch(
            &memory,
            source_time,
            self.processing_epochs.state(id, DREAM_PROCESSING_LANE),
            now_ns,
        )
    }

    pub(crate) fn mark_dream_processed(
        &mut self,
        id: MemoryId,
        epoch: u64,
        processed_at_ns: i64,
    ) -> Result<bool, MemoryError> {
        if !self.memories.contains_memory(id) {
            return Err(MemoryError::MissingMemory);
        }
        self.processing_epochs.put(
            &mut self.container,
            id,
            DREAM_PROCESSING_LANE,
            dream_processing_state(epoch, processed_at_ns),
        )
    }

    pub(crate) fn dream_was_processed(&self, id: MemoryId) -> bool {
        self.processing_epochs
            .state(id, DREAM_PROCESSING_LANE)
            .is_some_and(|state| state.checkpoint_at_ns.is_some())
    }
}

impl Phylactery {
    pub(crate) fn dream_eligible_epoch(
        &mut self,
        id: MemoryId,
        now_ns: i64,
    ) -> Result<Option<u64>, MemoryError> {
        let memory = self.memory(id)?;
        eligible_dream_epoch(
            &memory,
            memory_source_timestamp_ns(&memory),
            self.processing_epochs.state(id, DREAM_PROCESSING_LANE),
            now_ns,
        )
    }

    pub(crate) fn mark_dream_processed(
        &mut self,
        id: MemoryId,
        epoch: u64,
        processed_at_ns: i64,
    ) -> Result<bool, MemoryError> {
        if !self.memories.contains_memory(id) {
            return Err(MemoryError::MissingMemory);
        }
        self.processing_epochs.put(
            &mut self.container,
            id,
            DREAM_PROCESSING_LANE,
            dream_processing_state(epoch, processed_at_ns),
        )
    }

    pub(crate) fn dream_was_processed(&self, id: MemoryId) -> bool {
        self.processing_epochs
            .state(id, DREAM_PROCESSING_LANE)
            .is_some_and(|state| state.checkpoint_at_ns.is_some())
    }
}
