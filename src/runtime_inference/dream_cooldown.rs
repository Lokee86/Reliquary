use crate::chronos_processing_epoch::{
    ProcessingCadence, cadence_elapsed, epoch_has_advanced, processing_epoch,
};
use crate::memory_source_time::{memory_source_timestamp_ns, reliquary_memory_source_timestamp_ns};
use crate::memory_store::MemoryStore;
use crate::processing_epoch_model::ProcessingLaneId;
use crate::{Container, Cva, Memory, MemoryError, MemoryId, Phylactery};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

const DREAM_COOLDOWN_MAGIC_V1: [u8; 8] = *b"CVADREM1";
const DREAM_COOLDOWN_MAGIC_V2: [u8; 8] = *b"CVADREM2";
pub(crate) const DREAM_PROCESSING_LANE: ProcessingLaneId = ProcessingLaneId(1);
pub const DEFAULT_DREAM_REPROCESS_COOLDOWN_NS: i64 = 30 * 24 * 60 * 60 * 1_000_000_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DreamCooldownState {
    pub(crate) epoch: u64,
    pub(crate) processed_at_ns: Option<i64>,
}

#[derive(Default)]
pub(crate) struct DreamCooldownStore {
    states: HashMap<MemoryId, DreamCooldownState>,
}

impl DreamCooldownStore {
    pub(crate) fn ingest(&mut self, payload: &[u8]) -> Result<(), MemoryError> {
        if payload.len() < 8 {
            return Ok(());
        }
        let (id, state) = if payload[..8] == DREAM_COOLDOWN_MAGIC_V2 {
            if payload.len() != 56 {
                return Err(MemoryError::CorruptRecord("invalid Dream cooldown record"));
            }
            (
                MemoryId(payload[8..40].try_into().unwrap()),
                DreamCooldownState {
                    epoch: u64::from_le_bytes(payload[40..48].try_into().unwrap()),
                    processed_at_ns: Some(i64::from_le_bytes(payload[48..56].try_into().unwrap())),
                },
            )
        } else if payload[..8] == DREAM_COOLDOWN_MAGIC_V1 {
            if payload.len() != 48 {
                return Err(MemoryError::CorruptRecord("invalid Dream cooldown record"));
            }
            (
                MemoryId(payload[8..40].try_into().unwrap()),
                DreamCooldownState {
                    epoch: u64::from_le_bytes(payload[40..48].try_into().unwrap()),
                    processed_at_ns: None,
                },
            )
        } else {
            return Ok(());
        };
        self.states
            .entry(id)
            .and_modify(|current| *current = merge_state(*current, state))
            .or_insert(state);
        Ok(())
    }

    pub(crate) fn state(&self, id: MemoryId) -> Option<DreamCooldownState> {
        self.states.get(&id).copied()
    }

    pub(crate) fn records(&self) -> Vec<(MemoryId, DreamCooldownState)> {
        let mut records: Vec<_> = self
            .states
            .iter()
            .map(|(id, state)| (*id, *state))
            .collect();
        records.sort_by_key(|(id, _)| id.0);
        records
    }

    pub(crate) fn validate(&self, memories: &MemoryStore) -> Result<(), MemoryError> {
        if self
            .states
            .keys()
            .all(|memory_id| memories.contains_memory(*memory_id))
        {
            Ok(())
        } else {
            Err(MemoryError::CorruptRecord(
                "Dream cooldown references a missing Memory",
            ))
        }
    }

    pub(crate) fn put(
        &mut self,
        container: &mut Container,
        id: MemoryId,
        state: DreamCooldownState,
    ) -> Result<bool, MemoryError> {
        if self
            .state(id)
            .is_some_and(|current| merge_state(current, state) == current)
        {
            return Ok(false);
        }
        let merged = self
            .state(id)
            .map(|current| merge_state(current, state))
            .unwrap_or(state);
        let Some(processed_at_ns) = merged.processed_at_ns else {
            return Err(MemoryError::InvalidField("Dream processed timestamp"));
        };
        let mut payload = Vec::with_capacity(56);
        payload.extend_from_slice(&DREAM_COOLDOWN_MAGIC_V2);
        payload.extend_from_slice(&id.0);
        payload.extend_from_slice(&merged.epoch.to_le_bytes());
        payload.extend_from_slice(&processed_at_ns.to_le_bytes());
        container.append(&payload)?;
        self.states.insert(id, merged);
        Ok(true)
    }
}

pub(crate) fn merge_state(
    left: DreamCooldownState,
    right: DreamCooldownState,
) -> DreamCooldownState {
    DreamCooldownState {
        epoch: left.epoch.max(right.epoch),
        processed_at_ns: match (left.processed_at_ns, right.processed_at_ns) {
            (Some(left), Some(right)) => Some(left.max(right)),
            (Some(value), None) | (None, Some(value)) => Some(value),
            (None, None) => None,
        },
    }
}

fn dream_cadence() -> ProcessingCadence {
    ProcessingCadence::new(DEFAULT_DREAM_REPROCESS_COOLDOWN_NS)
        .expect("Dream reprocess cadence must be positive")
}

pub(crate) fn dream_epoch(source_time_ns: i64, now_ns: i64) -> u64 {
    processing_epoch(source_time_ns, now_ns, dream_cadence())
}

pub(crate) fn eligible_dream_epoch(
    memory: &Memory,
    source_time_ns: Option<i64>,
    last_processed: Option<DreamCooldownState>,
    now_ns: i64,
) -> Option<u64> {
    if memory.archived {
        return None;
    }
    if memory.lifecycle_state == "extracted" {
        return Some(
            source_time_ns
                .map(|source_time| dream_epoch(source_time, now_ns))
                .unwrap_or(0),
        );
    }

    if let Some(source_time_ns) = source_time_ns {
        let satisfied_epoch = last_processed
            .map(|state| state.epoch)
            .unwrap_or_else(|| dream_epoch(source_time_ns, memory.updated_at_ns));
        return epoch_has_advanced(source_time_ns, satisfied_epoch, now_ns, dream_cadence());
    }

    let last_processed_ns = last_processed
        .and_then(|state| state.processed_at_ns)
        .unwrap_or(memory.updated_at_ns);
    cadence_elapsed(last_processed_ns, now_ns, dream_cadence()).then_some(0)
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
        Ok(eligible_dream_epoch(
            &memory,
            source_time,
            self.dream_cooldowns.state(id),
            now_ns,
        ))
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
        self.dream_cooldowns.put(
            &mut self.container,
            id,
            DreamCooldownState {
                epoch,
                processed_at_ns: Some(processed_at_ns),
            },
        )
    }

    pub(crate) fn dream_cooldown_records(&self) -> Vec<(MemoryId, DreamCooldownState)> {
        self.dream_cooldowns.records()
    }

    pub(crate) fn dream_was_processed(&self, id: MemoryId) -> bool {
        self.dream_cooldowns
            .state(id)
            .is_some_and(|state| state.processed_at_ns.is_some())
    }
}

impl Phylactery {
    pub(crate) fn dream_eligible_epoch(
        &mut self,
        id: MemoryId,
        now_ns: i64,
    ) -> Result<Option<u64>, MemoryError> {
        let memory = self.memory(id)?;
        Ok(eligible_dream_epoch(
            &memory,
            memory_source_timestamp_ns(&memory),
            self.dream_cooldowns.state(id),
            now_ns,
        ))
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
        self.dream_cooldowns.put(
            &mut self.container,
            id,
            DreamCooldownState {
                epoch,
                processed_at_ns: Some(processed_at_ns),
            },
        )
    }

    pub(crate) fn dream_was_processed(&self, id: MemoryId) -> bool {
        self.dream_cooldowns
            .state(id)
            .is_some_and(|state| state.processed_at_ns.is_some())
    }

    pub(crate) fn dream_cooldown_records(&self) -> Vec<(MemoryId, DreamCooldownState)> {
        self.dream_cooldowns.records()
    }
}
