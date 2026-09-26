use crate::{Container, MemoryError, MemoryId};
use std::collections::HashMap;

const DREAM_COOLDOWN_MAGIC_V1: [u8; 8] = *b"CVADREM1";
const DREAM_COOLDOWN_MAGIC_V2: [u8; 8] = *b"CVADREM2";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct LegacyDreamCooldownState {
    pub(super) epoch: u64,
    pub(super) processed_at_ns: Option<i64>,
}

#[derive(Default)]
struct LegacyDreamCooldownStore {
    states: HashMap<MemoryId, LegacyDreamCooldownState>,
}

impl LegacyDreamCooldownStore {
    fn ingest(&mut self, payload: &[u8]) -> Result<(), MemoryError> {
        if payload.len() < 8 {
            return Ok(());
        }
        let (id, state) = if payload[..8] == DREAM_COOLDOWN_MAGIC_V2 {
            if payload.len() != 56 {
                return Err(MemoryError::CorruptRecord("invalid Dream cooldown record"));
            }
            (
                MemoryId(payload[8..40].try_into().unwrap()),
                LegacyDreamCooldownState {
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
                LegacyDreamCooldownState {
                    epoch: u64::from_le_bytes(payload[40..48].try_into().unwrap()),
                    processed_at_ns: None,
                },
            )
        } else {
            return Ok(());
        };
        self.states
            .entry(id)
            .and_modify(|current| *current = merge(*current, state))
            .or_insert(state);
        Ok(())
    }

    fn records(&self) -> Vec<(MemoryId, LegacyDreamCooldownState)> {
        let mut records: Vec<_> = self
            .states
            .iter()
            .map(|(id, state)| (*id, *state))
            .collect();
        records.sort_by_key(|(id, _)| id.0);
        records
    }
}

pub(super) fn legacy_dream_cooldown_records(
    container: &mut Container,
) -> Result<Vec<(MemoryId, LegacyDreamCooldownState)>, MemoryError> {
    let mut store = LegacyDreamCooldownStore::default();
    for chunk in container.chunks()? {
        let payload = container.read(chunk)?;
        store.ingest(&payload)?;
    }
    Ok(store.records())
}

fn merge(
    left: LegacyDreamCooldownState,
    right: LegacyDreamCooldownState,
) -> LegacyDreamCooldownState {
    LegacyDreamCooldownState {
        epoch: left.epoch.max(right.epoch),
        processed_at_ns: match (left.processed_at_ns, right.processed_at_ns) {
            (Some(left), Some(right)) => Some(left.max(right)),
            (Some(value), None) | (None, Some(value)) => Some(value),
            (None, None) => None,
        },
    }
}
