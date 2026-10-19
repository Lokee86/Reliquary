//! Memory-owned transformed transport. Other owners retain their payloads;
//! physical reference relocation uses the existing container-copy seam.
use super::StorageUpgradeError;
use crate::{Container, MemoryError, ObjectRef};
use std::collections::{BTreeMap, HashMap};

pub(super) fn transform(
    source: &mut Container,
    destination: &mut Container,
) -> Result<(), StorageUpgradeError> {
    let normalizations = if source
        .identity()
        .is_some_and(|identity| identity.file_kind == crate::FileKind::Reliquary)
    {
        let mut owner = crate::Cva::open_v1_read_only(source.path())?;
        owner.prepare_import_settlement_normalizations()?
    } else {
        Vec::new()
    };
    let mut state = crate::memory_rebuild::MemoryOpenState::new();
    let mut publications = BTreeMap::new();
    let mut imports = crate::imports::ImportEpochStore::default();
    // A separate scan observes the exact physical version frontier. Carrier
    // coordinates come from accepted markers, never from unversioned records.
    let mut scanned = Container::open_scanned_version(
        source.path(),
        Some(crate::FormatVersion { major: 1, minor: 0 }),
        |object, bytes, head| -> Result<(), StorageUpgradeError> {
            state.ingest(object, bytes, head)?;
            imports.ingest_at(Some(object), bytes)?;
            if let Some(version) = crate::memory_codec::decode_version(bytes)? {
                publications.insert(
                    version.record,
                    (version.global_version, version.memory_version),
                );
            }
            Ok(())
        },
    )?;
    imports.validate_legacy_frames(&mut scanned)?;
    let memories = state.finish(&mut scanned)?;
    let mut preceding = HashMap::new();
    let mut encoded = BTreeMap::new();
    for record in memories.records() {
        let bytes = if record.revision == 1 {
            crate::memory_compact_codec::encode_creation(
                &crate::memory_compact_contract::MemoryCreation::new(record.clone())?,
            )?
        } else {
            let previous = preceding
                .get(&record.id)
                .ok_or(MemoryError::InvalidVersion)?;
            crate::memory_compact_codec::encode_delta(
                &crate::memory_compact_contract::MemoryDelta::between(previous, record)?,
            )?
        };
        preceding.insert(record.id, record.clone());
        encoded.insert((record.global_version, record.memory_version), bytes);
    }
    let mut relocated = BTreeMap::<ObjectRef, ObjectRef>::new();
    source.visit_payloads(|object, payload| -> Result<(), StorageUpgradeError> {
        let transformed = if crate::imports::legacy_epoch::decode(payload)?.is_some() {
            let length = destination.backing_file().metadata()?.len();
            let hex = crate::imports::hash_file_prefix(destination.path(), length)?;
            let mut digest = [0; 32];
            for (index, value) in digest.iter_mut().enumerate() {
                *value = u8::from_str_radix(&hex[index * 2..index * 2 + 2], 16)
                    .map_err(|_| StorageUpgradeError::Migration("invalid prefix digest".into()))?;
            }
            crate::imports::legacy_epoch_transport::encode(
                payload,
                &crate::imports::legacy_epoch_transport::PrefixBinding { length, digest },
            )?
        } else if crate::memory_codec::decode_record(payload)?.is_some() {
            publications
                .get(&object)
                .and_then(|coordinates| encoded.get(coordinates))
                .cloned()
                // Interrupted, unpublished carriers remain inert.
                .unwrap_or_else(|| payload.to_vec())
        } else if let Some(completion) = crate::insomnia::completion::decode_completion(payload)
            .map_err(MemoryError::CorruptRecord)?
        {
            crate::insomnia::completion::encode_legacy_compact(payload, &completion)
                .map_err(MemoryError::CorruptRecord)?
        } else if let Some(publication) = crate::external_source::publication::decode(payload)? {
            crate::external_source::publication::encode(&publication)?
        } else {
            // Includes tombstones, their original markers, ticket frames and
            // transaction times. No new semantic publication is allocated.
            crate::storage_reclamation::relocate_payload(payload, &relocated)
                .map_err(StorageUpgradeError::Migration)?
        };
        relocated.insert(object, destination.append(&transformed)?);
        Ok(())
    })?;
    for payload in normalizations {
        destination.append(&payload)?;
    }
    Ok(())
}
