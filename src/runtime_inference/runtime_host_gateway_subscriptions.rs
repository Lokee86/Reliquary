//! Bounded, token-bound recipient state owned exclusively by the REL sequencer.
#![allow(dead_code)] // Internal core; P4.2/P4.3 expose snapshot/resync operations.

use super::events::RelEventEnvelope;
use super::{OwnerAccessToken, ReliquaryRuntimeHostError, operation};
use std::collections::{BTreeMap, VecDeque};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub(super) struct SubscriptionId(Uuid);

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum RelDelivery {
    Event(Arc<RelEventEnvelope>),
    ResyncRequired { epoch: u64, latest_cursor: u64 },
}

struct Subscriber {
    token: OwnerAccessToken,
    capacity: usize,
    pending: VecDeque<Arc<RelEventEnvelope>>,
    resync_cursor: Option<u64>,
    snapshot_cursor: Option<u64>,
}

#[derive(Default)]
pub(super) struct SubscriberRegistry {
    entries: BTreeMap<SubscriptionId, Subscriber>,
}

impl SubscriberRegistry {
    pub(super) fn register(
        &mut self,
        token: OwnerAccessToken,
        capacity: usize,
    ) -> Result<SubscriptionId, ReliquaryRuntimeHostError> {
        self.register_replacing(token, capacity, None)
    }

    /// Validate everything before retiring the previous recipient, including at
    /// the registry limit. A failed reconnect leaves the old queue intact.
    pub(super) fn register_replacing(
        &mut self,
        token: OwnerAccessToken,
        capacity: usize,
        previous: Option<SubscriptionId>,
    ) -> Result<SubscriptionId, ReliquaryRuntimeHostError> {
        if let Some(id) = previous {
            self.bound_mut(id, &token)?;
        }
        let retained = self.entries.len() - usize::from(previous.is_some());
        if capacity == 0 || capacity > 4096 || retained >= 1024 {
            return Err(operation(
                "REL subscriber capacity is outside its bounded limits",
            ));
        }
        let id = SubscriptionId(Uuid::new_v4());
        if self.entries.contains_key(&id) {
            return Err(operation("REL subscription identity collision"));
        }
        if let Some(previous) = previous {
            self.entries.remove(&previous);
        }
        self.entries.insert(
            id,
            Subscriber {
                token,
                capacity,
                pending: VecDeque::new(),
                resync_cursor: None,
                snapshot_cursor: None,
            },
        );
        Ok(id)
    }

    pub(super) fn publish(&mut self, event: Arc<RelEventEnvelope>) {
        for subscriber in self.entries.values_mut() {
            if (subscriber.resync_cursor.is_some() && subscriber.snapshot_cursor.is_none())
                || subscriber.pending.len() >= subscriber.capacity
            {
                subscriber.pending.clear();
                subscriber.resync_cursor = Some(event.cursor);
                subscriber.snapshot_cursor = None;
            } else {
                if subscriber.resync_cursor.is_some() {
                    subscriber.resync_cursor = Some(event.cursor);
                }
                subscriber.pending.push_back(Arc::clone(&event));
            }
        }
    }

    pub(super) fn take_next(
        &mut self,
        id: SubscriptionId,
        token: &OwnerAccessToken,
    ) -> Result<Option<RelDelivery>, ReliquaryRuntimeHostError> {
        let subscriber = self.bound_mut(id, token)?;
        if let Some(latest_cursor) = subscriber.resync_cursor {
            return Ok(Some(RelDelivery::ResyncRequired {
                epoch: token.mount_epoch,
                latest_cursor,
            }));
        }
        Ok(subscriber.pending.pop_front().map(RelDelivery::Event))
    }

    fn bound_mut(
        &mut self,
        id: SubscriptionId,
        token: &OwnerAccessToken,
    ) -> Result<&mut Subscriber, ReliquaryRuntimeHostError> {
        let subscriber = self
            .entries
            .get_mut(&id)
            .ok_or_else(|| operation("REL subscription is no longer registered"))?;
        let bound = &subscriber.token;
        if bound.instance_id != token.instance_id
            || bound.authorization != token.authorization
            || bound.owner_id != token.owner_id
            || bound.owner_grant_generation != token.owner_grant_generation
            || bound.mount_epoch != token.mount_epoch
            || bound.fence_generation != token.fence_generation
        {
            return Err(operation(
                "REL subscription belongs to a different attachment or grant generation",
            ));
        }
        Ok(subscriber)
    }

    /// Called only after an authoritative snapshot under the publication gate.
    /// Keep the latch until acknowledgement; buffer later events within capacity.
    pub(super) fn prepare_resync(
        &mut self,
        id: SubscriptionId,
        token: &OwnerAccessToken,
        cursor: u64,
    ) -> Result<(), ReliquaryRuntimeHostError> {
        let subscriber = self.bound_mut(id, token)?;
        if subscriber.resync_cursor.is_none() {
            return Err(operation("REL subscription does not require resync"));
        }
        subscriber.pending.clear();
        subscriber.resync_cursor = Some(cursor);
        subscriber.snapshot_cursor = Some(cursor);
        Ok(())
    }

    pub(super) fn acknowledge_resync(
        &mut self,
        id: SubscriptionId,
        token: &OwnerAccessToken,
        cursor: u64,
    ) -> Result<(), ReliquaryRuntimeHostError> {
        let subscriber = self.bound_mut(id, token)?;
        if subscriber.snapshot_cursor != Some(cursor) {
            return Err(operation(
                "REL resync snapshot is missing, stale or overflowed",
            ));
        }
        subscriber.snapshot_cursor = None;
        subscriber.resync_cursor = None;
        Ok(())
    }

    pub(super) fn disconnect(
        &mut self,
        id: SubscriptionId,
        token: &OwnerAccessToken,
    ) -> Result<(), ReliquaryRuntimeHostError> {
        self.bound_mut(id, token)?;
        self.entries.remove(&id);
        Ok(())
    }

    pub(super) fn remove_instance(&mut self, id: super::InstanceId) {
        self.entries
            .retain(|_, subscriber| subscriber.token.instance_id != id);
    }

    pub(super) fn clear(&mut self) {
        self.entries.clear();
    }
}

// Read-only harness observation. No production counters or mutation controls.
#[cfg(test)]
impl SubscriberRegistry {
    pub(super) fn p6_diagnostics(&self) -> serde_json::Value {
        let queued: usize = self.entries.values().map(|r| r.pending.len()).sum();
        let slots: usize = self.entries.values().map(|r| r.pending.capacity()).sum();
        serde_json::json!({
            "recipients": self.entries.len(),
            "queued_events": queued,
            "queue_slots": slots,
            "queue_slot_bytes": slots * std::mem::size_of::<Arc<RelEventEnvelope>>(),
            "resync_recipients": self.entries.values().filter(|r| r.resync_cursor.is_some()).count(),
            "retained_payload_bytes": null,
            "byte_scope": "broad feed has no maintained payload-byte counter; slot bytes exclude Arc and payload allocations"
        })
    }
}
