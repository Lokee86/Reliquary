//! Canonical P4 snapshot/register handshake. Public exports wait for convergence.
//! Lock order: attachment/owner leases -> sequencer -> one source reader at a time.
//! P3.7 supplies final coherent conversation observations; P4 never owns their policy.
#![allow(dead_code)] // P4 migration window: full publication and exports follow.

#[cfg(test)]
use super::events::{RelEventClass, RelInvalidation};
use super::events::{RelSequencer, RelSequencerGate};
use super::observation::{
    DurableWatermarks, HostOperationObservation, RelObservationSummary, SubmissionOrder,
};
use super::subscriptions::{RelDelivery, SubscriptionId};
use super::{
    GatewayRuntime, InstanceRuntime, OwnerAccessToken, OwnerAdmission, OwnerGrant, OwnerKind,
    ReliquaryRuntimeHostError, operation,
};
use crate::multiplexing_journal::{ConversationSubmissionId, RelOwnerId, SubmissionState};
use std::sync::{Arc, atomic::Ordering};

#[path = "runtime_host_gateway_snapshot_reader.rs"]
pub(super) mod reader;
#[cfg(test)]
use reader::source_watermarks;

const MAX_SNAPSHOT_ITEMS: usize = 16_384;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct RelCursor {
    pub mount_epoch: u64,
    pub cursor: u64,
}

/// Safe current-journal projection. Missing compacted branch metadata stays
/// unavailable; it is not an invented P3 branch or generation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct RelSubmissionSummary {
    pub id: ConversationSubmissionId,
    pub branch_id: Option<String>,
    pub state: SubmissionState,
    pub order: SubmissionOrder,
    pub transition_revision: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct RelProjectStatus {
    pub correlation_sequence: u64,
    pub repository_kind: crate::ProjectRepositoryKind,
    pub management: crate::ProjectRepositoryManagement,
}

/// Contains only broad-view metadata and source clocks. No writable handles,
/// request objects, submission/attachment bodies, checkpoints or PHY state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct RelSnapshotState {
    pub conversations: Vec<crate::ConversationSummary>,
    pub branches: Vec<crate::Branch>,
    pub submissions: Vec<RelSubmissionSummary>,
    pub host_work: Vec<HostOperationObservation>,
    pub processing: crate::InsomniaStats,
    /// Ephemeral gateway switch, never a durable semantic watermark.
    pub processing_enabled: bool,
    pub project: Option<RelProjectStatus>,
    pub lifecycle: OwnerAdmission,
    pub watermarks: DurableWatermarks,
    /// Final P3 policy/branch/generation observation, unchanged frozen shape.
    /// None until the P3.7 producer is available at P4.7 convergence.
    pub conversation_observation: Option<RelObservationSummary>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct RelSnapshot {
    pub rel_id: RelOwnerId,
    pub watermark: RelCursor,
    pub state: RelSnapshotState,
}

/// Opaque recipient authority; callers cannot manufacture or transfer tokens.
pub(super) struct RelSubscription {
    id: SubscriptionId,
    token: OwnerAccessToken,
}

impl GatewayRuntime {
    /// Always returns a fresh authoritative snapshot, then incrementals with
    /// strictly greater cursors. known_cursor is a validation hint, never replay.
    pub(super) fn subscribe_rel(
        &self,
        instance: &InstanceRuntime,
        owner_id: &str,
        known_cursor: Option<RelCursor>,
        capacity: usize,
    ) -> Result<(RelSnapshot, RelSubscription), ReliquaryRuntimeHostError> {
        let token = self.authorize(instance, owner_id, OwnerGrant::ViewRel)?;
        self.subscribe_rel_from_reader(&token, known_cursor, capacity, |gate| {
            let _ = gate; // Reader cannot release the publication guard.
            self.read_rel_snapshot_state(&token)
        })
    }

    /// Closed internal reader seam. It is not a client-supplied snapshot API.
    /// The production reader above uses authorized gateway-owned sources only.
    pub(super) fn subscribe_rel_from_reader(
        &self,
        token: &OwnerAccessToken,
        known_cursor: Option<RelCursor>,
        capacity: usize,
        read: impl FnOnce(
            &mut RelSequencerGate<'_>,
        ) -> Result<RelSnapshotState, ReliquaryRuntimeHostError>,
    ) -> Result<(RelSnapshot, RelSubscription), ReliquaryRuntimeHostError> {
        self.subscribe_rel_replacing(token, known_cursor, capacity, None, read)
    }

    /// Reconnect replaces exactly one live recipient at the snapshot cut. No
    /// event window is retained: even a valid same-epoch cursor needs a snapshot.
    pub(super) fn reconnect_rel(
        &self,
        instance: &InstanceRuntime,
        previous: &RelSubscription,
        known_cursor: Option<RelCursor>,
        capacity: usize,
    ) -> Result<(RelSnapshot, RelSubscription), ReliquaryRuntimeHostError> {
        if instance.instance_id() != previous.token.instance_id {
            return Err(operation("REL subscription belongs to another attachment"));
        }
        self.subscribe_rel_replacing(
            &previous.token,
            known_cursor,
            capacity,
            Some(previous.id),
            |_| self.read_rel_snapshot_state(&previous.token),
        )
    }

    fn subscribe_rel_replacing(
        &self,
        token: &OwnerAccessToken,
        known_cursor: Option<RelCursor>,
        capacity: usize,
        previous: Option<SubscriptionId>,
        read: impl FnOnce(
            &mut RelSequencerGate<'_>,
        ) -> Result<RelSnapshotState, ReliquaryRuntimeHostError>,
    ) -> Result<(RelSnapshot, RelSubscription), ReliquaryRuntimeHostError> {
        if token.kind != OwnerKind::Rel || token.grant != OwnerGrant::ViewRel {
            return Err(operation("REL subscription requires ViewRel"));
        }
        if capacity == 0 || capacity > 4096 {
            return Err(operation(
                "REL subscriber capacity is outside its bounded limits",
            ));
        }
        self.with_rel_distribution_admission(token, |sequencer| {
            let mut gate = sequencer.gate(token.mount_epoch)?;
            if known_cursor.is_some_and(|known| {
                known.mount_epoch != token.mount_epoch || known.cursor > gate.cursor()
            }) {
                return Err(operation(
                    "REL cursor is from another epoch or the future; fresh subscription required",
                ));
            }
            let state = read(&mut gate)?;
            self.validate_rel_distribution_token(token)?;
            if state.lifecycle != OwnerAdmission::Open {
                return Err(operation("REL snapshot is not from an open owner"));
            }
            if let Some(observation) = &state.conversation_observation {
                if observation.rel_id
                    != RelOwnerId::parse_canonical(&token.owner_id).map_err(operation)?
                    || observation.mount_epoch != token.mount_epoch
                {
                    return Err(operation(
                        "conversation observation belongs to another REL epoch",
                    ));
                }
            }
            // A snapshot updates source clocks, never the notification cursor.
            let watermark = RelCursor {
                mount_epoch: token.mount_epoch,
                cursor: gate.cursor(),
            };
            let rel_id = RelOwnerId::parse_canonical(&token.owner_id).map_err(operation)?;
            // Register last: failed readers, bounds and final authorization leave no recipient.
            let id = gate.register_replacing(token.clone(), capacity, previous)?;
            gate.observe_snapshot_watermarks(state.watermarks.clone());
            Ok((
                RelSnapshot {
                    rel_id,
                    watermark,
                    state,
                },
                RelSubscription {
                    id,
                    token: token.clone(),
                },
            ))
        })
    }

    /// Final delivery revalidates current attachment/grants and owner fencing.
    /// Possession of a subscription alone grants no current delivery authority.
    pub(super) fn next_rel_event(
        &self,
        instance: &InstanceRuntime,
        subscription: &RelSubscription,
    ) -> Result<Option<RelDelivery>, ReliquaryRuntimeHostError> {
        if instance.instance_id() != subscription.token.instance_id {
            return Err(operation("REL subscription belongs to another attachment"));
        }
        self.with_rel_distribution_admission(&subscription.token, |sequencer| {
            let mut gate = sequencer.gate(subscription.token.mount_epoch)?;
            self.validate_rel_distribution_token(&subscription.token)?;
            gate.take_next(subscription.id, &subscription.token)
        })
    }

    /// Read a fresh snapshot without releasing the latch. Later publications are
    /// buffered until the caller acknowledges this exact cut, or overflow again.
    pub(super) fn resync_rel(
        &self,
        instance: &InstanceRuntime,
        subscription: &RelSubscription,
    ) -> Result<RelSnapshot, ReliquaryRuntimeHostError> {
        if instance.instance_id() != subscription.token.instance_id {
            return Err(operation("REL subscription belongs to another attachment"));
        }
        self.with_rel_distribution_admission(&subscription.token, |sequencer| {
            let mut gate = sequencer.gate(subscription.token.mount_epoch)?;
            let state = self.read_rel_snapshot_state(&subscription.token)?;
            self.validate_rel_distribution_token(&subscription.token)?;
            let watermark = RelCursor {
                mount_epoch: subscription.token.mount_epoch,
                cursor: gate.cursor(),
            };
            gate.prepare_resync(subscription.id, &subscription.token)?;
            gate.observe_snapshot_watermarks(state.watermarks.clone());
            Ok(RelSnapshot {
                rel_id: RelOwnerId::parse_canonical(&subscription.token.owner_id)
                    .map_err(operation)?,
                watermark,
                state,
            })
        })
    }

    pub(super) fn acknowledge_rel_resync(
        &self,
        instance: &InstanceRuntime,
        subscription: &RelSubscription,
        watermark: RelCursor,
    ) -> Result<(), ReliquaryRuntimeHostError> {
        if instance.instance_id() != subscription.token.instance_id
            || watermark.mount_epoch != subscription.token.mount_epoch
        {
            return Err(operation(
                "REL resync acknowledgement belongs to another attachment or epoch",
            ));
        }
        self.with_rel_distribution_admission(&subscription.token, |sequencer| {
            let mut gate = sequencer.gate(subscription.token.mount_epoch)?;
            self.validate_rel_distribution_token(&subscription.token)?;
            gate.acknowledge_resync(subscription.id, &subscription.token, watermark.cursor)
        })
    }

    /// Releases only recipient state; it never cancels or changes REL work.
    pub(super) fn disconnect_rel(
        &self,
        instance: &InstanceRuntime,
        subscription: &RelSubscription,
    ) -> Result<(), ReliquaryRuntimeHostError> {
        if instance.instance_id() != subscription.token.instance_id {
            return Err(operation("REL subscription belongs to another attachment"));
        }
        self.with_rel_distribution_admission(&subscription.token, |sequencer| {
            let mut gate = sequencer.gate(subscription.token.mount_epoch)?;
            self.validate_rel_distribution_token(&subscription.token)?;
            gate.disconnect(subscription.id, &subscription.token)
        })
    }

    /// Common gate for future gateway mutation adapters. The caller performs
    /// only owner work here, synchronizes it, captures a safe summary and returns.
    /// Full existing-mutator coverage is P4.4/P4.7; it is not claimed by this seam.
    #[cfg(test)]
    pub(super) fn with_rel_distribution_mutation<T>(
        &self,
        token: &OwnerAccessToken,
        mutate: impl FnOnce(
            &mut crate::InteractionRuntime,
        ) -> Result<
            (T, RelEventClass, Vec<RelInvalidation>),
            ReliquaryRuntimeHostError,
        >,
    ) -> Result<T, ReliquaryRuntimeHostError> {
        if token.kind != OwnerKind::Rel || token.grant != OwnerGrant::ManageRel {
            return Err(operation("REL distribution mutation requires ManageRel"));
        }
        self.with_rel_distribution_admission(token, |sequencer| {
            let mut gate = sequencer.gate(token.mount_epoch)?;
            let (value, class, invalidations, watermarks) = self.with_rel(token, |runtime| {
                let (value, class, invalidations) = mutate(runtime)?;
                runtime.cva.sync().map_err(operation)?;
                Ok((
                    value,
                    class,
                    invalidations,
                    source_watermarks(runtime.cva()),
                ))
            })?;
            // Mutation is already authoritative. Distribution failure closes the
            // feed and demands a snapshot; it cannot undo/negate the committed result.
            let _ = gate.publish(class, watermarks, invalidations);
            Ok(value)
        })
    }

    pub(super) fn with_rel_distribution_admission<T>(
        &self,
        token: &OwnerAccessToken,
        action: impl FnOnce(&RelSequencer) -> Result<T, ReliquaryRuntimeHostError>,
    ) -> Result<T, ReliquaryRuntimeHostError> {
        if token.kind != OwnerKind::Rel {
            return Err(operation("REL distribution cannot observe a PHY"));
        }
        let attachment = self.attachment_by_id(token.instance_id)?;
        let _attachment_lease = attachment.leases.acquire_current()?;
        let (sequencer, fence, epoch) = {
            let rels = self
                .rel_executions
                .read()
                .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
            let rel = rels
                .get(&token.owner_id)
                .ok_or_else(|| operation("REL is no longer mounted"))?;
            (
                Arc::clone(&rel.sequencer),
                Arc::clone(&rel.fence),
                rel.mount_epoch.load(Ordering::SeqCst),
            )
        };
        if epoch != token.mount_epoch {
            return Err(operation("REL subscription belongs to a previous mount"));
        }
        let _owner_lease = fence.acquire(token.fence_generation)?;
        self.validate_rel_distribution_token(token)?;
        action(&sequencer)
    }

    fn validate_rel_distribution_token(
        &self,
        token: &OwnerAccessToken,
    ) -> Result<(), ReliquaryRuntimeHostError> {
        let attachment = self.attachment_by_id(token.instance_id)?;
        if attachment.leases.snapshot()?.0 != OwnerAdmission::Open {
            return Err(operation(
                "attachment admission closed during REL snapshot/delivery",
            ));
        }
        attachment
            .instance
            .authorize_owner_generation(
                &token.owner_id,
                token.grant,
                token.authorization,
                token.owner_grant_generation,
            )
            .map_err(operation)?;
        let rels = self
            .rel_executions
            .read()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
        let rel = rels
            .get(&token.owner_id)
            .ok_or_else(|| operation("REL is no longer mounted"))?;
        if rel.mount_epoch.load(Ordering::SeqCst) != token.mount_epoch
            || rel.fence.snapshot()? != (OwnerAdmission::Open, token.fence_generation)
        {
            return Err(operation("REL admission changed during snapshot/delivery"));
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "runtime_host_gateway_snapshots_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "runtime_host_gateway_reconnect_tests.rs"]
mod reconnect_tests;
