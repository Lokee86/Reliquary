//! REL notification authority. Semantic owners and the P3 journal retain their clocks.
//! Internal admission is consumed by P4.2/P4.4; no client subscription API yet.
#![allow(dead_code)] // P4 staged cut: publication/snapshot adapters follow the core.

use super::observation::DurableWatermarks;
use super::subscriptions::{RelDelivery, SubscriberRegistry, SubscriptionId};
use super::{OwnerAccessToken, ReliquaryRuntimeHostError, operation};
use crate::multiplexing_journal::{ConversationId, ConversationSubmissionId, RelOwnerId};
use std::sync::{Arc, Mutex, MutexGuard};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum RelEventClass {
    ArchiveChanged,
    ConversationOperationChanged,
    SemanticOwnerChanged,
    RetrievalInvalidated,
    ProcessingStatusChanged,
    SubmissionQueueChanged,
    HostWorkChanged,
    ProjectStateChanged,
    OwnerLifecycleChanged,
}

/// Identifiers only: bodies, prompts, paths and checkpoint text have no field here.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum RelInvalidation {
    Conversation(ConversationId),
    Submission(ConversationSubmissionId),
    Memory(crate::MemoryId),
    Entity(crate::EntityId),
    Community(crate::CommunityId),
    RetrievalProfile(crate::CompatibilityProfileId),
    ProjectCorrelation(u64),
    File(crate::FileId),
    PrincipalEntity(crate::EntityId),
    EphemeralProcessingEnabled(bool),
    HostCapability(super::RuntimeCapability),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum RelVisibility {
    AuthorizedRelViewers,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct RelEventEnvelope {
    pub rel_id: RelOwnerId,
    pub mount_epoch: u64,
    pub cursor: u64,
    pub class: RelEventClass,
    pub watermarks: DurableWatermarks,
    pub invalidations: Vec<RelInvalidation>,
    pub visibility: RelVisibility,
}

pub(super) struct RelSequencer {
    owner: RelOwnerId,
    state: Mutex<SequencerState>,
}

struct SequencerState {
    epoch: u64,
    cursor: u64,
    open: bool,
    watermarks: DurableWatermarks,
    subscribers: SubscriberRegistry,
}

/// A single gate for publication and the later snapshot/register handshake.
/// No provider/network I/O may be performed while retaining this guard.
pub(super) struct RelSequencerGate<'a> {
    owner: RelOwnerId,
    state: MutexGuard<'a, SequencerState>,
}

impl RelSequencer {
    pub(super) fn new(owner: &str, epoch: u64) -> Result<Self, ReliquaryRuntimeHostError> {
        let owner = RelOwnerId::parse_canonical(owner).map_err(operation)?;
        if epoch == 0 {
            return Err(operation("REL distribution requires a mounted epoch"));
        }
        Ok(Self {
            owner,
            state: Mutex::new(SequencerState {
                epoch,
                cursor: 0,
                open: true,
                watermarks: DurableWatermarks::default(),
                subscribers: SubscriberRegistry::default(),
            }),
        })
    }

    pub(super) fn current_gate(&self) -> Result<RelSequencerGate<'_>, ReliquaryRuntimeHostError> {
        let state = self
            .state
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
        if !state.open {
            return Err(operation("REL distribution is closed; reopen required"));
        }
        Ok(RelSequencerGate {
            owner: self.owner,
            state,
        })
    }

    pub(super) fn gate(
        &self,
        epoch: u64,
    ) -> Result<RelSequencerGate<'_>, ReliquaryRuntimeHostError> {
        let state = self
            .state
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
        if !state.open || state.epoch != epoch {
            return Err(operation(
                "REL distribution is closed or belongs to a previous mount",
            ));
        }
        Ok(RelSequencerGate {
            owner: self.owner,
            state,
        })
    }

    /// Idempotent teardown, including handles retained after unmount/shutdown.
    pub(super) fn close(&self) -> Result<(), ReliquaryRuntimeHostError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
        state.open = false;
        state.subscribers.clear();
        Ok(())
    }

    /// Only gateway lifecycle allocates epochs; the sequencer consumes that epoch.
    pub(super) fn reset_epoch(&self, epoch: u64) -> Result<(), ReliquaryRuntimeHostError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
        if epoch <= state.epoch {
            return Err(operation("REL distribution epoch must advance"));
        }
        state.epoch = epoch;
        state.cursor = 0;
        state.watermarks = DurableWatermarks::default();
        state.subscribers.clear();
        state.open = true;
        Ok(())
    }

    pub(super) fn remove_instance(
        &self,
        id: super::InstanceId,
    ) -> Result<(), ReliquaryRuntimeHostError> {
        self.state
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
            .subscribers
            .remove_instance(id);
        Ok(())
    }
}

impl RelSequencerGate<'_> {
    /// Unknown partial publication or maintenance continuity is never replayable.
    pub(super) fn invalidate(&mut self) {
        self.state.open = false;
        self.state.subscribers.clear();
    }

    pub(super) fn cursor(&self) -> u64 {
        self.state.cursor
    }
    pub(super) fn watermarks(&self) -> &DurableWatermarks {
        &self.state.watermarks
    }

    /// Snapshot source clocks are authoritative without allocating a notification.
    pub(super) fn observe_snapshot_watermarks(&mut self, watermarks: DurableWatermarks) {
        self.state.watermarks = watermarks;
    }

    /// Caller holds current gateway attachment/owner leases. P4.2 adds the
    /// authoritative snapshot before registration under this very same guard.
    pub(super) fn register(
        &mut self,
        token: OwnerAccessToken,
        capacity: usize,
    ) -> Result<SubscriptionId, ReliquaryRuntimeHostError> {
        self.validate_token(&token)?;
        self.state.subscribers.register(token, capacity)
    }

    pub(super) fn prepare_resync(
        &mut self,
        id: SubscriptionId,
        token: &OwnerAccessToken,
    ) -> Result<(), ReliquaryRuntimeHostError> {
        self.validate_token(token)?;
        let cursor = self.state.cursor;
        self.state.subscribers.prepare_resync(id, token, cursor)
    }

    pub(super) fn acknowledge_resync(
        &mut self,
        id: SubscriptionId,
        token: &OwnerAccessToken,
        cursor: u64,
    ) -> Result<(), ReliquaryRuntimeHostError> {
        self.validate_token(token)?;
        self.state.subscribers.acknowledge_resync(id, token, cursor)
    }

    pub(super) fn disconnect(
        &mut self,
        id: SubscriptionId,
        token: &OwnerAccessToken,
    ) -> Result<(), ReliquaryRuntimeHostError> {
        self.validate_token(token)?;
        self.state.subscribers.disconnect(id, token)
    }

    fn validate_token(&self, token: &OwnerAccessToken) -> Result<(), ReliquaryRuntimeHostError> {
        if !self.state.open
            || token.kind != super::OwnerKind::Rel
            || token.grant != super::OwnerGrant::ViewRel
            || token.mount_epoch != self.state.epoch
            || RelOwnerId::parse_canonical(&token.owner_id).map_err(operation)? != self.owner
        {
            return Err(operation(
                "subscription token is not for this REL epoch and ViewRel grant",
            ));
        }
        Ok(())
    }

    /// Called only after authoritative commit/sync. Delivery is bounded and
    /// cannot become the mutation's commit point. No retained general event log.
    pub(super) fn publish(
        &mut self,
        class: RelEventClass,
        watermarks: DurableWatermarks,
        invalidations: Vec<RelInvalidation>,
    ) -> Result<Arc<RelEventEnvelope>, ReliquaryRuntimeHostError> {
        if !self.state.open {
            return Err(operation("REL distribution is closed"));
        }
        if invalidations.len() > 128 {
            self.state.open = false;
            self.state.subscribers.clear();
            return Err(operation(
                "REL invalidation summary exceeds capacity; reopen and snapshot required",
            ));
        }
        let Some(cursor) = self.state.cursor.checked_add(1) else {
            self.state.open = false;
            self.state.subscribers.clear();
            return Err(operation(
                "REL cursor exhausted; reopen with a new gateway epoch and snapshot",
            ));
        };
        self.state.cursor = cursor;
        self.state.watermarks = watermarks.clone();
        let event = Arc::new(RelEventEnvelope {
            rel_id: self.owner,
            mount_epoch: self.state.epoch,
            cursor,
            class,
            watermarks,
            invalidations,
            visibility: RelVisibility::AuthorizedRelViewers,
        });
        self.state.subscribers.publish(Arc::clone(&event));
        Ok(event)
    }

    /// Gateway must revalidate current authority under a delivery lease before
    /// using this internal operation. Token comparison is additional isolation.
    pub(super) fn take_next(
        &mut self,
        id: SubscriptionId,
        token: &OwnerAccessToken,
    ) -> Result<Option<RelDelivery>, ReliquaryRuntimeHostError> {
        self.validate_token(token)?;
        self.state.subscribers.take_next(id, token)
    }
}

#[cfg(test)]
#[path = "runtime_host_gateway_events_tests.rs"]
mod tests;
