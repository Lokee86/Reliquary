use crate::evidence_encoding::{hex, sha256};
use reliquary_archive::{
    Branch, Episode, EpisodeBoundary, EpisodeOrigin, FragmentDefinition, HistoricalArchiveTurn,
    ImportReceipt, ProjectRepositoryKind, SourceAttachment,
};
use reliquary_owner_session::{ArchiveCutObservation, ArchiveTurnOrigin};
use serde_json::{Value, json};

pub(super) fn projection(observation: &ArchiveCutObservation) -> Value {
    let cut = &observation.snapshot;
    json!({
        "archive_version": cut.archive_version.to_string(),
        "activity_cut": cut.activity_cut.to_string(),
        "turns": cut.turns.iter().zip(&observation.turn_origins)
            .map(|(turn, origin)| turn_value(turn, origin)).collect::<Vec<_>>(),
        "branch_revisions": cut.branch_revisions.iter().map(|revision| json!({
            "archive_version": revision.archive_version.to_string(),
            "branch": branch_value(&revision.branch),
        })).collect::<Vec<_>>(),
        "branches": cut.branches.iter().map(branch_value).collect::<Vec<_>>(),
        "episodes": cut.episodes.iter().map(episode_value).collect::<Vec<_>>(),
        "import_receipts": cut.import_receipts.iter().map(import_receipt_value).collect::<Vec<_>>(),
        "fragment_definitions": cut.fragment_definitions.iter().map(fragment_value).collect::<Vec<_>>(),
        "conversation_tombstones": cut.conversation_tombstones.iter().map(|terminal| json!({
            "conversation_id": terminal.conversation_id,
            "archive_version": terminal.archive_version.to_string(),
        })).collect::<Vec<_>>(),
    })
}

fn turn_value(turn: &HistoricalArchiveTurn, origin: &ArchiveTurnOrigin) -> Value {
    let body = match &turn.content {
        Some(text) => json!({
            "availability": "present",
            "utf8_length": text.len().to_string(),
            "sha256": hex(&sha256(text.as_bytes())),
        }),
        None => json!({
            "availability": "pruned_terminal",
            "prune_manifest_sha256": origin.prune_manifest_digest.as_ref().map(|digest| hex(digest)),
        }),
    };
    json!({
        "node_id": turn.node.id,
        "conversation_id": turn.node.conversation_id,
        "parent_id": turn.node.parent_id,
        "role": turn.node.role,
        "principal_id": turn.node.principal_id,
        "timestamp_ns": turn.node.timestamp_ns.to_string(),
        "content_id": hex(&turn.node.content_id.0),
        "body": body,
        "archive_version": origin.archive_version.to_string(),
        "activity_position": turn.activity_position.map(|value| value.to_string()),
        "attachments": turn.attachments.iter().map(attachment_value).collect::<Vec<_>>(),
    })
}

fn attachment_value(attachment: &SourceAttachment) -> Value {
    let payload = match &attachment.project_reference {
        Some(file) => {
            let revision = &file.revision;
            let repository = &revision.repository;
            json!({
                "availability": "external_project_reference",
                "kind": match repository.kind {
                    ProjectRepositoryKind::Git => "git",
                    ProjectRepositoryKind::Lore => "lore",
                },
                "repository_id": repository.repository_id,
                "project_path": repository.project_path,
                "revision": revision.revision,
                "path": file.path,
                "content_hash": file.content_hash.as_ref().map(|digest| hex(digest)),
            })
        }
        None => json!({
            "availability": "present",
            "sha256": hex(&sha256(&attachment.bytes)),
        }),
    };
    json!({
        "id": hex(&attachment.id),
        "content_id": hex(&attachment.content_id.0),
        "filename": attachment.filename,
        "mime_type": attachment.mime_type,
        "byte_length": attachment.byte_length.to_string(),
        "payload": payload,
    })
}

fn branch_value(branch: &Branch) -> Value {
    json!({
        "id": branch.id,
        "conversation_id": branch.conversation_id,
        "leaf_node_id": branch.leaf_node_id,
        "canonical": branch.canonical,
    })
}

fn episode_value(episode: &Episode) -> Value {
    json!({
        "id": hex(&episode.id.0),
        "conversation_id": episode.conversation_id,
        "start_node_id": episode.start_node_id,
        "end_node_id": episode.end_node_id,
        "origin": match episode.origin {
            EpisodeOrigin::Live => "live",
            EpisodeOrigin::Import => "import",
        },
        "boundary": match episode.boundary {
            EpisodeBoundary::Size => "size",
            EpisodeBoundary::Inactivity => "inactivity",
            EpisodeBoundary::CreateMemory => "create_memory",
            EpisodeBoundary::ImportEnd => "import_end",
            EpisodeBoundary::Explicit => "explicit",
        },
        "source_through_ns": episode.source_through_ns.to_string(),
        "finalized_at_ns": episode.finalized_at_ns.to_string(),
    })
}

fn fragment_value(definition: &FragmentDefinition) -> Value {
    json!({
        "id": hex(&definition.fragment.id.0),
        "conversation_id": definition.fragment.conversation_id,
        "start_node_id": definition.fragment.start_node_id,
        "end_node_id": definition.fragment.end_node_id,
        "branch_id": definition.branch_id,
        "archive_cut": definition.archive_cut.to_string(),
        "turns": definition.config.turns.to_string(),
        "overlap": definition.config.overlap.to_string(),
        "close_tail": definition.close_tail,
    })
}

fn import_receipt_value(receipt: &ImportReceipt) -> Value {
    json!({
        "schema_version": receipt.schema_version.to_string(),
        "owner_uuid": hex(&receipt.owner_uuid),
        "import_key": hex(&receipt.import_key),
        "source_namespace": receipt.source_namespace,
        "source_format": receipt.source_format,
        "source_digest": hex(&receipt.source_digest),
        "receipt_digest": hex(&receipt.receipt_digest),
        "conversations": receipt.conversations.iter().map(|conversation| json!({
            "source_id": conversation.source_id,
            "conversation_id": conversation.conversation_id,
            "title": conversation.title,
            "branches": conversation.branches,
            "messages": conversation.messages.iter().map(|message| json!({
                "source_id": message.source_id,
                "revision": message.revision,
                "source_parent_id": message.source_parent_id,
                "replaces": message.replaces,
                "deleted": message.deleted,
                "node_id": message.node_id,
                "fingerprint": hex(&message.fingerprint),
                "artifacts": message.artifacts.iter().map(|artifact| json!({
                    "source_id": artifact.source_id,
                    "source_ref": artifact.source_ref,
                    "sha256": hex(&artifact.sha256),
                    "file_id": hex(&artifact.file_id),
                })).collect::<Vec<_>>(),
            })).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
    })
}
