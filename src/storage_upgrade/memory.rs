//! Memory-owned transformed transport. Other owners retain their payloads;
//! physical reference relocation uses the existing container-copy seam.
use super::StorageUpgradeError;
use crate::{Container, MemoryError, ObjectRef};
use std::collections::{BTreeMap, HashMap};

pub(super) fn transform(
    source: &mut Container,
    destination: &mut Container,
) -> Result<(), StorageUpgradeError> {
    transform_owners(
        source,
        destination,
        false,
        false,
        false,
        false,
        false,
        false,
        false,
        false,
        false,
    )
}

pub(super) fn transform_with_entities(
    source: &mut Container,
    destination: &mut Container,
) -> Result<(), StorageUpgradeError> {
    transform_owners(
        source,
        destination,
        true,
        false,
        false,
        false,
        false,
        false,
        false,
        false,
        false,
    )
}

pub(super) fn transform_with_relationships(
    source: &mut Container,
    destination: &mut Container,
) -> Result<(), StorageUpgradeError> {
    transform_owners(
        source,
        destination,
        true,
        true,
        false,
        false,
        false,
        false,
        false,
        false,
        false,
    )
}

pub(super) fn transform_with_ego(
    source: &mut Container,
    destination: &mut Container,
) -> Result<(), StorageUpgradeError> {
    transform_owners(
        source,
        destination,
        true,
        true,
        true,
        false,
        false,
        false,
        false,
        false,
        false,
    )
}

pub(super) fn transform_with_graph(
    source: &mut Container,
    destination: &mut Container,
) -> Result<(), StorageUpgradeError> {
    transform_owners(
        source,
        destination,
        true,
        true,
        true,
        true,
        false,
        false,
        false,
        false,
        false,
    )
}

pub(super) fn transform_with_memory_envelopes(
    source: &mut Container,
    destination: &mut Container,
) -> Result<(), StorageUpgradeError> {
    transform_owners(
        source,
        destination,
        true,
        true,
        true,
        true,
        true,
        false,
        false,
        false,
        false,
    )
}

pub(super) fn transform_with_entity_envelopes(
    source: &mut Container,
    destination: &mut Container,
) -> Result<(), StorageUpgradeError> {
    transform_owners(
        source,
        destination,
        true,
        true,
        true,
        true,
        true,
        true,
        false,
        false,
        false,
    )
}

pub(super) fn transform_with_relationship_envelopes(
    source: &mut Container,
    destination: &mut Container,
) -> Result<(), StorageUpgradeError> {
    transform_owners(
        source,
        destination,
        true,
        true,
        true,
        true,
        true,
        true,
        true,
        false,
        false,
    )
}

pub(super) fn transform_with_archive_envelopes(
    source: &mut Container,
    destination: &mut Container,
) -> Result<(), StorageUpgradeError> {
    transform_owners(
        source,
        destination,
        true,
        true,
        true,
        true,
        true,
        true,
        true,
        true,
        false,
    )
}

pub(super) fn transform_with_graph_envelopes(
    source: &mut Container,
    destination: &mut Container,
) -> Result<(), StorageUpgradeError> {
    transform_owners(
        source,
        destination,
        true,
        true,
        true,
        true,
        true,
        true,
        true,
        true,
        true,
    )
}

fn transform_owners(
    source: &mut Container,
    destination: &mut Container,
    migrate_entities: bool,
    migrate_relationships: bool,
    migrate_ego: bool,
    migrate_graph: bool,
    migrate_memory_envelopes: bool,
    migrate_entity_envelopes: bool,
    migrate_relationship_envelopes: bool,
    migrate_archive_envelopes: bool,
    migrate_graph_envelopes: bool,
) -> Result<(), StorageUpgradeError> {
    let (normalizations, archive_envelope_plan) = if source
        .identity()
        .is_some_and(|identity| identity.file_kind == crate::FileKind::Reliquary)
    {
        let mut owner = crate::Cva::open_v1_read_only(source.path())?;
        let normalizations = owner.prepare_import_settlement_normalizations()?;
        let archive = if migrate_archive_envelopes {
            super::archive_envelope::prepare(source, &owner.archive)?
        } else {
            super::archive_envelope::ArchiveEnvelopePlan::default()
        };
        (normalizations, archive)
    } else {
        if migrate_archive_envelopes {
            for object in source.chunks()? {
                if crate::archive_history_codec::decode_record_version(&source.read(object)?)?
                    .is_some()
                {
                    return Err(StorageUpgradeError::Migration(
                        "Archive publication in PHY input".into(),
                    ));
                }
            }
        }
        (
            Vec::new(),
            super::archive_envelope::ArchiveEnvelopePlan::default(),
        )
    };
    let graph_envelope_plan = if migrate_graph_envelopes {
        super::graph_envelope::prepare(source)?
    } else {
        super::graph_envelope::GraphEnvelopePlan::default()
    };
    let dream_begins = if migrate_graph_envelopes {
        super::dream_begin::prepare(source)?
    } else {
        BTreeMap::new()
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
            // This proof authenticates the original carrier hashes. Copying it
            // while rewriting those carriers would return an invalid owner.
            // Do not invent a replacement proof or another runtime bridge.
            if bytes.starts_with(b"CVAHPR01") {
                return Err(StorageUpgradeError::UnsupportedAuthority("CVAHPR01"));
            }
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
    let ego_carriers = if migrate_ego {
        super::ego::prepare(source, memories.memory_version())?
    } else {
        BTreeMap::new()
    };
    let entity_carriers = if migrate_entities && !migrate_entity_envelopes {
        super::entity::prepare(source)?
    } else {
        BTreeMap::new()
    };
    let relationship_carriers = if migrate_relationships && !migrate_relationship_envelopes {
        super::relationship::prepare(source)?
    } else {
        BTreeMap::new()
    };
    let graph_carriers = if migrate_graph && !migrate_graph_envelopes {
        super::graph::prepare(source)?
    } else {
        BTreeMap::new()
    };
    let envelope_plan = if migrate_memory_envelopes {
        super::memory_envelope::prepare(source, &memories)?
    } else {
        super::memory_envelope::MemoryEnvelopePlan::default()
    };
    let entity_envelope_plan = if migrate_entity_envelopes {
        super::entity_envelope::prepare(source)?
    } else {
        super::entity_envelope::EntityEnvelopePlan::default()
    };
    let relationship_envelope_plan = if migrate_relationship_envelopes {
        super::relationship_envelope::prepare(source)?
    } else {
        super::relationship_envelope::RelationshipEnvelopePlan::default()
    };
    let mut preceding = HashMap::new();
    let mut encoded = BTreeMap::new();
    for record in memories
        .records()
        .iter()
        .filter(|_| !migrate_memory_envelopes)
    {
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
    let mut dream_pairs = crate::dream_pair_history::DreamPairStore::default();
    let mut interaction_streams =
        crate::interaction_stream_store::InteractionStreamStore::default();
    source.visit_payloads(|object, payload| -> Result<(), StorageUpgradeError> {
        if migrate_graph_envelopes {
            if let Some(record) = crate::interaction_stream_codec::decode(payload)? {
                interaction_streams.put(destination, record)?;
                return Ok(());
            }
            if let Some((left, right)) =
                crate::dream_pair_history::DreamPairStore::decode_legacy_pair(payload)?
            {
                // Pair membership is clock-neutral and has no inbound physical
                // references. Emit local mappings before its original pair slot.
                dream_pairs.put(destination, left, right)?;
                return Ok(());
            }
        }
        // Only accepted direct carriers are omitted. Orphans remain inert,
        // and each replacement stays at the original publication marker slot.
        if envelope_plan.carriers.contains(&object)
            || entity_envelope_plan.carriers.contains(&object)
            || relationship_envelope_plan.carriers.contains(&object)
            || archive_envelope_plan.carriers.contains(&object)
            || graph_envelope_plan.carriers.contains(&object)
        {
            return Ok(());
        }
        let transformed = if let Some(begin) = dream_begins.get(&object) {
            begin.clone()
        } else if let Some(envelope) = envelope_plan.markers.get(&object) {
            envelope.clone()
        } else if let Some(envelope) = entity_envelope_plan.markers.get(&object) {
            envelope.clone()
        } else if let Some(envelope) = relationship_envelope_plan.markers.get(&object) {
            envelope.clone()
        } else if let Some(envelope) = archive_envelope_plan.markers.get(&object) {
            envelope.clone()
        } else if let Some(envelope) = graph_envelope_plan.markers.get(&object) {
            envelope.clone()
        } else if let Some(entity) = entity_carriers.get(&object) {
            entity.clone()
        } else if let Some(relationship) = relationship_carriers.get(&object) {
            relationship.clone()
        } else if let Some(ego) = ego_carriers.get(&object) {
            ego.clone()
        } else if let Some(graph) = graph_carriers.get(&object) {
            graph.clone()
        } else if crate::imports::legacy_epoch::decode(payload)?.is_some() {
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
