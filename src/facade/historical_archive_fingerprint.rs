//! Canonical decoded Archive publication history and payload at a local cut.

use crate::archive_codec::ArchiveRecord;
use crate::historical_fingerprint::{HistoricalFingerprint, OwnerProjection};
use crate::{Branch, ContentId, Cva, Node, StoredFile};
use serde_json::{Value, json};
use std::collections::BTreeMap;

pub(crate) struct ArchiveCut {
    pub(crate) fingerprint: HistoricalFingerprint,
    pub(crate) branches: BTreeMap<(String, String), Branch>,
}

pub(crate) fn archive_cut(cva: &mut Cva, cut: u64) -> Result<ArchiveCut, String> {
    if cut > cva.archive_version() {
        return Err("Archive fingerprint cut exceeds retained head".into());
    }
    let owner = cva
        .owner_uuid()
        .ok_or("Archive fingerprint needs owner UUID")?;
    let mut projection = OwnerProjection::new(owner, "archive", cut, None)?;
    let mut branches = BTreeMap::new();
    let mut contents = BTreeMap::new();
    // Accepted owner publications are the visibility authority. A physical scan
    // alone would include inert backing and could invent historical state.
    let versions = cva.archive.record_versions().to_vec();
    for version in versions
        .into_iter()
        .take_while(|v| v.archive_version <= cut)
    {
        let (record, applying) = cva
            .archive
            .read_publication(&mut cva.container, version)
            .map_err(|e| e.to_string())?;
        let value = match record {
            ArchiveRecord::Node(value) => json!({"node": node(cva, &value, &mut contents)?}),
            ArchiveRecord::Branch(value) => {
                if applying.archive_version == version.archive_version {
                    branches.insert(
                        (value.conversation_id.clone(), value.id.clone()),
                        value.clone(),
                    );
                }
                json!({"branch": {"id": value.id, "conversation": value.conversation_id,
                    "leaf": value.leaf_node_id, "canonical": value.canonical}})
            }
            ArchiveRecord::ConversationMetadata(value) => json!({"conversation": {
                "id": value.conversation_id, "title": value.title, "active": value.active,
                "policy": match value.policy {
                    crate::ConversationPolicy::SequentialChronological => "sequential_chronological",
                    crate::ConversationPolicy::VisibleAutoBranch => "visible_auto_branch",
                }, "policy_revision": value.policy_revision}}),
            ArchiveRecord::ConversationTombstone(value) => json!({"tombstone": {
                "conversation": value.conversation_id, "archive_version": value.archive_version,
                "global_version": value.global_version, "nodes": value.closure.node_ids,
                "branches": value.closure.branch_ids,
                "fragments": value.closure.fragment_ids.iter().map(|id| hex(&id.0)).collect::<Vec<_>>(),
                "episodes": value.closure.episode_ids.iter().map(|id| hex(&id.0)).collect::<Vec<_>>(),
                "files": value.closure.file_ids.iter().map(|id| hex(&id.0)).collect::<Vec<_>>(),
                "metadata_versions": value.closure.metadata_archive_versions}}),
            ArchiveRecord::Fragment(value) => json!({"fragment": {
                "id": hex(&value.id.0), "conversation": value.conversation_id,
                "start": value.start_node_id, "end": value.end_node_id}}),
            ArchiveRecord::Episode(value) => json!({"episode": {
                "id": hex(&value.id.0), "conversation": value.conversation_id,
                "start": value.start_node_id, "end": value.end_node_id,
                "origin": value.origin.tag(), "boundary": value.boundary.tag(),
                "source_through_ns": value.source_through_ns,
                "finalized_at_ns": value.finalized_at_ns}}),
            ArchiveRecord::File(value) => json!({"file": file(cva, &value, &mut contents)?}),
            ArchiveRecord::IngestedTurn(value) => {
                let mut files = Vec::new();
                for attachment in &value.attachments {
                    files.push(file(cva, attachment, &mut contents)?);
                }
                json!({"turn": {"node": node(cva, &value.node, &mut contents)?, "attachments": files}})
            }
            ArchiveRecord::FileMemoryLink(value) => json!({"file_memory_link": {
                "file_id": hex(&value.file_id.0), "memory_id": hex(&value.memory_id.0)}}),
            ArchiveRecord::ImportReceipt(value) => {
                json!({"import_receipt": serde_json::to_value(value).map_err(|e| e.to_string())?})
            }
            ArchiveRecord::Content(..) | ArchiveRecord::Other => {
                return Err("Archive publication does not resolve to semantic authority".into());
            }
        };
        projection.insert("publication", version.archive_version.to_be_bytes(), json!({
            "archive_version": version.archive_version, "global_version": version.global_version,
            "transaction_time_ns": cva.container.transaction_time_ns(version.global_version),
            "record": value,
        }))?;
    }
    for (id, value) in contents {
        projection.insert("content", id, value)?;
    }
    Ok(ArchiveCut {
        fingerprint: projection.fingerprint()?,
        branches,
    })
}

fn node(
    cva: &mut Cva,
    value: &Node,
    contents: &mut BTreeMap<[u8; 32], Value>,
) -> Result<Value, String> {
    content(cva, value.content_id, contents)?;
    Ok(
        json!({"id": value.id, "conversation": value.conversation_id,
        "parent": value.parent_id, "role": value.role, "principal": value.principal_id,
        "timestamp_ns": value.timestamp_ns, "content_id": hex(&value.content_id.0)}),
    )
}

fn file(
    cva: &mut Cva,
    value: &StoredFile,
    contents: &mut BTreeMap<[u8; 32], Value>,
) -> Result<Value, String> {
    let reference = cva.project_files.get(value.id).map(|r| json!({
        "kind": match r.revision.repository.kind {
            crate::ProjectRepositoryKind::Lore => "lore", crate::ProjectRepositoryKind::Git => "git",
        }, "repository_id": r.revision.repository.repository_id,
        "project_path": r.revision.repository.project_path, "revision": r.revision.revision,
        "path": r.path, "content_hash": r.content_hash.map(|id| hex(&id)),
    }));
    if reference.is_none() || cva.archive.contents.contains(value.content_id) {
        content(cva, value.content_id, contents)?;
    }
    Ok(
        json!({"id": hex(&value.id.0), "content_id": hex(&value.content_id.0),
        "filename": value.filename, "mime_type": value.mime_type,
        "byte_length": value.byte_length, "project_reference": reference}),
    )
}

fn content(
    cva: &mut Cva,
    id: ContentId,
    contents: &mut BTreeMap<[u8; 32], Value>,
) -> Result<(), String> {
    if contents.contains_key(&id.0) {
        return Ok(());
    }
    let value = if let Some(marker) = cva.archive.prune.markers().get(&id) {
        json!({"pruned": {"bytes": marker.byte_length, "manifest": hex(&marker.manifest_digest)}})
    } else {
        let bytes = cva
            .archive
            .content_bytes(&mut cva.container, id)
            .map_err(|e| e.to_string())?;
        json!({"bytes_hex": hex(&bytes)})
    };
    contents.insert(id.0, value);
    Ok(())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[path = "historical_archive_fingerprint_tests.rs"]
mod tests;
