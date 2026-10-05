//! Validate frozen source identities and synchronized UTF-8 content, not P3 policy.
use super::super::observation::CheckpointContentIdentity;
use super::*;
use sha2::{Digest, Sha256};

pub(super) fn checkpoint(
    key: &StreamKey,
    content: &GenerationCheckpointContent,
) -> Result<(), ReliquaryRuntimeHostError> {
    let o = &content.observation;
    if o.rel_id != key.rel_id
        || o.mount_epoch != key.mount_epoch
        || o.conversation_id != key.conversation_id
        || !key.matches(&o.generation)
        || o.durable_checkpoint_sequence == 0
        || content.text.len() > MAX_TEXT_BYTES
    {
        return Err(operation(
            "checkpoint is outside the exact durable stream scope/bounds",
        ));
    }
    let (bytes, digest) = match &o.content {
        CheckpointContentIdentity::Prefix { utf8_bytes, sha256 } => (*utf8_bytes, sha256),
        CheckpointContentIdentity::Delta {
            after_checkpoint_sequence,
            utf8_bytes,
            sha256,
        } => {
            if *after_checkpoint_sequence >= o.durable_checkpoint_sequence {
                return Err(operation(
                    "checkpoint predecessor must precede its durable sequence",
                ));
            }
            (*utf8_bytes, sha256)
        }
    };
    let actual: [u8; 32] = Sha256::digest(content.text.as_bytes()).into();
    if bytes != content.text.len() as u64 || digest != &actual {
        return Err(operation(
            "checkpoint text does not match synchronized content identity",
        ));
    }
    Ok(())
}

pub(super) fn snapshot<'a>(
    key: &StreamKey,
    source: &'a StreamSourceSnapshot,
) -> Result<&'a GenerationObservation, ReliquaryRuntimeHostError> {
    let c = &source.conversation;
    if c.rel_id != key.rel_id
        || c.mount_epoch != key.mount_epoch
        || c.conversation_id != key.conversation_id
        || [
            c.branches.len(),
            c.submissions.len(),
            c.generations.len(),
            c.host_operations.len(),
        ]
        .into_iter()
        .any(|n| n > 16_384)
        || c.branches
            .iter()
            .filter(|b| b.branch_id == key.branch_id)
            .count()
            != 1
    {
        return Err(operation(
            "stream snapshot does not prove exact conversation/branch scope",
        ));
    }
    let mut matches = c.generations.iter().filter(|g| key.matches(&g.identity));
    let generation = matches
        .next()
        .ok_or_else(|| operation("generation is absent from authoritative snapshot"))?;
    if matches.next().is_some() {
        return Err(operation(
            "duplicate generation identity in stream snapshot",
        ));
    }
    match (&source.checkpoint, generation.durable_checkpoint_sequence) {
        (None, None) => {}
        (Some(content), Some(sequence)) => {
            checkpoint(key, content)?;
            if content.observation.durable_checkpoint_sequence != sequence
                || content.observation.status != generation.status
                || !matches!(
                    content.observation.content,
                    CheckpointContentIdentity::Prefix { .. }
                )
            {
                return Err(operation(
                    "late join requires the exact current durable full prefix",
                ));
            }
        }
        _ => {
            return Err(operation(
                "snapshot checkpoint is unavailable or inconsistent",
            ));
        }
    }
    Ok(generation)
}
