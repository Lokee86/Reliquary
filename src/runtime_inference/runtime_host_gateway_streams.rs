//! Exact durable conversation streams. P3 supplies source facts; P4 owns delivery.
//! The provider is an internal, short owner-read seam, never network/model I/O.
//! Production provider binding and post-sync call sites belong to P4.7.
#![allow(dead_code)]

use super::observation::{
    ConversationObservationSnapshot, GenerationCheckpointContent, GenerationIdentity,
    GenerationObservation, GenerationStatus,
};
use super::rel_journal_owner_id;
use super::{
    GatewayRuntime, InstanceRuntime, OwnerAccessToken, OwnerGrant, ReliquaryRuntimeHostError,
    operation,
};
use crate::multiplexing_journal::{ConversationId, RelOwnerId};
use std::sync::Arc;
use uuid::Uuid;

#[path = "runtime_host_gateway_stream_registry.rs"]
mod registry;
#[path = "runtime_host_gateway_stream_validation.rs"]
mod validation;
pub(super) use registry::StreamHub;

pub(super) const MAX_STREAMS: usize = 256;
pub(super) const MAX_RECIPIENTS: usize = 1024;
pub(super) const MAX_QUEUE: usize = 128;
pub(super) const MAX_TEXT_BYTES: usize = 1024 * 1024;

/// Exact explicit scope. Selecting another conversation never retargets a handle.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub(super) struct StreamKey {
    pub rel_id: RelOwnerId,
    pub mount_epoch: u64,
    pub conversation_id: ConversationId,
    pub operation_id: String,
    pub generation_id: String,
    pub branch_id: String,
}
impl StreamKey {
    fn matches(&self, identity: &GenerationIdentity) -> bool {
        self.operation_id == identity.operation_id
            && self.generation_id == identity.generation_id
            && self.branch_id == identity.branch_id
    }
    fn validate_shape(&self) -> Result<(), ReliquaryRuntimeHostError> {
        if self.mount_epoch == 0
            || self.conversation_id.as_str().len() > 512
            || [&self.operation_id, &self.generation_id, &self.branch_id]
                .iter()
                .any(|id| id.is_empty() || id.len() > 512 || id.chars().any(char::is_control))
        {
            return Err(operation("stream identity is outside its bounded scope"));
        }
        Ok(())
    }
    fn validate_token(&self, token: &OwnerAccessToken) -> Result<(), ReliquaryRuntimeHostError> {
        self.validate_shape()?;
        if token.kind != super::OwnerKind::Rel
            || token.grant != OwnerGrant::ViewRel
            || self.mount_epoch != token.mount_epoch
            || self.rel_id != rel_journal_owner_id(&token.owner_id)?
            || [&self.operation_id, &self.generation_id, &self.branch_id]
                .iter()
                .any(|id| id.is_empty() || id.len() > 512 || id.chars().any(char::is_control))
        {
            return Err(operation(
                "stream scope does not match an authorized REL epoch",
            ));
        }
        Ok(())
    }
}

/// P3 reconstructs a full durable prefix. Conversation metadata remains body-free.
#[derive(Clone)]
pub(super) struct StreamSourceSnapshot {
    pub conversation: ConversationObservationSnapshot,
    pub checkpoint: Option<GenerationCheckpointContent>,
}
pub(super) trait ConversationStreamProvider {
    /// Called under attachment/owner admission and the mounted publication gate.
    /// Producer locks must follow gate -> coordinator -> source, with no I/O wait.
    fn snapshot(&self, key: &StreamKey) -> Result<StreamSourceSnapshot, ReliquaryRuntimeHostError>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub(super) struct StreamSubscriptionId(Uuid);
pub(super) struct StreamSubscription {
    id: StreamSubscriptionId,
    token: OwnerAccessToken,
    key: StreamKey,
}
pub(super) struct StreamSnapshot {
    pub key: StreamKey,
    /// Delivery position only, independent of journal/checkpoint/Archive clocks.
    pub cursor: u64,
    pub source: StreamSourceSnapshot,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum StreamPayload {
    Checkpoint(GenerationCheckpointContent),
    Status(GenerationObservation),
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct StreamEvent {
    pub key: StreamKey,
    pub cursor: u64,
    pub payload: StreamPayload,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum StreamDelivery {
    Event(Arc<StreamEvent>),
    ResyncRequired { key: StreamKey, latest_cursor: u64 },
}

impl GatewayRuntime {
    pub(super) fn join_conversation_stream(
        &self,
        instance: &InstanceRuntime,
        key: StreamKey,
        capacity: usize,
        provider: &impl ConversationStreamProvider,
    ) -> Result<(StreamSnapshot, StreamSubscription), ReliquaryRuntimeHostError> {
        let token = self.authorize(
            instance,
            &format!("rel-{}", key.rel_id.as_uuid()),
            OwnerGrant::ViewRel,
        )?;
        self.join_stream_bound(&token, key, capacity, None, provider)
    }

    /// Recovery replaces only this recipient after a fresh authoritative prefix.
    /// No acknowledgement or cursor replay can make an old token current.
    pub(super) fn reconnect_conversation_stream(
        &self,
        instance: &InstanceRuntime,
        previous: &StreamSubscription,
        capacity: usize,
        provider: &impl ConversationStreamProvider,
    ) -> Result<(StreamSnapshot, StreamSubscription), ReliquaryRuntimeHostError> {
        if instance.instance_id() != previous.token.instance_id {
            return Err(operation("stream belongs to another attachment"));
        }
        self.join_stream_bound(
            &previous.token,
            previous.key.clone(),
            capacity,
            Some(previous.id),
            provider,
        )
    }

    fn join_stream_bound(
        &self,
        token: &OwnerAccessToken,
        key: StreamKey,
        capacity: usize,
        previous: Option<StreamSubscriptionId>,
        provider: &impl ConversationStreamProvider,
    ) -> Result<(StreamSnapshot, StreamSubscription), ReliquaryRuntimeHostError> {
        key.validate_token(token)?;
        if capacity == 0 || capacity > MAX_QUEUE {
            return Err(operation(
                "stream queue capacity is outside its bounded limits",
            ));
        }
        self.with_rel_distribution_admission(token, |sequencer| {
            let mut gate = sequencer.gate(token.mount_epoch)?;
            let source = provider.snapshot(&key)?;
            let generation = validation::snapshot(&key, &source)?;
            self.validate_rel_distribution_token(token)?;
            let (id, cursor) =
                gate.streams()
                    .register(&key, token.clone(), capacity, previous, generation)?;
            Ok((
                StreamSnapshot {
                    key: key.clone(),
                    cursor,
                    source,
                },
                StreamSubscription {
                    id,
                    token: token.clone(),
                    key,
                },
            ))
        })
    }

    pub(super) fn next_conversation_stream(
        &self,
        instance: &InstanceRuntime,
        subscription: &StreamSubscription,
    ) -> Result<Option<StreamDelivery>, ReliquaryRuntimeHostError> {
        self.with_stream_recipient(instance, subscription, |hub| {
            hub.take_next(subscription.id, &subscription.key, &subscription.token)
        })
    }
    pub(super) fn disconnect_conversation_stream(
        &self,
        instance: &InstanceRuntime,
        subscription: &StreamSubscription,
    ) -> Result<(), ReliquaryRuntimeHostError> {
        self.with_stream_recipient(instance, subscription, |hub| {
            hub.disconnect(subscription.id, &subscription.key, &subscription.token)
        })
    }
    fn with_stream_recipient<T>(
        &self,
        instance: &InstanceRuntime,
        subscription: &StreamSubscription,
        action: impl FnOnce(&mut StreamHub) -> Result<T, ReliquaryRuntimeHostError>,
    ) -> Result<T, ReliquaryRuntimeHostError> {
        if instance.instance_id() != subscription.token.instance_id {
            return Err(operation("stream belongs to another attachment"));
        }
        subscription.key.validate_token(&subscription.token)?;
        self.with_rel_distribution_admission(&subscription.token, |sequencer| {
            let mut gate = sequencer.gate(subscription.token.mount_epoch)?;
            self.validate_rel_distribution_token(&subscription.token)?;
            action(gate.streams())
        })
    }
}

// Post-sync producer calls use RelSequencerGate::publish_stream. No producer or
// mutable InteractionRuntime session inspection is introduced on the P4 branch.
#[cfg(test)]
#[path = "runtime_host_gateway_streams_tests.rs"]
mod tests;
