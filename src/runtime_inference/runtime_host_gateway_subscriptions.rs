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
        if capacity == 0 || capacity > 4096 || self.entries.len() >= 1024 {
            return Err(operation(
                "REL subscriber capacity is outside its bounded limits",
            ));
        }
        let id = SubscriptionId(Uuid::new_v4());
        if self.entries.contains_key(&id) {
            return Err(operation("REL subscription identity collision"));
        }
        self.entries.insert(
            id,
            Subscriber {
                token,
                capacity,
                pending: VecDeque::new(),
                resync_cursor: None,
            },
        );
        Ok(id)
    }

    pub(super) fn publish(&mut self, event: Arc<RelEventEnvelope>) {
        for subscriber in self.entries.values_mut() {
            if subscriber.resync_cursor.is_some() || subscriber.pending.len() >= subscriber.capacity
            {
                subscriber.pending.clear();
                subscriber.resync_cursor = Some(event.cursor);
            } else {
                subscriber.pending.push_back(Arc::clone(&event));
            }
        }
    }

    pub(super) fn take_next(
        &mut self,
        id: SubscriptionId,
        token: &OwnerAccessToken,
    ) -> Result<Option<RelDelivery>, ReliquaryRuntimeHostError> {
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
        if let Some(latest_cursor) = subscriber.resync_cursor {
            return Ok(Some(RelDelivery::ResyncRequired {
                epoch: bound.mount_epoch,
                latest_cursor,
            }));
        }
        Ok(subscriber.pending.pop_front().map(RelDelivery::Event))
    }

    pub(super) fn remove_instance(&mut self, id: super::InstanceId) {
        self.entries
            .retain(|_, subscriber| subscriber.token.instance_id != id);
    }

    pub(super) fn clear(&mut self) {
        self.entries.clear();
    }
}
