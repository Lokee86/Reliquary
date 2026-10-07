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
use std::sync::Arc;

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
            viewer: Some(viewer),
            stream: None,
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
    viewer: Option<ConversationViewerToken>,
    stream: Option<StreamSubscription>,
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
        if request.conversation_id != self.conversation_id {
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
    /// Native interruption retains the original epoch. External cancellation
    /// requires a token-bound owner seam before this adapter can expose it.
    pub fn cancel_generation(
        &self,
        operation_id: &str,
    ) -> Result<GatewaySubmissionReceipt, LiveRuntimeError> {
        self.validate()?;
        let native = self.admitted(|| {
            let observation = self.gateway.conversation_observation_snapshot(
                &self.instance,
                self.token.owner_id(),
                &self.conversation_id,
            )?;
            if !observation
                .generations
                .iter()
                .any(|g| g.identity.operation_id == operation_id)
            {
                return Ok(None);
            }
            self.gateway
                .cancel_conversation_generation(
                    &self.instance,
                    self.token.owner_id(),
                    &self.conversation_id,
                    operation_id,
                )
                .map(Some)
        })?;
        native.ok_or(LiveRuntimeError::UnsupportedDurableEvent)
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
        let observation = self
            .gateway
            .conversation_observation_snapshot(
                &self.instance,
                self.token.owner_id(),
                &self.conversation_id,
            )
            .map_err(unavailable)?;
        let generation = observation
            .generations
            .iter()
            .find(|g| g.identity.operation_id == operation_id)
            .ok_or(LiveRuntimeError::OperationUnavailable)?;
        let key = StreamKey {
            rel_id: RelOwnerId::parse_canonical(self.token.owner_id())
                .map_err(|_| LiveRuntimeError::OperationUnavailable)?,
            mount_epoch: self.token.mount_epoch,
            conversation_id: ConversationId::new(self.conversation_id.clone())
                .map_err(|_| LiveRuntimeError::OperationUnavailable)?,
            operation_id: generation.identity.operation_id.clone(),
            generation_id: generation.identity.generation_id.clone(),
            branch_id: generation.identity.branch_id.clone(),
        };
        let (snapshot, stream) = self
            .gateway
            .subscribe_authoritative_conversation_stream(&self.instance, key, capacity)
            .map_err(unavailable)?;
        let result = project_snapshot(snapshot)?;
        self.stream = Some(stream);
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
        let (snapshot, replacement) = self
            .gateway
            .reconnect_authoritative_conversation_stream(&self.instance, previous, capacity)
            .map_err(unavailable)?;
        let result = project_snapshot(snapshot)?;
        self.stream = Some(replacement);
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
            super::CapabilityState::Ready => LiveCapability::Ready,
            super::CapabilityState::Unavailable(_) => LiveCapability::Unavailable,
            super::CapabilityState::UnknownStatusHeld => LiveCapability::UnknownStatusHeld,
        };
        self.validate()?;
        Ok(LiveDiagnostics {
            mount_epoch: self.token.mount_epoch,
            workers,
            background_enabled,
            conversation_execution,
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
