use super::*;

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub struct SubmissionId(String);
impl SubmissionId {
    pub fn new(value: impl Into<String>) -> Result<Self, QueueError> {
        let value = value.into();
        validate_id(&value, "submission id")?;
        Ok(Self(value))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub struct ConversationSubmissionId {
    pub conversation_id: String,
    pub submission_id: SubmissionId,
}
impl ConversationSubmissionId {
    pub fn new(
        conversation_id: impl Into<String>,
        submission_id: SubmissionId,
    ) -> Result<Self, QueueError> {
        let conversation_id = conversation_id.into();
        validate_id(&conversation_id, "conversation id")?;
        Ok(Self {
            conversation_id,
            submission_id,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct QueuedSubmission {
    pub id: ConversationSubmissionId,
    pub principal_id: String,
    pub content: String,
    pub payload_hash: [u8; 32],
}
impl QueuedSubmission {
    pub fn new(
        id: ConversationSubmissionId,
        principal_id: impl Into<String>,
        content: impl Into<String>,
    ) -> Result<Self, QueueError> {
        let principal_id = principal_id.into();
        validate_id(&principal_id, "principal id")?;
        let content = content.into();
        let payload_hash = Sha256::digest(content.as_bytes()).into();
        Ok(Self {
            id,
            principal_id,
            content,
            payload_hash,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct FinalizeCapture {
    pub branch_id: String,
    pub parent_node_id: Option<String>,
    pub committed_timestamp_ns: i64,
    pub verified_principal_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ArchiveReceipt {
    pub node_id: String,
    pub branch_id: String,
    pub parent_node_id: Option<String>,
    pub principal_id: String,
    pub committed_timestamp_ns: i64,
    pub payload_hash: [u8; 32],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum QueueState {
    Pending,
    Cancelled,
    Finalizing,
    Committed,
}

/// A host execution stage whose operation journal is attached to one submission.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub enum RuntimeCapability {
    ConversationExecution,
    Insomnia,
    Dream,
    Embedding,
    ProjectEnvironment,
    MemoryDelivery,
}
impl RuntimeCapability {
    pub const ALL: [Self; 6] = [
        Self::ConversationExecution,
        Self::Insomnia,
        Self::Dream,
        Self::Embedding,
        Self::ProjectEnvironment,
        Self::MemoryDelivery,
    ];
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum HostOperationStatus {
    Pending,
    Running,
    Completed,
    Cancelled,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum HostCancelOutcome {
    CancelledBeforeStart,
    RunningCancellationRequested,
    AlreadyTerminal(HostOperationStatus),
    Unknown,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum DurableHostOperationPhase {
    HandoffIntent,
    Accepted,
    Pending,
    Running,
    CancelIntent,
    RunningCancellationRequested,
    Held,
    Cancelled,
    Completed,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DurableHostOperation {
    pub stable_id: String,
    pub payload_digest: [u8; 32],
    pub phase: DurableHostOperationPhase,
    pub transition_revision: u64,
    pub cancel_requested: bool,
    /// Last authoritative non-Unknown status, retained while phase is Held.
    pub last_known_phase: Option<DurableHostOperationPhase>,
}
impl DurableHostOperation {
    pub fn held(&self) -> bool {
        self.phase == DurableHostOperationPhase::Held
            || self.phase == DurableHostOperationPhase::HandoffIntent
    }
    pub fn requires_reconciliation(&self) -> bool {
        self.held()
            || matches!(
                self.phase,
                DurableHostOperationPhase::Accepted
                    | DurableHostOperationPhase::Pending
                    | DurableHostOperationPhase::Running
                    | DurableHostOperationPhase::CancelIntent
                    | DurableHostOperationPhase::RunningCancellationRequested
            )
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct HandoffReceipt {
    pub stable_id: String,
    pub payload_digest: [u8; 32],
    pub durably_accepted: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SubmissionStatus {
    pub state: QueueState,
    pub admission_sequence: u64,
    pub transition_revision: u64,
    pub finalize_capture: Option<FinalizeCapture>,
    pub archive_receipt: Option<ArchiveReceipt>,
    pub host_operations: BTreeMap<RuntimeCapability, DurableHostOperation>,
    pub generation: Option<DurableGeneration>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum DurableGenerationPhase {
    Pending,
    Running,
    CancellationRequested,
    Interrupted,
    Held,
    Unknown,
    Completed,
    Cancelled,
}
impl DurableGenerationPhase {
    pub fn terminal(self) -> bool {
        matches!(self, Self::Interrupted | Self::Completed | Self::Cancelled)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DurableGenerationCheckpoint {
    pub sequence: u64,
    /// Per-generation recipient-stream watermark, independent from the journal
    /// frame sequence and from checkpoint sequence.
    pub delivery_cursor: u64,
    pub utf8_bytes: u64,
    pub sha256: [u8; 32],
    /// The bounded full prefix is included in the synced queue event so replay
    /// can never expose content whose checkpoint metadata has not been synced.
    pub content_prefix: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DurableGeneration {
    pub operation_id: String,
    pub generation_id: String,
    pub branch_id: String,
    pub parent_node_id: String,
    pub started_timestamp_ns: i64,
    pub phase: DurableGenerationPhase,
    pub transition_revision: u64,
    /// Per-generation stream position for checkpoint and status delivery.
    #[serde(default)]
    pub stream_cursor: u64,
    pub checkpoint: Option<DurableGenerationCheckpoint>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub(crate) struct GenerationStreamRecord {
    pub cursor: u64,
    pub phase: DurableGenerationPhase,
    pub checkpoint: Option<DurableGenerationCheckpoint>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum GenerationEventReplay {
    Events {
        latest_cursor: u64,
        items: Vec<GenerationStreamRecord>,
    },
    ResyncRequired {
        latest_cursor: u64,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GenerationReplay {
    Checkpoints {
        latest_cursor: u64,
        items: Vec<DurableGenerationCheckpoint>,
    },
    ResyncRequired {
        latest_cursor: u64,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EnqueueOutcome {
    Durable(SubmissionStatus),
    Idempotent(SubmissionStatus),
}

#[derive(Debug)]
pub enum QueueError {
    Io(std::io::Error),
    Serialization(serde_json::Error),
    InvalidIdentifier(&'static str),
    OwnerMismatch,
    Corrupt(&'static str),
    DuplicatePayloadConflict,
    MissingSubmission,
    InvalidState,
    StaleRevision { expected: u64, actual: u64 },
    ArchiveIdentityConflict,
    HostOperationConflict,
    InvalidReplayCursor,
    InvalidReplayBound,
    SequenceOverflow,
    JournalFull,
    RecoveryRequired,
    UnsupportedDurability,
}
impl fmt::Display for QueueError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "conversation queue I/O error: {error}"),
            Self::Serialization(error) => write!(f, "conversation queue encoding error: {error}"),
            Self::InvalidIdentifier(kind) => write!(f, "invalid {kind}"),
            Self::OwnerMismatch => {
                f.write_str("conversation queue belongs to a different REL owner")
            }
            Self::Corrupt(reason) => write!(f, "conversation queue corruption: {reason}"),
            Self::DuplicatePayloadConflict => {
                f.write_str("submission identity conflicts with stored payload")
            }
            Self::MissingSubmission => f.write_str("submission is not in the queue"),
            Self::InvalidState => {
                f.write_str("submission is not in a state that permits this transition")
            }
            Self::StaleRevision { expected, actual } => write!(
                f,
                "stale conversation revision: expected {expected}, actual {actual}"
            ),
            Self::ArchiveIdentityConflict => {
                f.write_str("Archive receipt does not match the captured submission")
            }
            Self::HostOperationConflict => {
                f.write_str("host operation identity or durable receipt conflicts")
            }
            Self::InvalidReplayCursor => {
                f.write_str("generation replay cursor is outside the durable checkpoint range")
            }
            Self::InvalidReplayBound => {
                f.write_str("generation replay bound is outside the supported limits")
            }
            Self::SequenceOverflow => f.write_str("conversation queue sequence overflow"),
            Self::JournalFull => f.write_str("conversation queue reached its size limit"),
            Self::RecoveryRequired => {
                f.write_str("conversation queue is fenced after a failed durable append")
            }
            Self::UnsupportedDurability => {
                f.write_str("required conversation queue durability is unavailable")
            }
        }
    }
}
impl std::error::Error for QueueError {}
impl From<std::io::Error> for QueueError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}
impl From<serde_json::Error> for QueueError {
    fn from(error: serde_json::Error) -> Self {
        Self::Serialization(error)
    }
}
