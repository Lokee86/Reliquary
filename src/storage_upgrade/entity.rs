//! Entity-owned transport preparation for a V1 storage upgrade.
//!
//! This scan is read-only. It returns replacement payloads keyed by the exact
//! source carrier references accepted by Entity version markers; callers can
//! use the existing relocation pass for every other payload and marker.
use super::StorageUpgradeError;
use crate::entity_codec::decode_version;
use crate::entity_compact_codec::{encode_creation, encode_delta};
use crate::entity_compact_contract::{EntityCreation, EntityDelta};
use crate::entity_model::EntityRecord;
use crate::{Container, ObjectRef};
use std::collections::{BTreeMap, HashMap};

pub(super) fn prepare(
    source: &Container,
) -> Result<BTreeMap<ObjectRef, Vec<u8>>, StorageUpgradeError> {
    let mut state = crate::entity_rebuild::EntityOpenState::new();
    let mut publications = BTreeMap::new();
    let scanned = Container::open_scanned_version(
        source.path(),
        Some(crate::FormatVersion { major: 1, minor: 0 }),
        |object, bytes, head| -> Result<(), StorageUpgradeError> {
            state.ingest(object, bytes, head)?;
            if let Some(version) = decode_version(bytes)? {
                if publications
                    .insert(
                        (version.global_version, version.entity_version),
                        version.record,
                    )
                    .is_some()
                {
                    return Err(StorageUpgradeError::Migration(
                        "duplicate Entity publication coordinates".into(),
                    ));
                }
            }
            Ok(())
        },
    )?;
    let entities = state.finish(&scanned)?;

    let mut preceding = HashMap::<crate::EntityId, EntityRecord>::new();
    let mut encoded = BTreeMap::new();
    for record in entities.records() {
        let carrier = publications
            .get(&(record.global_version, record.entity_version))
            .copied()
            .ok_or_else(|| {
                StorageUpgradeError::Migration(
                    "accepted Entity revision lacks its source publication marker".into(),
                )
            })?;
        let bytes = if record.revision == 1 {
            encode_creation(&EntityCreation::new(record.clone())?)?
        } else {
            let previous = preceding.get(&record.id).ok_or_else(|| {
                StorageUpgradeError::Migration(
                    "accepted Entity revision lacks its predecessor".into(),
                )
            })?;
            encode_delta(&EntityDelta::between(previous, record)?)?
        };
        preceding.insert(record.id, record.clone());
        encoded.insert(carrier, bytes);
    }
    Ok(encoded)
}
