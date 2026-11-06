//! Independent REL replay admission for terminal Archive retention.
use super::*;
use std::collections::{BTreeMap, BTreeSet};

pub(crate) fn validate_archive_prune(
    archive: &Archive,
    maintenance: &OwnerMaintenanceStore,
) -> Result<(), OwnerSessionError> {
    let invalid = || OwnerSessionError::Record("invalid Archive prune retention evidence".into());
    let pruned = archive
        .pruned_turns()
        .into_iter()
        .map(|(node, digest)| ((node.conversation_id, node.id), digest))
        .collect::<BTreeMap<_, _>>();
    let mut authorized = BTreeMap::<(String, String), [u8; 32]>::new();
    let mut selected = BTreeSet::<String>::new();

    for (digest, manifest) in maintenance.manifests() {
        if !manifest
            .descriptors
            .iter()
            .any(|d| d.target.domain == RetentionDomain::Archive)
        {
            continue;
        }
        if archive.owner_id() != Some(manifest.owner_uuid) {
            return Err(invalid());
        }
        for descriptor in &manifest.descriptors {
            if descriptor.target.domain != RetentionDomain::Archive {
                continue;
            }
            let conversation =
                std::str::from_utf8(&descriptor.target.identity).map_err(|_| invalid())?;
            if conversation.is_empty() || !selected.insert(conversation.to_owned()) {
                return Err(invalid());
            }
            let tombstone = archive
                .conversation_tombstone(conversation)
                .ok_or_else(invalid)?;
            let mut tombstone_hash = Sha256::new();
            tombstone_hash.update(b"reliquary-conversation-tombstone-v1\0");
            hash_text(&mut tombstone_hash, conversation);
            tombstone_hash.update(tombstone.archive_version.to_le_bytes());
            let tombstone_digest: [u8; 32] = tombstone_hash.finalize().into();

            let mut structure = Sha256::new();
            structure.update(b"reliquary-archive-retained-structure-v1\0");
            hash_text(&mut structure, conversation);
            let mut payloads = Vec::new();
            for source in archive
                .accepted_turn_sources()
                .into_iter()
                .filter(|source| source.conversation_id == conversation)
            {
                let node = archive
                    .node(conversation, &source.node_id)
                    .ok_or_else(invalid)?;
                hash_text(&mut structure, &node.id);
                structure.update(node.content_id.0);
                let key = (conversation.to_owned(), node.id.clone());
                if pruned.get(&key) != Some(digest) || authorized.insert(key, *digest).is_some() {
                    return Err(invalid());
                }
                payloads.push(PrunedPayload::ArchiveContent(turn_identity(
                    conversation,
                    &node.id,
                )));
            }
            payloads.sort();
            let structure_digest: [u8; 32] = structure.finalize().into();
            if payloads.is_empty()
                || descriptor.payloads != payloads
                || descriptor.tombstone_global_version.is_some()
                || descriptor.tombstone_domain_version != Some(tombstone.archive_version)
                || descriptor.tombstone_digest != tombstone_digest
                || descriptor.retained_structure_digest != structure_digest
            {
                return Err(invalid());
            }
        }
    }

    if pruned != authorized {
        return Err(invalid());
    }
    // Pruned body identities cannot be silently backed by another retained
    // conversation, nor can they be supplied as independently retained files.
    let pruned_content = archive
        .accepted_turn_sources()
        .into_iter()
        .filter(|source| selected.contains(&source.conversation_id))
        .filter_map(|source| {
            archive
                .node(&source.conversation_id, &source.node_id)
                .map(|node| node.content_id)
        })
        .collect::<BTreeSet<_>>();
    for source in archive.accepted_turn_sources() {
        let node = archive
            .node(&source.conversation_id, &source.node_id)
            .ok_or_else(invalid)?;
        if (!selected.contains(&source.conversation_id)
            && pruned_content.contains(&node.content_id))
            || archive
                .files_for_source(&source.conversation_id, &source.node_id)
                .iter()
                .any(|file| pruned_content.contains(&file.content_id))
        {
            return Err(invalid());
        }
    }
    Ok(())
}
