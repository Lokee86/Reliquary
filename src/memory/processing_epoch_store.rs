use crate::memory_store::MemoryStore;
use crate::processing_epoch_model::{ProcessingEpochState, ProcessingLaneId};
use crate::{Container, MemoryError, MemoryId};
use std::collections::{BTreeMap, HashMap};

const PROCESSING_EPOCH_MAGIC: [u8; 8] = *b"CVAPEP01";
const PROCESSING_EPOCH_RECORD_LEN: usize = 63;

#[derive(Default)]
pub(crate) struct ProcessingEpochStore {
    states: HashMap<(MemoryId, ProcessingLaneId), ProcessingEpochState>,
}

impl ProcessingEpochStore {
    pub(crate) fn ingest(&mut self, payload: &[u8]) -> Result<(), MemoryError> {
        if payload.len() < PROCESSING_EPOCH_MAGIC.len()
            || payload[..PROCESSING_EPOCH_MAGIC.len()] != PROCESSING_EPOCH_MAGIC
        {
            return Ok(());
        }
        if payload.len() != PROCESSING_EPOCH_RECORD_LEN {
            return Err(MemoryError::CorruptRecord(
                "invalid processing epoch record",
            ));
        }

        let memory_id = MemoryId(payload[8..40].try_into().unwrap());
        let lane_id = ProcessingLaneId(u16::from_le_bytes(payload[40..42].try_into().unwrap()));
        let state = ProcessingEpochState {
            satisfied_through_epoch: u64::from_le_bytes(payload[42..50].try_into().unwrap()),
            cadence_version: u32::from_le_bytes(payload[50..54].try_into().unwrap()),
            checkpoint_at_ns: decode_checkpoint(payload[54], &payload[55..63])?,
        };
        let key = (memory_id, lane_id);

        match self.states.get(&key).copied() {
            Some(current) => {
                let merged = merge_same_cadence(current, state).ok_or(
                    MemoryError::CorruptRecord("processing epoch cadence version conflict"),
                )?;
                self.states.insert(key, merged);
            }
            None => {
                self.states.insert(key, state);
            }
        }
        Ok(())
    }

    pub(crate) fn state(
        &self,
        memory_id: MemoryId,
        lane_id: ProcessingLaneId,
    ) -> Option<ProcessingEpochState> {
        self.states.get(&(memory_id, lane_id)).copied()
    }

    pub(crate) fn records(&self) -> Vec<(MemoryId, ProcessingLaneId, ProcessingEpochState)> {
        let mut records: Vec<_> = self
            .states
            .iter()
            .map(|((memory_id, lane_id), state)| (*memory_id, *lane_id, *state))
            .collect();
        records.sort_by_key(|(memory_id, lane_id, _)| (memory_id.0, lane_id.0));
        records
    }

    pub(crate) fn validate(&self, memories: &MemoryStore) -> Result<(), MemoryError> {
        if self
            .states
            .keys()
            .all(|(memory_id, _)| memories.contains_memory(*memory_id))
        {
            Ok(())
        } else {
            Err(MemoryError::CorruptRecord(
                "processing epoch references a missing Memory",
            ))
        }
    }

    pub(crate) fn put(
        &mut self,
        container: &mut Container,
        memory_id: MemoryId,
        lane_id: ProcessingLaneId,
        state: ProcessingEpochState,
    ) -> Result<bool, MemoryError> {
        let key = (memory_id, lane_id);
        let merged = match self.states.get(&key).copied() {
            Some(current) => merge_same_cadence(current, state).ok_or(
                MemoryError::InvalidField("Processing epoch cadence version"),
            )?,
            None => state,
        };
        if self
            .states
            .get(&key)
            .is_some_and(|current| *current == merged)
        {
            return Ok(false);
        }

        container.append(&encode_record(memory_id, lane_id, merged))?;
        self.states.insert(key, merged);
        Ok(true)
    }
}

pub(crate) fn merge_processing_epoch_records(
    records: impl IntoIterator<Item = (MemoryId, ProcessingLaneId, ProcessingEpochState)>,
) -> Result<Vec<(MemoryId, ProcessingLaneId, ProcessingEpochState)>, MemoryError> {
    let mut merged = BTreeMap::new();
    for (memory_id, lane_id, state) in records {
        let key = (memory_id.0, lane_id.0);
        match merged.get(&key).copied() {
            Some((_, _, current)) => {
                let state = merge_same_cadence(current, state).ok_or(MemoryError::InvalidField(
                    "Processing epoch cadence version",
                ))?;
                merged.insert(key, (memory_id, lane_id, state));
            }
            None => {
                merged.insert(key, (memory_id, lane_id, state));
            }
        }
    }
    Ok(merged.into_values().collect())
}

pub(crate) fn merge_same_cadence(
    left: ProcessingEpochState,
    right: ProcessingEpochState,
) -> Option<ProcessingEpochState> {
    if left.cadence_version != right.cadence_version {
        return None;
    }
    Some(ProcessingEpochState {
        satisfied_through_epoch: left
            .satisfied_through_epoch
            .max(right.satisfied_through_epoch),
        cadence_version: left.cadence_version,
        checkpoint_at_ns: match (left.checkpoint_at_ns, right.checkpoint_at_ns) {
            (Some(left), Some(right)) => Some(left.max(right)),
            (Some(value), None) | (None, Some(value)) => Some(value),
            (None, None) => None,
        },
    })
}

fn encode_record(
    memory_id: MemoryId,
    lane_id: ProcessingLaneId,
    state: ProcessingEpochState,
) -> [u8; PROCESSING_EPOCH_RECORD_LEN] {
    let mut payload = [0_u8; PROCESSING_EPOCH_RECORD_LEN];
    payload[..8].copy_from_slice(&PROCESSING_EPOCH_MAGIC);
    payload[8..40].copy_from_slice(&memory_id.0);
    payload[40..42].copy_from_slice(&lane_id.0.to_le_bytes());
    payload[42..50].copy_from_slice(&state.satisfied_through_epoch.to_le_bytes());
    payload[50..54].copy_from_slice(&state.cadence_version.to_le_bytes());
    if let Some(checkpoint_at_ns) = state.checkpoint_at_ns {
        payload[54] = 1;
        payload[55..63].copy_from_slice(&checkpoint_at_ns.to_le_bytes());
    }
    payload
}

fn decode_checkpoint(tag: u8, bytes: &[u8]) -> Result<Option<i64>, MemoryError> {
    match tag {
        0 if bytes.iter().all(|byte| *byte == 0) => Ok(None),
        1 => Ok(Some(i64::from_le_bytes(bytes.try_into().unwrap()))),
        _ => Err(MemoryError::CorruptRecord(
            "invalid processing epoch checkpoint",
        )),
    }
}
