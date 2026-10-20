//! Relationship-owned transport preparation for a V1 storage upgrade.
//!
//! This scan is read-only. It returns replacement payloads keyed by the exact
//! source carrier references accepted by Relationship version markers; callers can
//! use the existing relocation pass for every other payload and marker.
use super::StorageUpgradeError;
use crate::relationship_codec::decode_version;
use crate::relationship_compact_codec::{encode_creation, encode_delta};
use crate::relationship_compact_contract::{RelationshipCreation, RelationshipDelta};
use crate::relationship_model::RelationshipRecord;
use crate::{Container, ObjectRef};
use std::collections::{BTreeMap, HashMap};

pub(super) fn prepare(
    source: &Container,
) -> Result<BTreeMap<ObjectRef, Vec<u8>>, StorageUpgradeError> {
    let mut state = crate::relationship_rebuild::RelationshipOpenState::new();
    let mut publications = BTreeMap::new();
    let scanned = Container::open_scanned_version(
        source.path(),
        Some(crate::FormatVersion { major: 1, minor: 0 }),
        |object, bytes, head| -> Result<(), StorageUpgradeError> {
            state.ingest(object, bytes, head)?;
            if let Some(version) = decode_version(bytes)? {
                if publications
                    .insert(
                        (version.global_version, version.relationship_version),
                        version.record,
                    )
                    .is_some()
                {
                    return Err(StorageUpgradeError::Migration(
                        "duplicate Relationship publication coordinates".into(),
                    ));
                }
            }
            Ok(())
        },
    )?;
    let relationships = state.finish(&scanned)?;

    let mut preceding = HashMap::<crate::RelationshipId, RelationshipRecord>::new();
    let mut encoded = BTreeMap::new();
    for record in relationships.records() {
        let carrier = publications
            .get(&(record.global_version, record.relationship_version))
            .copied()
            .ok_or_else(|| {
                StorageUpgradeError::Migration(
                    "accepted Relationship revision lacks its source publication marker".into(),
                )
            })?;
        let bytes = if record.revision == 1 {
            encode_creation(&RelationshipCreation::new(record.clone())?)?
        } else {
            let previous = preceding.get(&record.id).ok_or_else(|| {
                StorageUpgradeError::Migration(
                    "accepted Relationship revision lacks its predecessor".into(),
                )
            })?;
            encode_delta(&RelationshipDelta::between(previous, record)?)?
        };
        preceding.insert(record.id, record.clone());
        encoded.insert(carrier, bytes);
    }
    Ok(encoded)
}
