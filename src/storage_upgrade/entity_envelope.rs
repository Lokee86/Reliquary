//! Entity-owned offline publication-envelope conversion at accepted link slots.
use super::StorageUpgradeError;
use crate::container::publication_envelope::{self, PublicationClock, PublicationEnvelope};
use crate::entity_compact_codec::publication::{self, EntityPublication};
use crate::{Container, EntityError, ObjectRef};
use std::collections::{BTreeMap, HashSet};

#[derive(Default)]
pub(super) struct EntityEnvelopePlan {
    pub(super) markers: BTreeMap<ObjectRef, Vec<u8>>,
    pub(super) carriers: HashSet<ObjectRef>,
}

pub(super) fn prepare(source: &mut Container) -> Result<EntityEnvelopePlan, StorageUpgradeError> {
    // This owner scan validates actual accepted V1 history before any output.
    let records = super::entity::prepare(source)?;
    let mut plan = EntityEnvelopePlan::default();
    for object in source.chunks()? {
        let payload = source.read(object)?;
        let Some(version) = crate::entity_codec::decode_version(&payload)? else {
            continue;
        };
        let publication = if let Some(record) = records.get(&version.record) {
            if record.starts_with(b"CVAENC") {
                EntityPublication::Creation(crate::entity_compact_codec::decode_creation(record)?)
            } else {
                EntityPublication::Delta(crate::entity_compact_codec::decode_delta(record)?)
            }
        } else {
            let carrier = source.read(version.record)?;
            if let Some(terminal) = crate::entity_codec::decode_direct_tombstone(&carrier)? {
                EntityPublication::DirectTombstone {
                    id: terminal.id,
                    revision: terminal.revision,
                    entity_version: version.entity_version,
                }
            } else {
                let terminal = crate::entity_codec::decode_tombstone(&carrier)?
                    .ok_or(EntityError::InvalidVersion)?;
                EntityPublication::MergeTombstone {
                    id: terminal.id,
                    revision: terminal.revision,
                    entity_version: version.entity_version,
                    replacement_id: terminal.replacement_id,
                }
            }
        };
        if !plan.carriers.insert(version.record) {
            return Err(StorageUpgradeError::Migration(
                "Entity carrier has repeated publication markers".into(),
            ));
        }
        let body = publication::encode(&publication)?;
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
