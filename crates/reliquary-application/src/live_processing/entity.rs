//! Recover mention spans from the owner-accepted extraction and exact Memory body.
use crate::{EntityMemoryMention, TextField, entity_workflow::mention_key};
use reliquary_entity_resolution::MentionSpan;
use reliquary_insomnia::{InsomniaExtraction, InsomniaOwnership, TextField as ExtractedField};
use reliquary_owner_session::{
    ManagementRead, OwnerSessionError,
    model::{EpisodeId, MemoryBodyId, MemoryId, memory_body_id},
};
use serde::Deserialize;

pub struct LiveEntityInput {
    pub mention: EntityMemoryMention,
    body: MemoryBodyId,
}

impl LiveEntityInput {
    /// None means the exact accepted body is no longer available.
    pub fn resolved(&self, read: &ManagementRead<'_>) -> Option<bool> {
        let input = &self.mention;
        let (record, body) = read.memory_projection(input.memory_id, None).ok()?;
        if record.body_id != self.body {
            return None;
        }
        let key = mention_key(
            input.memory_id,
            &MentionSpan {
                field: input.field,
                start_byte: input.start_byte,
                end_byte: input.end_byte,
                text: input.text.clone(),
            },
        )
        .ok()?;
        if read.mention(key).is_some() {
            return Some(true);
        }
        (!record.archived && body.is_some()).then_some(false)
    }

    pub fn work_id(&self) -> String {
        let hex = |bytes: &[u8]| bytes.iter().map(|b| format!("{b:02x}")).collect::<String>();
        let input = &self.mention;
        format!(
            "{}:{}:{:?}:{}:{}",
            hex(&input.owner_id),
            hex(&input.memory_id.0),
            input.field,
            input.start_byte,
            input.end_byte
        )
    }
}

#[derive(Deserialize)]
struct Checkpoint {
    version: u32,
    owner: [u8; 16],
    episode: [u8; 32],
    model: String,
    destination: Option<[u8; 16]>,
    extraction: InsomniaExtraction,
}

/// The caller supplies only the local IDs and the authorized destination's
/// IDs from the matching receipt, in accepted publication order.
pub fn accepted_live_mentions(
    owner: [u8; 16],
    episode: EpisodeId,
    model: &str,
    local: &[MemoryId],
    routed: Option<([u8; 16], &[MemoryId])>,
    payload: &[u8],
) -> Result<Vec<LiveEntityInput>, OwnerSessionError> {
    let bad = || {
        OwnerSessionError::Record(
            "LIVE extraction checkpoint differs from its accepted output".into(),
        )
    };
    let checkpoint: Checkpoint = serde_json::from_slice(payload).map_err(|_| bad())?;
    if checkpoint.version != 1
        || checkpoint.owner != owner
        || checkpoint.episode != episode.0
        || checkpoint.model != model
        || !checkpoint.extraction.rejected.is_empty()
    {
        return Err(bad());
    }

    let (project, user): (Vec<_>, Vec<_>) = checkpoint
        .extraction
        .candidates
        .into_iter()
        .partition(|candidate| candidate.ownership == InsomniaOwnership::Project);
    if project.len() != local.len() {
        return Err(bad());
    }
    let mut mentions = Vec::new();
    let mut collect =
        |owner_id, ids: &[MemoryId], candidates: Vec<reliquary_insomnia::InsomniaCandidate>| {
            for (&memory_id, candidate) in ids.iter().zip(candidates) {
                let body = memory_body_id(&candidate.title, &candidate.content);
                mentions.extend(candidate.routing_mentions.into_iter().map(|span| {
                    LiveEntityInput {
                        body,
                        mention: EntityMemoryMention {
                            owner_id,
                            memory_id,
                            field: match span.field {
                                ExtractedField::Title => TextField::Title,
                                ExtractedField::Content => TextField::Content,
                            },
                            start_byte: span.start_byte,
                            end_byte: span.end_byte,
                            text: span.text,
                        },
                    }
                }));
            }
        };
    collect(owner, local, project);
    if let Some((destination, ids)) = routed {
        if !ids.is_empty() {
            if checkpoint.destination != Some(destination) || ids.len() != user.len() {
                return Err(bad());
            }
            collect(destination, ids, user);
        }
    }
    Ok(mentions)
}
