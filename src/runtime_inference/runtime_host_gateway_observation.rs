//! Frozen P3/P4 observation data contract. P3 supplies authoritative durable
//! conversation facts; P4 owns cursors, subscribers and delivery.
//! These values contain no writable handles, transport state or private PHY data.
//! Producers and their synchronization are implemented by P3, not by this module.
#![allow(dead_code)] // Shared contract is consumed as the P3/P4 lanes implement it.

use crate::multiplexing_journal::{
    ConversationId, ConversationSubmissionId, DurableHostOperationPhase, RelOwnerId,
    SubmissionState,
};

// One policy vocabulary; Archive is the canonical durable owner.
pub(super) use crate::ConversationPolicy;

/// Source watermarks only. None means unavailable, never a fabricated zero clock.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct DurableWatermarks {
    pub global_version: Option<u64>,
    pub archive_version: Option<u64>,
    pub memory_version: Option<u64>,
    pub entity_version: Option<u64>,
    pub graph_version: Option<u64>,
    pub ego_mutation_version: Option<u64>,
    pub vector_version: Option<u64>,
    pub project_correlation_sequence: Option<u64>,
    pub conversation_journal_sequence: Option<u64>,
}

/// Exactly the journal's ordering tuple; client time is deliberately absent.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub(super) struct SubmissionOrder {
    pub verified_receive_ns: i64,
    pub tie_key: [u8; 16],
    pub admission_sequence: u64,
}

/// Safe queue projection, never Submission or its request/body/attachments.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct SubmissionObservation {
    pub id: ConversationSubmissionId,
    pub branch_id: String,
    pub state: SubmissionState,
    pub order: SubmissionOrder,
    pub transition_revision: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct BranchObservation {
    pub branch_id: String,
    pub head_node_id: Option<String>,
    pub fork_parent_node_id: Option<String>,
    pub display_label: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum GenerationStatus {
    Pending,
    Running,
    CancellationRequested,
    Interrupted,
    Unknown,
    Completed,
    Cancelled,
}

/// Stable branch-local continuation identity, independent of viewer connections.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct GenerationIdentity {
    pub operation_id: String,
    pub generation_id: String,
    pub branch_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct GenerationObservation {
    pub identity: GenerationIdentity,
    pub status: GenerationStatus,
    pub transition_revision: u64,
    /// None when no checkpoint has been durably synchronized.
    pub durable_checkpoint_sequence: Option<u64>,
}

/// Journal-owned host work status, without payload digests or request bodies.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct HostOperationObservation {
    pub submission_id: ConversationSubmissionId,
    pub operation_id: String,
    pub capability: super::RuntimeCapability,
    pub phase: DurableHostOperationPhase,
    pub transition_revision: u64,
    pub cancel_requested: bool,
}

/// P3-owned snapshot. Selected branch is a read projection supplied for the
/// requesting instance, never a conversation-global selection authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ConversationObservationSnapshot {
    pub rel_id: RelOwnerId,
    pub mount_epoch: u64,
    pub conversation_id: ConversationId,
    pub selected_branch_id: Option<String>,
    pub selected_head_node_id: Option<String>,
    pub policy: ConversationPolicy,
    pub policy_revision: u64,
    pub branches: Vec<BranchObservation>,
    pub submissions: Vec<SubmissionObservation>,
    /// Includes independent branches and interrupted/terminal operations.
    pub generations: Vec<GenerationObservation>,
    pub host_operations: Vec<HostOperationObservation>,
    pub durable_journal_sequence: u64,
}

/// Broad REL snapshot has no instance selection, checkpoint identity or text.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct RelConversationSummary {
    pub conversation_id: ConversationId,
    pub policy: ConversationPolicy,
    pub policy_revision: u64,
    pub branches: Vec<BranchObservation>,
    pub submissions: Vec<SubmissionObservation>,
    pub generations: Vec<GenerationObservation>,
    pub host_operations: Vec<HostOperationObservation>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct RelObservationSummary {
    pub rel_id: RelOwnerId,
    pub mount_epoch: u64,
    pub conversations: Vec<RelConversationSummary>,
    pub durable_watermarks: DurableWatermarks,
}

/// Named post-durability facts. These are not event envelopes and have no P4
/// cursor. BranchCreated follows durable capture and branch publication; text never enters
/// this broad-summary contract.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum ConversationTransition {
    SubmissionChanged(SubmissionObservation),
    FinalizeWon {
        submission_id: ConversationSubmissionId,
        branch: BranchObservation,
        policy_revision: u64,
    },
    ArchiveCommitted {
        submission_id: ConversationSubmissionId,
        branch: BranchObservation,
        node_id: String,
    },
    PostCommitRecorded {
        submission_id: ConversationSubmissionId,
    },
    BranchCreated(BranchObservation),
    PolicyChanged {
        policy: ConversationPolicy,
        revision: u64,
    },
    GenerationChanged(GenerationObservation),
    HostOperationChanged(HostOperationObservation),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ConversationDurableDelta {
    pub rel_id: RelOwnerId,
    pub mount_epoch: u64,
    pub conversation_id: ConversationId,
    pub durable_watermarks: DurableWatermarks,
    pub transition: ConversationTransition,
}

/// Exact synchronized content identity; prefix/delta sequences are durable P3
/// checkpoint positions, not P4 delivery cursors.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum CheckpointContentIdentity {
    Prefix {
        utf8_bytes: u64,
        sha256: [u8; 32],
    },
    Delta {
        after_checkpoint_sequence: u64,
        utf8_bytes: u64,
        sha256: [u8; 32],
    },
}

/// Stream-scoped observation, deliberately excluded from REL summaries.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct GenerationCheckpointObservation {
    pub rel_id: RelOwnerId,
    pub mount_epoch: u64,
    pub conversation_id: ConversationId,
    pub generation: GenerationIdentity,
    pub durable_checkpoint_sequence: u64,
    pub status: GenerationStatus,
    pub content: CheckpointContentIdentity,
}

/// Text is supplied only to the separately authorized exact stream adapter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct GenerationCheckpointContent {
    pub observation: GenerationCheckpointObservation,
    pub text: String,
}
