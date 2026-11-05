//! Pure validation and application of host-owned source change batches.
//!
//! This module changes descriptor metadata only. Host text remains transient,
//! and these host message identities never become native Archive identities.

use reliquary_owner_session::records::{
    ExternalReferenceSnapshotManifest,
    external_index::model::{
        EXTERNAL_SOURCE_CONTRACT, ExternalMessageDescriptor, ExternalSourceError,
        MAX_EXTERNAL_BATCH, identifier,
    },
};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExternalChangeKind {
    Upsert(ExternalMessageDescriptor),
    Visibility {
        conversation_id: String,
        message_ids: Vec<String>,
        active: bool,
    },
    DeleteConversation {
        conversation_id: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExternalConversationChange {
    pub sequence: u64,
    pub kind: ExternalChangeKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExternalChangeBatch {
    /// Oldest retained feed sequence. A cursor before this floor needs rebuild.
    pub feed_floor: u64,
    /// High water mark observed by the host for this batch request.
    pub feed_watermark: u64,
    pub changes: Vec<ExternalConversationChange>,
}

/// Validate a bounded contiguous host feed page and apply it to the accepted
/// descriptor manifest. This function performs no host I/O or durable writes.
pub fn apply_external_change_batch(
    manifest: &ExternalReferenceSnapshotManifest,
    batch: &ExternalChangeBatch,
) -> Result<ExternalReferenceSnapshotManifest, ExternalSourceError> {
    manifest.validate()?;
    if manifest.contract_version != EXTERNAL_SOURCE_CONTRACT
        || batch.changes.len() > MAX_EXTERNAL_BATCH
        || batch.feed_watermark < manifest.watermark
    {
        return Err(ExternalSourceError::InvalidContract);
    }
    if batch.changes.is_empty() {
        if batch.feed_watermark != manifest.watermark {
            return Err(ExternalSourceError::InvalidContract);
        }
        return Ok(manifest.clone());
    }
    if manifest
        .watermark
        .checked_add(1)
        .is_none_or(|next| next < batch.feed_floor)
    {
        return Err(ExternalSourceError::RebuildRequired);
    }

    let mut rows: BTreeMap<_, _> = manifest
        .descriptors
        .iter()
        .cloned()
        .map(|descriptor| {
            (
                (
                    descriptor.reference.conversation_id.clone(),
                    descriptor.reference.message_id.clone(),
                ),
                descriptor,
            )
        })
        .collect();
    let mut sequence = manifest.watermark;
    for change in &batch.changes {
        sequence = sequence.checked_add(1).ok_or(ExternalSourceError::Bounds)?;
        if change.sequence != sequence || sequence > batch.feed_watermark {
            return Err(ExternalSourceError::InvalidContract);
        }
        match &change.kind {
            ExternalChangeKind::Upsert(descriptor) => {
                descriptor.validate(&manifest.namespace)?;
                rows.insert(
                    (
                        descriptor.reference.conversation_id.clone(),
                        descriptor.reference.message_id.clone(),
                    ),
                    descriptor.clone(),
                );
            }
            ExternalChangeKind::Visibility {
                conversation_id,
                message_ids,
                active,
            } => {
                identifier(conversation_id)?;
                if message_ids.is_empty() || message_ids.len() > MAX_EXTERNAL_BATCH {
                    return Err(ExternalSourceError::Bounds);
                }
                let mut seen = std::collections::BTreeSet::new();
                for message_id in message_ids {
                    identifier(message_id)?;
                    if !seen.insert(message_id.as_str()) {
                        return Err(ExternalSourceError::InvalidContract);
                    }
                    let descriptor = rows
                        .get_mut(&(conversation_id.clone(), message_id.clone()))
                        .ok_or(ExternalSourceError::InvalidContract)?;
                    descriptor.active = *active;
                }
            }
            ExternalChangeKind::DeleteConversation { conversation_id } => {
                identifier(conversation_id)?;
                rows.retain(|(conversation, _), _| conversation != conversation_id);
            }
        }
    }
    if batch.changes.len() < MAX_EXTERNAL_BATCH && sequence != batch.feed_watermark {
        return Err(ExternalSourceError::InvalidContract);
    }

    let mut updated = manifest.clone();
    updated.watermark = sequence;
    updated.descriptors = rows.into_values().collect();
    updated.validate()?;
    Ok(updated)
}

#[cfg(test)]
mod tests {
    use super::*;
    use reliquary_owner_session::records::external_index::ExternalMessageRef;

    fn descriptor(conversation: &str, message: &str, active: bool) -> ExternalMessageDescriptor {
        ExternalMessageDescriptor {
            reference: ExternalMessageRef {
                source_namespace: "host".into(),
                conversation_id: conversation.into(),
                message_id: message.into(),
                content_hash: [message.as_bytes()[0]; 32],
            },
            role: "user".into(),
            principal_id: Some("principal".into()),
            timestamp_ns: 10,
            active,
        }
    }

    fn manifest(watermark: u64) -> ExternalReferenceSnapshotManifest {
        let mut descriptors = vec![
            descriptor("conversation-a", "message-a", true),
            descriptor("conversation-a", "message-b", true),
            descriptor("conversation-b", "message-c", true),
        ];
        descriptors.sort_by(|a, b| {
            (&a.reference.conversation_id, &a.reference.message_id)
                .cmp(&(&b.reference.conversation_id, &b.reference.message_id))
        });
        ExternalReferenceSnapshotManifest {
            namespace: "host".into(),
            contract_version: EXTERNAL_SOURCE_CONTRACT,
            watermark,
            descriptors,
        }
    }

    fn change(sequence: u64, kind: ExternalChangeKind) -> ExternalConversationChange {
        ExternalConversationChange { sequence, kind }
    }

    #[test]
    fn feed_applies_upsert_visibility_and_delete_without_source_text() {
        let input = manifest(7);
        let page = ExternalChangeBatch {
            feed_floor: 1,
            feed_watermark: 10,
            changes: vec![
                change(
                    8,
                    ExternalChangeKind::Upsert(descriptor("conversation-a", "message-a", true)),
                ),
                change(
                    9,
                    ExternalChangeKind::Visibility {
                        conversation_id: "conversation-a".into(),
                        message_ids: vec!["message-b".into()],
                        active: false,
                    },
                ),
                change(
                    10,
                    ExternalChangeKind::DeleteConversation {
                        conversation_id: "conversation-b".into(),
                    },
                ),
            ],
        };

        let applied = apply_external_change_batch(&input, &page).unwrap();
        assert_eq!(applied.watermark, 10);
        assert_eq!(applied.descriptors.len(), 2);
        assert!(
            !applied
                .descriptors
                .iter()
                .find(|row| row.reference.message_id == "message-b")
                .unwrap()
                .active
        );
        assert!(
            applied
                .descriptors
                .iter()
                .all(|row| row.reference.conversation_id == "conversation-a")
        );
    }

    #[test]
    fn exact_empty_page_is_noop_and_feed_floor_requires_rebuild() {
        let input = manifest(4);
        let empty = apply_external_change_batch(
            &input,
            &ExternalChangeBatch {
                feed_floor: 1,
                feed_watermark: 4,
                changes: Vec::new(),
            },
        )
        .unwrap();
        assert_eq!(empty, input);

        let error = apply_external_change_batch(
            &manifest(4),
            &ExternalChangeBatch {
                feed_floor: 6,
                feed_watermark: 8,
                changes: vec![change(
                    5,
                    ExternalChangeKind::DeleteConversation {
                        conversation_id: "conversation-a".into(),
                    },
                )],
            },
        );
        assert_eq!(error, Err(ExternalSourceError::RebuildRequired));
    }

    #[test]
    fn feed_rejects_gaps_unknown_visibility_ids_and_incomplete_short_pages() {
        let input = manifest(3);
        for page in [
            ExternalChangeBatch {
                feed_floor: 1,
                feed_watermark: 5,
                changes: vec![change(
                    5,
                    ExternalChangeKind::DeleteConversation {
                        conversation_id: "conversation-a".into(),
                    },
                )],
            },
            ExternalChangeBatch {
                feed_floor: 1,
                feed_watermark: 4,
                changes: vec![change(
                    4,
                    ExternalChangeKind::Visibility {
                        conversation_id: "conversation-a".into(),
                        message_ids: vec!["missing".into()],
                        active: false,
                    },
                )],
            },
            ExternalChangeBatch {
                feed_floor: 1,
                feed_watermark: 5,
                changes: vec![change(
                    4,
                    ExternalChangeKind::DeleteConversation {
                        conversation_id: "conversation-a".into(),
                    },
                )],
            },
        ] {
            assert!(apply_external_change_batch(&input, &page).is_err());
        }
    }

    #[test]
    fn feed_accepts_full_page_before_host_watermark() {
        let input = manifest(1);
        let changes = (2..(2 + MAX_EXTERNAL_BATCH as u64))
            .map(|sequence| {
                change(
                    sequence,
                    ExternalChangeKind::Visibility {
                        conversation_id: "conversation-a".into(),
                        message_ids: vec!["message-a".into()],
                        active: sequence % 2 == 0,
                    },
                )
            })
            .collect();
        let result = apply_external_change_batch(
            &input,
            &ExternalChangeBatch {
                feed_floor: 1,
                feed_watermark: 100,
                changes,
            },
        )
        .unwrap();
        assert_eq!(result.watermark, 1 + MAX_EXTERNAL_BATCH as u64);
    }
}
