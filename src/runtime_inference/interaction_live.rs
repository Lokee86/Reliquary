//! Host-facing live sessions over the existing Gateway authority.
//! This module owns recipient state only. It never owns a Cva, journal or queue.
use super::conversation::{
    ConversationViewerToken, GatewayConversationSubmission, GatewaySubmissionReceipt,
};
use super::observation::{CheckpointContentIdentity, GenerationStatus};
use super::streams::{
    StreamDelivery, StreamKey, StreamPayload, StreamSnapshot, StreamSubscription,
};
use super::{
    GatewayRuntime, InstanceRuntime, OwnerAccessToken, OwnerGrant, ReliquaryRuntimeHostError,
};
use crate::multiplexing_journal::{ConversationId, RelOwnerId};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveRuntimeError {
    FreshSessionRequired,
    OperationUnavailable,
    Closed,
    UnsupportedDurableEvent,
}
impl std::fmt::Display for LiveRuntimeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::FreshSessionRequired => "fresh authorized session required",
            Self::OperationUnavailable => "runtime operation unavailable",
            Self::Closed => "live session is closed",
            Self::UnsupportedDurableEvent => "durable event owner support is unavailable",
        })
    }
}
impl std::error::Error for LiveRuntimeError {}
fn unavailable(_: ReliquaryRuntimeHostError) -> LiveRuntimeError {
    LiveRuntimeError::OperationUnavailable
}

/// Existing P3 status vocabulary, without protocol-specific states.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveGenerationStatus {
    Pending,
    Running,
    CancellationRequested,
    Interrupted,
    Unknown,
    Completed,
    Cancelled,
}
impl From<GenerationStatus> for LiveGenerationStatus {
    fn from(value: GenerationStatus) -> Self {
        match value {
            GenerationStatus::Pending => Self::Pending,
            GenerationStatus::Running => Self::Running,
            GenerationStatus::CancellationRequested => Self::CancellationRequested,
            GenerationStatus::Interrupted => Self::Interrupted,
            GenerationStatus::Unknown => Self::Unknown,
            GenerationStatus::Completed => Self::Completed,
            GenerationStatus::Cancelled => Self::Cancelled,
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LiveGeneration {
    pub operation_id: String,
    pub message_id: String,
    pub branch_id: String,
    pub status: LiveGenerationStatus,
    pub checkpoint_sequence: Option<u64>,
}
/// Exact authorized native-generation receipt projection; never a local log.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveToolEventKind {
    Invocation,
    Result,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LiveToolEvent {
    pub generation: LiveGeneration,
    pub sequence: u64,
    pub invocation_id: String,
    pub kind: LiveToolEventKind,
    pub name: String,
    pub content: String,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LiveGeneratedArtifact {
    pub generation: LiveGeneration,
    pub file_id: crate::FileId,
    pub content_id: crate::ContentId,
    pub filename: String,
    pub mime_type: Option<String>,
    pub byte_length: u64,
}

#[path = "interaction_live_resume.rs"]
mod resume;
/// Durable identity and position hints, never a capability or writer lease.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LiveResumeToken {
    pub owner_id: String,
    pub conversation_id: String,
    pub branch_id: String,
    pub operation_id: String,
    pub message_id: String,
    pub cursor: LiveStreamCursor,
    pub checkpoint_sequence: Option<u64>,
}

/// Stream positions are delivery hints, never acknowledgement of semantic work.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveStreamCursor {
    pub mount_epoch: u64,
    pub delivery_sequence: u64,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LiveCheckpointShape {
    Prefix,
    Delta { after_checkpoint_sequence: u64 },
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LiveCheckpoint {
    pub generation: LiveGeneration,
    pub sequence: u64,
    pub shape: LiveCheckpointShape,
    pub text: String,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LiveStreamSnapshot {
    pub cursor: LiveStreamCursor,
    pub generation: LiveGeneration,
    pub checkpoint: Option<LiveCheckpoint>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LiveStreamEvent {
    Checkpoint {
        cursor: LiveStreamCursor,
        checkpoint: LiveCheckpoint,
    },
    Status {
        cursor: LiveStreamCursor,
        generation: LiveGeneration,
    },
    ResyncRequired {
        cursor: LiveStreamCursor,
    },
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveCapability {
    NativeOnly,
    /// Host execution negotiated; Reliquary retains pending admission.
    ExecutionOnlyReady,
    Ready,
    Unavailable,
    UnknownStatusHeld,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveDiagnostics {
    pub mount_epoch: u64,
    pub workers: usize,
    pub background_enabled: bool,
    pub conversation_execution: LiveCapability,
    pub pending_submissions: usize,
    pub held_host_operations: usize,
    pub cancellation_requests: usize,
    pub resync_required: bool,
}

/// A verified instance adapter. Replacing this value creates no semantic writer.
/// Protocol negotiation belongs to the host; capability authority is read afresh.
pub struct LiveRuntimeAdapter {
    gateway: Arc<GatewayRuntime>,
    instance: Arc<InstanceRuntime>,
}
impl GatewayRuntime {
    pub fn live_runtime_adapter(
        self: &Arc<Self>,
        instance: Arc<InstanceRuntime>,
    ) -> LiveRuntimeAdapter {
        LiveRuntimeAdapter {
            gateway: Arc::clone(self),
            instance,
        }
    }
}
impl LiveRuntimeAdapter {
    pub fn quiesce_owner(&self, owner_id: &str) -> Result<(), LiveRuntimeError> {
        self.gateway
            .quiesce_rel_for(&self.instance, owner_id)
            .map_err(unavailable)
    }
    pub fn reconcile_quiesced_owner(&self, owner_id: &str) -> Result<(), LiveRuntimeError> {
        self.gateway
            .reconcile_quiesced_rel_for(&self.instance, owner_id)
            .map_err(unavailable)
    }
    pub fn reopen_owner(&self, owner_id: &str) -> Result<u64, LiveRuntimeError> {
        self.gateway
            .reopen_rel_for(&self.instance, owner_id)
            .map_err(unavailable)
    }
    /// Admit the first turn through the existing durable journal. The session
    /// identity exists at acceptance; Archive branch creation is asynchronous.
    /// Retrying the same immutable first turn never creates a second writer.
    pub fn create_session(
        &self,
        owner_id: &str,
        first_turn: GatewayConversationSubmission,
    ) -> Result<(LiveSession, GatewaySubmissionReceipt), LiveRuntimeError> {
        if first_turn.expected_parent_node_id.is_some()
            || first_turn.expected_conversation_revision != 0
        {
            return Err(LiveRuntimeError::OperationUnavailable);
        }
        let token = self
            .gateway
            .authorize(&self.instance, owner_id, OwnerGrant::ViewRel)
            .map_err(unavailable)?;
        let conversation_id = first_turn.conversation_id.clone();
        let branch_id = first_turn.branch_id.clone();
        let (viewer, receipt) = self
            .gateway
            .with_rel_distribution_admission(&token, |_| {
                let receipt =
                    self.gateway
                        .submit_conversation(&self.instance, owner_id, first_turn)?;
                let viewer = self.gateway.attach_conversation_viewer(
                    &self.instance,
                    owner_id,
                    &conversation_id,
                )?;
                Ok((viewer, receipt))
            })
            .map_err(unavailable)?;
        Ok((
            LiveSession {
                gateway: Arc::clone(&self.gateway),
                instance: Arc::clone(&self.instance),
                token,
                conversation_id,
                branch_id,
                viewer: Some(viewer),
                stream: None,
                resync_required: AtomicBool::new(false),
            },
            receipt,
        ))
    }

    /// Open an existing durable branch. Viewer lifetime does not own its work.
    pub fn open_session(
        &self,
        owner_id: &str,
        conversation_id: &str,
        branch_id: &str,
    ) -> Result<LiveSession, LiveRuntimeError> {
        let token = self
            .gateway
            .authorize(&self.instance, owner_id, OwnerGrant::ViewRel)
            .map_err(unavailable)?;
        let viewer = self
            .gateway
            .with_rel_distribution_admission(&token, |_| {
                self.gateway.select_managed_conversation(
                    &self.instance,
                    owner_id,
                    conversation_id,
                    branch_id,
                )?;
                self.gateway
                    .attach_conversation_viewer(&self.instance, owner_id, conversation_id)
            })
            .map_err(unavailable)?;
        Ok(LiveSession {
            gateway: Arc::clone(&self.gateway),
            instance: Arc::clone(&self.instance),
            token,
            conversation_id: conversation_id.to_owned(),
            branch_id: branch_id.to_owned(),
            viewer: Some(viewer),
            stream: None,
            resync_required: AtomicBool::new(false),
        })
    }
}

/// Opaque session authority retains the original mount and grant generations.
/// No mutable owner handle or transport callback crosses this boundary.
pub struct LiveSession {
    gateway: Arc<GatewayRuntime>,
    instance: Arc<InstanceRuntime>,
    token: OwnerAccessToken,
    conversation_id: String,
    branch_id: String,
    viewer: Option<ConversationViewerToken>,
    stream: Option<StreamSubscription>,
    resync_required: AtomicBool,
}
impl LiveSession {
    fn validate(&self) -> Result<(), LiveRuntimeError> {
        if self.viewer.is_none() {
            return Err(LiveRuntimeError::Closed);
        }
        self.gateway
            .with_rel_distribution_admission(&self.token, |_| Ok(()))
            .map_err(|_| LiveRuntimeError::FreshSessionRequired)
    }
    fn admitted<T>(
        &self,
        action: impl FnOnce() -> Result<T, ReliquaryRuntimeHostError>,
    ) -> Result<T, LiveRuntimeError> {
        self.validate()?;
        self.gateway
            .with_rel_distribution_admission(&self.token, |_| action())
            .map_err(unavailable)
    }
    pub fn submit_turn(
        &self,
        request: GatewayConversationSubmission,
    ) -> Result<GatewaySubmissionReceipt, LiveRuntimeError> {
        self.validate()?;
        if request.conversation_id != self.conversation_id || request.branch_id != self.branch_id {
            return Err(LiveRuntimeError::OperationUnavailable);
        }
        self.admitted(|| {
            self.gateway
                .submit_conversation(&self.instance, self.token.owner_id(), request)
        })
    }
    pub fn turn_status(
        &self,
        submission_id: &str,
    ) -> Result<GatewaySubmissionReceipt, LiveRuntimeError> {
        self.validate()?;
        self.admitted(|| {
            self.gateway.conversation_submission_status(
                &self.instance,
                self.token.owner_id(),
                &self.conversation_id,
                submission_id,
            )
        })
    }
    pub fn cancel_pending(
        &self,
        submission_id: &str,
    ) -> Result<GatewaySubmissionReceipt, LiveRuntimeError> {
        self.validate()?;
        self.admitted(|| {
            self.gateway.cancel_conversation_submission(
                &self.instance,
                self.token.owner_id(),
                &self.conversation_id,
                submission_id,
            )
        })
    }
    /// Cancellation retains the original owner and grant generation at every
    /// durable stage. Provider I/O runs after the capture lease is released.
    pub fn cancel_generation(
        &self,
        operation_id: &str,
    ) -> Result<GatewaySubmissionReceipt, LiveRuntimeError> {
        let token = self.admitted(|| {
            let token = self.gateway.authorize(
                &self.instance,
                self.token.owner_id(),
                OwnerGrant::CancelSharedGeneration,
            )?;
            if token.instance_id != self.token.instance_id
                || token.mount_epoch != self.token.mount_epoch
                || token.fence_generation != self.token.fence_generation
                || token.owner_grant_generation != self.token.owner_grant_generation
            {
                return Err(super::operation("session authority changed"));
            }
            Ok(token)
        })?;
        let receipt = self
            .gateway
            .cancel_conversation_generation_admitted(&token, &self.conversation_id, operation_id)
            .map_err(unavailable)?;
        self.validate()?;
        Ok(receipt)
    }
    /// Register only the exact generation proven by the current durable observation.
    pub fn watch_generation(
        &mut self,
        operation_id: &str,
        capacity: usize,
    ) -> Result<LiveStreamSnapshot, LiveRuntimeError> {
        self.validate()?;
        if self.stream.is_some() {
            return Err(LiveRuntimeError::OperationUnavailable);
        }
        let (snapshot, stream) = self.admitted(|| {
            let observation = self.gateway.conversation_observation_snapshot(
                &self.instance,
                self.token.owner_id(),
                &self.conversation_id,
            )?;
            let generation = observation
                .generations
                .iter()
                .find(|g| {
                    g.identity.operation_id == operation_id
                        && g.identity.branch_id == self.branch_id
                })
                .ok_or_else(|| super::operation("generation unavailable"))?;
            let key = StreamKey {
                rel_id: RelOwnerId::parse_canonical(self.token.owner_id())
                    .map_err(|_| super::operation("invalid owner identity"))?,
                mount_epoch: self.token.mount_epoch,
                conversation_id: ConversationId::new(self.conversation_id.clone())
                    .map_err(super::operation)?,
                operation_id: generation.identity.operation_id.clone(),
                generation_id: generation.identity.generation_id.clone(),
                branch_id: generation.identity.branch_id.clone(),
            };
            self.gateway
                .subscribe_authoritative_conversation_stream(&self.instance, key, capacity)
        })?;
        let result = match project_snapshot(snapshot) {
            Ok(result) => result,
            Err(error) => {
                let _ = self
                    .gateway
                    .disconnect_conversation_stream(&self.instance, &stream);
                return Err(error);
            }
        };
        self.stream = Some(stream);
        self.validate()?;
        Ok(result)
    }
    /// Always replace the recipient at an authoritative prefix before future deltas.
    /// No cursor replay or Unknown host operation dispatch occurs here.
    pub fn resume_stream(
        &mut self,
        capacity: usize,
    ) -> Result<LiveStreamSnapshot, LiveRuntimeError> {
        self.validate()?;
        let previous = self
            .stream
            .as_ref()
            .ok_or(LiveRuntimeError::OperationUnavailable)?;
        let (snapshot, replacement) = self.admitted(|| {
            self.gateway.reconnect_authoritative_conversation_stream(
                &self.instance,
                previous,
                capacity,
            )
        })?;
        let result = project_snapshot(snapshot)?;
        self.stream = Some(replacement);
        self.resync_required.store(false, Ordering::SeqCst);
        self.validate()?;
        Ok(result)
    }
    pub fn next_event(&self) -> Result<Option<LiveStreamEvent>, LiveRuntimeError> {
        self.validate()?;
        let stream = self
            .stream
            .as_ref()
            .ok_or(LiveRuntimeError::OperationUnavailable)?;
        self.gateway
            .next_conversation_stream(&self.instance, stream)
            .map_err(unavailable)
            .map(|event| {
                event.map(|delivery| match delivery {
                    StreamDelivery::ResyncRequired { key, latest_cursor } => {
                        self.resync_required.store(true, Ordering::SeqCst);
                        LiveStreamEvent::ResyncRequired {
                            cursor: LiveStreamCursor {
                                mount_epoch: key.mount_epoch,
                                delivery_sequence: latest_cursor,
                            },
                        }
                    }
                    StreamDelivery::Event(event) => {
                        let cursor = LiveStreamCursor {
                            mount_epoch: event.key.mount_epoch,
                            delivery_sequence: event.cursor,
                        };
                        match &event.payload {
                            StreamPayload::Checkpoint(content) => LiveStreamEvent::Checkpoint {
                                cursor,
                                checkpoint: project_checkpoint(content.clone()),
                            },
                            StreamPayload::Status(status) => LiveStreamEvent::Status {
                                cursor,
                                generation: project_generation(status),
                            },
                        }
                    }
                })
            })
    }
    fn native_generation(&self, operation_id: &str) -> Result<LiveGeneration, LiveRuntimeError> {
        let observation = self.admitted(|| {
            self.gateway.conversation_observation_snapshot(
                &self.instance,
                self.token.owner_id(),
                &self.conversation_id,
            )
        })?;
        observation
            .generations
            .iter()
            .find(|g| {
                g.identity.operation_id == operation_id && g.identity.branch_id == self.branch_id
            })
            .map(project_generation)
            .ok_or(LiveRuntimeError::UnsupportedDurableEvent)
    }
    /// Read synchronized Echo tool receipts in their immutable source sequence.
    /// The trusted generation owner validates invocation/result acceptance.
    pub fn tool_events(&self, operation_id: &str) -> Result<Vec<LiveToolEvent>, LiveRuntimeError> {
        let generation = self.native_generation(operation_id)?;
        let events = self.admitted(|| {
            self.gateway.echo_events_for(
                &self.instance,
                self.token.owner_id(),
                &self.conversation_id,
                &generation.message_id,
            )
        })?;
        super::conversation::validate_generation_tool_source(&events).map_err(unavailable)?;
        let mut result = Vec::new();
        for event in events {
            let kind = match event.kind {
                crate::EchoEventKind::ToolCall => LiveToolEventKind::Invocation,
                crate::EchoEventKind::ToolResult => LiveToolEventKind::Result,
                _ => continue,
            };
            result.push(LiveToolEvent {
                generation: generation.clone(),
                sequence: event.sequence,
                invocation_id: event
                    .correlation_id
                    .ok_or(LiveRuntimeError::OperationUnavailable)?,
                kind,
                name: event.name.ok_or(LiveRuntimeError::OperationUnavailable)?,
                content: event.content,
            });
        }
        self.validate()?;
        Ok(result)
    }
    /// Artifacts become visible only through the finalized assistant's exact
    /// Archive attachment receipt; source or ancestry alone is not authority.
    pub fn generated_artifacts(
        &self,
        operation_id: &str,
    ) -> Result<Vec<LiveGeneratedArtifact>, LiveRuntimeError> {
        let generation = self.native_generation(operation_id)?;
        if generation.status != LiveGenerationStatus::Completed {
            return Ok(Vec::new());
        }
        let files = self.admitted(|| {
            self.gateway.files_for_source_for(
                &self.instance,
                self.token.owner_id(),
                &self.conversation_id,
                &generation.message_id,
            )
        })?;
        self.validate()?;
        Ok(files
            .into_iter()
            .map(|file| LiveGeneratedArtifact {
                generation: generation.clone(),
                file_id: file.id,
                content_id: file.content_id,
                filename: file.filename,
                mime_type: file.mime_type,
                byte_length: file.byte_length,
            })
            .collect())
    }
    pub fn diagnostics(&self) -> Result<LiveDiagnostics, LiveRuntimeError> {
        self.validate()?;
        let (workers, background_enabled) = self
            .gateway
            .background_worker_state_for(&self.instance, self.token.owner_id())
            .map_err(unavailable)?;
        let conversation_execution = self
            .gateway
            .capability_state(super::RuntimeCapability::ConversationExecution)
            .map_err(unavailable)?;
        let conversation_execution = match conversation_execution {
            super::CapabilityState::NativeOnly => LiveCapability::NativeOnly,
            super::CapabilityState::ExecutionOnlyReady => LiveCapability::ExecutionOnlyReady,
            super::CapabilityState::Ready => LiveCapability::Ready,
            super::CapabilityState::Unavailable(_) => LiveCapability::Unavailable,
            super::CapabilityState::UnknownStatusHeld => LiveCapability::UnknownStatusHeld,
        };
        let observation = self.admitted(|| {
            self.gateway.conversation_observation_snapshot(
                &self.instance,
                self.token.owner_id(),
                &self.conversation_id,
            )
        })?;
        self.validate()?;
        Ok(LiveDiagnostics {
            mount_epoch: self.token.mount_epoch,
            workers,
            background_enabled,
            conversation_execution,
            pending_submissions: observation
                .submissions
                .iter()
                .filter(|s| s.state == crate::multiplexing_journal::SubmissionState::Pending)
                .count(),
            held_host_operations: observation
                .host_operations
                .iter()
                .filter(|h| {
                    h.phase == crate::multiplexing_journal::DurableHostOperationPhase::Unknown
                })
                .count(),
            cancellation_requests: observation
                .host_operations
                .iter()
                .filter(|h| h.cancel_requested)
                .count()
                + observation
                    .generations
                    .iter()
                    .filter(|g| g.status == GenerationStatus::CancellationRequested)
                    .count(),
            resync_required: self.resync_required.load(Ordering::SeqCst),
        })
    }
    pub fn wake_background(&self) -> Result<(), LiveRuntimeError> {
        self.admitted(|| {
            self.gateway
                .wake_rel_background_for(&self.instance, self.token.owner_id())
        })
    }
    /// Shutdown of this recipient never cancels accepted semantic work.
    pub fn close(&mut self) -> Result<(), LiveRuntimeError> {
        if self.viewer.is_none() {
            return Ok(());
        }
        if let Some(stream) = self.stream.take() {
            let _ = self
                .gateway
                .disconnect_conversation_stream(&self.instance, &stream);
        }
        let viewer = self.viewer.take().expect("checked above");
        self.gateway
            .detach_conversation_viewer(viewer)
            .map_err(unavailable)
    }
}
impl Drop for LiveSession {
    fn drop(&mut self) {
        let _ = self.close();
    }
}
fn project_generation(g: &super::observation::GenerationObservation) -> LiveGeneration {
    LiveGeneration {
        operation_id: g.identity.operation_id.clone(),
        message_id: g.identity.generation_id.clone(),
        branch_id: g.identity.branch_id.clone(),
        status: g.status.into(),
        checkpoint_sequence: g.durable_checkpoint_sequence,
    }
}
fn project_checkpoint(c: super::observation::GenerationCheckpointContent) -> LiveCheckpoint {
    LiveCheckpoint {
        generation: LiveGeneration {
            operation_id: c.observation.generation.operation_id.clone(),
            message_id: c.observation.generation.generation_id.clone(),
            branch_id: c.observation.generation.branch_id.clone(),
            status: c.observation.status.into(),
            checkpoint_sequence: Some(c.observation.durable_checkpoint_sequence),
        },
        sequence: c.observation.durable_checkpoint_sequence,
        shape: match c.observation.content {
            CheckpointContentIdentity::Prefix { .. } => LiveCheckpointShape::Prefix,
            CheckpointContentIdentity::Delta {
                after_checkpoint_sequence,
                ..
            } => LiveCheckpointShape::Delta {
                after_checkpoint_sequence,
            },
        },
        text: c.text,
    }
}
fn project_snapshot(s: StreamSnapshot) -> Result<LiveStreamSnapshot, LiveRuntimeError> {
    let generation = s
        .source
        .conversation
        .generations
        .iter()
        .find(|g| {
            g.identity.operation_id == s.key.operation_id
                && g.identity.generation_id == s.key.generation_id
                && g.identity.branch_id == s.key.branch_id
        })
        .ok_or(LiveRuntimeError::OperationUnavailable)?;
    Ok(LiveStreamSnapshot {
        cursor: LiveStreamCursor {
            mount_epoch: s.key.mount_epoch,
            delivery_sequence: s.cursor,
        },
        generation: project_generation(generation),
        checkpoint: s.source.checkpoint.map(project_checkpoint),
    })
}

#[cfg(test)]
#[path = "interaction_live_tests.rs"]
mod tests;
