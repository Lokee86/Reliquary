//! Owner-bound operational queue groundwork with its own versioned framing.
//!
//! This journal is deliberately separate from REL semantic data. It records
//! accepted provisional submissions and the winner of cancel/finalize races.
//! Only the trusted Instance finalization path records Archive commitment, after
//! the mounted REL writer syncs the turn and branch. Host status recorders remain
//! private. Provider dispatch, recipient streaming, and reconnect transport are
//! not implemented; checkpoint reads are bounded local replay only, so H03 is
//! not complete.

use crate::OwnerId;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, VecDeque},
    fmt,
    fs::{self, File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};

// Deliberately distinct from the reference RQMUXJ01: this reduced groundwork
// does not implement the reference's complete event/snapshot payload schema.
const JOURNAL_MAGIC: &[u8; 8] = b"RQGQ0001";
const HEADER_BYTES: u64 = 56;
const CHECKSUM_BYTES: usize = 32;
const FRAME_LIMIT: usize = 16 * 1024 * 1024;
const JOURNAL_LIMIT: u64 = 1024 * 1024 * 1024;
const MAX_CHECKPOINT_TEXT_BYTES: usize = 1024 * 1024;
const MAX_REPLAY_CHECKPOINTS: usize = 64;
const MAX_REPLAY_BYTES: usize = 4 * 1024 * 1024;

mod queue_types;
pub use queue_types::*;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct Entry {
    submission: QueuedSubmission,
    state: QueueState,
    admission_sequence: u64,
    transition_revision: u64,
    finalize_capture: Option<FinalizeCapture>,
    archive_receipt: Option<ArchiveReceipt>,
    host_operations: BTreeMap<RuntimeCapability, DurableHostOperation>,
    generation: Option<DurableGeneration>,
    checkpoint_history: VecDeque<DurableGenerationCheckpoint>,
    generation_event_history: VecDeque<GenerationStreamRecord>,
}
impl Entry {
    fn status(&self) -> SubmissionStatus {
        SubmissionStatus {
            state: self.state,
            admission_sequence: self.admission_sequence,
            transition_revision: self.transition_revision,
            finalize_capture: self.finalize_capture.clone(),
            archive_receipt: self.archive_receipt.clone(),
            host_operations: self.host_operations.clone(),
            generation: self.generation.clone(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct Projection {
    owner: [u8; 16],
    sequence: u64,
    conversation_revisions: BTreeMap<String, u64>,
    entries: BTreeMap<ConversationSubmissionId, Entry>,
}
impl Projection {
    fn empty(owner: OwnerId) -> Self {
        Self {
            owner: owner.as_bytes(),
            sequence: 0,
            conversation_revisions: BTreeMap::new(),
            entries: BTreeMap::new(),
        }
    }
    fn revision(&self, conversation_id: &str) -> u64 {
        self.conversation_revisions
            .get(conversation_id)
            .copied()
            .unwrap_or(0)
    }
    fn apply(&mut self, frame: &Frame) -> Result<(), QueueError> {
        if frame.sequence
            != self
                .sequence
                .checked_add(1)
                .ok_or(QueueError::SequenceOverflow)?
        {
            return Err(QueueError::Corrupt("sequence gap"));
        }
        match &frame.event {
            Event::Enqueued { submission } => {
                validate_id(&submission.id.conversation_id, "conversation id")?;
                validate_id(submission.id.submission_id.as_str(), "submission id")?;
                validate_id(&submission.principal_id, "principal id")?;
                let payload_hash: [u8; 32] = Sha256::digest(submission.content.as_bytes()).into();
                if payload_hash != submission.payload_hash {
                    return Err(QueueError::Corrupt("submission payload hash mismatch"));
                }
                if self.entries.contains_key(&submission.id) {
                    return Err(QueueError::Corrupt("duplicate enqueue"));
                }
                self.entries.insert(
                    submission.id.clone(),
                    Entry {
                        submission: submission.clone(),
                        state: QueueState::Pending,
                        admission_sequence: frame.sequence,
                        transition_revision: 1,
                        finalize_capture: None,
                        archive_receipt: None,
                        host_operations: BTreeMap::new(),
                        generation: None,
                        checkpoint_history: VecDeque::new(),
                        generation_event_history: VecDeque::new(),
                    },
                );
            }
            Event::Cancelled {
                id,
                expected_revision,
            } => {
                if self.revision(&id.conversation_id) != *expected_revision {
                    return Err(QueueError::Corrupt("stale cancel revision"));
                }
                let entry = self
                    .entries
                    .get_mut(id)
                    .ok_or(QueueError::Corrupt("missing cancel target"))?;
                if entry.state != QueueState::Pending {
                    return Err(QueueError::Corrupt("cancel lost state"));
                }
                if !entry
                    .host_operations
                    .values()
                    .all(|op| op.phase == DurableHostOperationPhase::Cancelled)
                    || entry.generation.as_ref().is_some_and(|generation| {
                        generation.phase != DurableGenerationPhase::Cancelled
                    })
                {
                    return Err(QueueError::Corrupt("cancel before host cancellation"));
                }
                entry.state = QueueState::Cancelled;
                entry.transition_revision += 1;
            }
            Event::FinalizeWon {
                id,
                expected_revision,
                capture,
            } => {
                if self.revision(&id.conversation_id) != *expected_revision {
                    return Err(QueueError::Corrupt("stale finalize revision"));
                }
                let entry = self
                    .entries
                    .get_mut(id)
                    .ok_or(QueueError::Corrupt("missing finalize target"))?;
                if entry.state != QueueState::Pending
                    || capture.verified_principal_id != entry.submission.principal_id
                    || !entry
                        .host_operations
                        .values()
                        .all(|op| op.phase == DurableHostOperationPhase::Completed)
                    || entry.generation.as_ref().is_some_and(|generation| {
                        generation.phase != DurableGenerationPhase::Completed
                    })
                {
                    return Err(QueueError::Corrupt("invalid finalize claim"));
                }
                entry.state = QueueState::Finalizing;
                entry.finalize_capture = Some(capture.clone());
                entry.transition_revision += 1;
            }
            Event::ArchiveCommitted { id, receipt } => {
                let entry = self
                    .entries
                    .get_mut(id)
                    .ok_or(QueueError::Corrupt("missing commit target"))?;
                validate_receipt(entry, receipt)?;
                if entry.state != QueueState::Finalizing {
                    return Err(QueueError::Corrupt("commit before finalize"));
                }
                entry.state = QueueState::Committed;
                entry.archive_receipt = Some(receipt.clone());
                entry.transition_revision += 1;
            }
            Event::HostOperationUpdated {
                id,
                capability,
                operation,
            } => {
                let entry = self
                    .entries
                    .get_mut(id)
                    .ok_or(QueueError::Corrupt("missing host operation target"))?;
                if entry.state != QueueState::Pending {
                    return Err(QueueError::Corrupt(
                        "host operation after native finalization",
                    ));
                }
                validate_host_operation_transition(
                    entry.host_operations.get(capability),
                    operation,
                )?;
                entry.host_operations.insert(*capability, operation.clone());
            }
            Event::GenerationUpdated { id, generation } => {
                let entry = self
                    .entries
                    .get_mut(id)
                    .ok_or(QueueError::Corrupt("missing generation target"))?;
                let new_checkpoint =
                    validate_generation_update(entry.generation.as_ref(), generation)?;
                if let Some(checkpoint) = new_checkpoint.clone() {
                    entry.checkpoint_history.push_back(checkpoint);
                    while entry.checkpoint_history.len() > MAX_REPLAY_CHECKPOINTS
                        || checkpoint_history_bytes(&entry.checkpoint_history) > MAX_REPLAY_BYTES
                    {
                        entry.checkpoint_history.pop_front();
                    }
                }
                if entry.generation.is_some() {
                    entry
                        .generation_event_history
                        .push_back(GenerationStreamRecord {
                            cursor: generation.stream_cursor,
                            phase: generation.phase,
                            checkpoint: new_checkpoint,
                        });
                    while entry.generation_event_history.len() > MAX_REPLAY_CHECKPOINTS
                        || generation_event_history_bytes(&entry.generation_event_history)
                            > MAX_REPLAY_BYTES
                    {
                        entry.generation_event_history.pop_front();
                    }
                }
                entry.generation = Some(generation.clone());
            }
        }
        if !matches!(
            frame.event,
            Event::HostOperationUpdated { .. } | Event::GenerationUpdated { .. }
        ) {
            let conversation_id = frame.event.id().conversation_id.clone();
            *self
                .conversation_revisions
                .entry(conversation_id)
                .or_default() += 1;
        }
        self.sequence = frame.sequence;
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct Frame {
    sequence: u64,
    event: Event,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
enum Event {
    Enqueued {
        submission: QueuedSubmission,
    },
    Cancelled {
        id: ConversationSubmissionId,
        expected_revision: u64,
    },
    FinalizeWon {
        id: ConversationSubmissionId,
        expected_revision: u64,
        capture: FinalizeCapture,
    },
    ArchiveCommitted {
        id: ConversationSubmissionId,
        receipt: ArchiveReceipt,
    },
    HostOperationUpdated {
        id: ConversationSubmissionId,
        capability: RuntimeCapability,
        operation: DurableHostOperation,
    },
    GenerationUpdated {
        id: ConversationSubmissionId,
        generation: DurableGeneration,
    },
}
impl Event {
    fn id(&self) -> &ConversationSubmissionId {
        match self {
            Self::Enqueued { submission } => &submission.id,
            Self::Cancelled { id, .. }
            | Self::FinalizeWon { id, .. }
            | Self::ArchiveCommitted { id, .. }
            | Self::HostOperationUpdated { id, .. }
            | Self::GenerationUpdated { id, .. } => id,
        }
    }
}

/// Proof created only inside the trusted Archive publication integration.
/// No public constructor is provided; only the gateway's Archive publication
/// path can mint this after the REL writer has synced the turn and branch.
pub(crate) struct ArchiveCommitProof(ArchiveReceipt);
impl ArchiveCommitProof {
    pub(super) fn from_archive(receipt: ArchiveReceipt) -> Self {
        Self(receipt)
    }
}

/// One exclusive, REL-owner-qualified durable provisional conversation queue.
pub struct ConversationQueue {
    owner: OwnerId,
    path: PathBuf,
    file: File,
    projection: Projection,
    failed: bool,
    writer_lock: Option<File>,
}
mod validation;
use validation::*;

#[cfg(test)]
mod tests;

mod queue_generation;
mod queue_host;
mod queue_io;
mod queue_native;
mod queue_reads;
