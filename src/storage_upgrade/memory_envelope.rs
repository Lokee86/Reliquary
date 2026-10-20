//! Memory-owned offline envelope transport, prepared from admitted V1 facts.
use super::StorageUpgradeError;
use crate::container::publication_envelope::{self, PublicationClock, PublicationEnvelope};
use crate::memory_compact_codec::publication::{self, MemoryPublication};
use crate::memory_compact_contract::{MemoryCreation, MemoryDelta};
use crate::{Container, MemoryError, ObjectRef};
use std::collections::{BTreeMap, HashMap, HashSet};

#[derive(Default)]
pub(super) struct MemoryEnvelopePlan {
    pub(super) markers: BTreeMap<ObjectRef, Vec<u8>>,
    pub(super) carriers: HashSet<ObjectRef>,
}

pub(super) fn prepare(
    source: &mut Container,
    memories: &crate::memory_store::MemoryStore,
) -> Result<MemoryEnvelopePlan, StorageUpgradeError> {
    let mut preceding = HashMap::new();
    let mut bodies = BTreeMap::new();
    // Grouped revisions participate as predecessors, but only a direct marker
    // below authorizes an envelope. Group members stay inside their transaction.
    for record in memories.records() {
        let publication = if record.revision == 1 {
            MemoryPublication::Creation(MemoryCreation::new(record.clone())?)
        } else {
            let previous = preceding
                .get(&record.id)
                .ok_or(MemoryError::InvalidVersion)?;
            MemoryPublication::Delta(MemoryDelta::between(previous, record)?)
        };
        preceding.insert(record.id, record.clone());
        bodies.insert(
            (record.global_version, record.memory_version),
            publication::encode(&publication)?,
        );
    }
    let mut plan = MemoryEnvelopePlan::default();
    for object in source.chunks()? {
        let payload = source.read(object)?;
        let Some(version) = crate::memory_codec::decode_version(&payload)? else {
            continue;
        };
        let body =
            if let Some(body) = bodies.remove(&(version.global_version, version.memory_version)) {
                body
            } else {
                let carrier = source.read(version.record)?;
                let terminal = crate::memory_codec::decode_tombstone(&carrier)?
                    .ok_or(MemoryError::InvalidVersion)?;
                publication::encode(&MemoryPublication::Tombstone {
                    id: terminal.id,
                    revision: terminal.revision,
                    memory_version: version.memory_version,
                })?
            };
        if !plan.carriers.insert(version.record) {
            return Err(StorageUpgradeError::Migration(
                "Memory carrier has repeated publication markers".into(),
            ));
        }
        plan.markers.insert(
            object,
            publication_envelope::encode(PublicationEnvelope {
                global_version: version.global_version,
                clock: PublicationClock::BindReserved,
                body: &body,
            })?,
        );
    }
    Ok(plan)
}
