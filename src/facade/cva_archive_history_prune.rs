//! Multi-owner retention proof for Archive payload markers.
use crate::owner_maintenance::{
    HistoryRetentionDescriptor, PrunedPayload, RetentionDomain, RetentionTarget,
};
use crate::{Archive, ArchiveError, ContentId};
use reliquary_archive::archive_codec::{ArchiveRecord, encode_conversation_tombstone};
use reliquary_container::Container;
use sha2::{Digest, Sha256};
use std::collections::HashMap;

pub(crate) fn archive_retention_descriptor(
    archive: &Archive,
    container: &mut Container,
    conversation_id: &str,
    watermark: u64,
    lengths: &HashMap<ContentId, u64>,
) -> Result<HistoryRetentionDescriptor, ArchiveError> {
    let tombstone = archive
        .conversation_tombstone(conversation_id)
        .filter(|value| value.global_version <= watermark)
        .ok_or(ArchiveError::MissingConversation)?;
    let mut structure = Sha256::new();
    let mut payloads = std::collections::BTreeSet::new();
    for version in archive
        .record_versions()
        .iter()
        .filter(|version| version.global_version <= watermark)
    {
        let (record, _) = archive.read_publication(container, *version)?;
        let stored = container.read(version.record)?;
        let raw = if reliquary_container::publication_envelope::decode(&stored)?.is_some() {
            reliquary_archive::archive_codec::publication::comparison_carrier(&record)?
        } else {
            stored
        };
        let relevant = match &record {
            ArchiveRecord::Node(node) => node.conversation_id == conversation_id,
            ArchiveRecord::IngestedTurn(turn) => turn.node.conversation_id == conversation_id,
            ArchiveRecord::Branch(branch) => branch.conversation_id == conversation_id,
            ArchiveRecord::ConversationMetadata(metadata) => {
                metadata.conversation_id == conversation_id
            }
            ArchiveRecord::ConversationTombstone(value) => value.conversation_id == conversation_id,
            ArchiveRecord::Fragment(fragment) => fragment.conversation_id == conversation_id,
            ArchiveRecord::Episode(episode) => episode.conversation_id == conversation_id,
            ArchiveRecord::File(file) => tombstone.closure.file_ids.contains(&file.id),
            ArchiveRecord::FileMemoryLink(link) => {
                tombstone.closure.file_ids.contains(&link.file_id)
            }
            _ => false,
        };
        if !relevant {
            continue;
        }
        structure.update(version.global_version.to_le_bytes());
        structure.update(version.archive_version.to_le_bytes());
        hash_bytes(&mut structure, &raw);
        let node = match &record {
            ArchiveRecord::Node(node) => Some(node),
            ArchiveRecord::IngestedTurn(turn) => Some(&turn.node),
            _ => None,
        };
        if let Some(node) = node {
            if !lengths.contains_key(&node.content_id) {
                return Err(ArchiveError::MissingContent);
            }
            payloads.insert(PrunedPayload::ArchiveContent(node.content_id.0));
        }
    }
    Ok(HistoryRetentionDescriptor {
        target: RetentionTarget {
            domain: RetentionDomain::Archive,
            identity: conversation_id.as_bytes().to_vec(),
        },
        tombstone_global_version: Some(tombstone.global_version),
        tombstone_domain_version: Some(tombstone.archive_version),
        tombstone_digest: Sha256::digest(encode_conversation_tombstone(tombstone)?).into(),
        retained_structure_digest: structure.finalize().into(),
        payloads: payloads.into_iter().collect(),
    })
}

pub(crate) fn validate_archive_prune_history(
    archive: &Archive,
    container: &mut Container,
    maintenance: &crate::owner_maintenance::OwnerMaintenanceStore,
) -> Result<(), ArchiveError> {
    let invalid = || ArchiveError::CorruptRecord("prune evidence does not match source history");
    let mut all_authorized = std::collections::HashSet::<ContentId>::new();
    for (digest, manifest) in maintenance.manifests() {
        let mut lengths = HashMap::new();
        let mut selected = std::collections::BTreeSet::new();
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
                let marker = archive
                    .pruned_content_markers()
                    .get(&id)
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
            if archive_retention_descriptor(
                archive,
                container,
                conversation,
                manifest.source_global_version,
                &lengths,
            )? != *descriptor
            {
                return Err(invalid());
            }
        }
        for version in archive
            .record_versions()
            .iter()
            .filter(|version| version.global_version <= manifest.source_global_version)
        {
            let (record, _) = archive.read_publication(container, *version)?;
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
    if all_authorized.len() != archive.pruned_content_markers().len() {
        return Err(invalid());
    }
    Ok(())
}

pub(crate) fn validate_archive_markers(
    archive: &Archive,
    maintenance: &crate::owner_maintenance::OwnerMaintenanceStore,
) -> Result<(), ArchiveError> {
    for marker in archive.pruned_content_markers().values() {
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

fn hash_bytes(hash: &mut Sha256, bytes: &[u8]) {
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
}
