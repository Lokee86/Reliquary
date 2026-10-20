//! Relationship-owned offline publication-envelope conversion at accepted link slots.
use super::StorageUpgradeError;
use crate::container::publication_envelope::{self, PublicationClock, PublicationEnvelope};
use crate::relationship_compact_codec::publication::{self, RelationshipPublication};
use crate::{Container, ObjectRef, RelationshipError};
use std::collections::{BTreeMap, HashSet};

#[derive(Default)]
pub(super) struct RelationshipEnvelopePlan {
    pub(super) markers: BTreeMap<ObjectRef, Vec<u8>>,
    pub(super) carriers: HashSet<ObjectRef>,
}

pub(super) fn prepare(
    source: &mut Container,
) -> Result<RelationshipEnvelopePlan, StorageUpgradeError> {
    // This owner scan validates actual accepted V1 history before any output.
    let records = super::relationship::prepare(source)?;
    let mut plan = RelationshipEnvelopePlan::default();
    for object in source.chunks()? {
        let payload = source.read(object)?;
        let Some(version) = crate::relationship_codec::decode_version(&payload)? else {
            continue;
        };
        let publication = if let Some(record) = records.get(&version.record) {
            if record.starts_with(b"CVARNC") {
                RelationshipPublication::Creation(
                    crate::relationship_compact_codec::decode_creation(record)?,
                )
            } else {
                RelationshipPublication::Delta(crate::relationship_compact_codec::decode_delta(
                    record,
                )?)
            }
        } else {
            let carrier = source.read(version.record)?;
            let terminal = crate::relationship_codec::decode_tombstone(&carrier)?
                .ok_or(RelationshipError::InvalidVersion)?;
            RelationshipPublication::Tombstone {
                id: terminal.id,
                revision: terminal.revision,
                relationship_version: version.relationship_version,
            }
        };
        if !plan.carriers.insert(version.record) {
            return Err(StorageUpgradeError::Migration(
                "Relationship carrier has repeated publication markers".into(),
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
