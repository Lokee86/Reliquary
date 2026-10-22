//! Existing history-proof composition above independently owned stores.
use sha2::{Digest, Sha256};

/// Fingerprint semantic publications through a frozen global watermark while
/// normalizing relocatable references. Unpublished payload chunks do not enter
/// this digest; domain descriptors bind any payload selected for removal.
pub(crate) fn owner_history_digest(
    container: &mut crate::Container,
    watermark: u64,
    graph: &crate::graph_store::GraphStore,
) -> Result<[u8; 32], String> {
    if watermark > container.latest_version() {
        return Err("history watermark exceeds owner clock".into());
    }
    let mut hash = Sha256::new();
    hash.update(watermark.to_le_bytes());
    for global in 1..=watermark {
        hash.update(global.to_le_bytes());
        match container.transaction_time_ns(global) {
            Some(time) => {
                hash.update([1]);
                hash.update(time.to_le_bytes());
            }
            None => hash.update([0]),
        }
    }
    let objects = container.chunks().map_err(|error| error.to_string())?;
    let normalized = objects
        .iter()
        .map(|object| (*object, crate::ObjectRef::from_legacy_bytes([0; 16])))
        .collect::<std::collections::BTreeMap<_, _>>();
    let mut archive_applying = std::collections::HashMap::<u64, [u8; 32]>::new();
    for object in objects {
        let raw = container.read(object).map_err(|error| error.to_string())?;
        if let Some(envelope) = crate::container::publication_envelope::decode(&raw)
            .map_err(|error| error.to_string())?
        {
            use crate::archive_codec::publication::{self, ArchivePublication};
            if let Some(publication) = publication::decode(envelope.body, envelope.global_version)
                .map_err(|error| error.to_string())?
            {
                if envelope.global_version <= watermark {
                    let local = publication.archive_version();
                    let digest = match publication {
                        ArchivePublication::Record { record, .. } => {
                            let digest = Sha256::digest(
                                publication::comparison_carrier(&record)
                                    .map_err(|error| error.to_string())?,
                            )
                            .into();
                            archive_applying.insert(local, digest);
                            digest
                        }
                        ArchivePublication::Repeat {
                            first_applying_archive_version,
                            ..
                        } => *archive_applying
                            .get(&first_applying_archive_version)
                            .ok_or("Archive repeat proof anchor is not applying history")?,
                    };
                    let marker = crate::archive_history_codec::encode_record_version(
                        crate::ArchiveRecordVersion {
                            global_version: envelope.global_version,
                            archive_version: local,
                            record: crate::ObjectRef::from_legacy_bytes([0; 16]),
                        },
                    );
                    digest_bytes(&mut hash, &marker);
                    digest_bytes(&mut hash, &digest);
                }
                continue;
            }
        }
        if let Some((global, marker, carrier)) =
            crate::memory_compact_codec::publication::history_evidence(&raw)
                .map_err(|error| error.to_string())?
        {
            if global <= watermark {
                digest_bytes(&mut hash, &marker);
                let digest: [u8; 32] = Sha256::digest(&carrier).into();
                digest_bytes(&mut hash, &digest);
            }
            continue;
        }
        if let Some((global, marker, digest)) =
            crate::entity_compact_codec::publication::history_evidence(&raw)
                .map_err(|error| error.to_string())?
        {
            if global <= watermark {
                digest_bytes(&mut hash, &marker);
                digest_bytes(&mut hash, &digest);
            }
            continue;
        }
        if let Some((global, marker, digest)) =
            crate::relationship_compact_codec::publication::history_evidence(&raw)
                .map_err(|error| error.to_string())?
        {
            if global <= watermark {
                digest_bytes(&mut hash, &marker);
                digest_bytes(&mut hash, &digest);
            }
            continue;
        }
        if let Some(envelope) = crate::container::publication_envelope::decode(&raw)
            .map_err(|error| error.to_string())?
            && let Some(publication) = crate::graph_compact_codec::publication::decode(
                envelope.body,
                envelope.global_version,
            )
            .map_err(|error| error.to_string())?
        {
            if envelope.global_version <= watermark {
                let marker =
                    crate::graph_codec::encode_version(crate::graph_codec::GraphVersionPayload {
                        global_version: envelope.global_version,
                        graph_version: publication.graph_version,
                        mutation: crate::ObjectRef::from_legacy_bytes([0; 16]),
                    });
                digest_bytes(&mut hash, &marker);
                let digest = graph
                    .compact_carrier_digest(&raw)
                    .map_err(|error| error.to_string())?
                    .ok_or("missing native Graph proof evidence")?;
                digest_bytes(&mut hash, &digest);
            }
            continue;
        }
        let publication = crate::vector_generation_codec::decode_version(&raw)
            .map_err(|error| error.to_string())?
            .map(|version| (version.global_version, version.record));
        if let Some((global, record)) = publication {
            if global <= watermark {
                let normalized = crate::storage_reclamation::relocate_payload(&raw, &normalized)?;
                digest_bytes(&mut hash, &normalized);
                let bytes = container.read(record).map_err(|error| error.to_string())?;
                let digest: [u8; 32] = if let Some(digest) = graph
                    .compact_carrier_digest(&bytes)
                    .map_err(|error| error.to_string())?
                {
                    digest
                } else if let Some((_, _, proof)) =
                    crate::entity_description_proof::compact_description_proof(&bytes)
                        .map_err(|error| error.to_string())?
                {
                    proof.original_carrier_digest
                } else if let Some((_, _, proof)) =
                    crate::relationship_description_proof::compact_description_proof(&bytes)
                        .map_err(|error| error.to_string())?
                {
                    proof.original_carrier_digest
                } else {
                    Sha256::digest(&bytes).into()
                };

                digest_bytes(&mut hash, &digest);
            }
            continue;
        }
        let range = crate::insomnia::completion::decode_embedded_version_range(&raw)
            .map_err(str::to_owned)?
            .or(
                crate::external_source::publication::decode_embedded_version_range(&raw)
                    .map_err(|error| error.to_string())?
                    .map(|(start, count, time)| (start, count, Some(time))),
            );
        if let Some((start, count, _)) = range
            && count > 0
            && start
                .checked_add(u64::from(count) - 1)
                .is_some_and(|last| last <= watermark)
        {
            digest_bytes(&mut hash, &raw);
        }
    }
    Ok(hash.finalize().into())
}

fn digest_bytes(hash: &mut Sha256, bytes: &[u8]) {
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
}

#[cfg(test)]
#[path = "graph_compact_proof_tests.rs"]
mod graph_compact_proof_tests;

pub(crate) fn validate_source_history(
    maintenance: &crate::owner_maintenance::OwnerMaintenanceStore,
    container: &mut crate::Container,
    graph: &crate::graph_store::GraphStore,
) -> Result<(), String> {
    for manifest in maintenance.manifests().values() {
        if Some(manifest.owner_uuid) != container.owner_uuid()
            || manifest.source_global_version > container.latest_version()
            || owner_history_digest(container, manifest.source_global_version, graph)?
                != manifest.source_history_digest
        {
            return Err("maintenance proof does not match its source owner history".into());
        }
    }
    Ok(())
}
