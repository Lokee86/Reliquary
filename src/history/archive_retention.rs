//! Match Maintenance manifests to Archive-owned retention proofs.
use crate::archive::history_prune::ArchiveRetentionProof;
use crate::archive::history_prune::PrunedArchiveContent;
use crate::archive_codec::ArchiveRecord;
use crate::owner_lifecycle::HistorySource;
use crate::owner_maintenance::{
    HistoryPruneManifest, HistoryRetentionDescriptor, OwnerMaintenanceStore, PrunedPayload,
    RetentionDomain, RetentionTarget,
};
use crate::{Archive, ArchiveError, ContentId};
use std::collections::{BTreeSet, HashMap, HashSet};

pub(crate) fn descriptor(
    archive: &Archive,
    source: &HistorySource<'_>,
    conversation_id: &str,
    watermark: u64,
    lengths: &HashMap<ContentId, u64>,
) -> Result<HistoryRetentionDescriptor, ArchiveError> {
    let proof = source.archive_retention_proof(archive, conversation_id, watermark, lengths)?;
    Ok(to_descriptor(proof))
}

fn to_descriptor(proof: ArchiveRetentionProof) -> HistoryRetentionDescriptor {
    let mut payloads: Vec<_> = proof
        .payloads
        .into_iter()
        .map(|(id, _)| PrunedPayload::ArchiveContent(id.0))
        .collect();
    payloads.sort();
    HistoryRetentionDescriptor {
        target: RetentionTarget {
            domain: RetentionDomain::Archive,
            identity: proof.conversation_id.into_bytes(),
        },
        tombstone_global_version: Some(proof.tombstone_global_version),
        tombstone_domain_version: Some(proof.tombstone_archive_version),
        tombstone_digest: proof.tombstone_digest,
        retained_structure_digest: proof.retained_structure_digest,
        payloads,
    }
}

pub(crate) fn validate(
    archive: &Archive,
    source: &HistorySource<'_>,
    maintenance: &OwnerMaintenanceStore,
) -> Result<(), ArchiveError> {
    let invalid = || ArchiveError::CorruptRecord("prune evidence does not match source history");
    let markers: Vec<_> = archive.pruned_content_markers().collect();
    validate_marker_bindings(&markers, maintenance)?;
    let mut all_authorized = HashSet::<ContentId>::new();
    for (digest, manifest) in maintenance.manifests() {
        let mut lengths = HashMap::new();
        let mut selected = BTreeSet::new();
        for descriptor in &manifest.descriptors {
            if descriptor.target.domain != RetentionDomain::Archive {
                continue;
            }
            let conversation =
                std::str::from_utf8(&descriptor.target.identity).map_err(|_| invalid())?;
            if !selected.insert(conversation) {
                return Err(invalid());
            }
            for payload in &descriptor.payloads {
                let PrunedPayload::ArchiveContent(raw_id) = payload else {
                    return Err(invalid());
                };
                let id = ContentId(*raw_id);
                let marker = markers
                    .iter()
                    .find(|marker| marker.id == id)
                    .copied()
                    .ok_or_else(invalid)?;
                if marker.manifest_digest != *digest
                    || lengths
                        .insert(id, marker.byte_length)
                        .is_some_and(|old| old != marker.byte_length)
                    || !all_authorized.insert(id)
                {
                    return Err(invalid());
                }
            }
        }
        for descriptor in &manifest.descriptors {
            if descriptor.target.domain != RetentionDomain::Archive {
                continue;
            }
            let conversation =
                std::str::from_utf8(&descriptor.target.identity).map_err(|_| invalid())?;
            if self_descriptor(archive, source, conversation, manifest, &lengths)? != *descriptor {
                return Err(invalid());
            }
        }
        for version in archive
            .record_versions()
            .iter()
            .filter(|version| version.global_version <= manifest.source_global_version)
        {
            let record = source.archive_publication(archive, *version)?;
            let protected = match &record {
                ArchiveRecord::Node(node) => {
                    lengths.contains_key(&node.content_id)
                        && !selected.contains(node.conversation_id.as_str())
                }
                ArchiveRecord::IngestedTurn(turn) => {
                    (lengths.contains_key(&turn.node.content_id)
                        && !selected.contains(turn.node.conversation_id.as_str()))
                        || turn
                            .attachments
                            .iter()
                            .any(|file| lengths.contains_key(&file.content_id))
                }
                ArchiveRecord::File(file) => lengths.contains_key(&file.content_id),
                _ => false,
            };
            if protected {
                return Err(invalid());
            }
        }
    }
    if all_authorized.len() != markers.len() {
        return Err(invalid());
    }
    Ok(())
}

fn validate_marker_bindings(
    markers: &[&PrunedArchiveContent],
    maintenance: &OwnerMaintenanceStore,
) -> Result<(), ArchiveError> {
    for marker in markers {
        if !maintenance
            .get(&marker.manifest_digest)
            .is_some_and(|manifest| {
                manifest.descriptors.iter().any(|descriptor| {
                    descriptor.target.domain == RetentionDomain::Archive
                        && descriptor
                            .payloads
                            .contains(&PrunedPayload::ArchiveContent(marker.id.0))
                })
            })
        {
            return Err(ArchiveError::CorruptRecord("unbound pruned content marker"));
        }
    }
    Ok(())
}

fn self_descriptor(
    archive: &Archive,
    source: &HistorySource<'_>,
    conversation: &str,
    manifest: &HistoryPruneManifest,
    lengths: &HashMap<ContentId, u64>,
) -> Result<HistoryRetentionDescriptor, ArchiveError> {
    descriptor(
        archive,
        source,
        conversation,
        manifest.source_global_version,
        lengths,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::owner_maintenance::{HistoryPruneManifest, RetentionTarget};

    #[test]
    fn marker_binding_requires_manifest_authority() {
        let descriptor = HistoryRetentionDescriptor {
            target: RetentionTarget {
                domain: RetentionDomain::Archive,
                identity: b"retired-conversation".to_vec(),
            },
            tombstone_global_version: Some(2),
            tombstone_domain_version: Some(1),
            tombstone_digest: [3; 32],
            retained_structure_digest: [4; 32],
            payloads: vec![PrunedPayload::ArchiveContent([5; 32])],
        };
        let manifest = HistoryPruneManifest {
            owner_uuid: [1; 16],
            source_global_version: 2,
            source_history_digest: [2; 32],
            descriptors: vec![descriptor],
        };
        let digest = manifest.digest().unwrap();
        let marker = PrunedArchiveContent {
            id: ContentId([5; 32]),
            byte_length: 17,
            manifest_digest: digest,
        };
        let mut maintenance = OwnerMaintenanceStore::default();
        assert!(validate_marker_bindings(&[&marker], &maintenance).is_err());
        maintenance.ingest(&manifest.encode().unwrap()).unwrap();
        assert!(validate_marker_bindings(&[&marker], &maintenance).is_ok());
    }
}
