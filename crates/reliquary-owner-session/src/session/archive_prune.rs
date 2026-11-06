//! REL-only terminal Archive content pruning with retained structural history.

use super::*;
use crate::records::{
    HistoryPruneManifest, HistoryRetentionDescriptor, PrunedPayload, RetentionDomain,
    RetentionTarget,
};
use crate::{VerifiedOwnerCopyPlan, staged_copy::StagedCopy};
use reliquary_vector_store::generation::{generation_receipt_target, relocate_generation_receipt};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Read,
};

mod postflight;
mod preflight;
mod proof;
pub(super) use proof::validate_archive_prune;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArchivePruneReport {
    pub owner_id: [u8; 16],
    pub pruned_conversations: usize,
    pub pruned_turns: usize,
    pub source_bytes: u64,
    pub transformed_bytes: u64,
    pub manifest_digest: [u8; 32],
}

pub fn prune_terminal_archive_copy(
    plan: &VerifiedOwnerCopyPlan,
    source_path: &Path,
    destination: &Path,
    conversations: &[String],
    admission: &GraphAdmission,
) -> Result<ArchivePruneReport, String> {
    if plan.file_kind() != FileKind::Reliquary {
        return Err("Archive pruning is REL-only".into());
    }
    if conversations.is_empty() || conversations.len() > 128 {
        return Err("Archive pruning requires 1..=128 explicit conversations".into());
    }
    if destination.exists() {
        return Err("maintenance destination already exists".into());
    }
    let selected = conversations.iter().cloned().collect::<BTreeSet<_>>();
    if selected.len() != conversations.len() {
        return Err("duplicate Archive prune target".into());
    }
    let source = Container::open_read_only_as(source_path, FileKind::Reliquary)
        .map_err(|e| e.to_string())?;
    plan.verify_source(&source, source_path)?;
    if !OwnerMaintenanceStore::open(&source)?.is_empty() {
        return Err("repeated destructive pruning requires a new lineage contract".into());
    }
    let session = RelSession::open(&source, admission, [], []).map_err(|e| e.to_string())?;
    for conversation in &selected {
        if session
            .archive
            .conversation_tombstone(conversation)
            .is_none()
        {
            return Err("Archive prune target is not terminally tombstoned".into());
        }
    }
    preflight::verify_dependencies(&session, &selected)?;
    let mut descriptors = Vec::new();
    for conversation in &selected {
        let tombstone = session
            .archive
            .conversation_tombstone(conversation)
            .unwrap();
        let nodes = session
            .archive
            .accepted_turn_sources()
            .into_iter()
            .filter(|source| &source.conversation_id == conversation)
            .collect::<Vec<_>>();
        if nodes.is_empty() {
            return Err("Archive tombstone has no retained conversation structure".into());
        }
        let mut structure = Sha256::new();
        structure.update(b"reliquary-archive-retained-structure-v1\0");
        hash_text(&mut structure, conversation);
        let mut payloads = Vec::new();
        for source in nodes {
            let node = session.archive.node(conversation, &source.node_id).unwrap();
            hash_text(&mut structure, &node.id);
            structure.update(node.content_id.0);
            payloads.push(PrunedPayload::ArchiveContent(turn_identity(
                conversation,
                &node.id,
            )));
        }
        payloads.sort();
        let mut tombstone_digest = Sha256::new();
        tombstone_digest.update(b"reliquary-conversation-tombstone-v1\0");
        hash_text(&mut tombstone_digest, conversation);
        tombstone_digest.update(tombstone.archive_version.to_le_bytes());
        descriptors.push(HistoryRetentionDescriptor {
            target: RetentionTarget {
                domain: RetentionDomain::Archive,
                identity: conversation.as_bytes().to_vec(),
            },
            tombstone_global_version: None,
            tombstone_domain_version: Some(tombstone.archive_version),
            tombstone_digest: tombstone_digest.finalize().into(),
            retained_structure_digest: structure.finalize().into(),
            payloads,
        });
    }
    descriptors.sort_by(|a, b| a.target.cmp(&b.target));
    let manifest = HistoryPruneManifest {
        owner_uuid: source.owner_id(),
        source_global_version: source.latest_version(),
        source_history_digest: plan.source_digest(),
        descriptors,
    };
    let manifest_digest = manifest.digest()?;
    let manifest_bytes = manifest.encode()?;
    let source_bytes = fs::metadata(source_path).map_err(|e| e.to_string())?.len();
    let staged = StagedCopy::create(destination)?;
    let mut target = Container::create(&staged.path, source.identity(), source.owner_id())
        .map_err(|e| e.to_string())?;
    let mut replaced = BTreeSet::new();
    // Terminal text markers shrink accepted frames, shifting later physical
    // vector-generation offsets even when their own payloads are retained.
    let mut relocated_generations = BTreeMap::new();
    let mut cursor = source.physical_cursor().map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    while let Some(mut frame) = cursor.next_frame().map_err(|e| e.to_string())? {
        bytes.clear();
        frame.read_to_end(&mut bytes).map_err(|e| e.to_string())?;
        let payload = if let Some((pruned, node)) =
            reliquary_archive::prune_turn_frame(&bytes, manifest_digest)
                .map_err(|e| e.to_string())?
            && selected.contains(&node.conversation_id)
        {
            if !replaced.insert((node.conversation_id, node.id)) {
                return Err("duplicate selected Archive turn".into());
            }
            pruned
        } else if let Some(old) = generation_receipt_target(&bytes).map_err(|e| e.to_string())? {
            let new = *relocated_generations
                .get(&old.offset)
                .ok_or("Archive prune generation receipt lost its source record")?;
            relocate_generation_receipt(&bytes, new)
                .map_err(|e| e.to_string())?
                .to_vec()
        } else {
            bytes.clone()
        };
        let written = target.append(&payload).map_err(|e| e.to_string())?;
        if bytes.starts_with(b"RELVGEN1") {
            relocated_generations.insert(frame.object().offset, written);
        }
    }
    let expected = manifest
        .descriptors
        .iter()
        .map(|descriptor| descriptor.payloads.len())
        .sum::<usize>();
    if replaced.len() != expected {
        return Err("selected Archive turn is not independently removable".into());
    }
    target.append(&manifest_bytes).map_err(|e| e.to_string())?;
    target.sync().map_err(|e| e.to_string())?;
    drop(target);
    plan.verify_source(&source, source_path)?;
    let copy = Container::open_read_only_as(&staged.path, FileKind::Reliquary)
        .map_err(|e| e.to_string())?;
    let reopened = RelSession::open(&copy, admission, [], []).map_err(|e| e.to_string())?;
    if copy.identity() != source.identity()
        || copy.owner_id() != source.owner_id()
        || copy.latest_version() != source.latest_version()
        || reopened.archive.archive_version() != session.archive.archive_version()
    {
        return Err("Archive prune changed owner identity or clocks".into());
    }
    postflight::verify_reopened(&session, &reopened, &selected)?;
    drop(reopened);
    drop(copy);
    plan.verify_source(&source, source_path)?;
    fs::hard_link(&staged.path, destination).map_err(|e| e.to_string())?;
    Ok(ArchivePruneReport {
        owner_id: source.owner_id(),
        pruned_conversations: selected.len(),
        pruned_turns: replaced.len(),
        source_bytes,
        transformed_bytes: fs::metadata(destination).map_err(|e| e.to_string())?.len(),
        manifest_digest,
    })
}

fn turn_identity(conversation: &str, node: &str) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"reliquary-archive-turn-prune-v1\0");
    for value in [conversation, node] {
        hash_text(&mut hash, value);
    }
    hash.finalize().into()
}

fn hash_text(hash: &mut Sha256, value: &str) {
    hash.update((value.len() as u64).to_le_bytes());
    hash.update(value.as_bytes());
}

#[cfg(test)]
#[path = "archive_prune/tests.rs"]
mod tests;
#[cfg(test)]
#[path = "archive_prune/tests_lifecycle.rs"]
mod tests_lifecycle;
#[cfg(test)]
#[path = "archive_prune/tests_negative.rs"]
mod tests_negative;
#[cfg(test)]
#[path = "archive_prune/tests_shared.rs"]
mod tests_shared;
#[cfg(test)]
#[path = "archive_prune/tests_vectors.rs"]
mod tests_vectors;
