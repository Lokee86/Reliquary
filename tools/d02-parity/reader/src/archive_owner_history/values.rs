use crate::evidence_encoding::{hex, sha256};
use reliquary_archive::{
    Episode, EpisodeBoundary, EpisodeOrigin, HistoricalArchiveTurn, ProjectCorrelation,
    ProjectFileRef, ProjectRepositoryKind, ProjectRevisionRef, SourceAttachment,
};
use serde_json::{Value, json};
use std::collections::BTreeMap;

pub(super) type Content = BTreeMap<[u8; 32], Value>;
pub(super) type Project = BTreeMap<[u8; 32], Value>;

pub(super) fn turn_value(
    turn: &HistoricalArchiveTurn,
    content: &mut Content,
    projects: &mut Project,
) -> Result<Value, String> {
    let node = &turn.node;
    let body = turn
        .content
        .as_ref()
        .ok_or("archive_retained_turn_body_unavailable")?;
    insert_content(content, node.content_id.0, body.as_bytes())?;
    let attachments = turn
        .attachments
        .iter()
        .map(|attachment| attachment_value(attachment, content, projects))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(json!({"turn": {"node": {
        "id": node.id, "conversation": node.conversation_id, "parent": node.parent_id,
        "role": node.role, "principal": node.principal_id, "timestamp_ns": node.timestamp_ns,
        "content_id": hex(&node.content_id.0),
    }, "attachments": attachments}}))
}

fn insert_content(content: &mut Content, identity: [u8; 32], bytes: &[u8]) -> Result<(), String> {
    if sha256(bytes) != identity {
        return Err("archive_retained_body_identity_differs".into());
    }
    content
        .entry(identity)
        .or_insert_with(|| json!({"bytes_hex": hex(bytes)}));
    Ok(())
}

fn attachment_value(
    attachment: &SourceAttachment,
    content: &mut Content,
    projects: &mut Project,
) -> Result<Value, String> {
    let reference = match &attachment.project_reference {
        Some(reference) => {
            let binding = project_file_binding(attachment.id, reference);
            if let Some(old) = projects.insert(attachment.id, binding.clone()) {
                if old != binding {
                    return Err("archive_conflicting_project_file_binding".into());
                }
            }
            let mut value = revision_value(&reference.revision);
            value["path"] = json!(reference.path);
            value["content_hash"] = json!(reference.content_hash.map(|digest| hex(&digest)));
            Some(value)
        }
        None => {
            if u64::try_from(attachment.bytes.len()).ok() != Some(attachment.byte_length) {
                return Err("archive_retained_attachment_extent_differs".into());
            }
            insert_content(content, attachment.content_id.0, &attachment.bytes)?;
            None
        }
    };
    Ok(json!({
        "id": hex(&attachment.id), "content_id": hex(&attachment.content_id.0),
        "filename": attachment.filename, "mime_type": attachment.mime_type,
        "byte_length": attachment.byte_length, "project_reference": reference,
    }))
}

pub(super) fn episode_value(episode: &Episode) -> Value {
    json!({"episode": {
        "id": hex(&episode.id.0), "conversation": episode.conversation_id,
        "start": episode.start_node_id, "end": episode.end_node_id,
        "origin": match episode.origin { EpisodeOrigin::Live => 1, EpisodeOrigin::Import => 2 },
        "boundary": match episode.boundary {
            EpisodeBoundary::Size => 1, EpisodeBoundary::Inactivity => 2,
            EpisodeBoundary::CreateMemory => 3, EpisodeBoundary::ImportEnd => 4,
            EpisodeBoundary::Explicit => 5,
        },
        "source_through_ns": episode.source_through_ns, "finalized_at_ns": episode.finalized_at_ns,
    }})
}

fn revision_value(revision: &ProjectRevisionRef) -> Value {
    let repository = &revision.repository;
    json!({
        "kind": match repository.kind { ProjectRepositoryKind::Git => "git", ProjectRepositoryKind::Lore => "lore" },
        "repository_id": repository.repository_id, "project_path": repository.project_path,
        "revision": revision.revision,
    })
}

fn project_file_binding(id: [u8; 32], reference: &ProjectFileRef) -> Value {
    json!({
        "file_id": id, "project_revision": revision_value(&reference.revision),
        "path": reference.path, "content_hash": reference.content_hash,
    })
}

pub(super) fn project_correlation(record: &ProjectCorrelation) -> Value {
    json!({
        "sequence": record.sequence, "global_version": record.global_version,
        "archive_version": record.archive_version, "memory_version": record.memory_version,
        "repository_management": record.repository_management,
        "project_revision": revision_value(&record.project_revision),
    })
}
