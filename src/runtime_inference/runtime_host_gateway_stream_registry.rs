//! Stream delivery state only. No checkpoint reconstruction or semantic policy.
use super::super::observation::CheckpointContentIdentity;
use super::*;
use std::collections::{BTreeMap, VecDeque};

struct StreamState {
    cursor: u64,
    checkpoint: Option<u64>,
    revision: u64,
    status: GenerationStatus,
}
struct Recipient {
    key: StreamKey,
    token: OwnerAccessToken,
    capacity: usize,
    pending: VecDeque<Arc<StreamEvent>>,
    resync: Option<u64>,
    pending_text_bytes: usize,
}
#[derive(Default)]
pub(in super::super) struct StreamHub {
    streams: BTreeMap<StreamKey, StreamState>,
    recipients: BTreeMap<StreamSubscriptionId, Recipient>,
}
impl StreamHub {
    pub(in super::super) fn register(
        &mut self,
        key: &StreamKey,
        token: OwnerAccessToken,
        capacity: usize,
        previous: Option<StreamSubscriptionId>,
        source: &GenerationObservation,
    ) -> Result<(StreamSubscriptionId, u64), ReliquaryRuntimeHostError> {
        key.validate_token(&token)?;
        if let Some(id) = previous {
            self.bound(id, key, &token)?;
        }
        if capacity == 0
            || capacity > MAX_QUEUE
            || self.recipients.len() - usize::from(previous.is_some()) >= MAX_RECIPIENTS
            || (!self.streams.contains_key(key) && self.streams.len() >= MAX_STREAMS)
        {
            return Err(operation("stream/recipient capacity exhausted"));
        }
        if let Some(state) = self.streams.get(key) {
            if state.checkpoint != source.durable_checkpoint_sequence
                || state.revision != source.transition_revision
                || state.status != source.status
            {
                return Err(operation(
                    "stream source changed outside its publication gate",
                ));
            }
        }
        let id = StreamSubscriptionId(Uuid::new_v4());
        if self.recipients.contains_key(&id) {
            return Err(operation("stream recipient identity collision"));
        }
        let state = self.streams.entry(key.clone()).or_insert(StreamState {
            cursor: 0,
            checkpoint: source.durable_checkpoint_sequence,
            revision: source.transition_revision,
            status: source.status,
        });
        let cursor = state.cursor;
        if let Some(id) = previous {
            self.recipients.remove(&id);
        }
        self.recipients.insert(
            id,
            Recipient {
                key: key.clone(),
                token,
                capacity,
                pending: VecDeque::new(),
                resync: None,
                pending_text_bytes: 0,
            },
        );
        Ok((id, cursor))
    }

    /// The caller has already synchronized P3 state, while retaining the mounted
    /// gate. A content value is never evidence that persistence happened.
    pub(in super::super) fn publish(
        &mut self,
        key: StreamKey,
        generation: GenerationObservation,
        checkpoint: Option<GenerationCheckpointContent>,
    ) -> Result<Arc<StreamEvent>, ReliquaryRuntimeHostError> {
        key.validate_shape()?;
        if generation.durable_checkpoint_sequence == Some(0) {
            return Err(operation("durable checkpoint sequence cannot be zero"));
        }
        if !key.matches(&generation.identity) {
            return Err(operation("stream fact names another generation"));
        }
        if let Some(content) = &checkpoint {
            validation::checkpoint(&key, content)?;
            if generation.durable_checkpoint_sequence
                != Some(content.observation.durable_checkpoint_sequence)
                || generation.status != content.observation.status
            {
                return Err(operation(
                    "checkpoint and generation durability facts disagree",
                ));
            }
        }
        if !self.streams.contains_key(&key) && self.streams.len() >= MAX_STREAMS {
            return Err(operation(
                "stream identity capacity exhausted; new epoch required",
            ));
        }
        if let Some(state) = self.streams.get(&key) {
            if generation.transition_revision <= state.revision {
                return Err(operation(
                    "stream fact is duplicate or precedes durable revision",
                ));
            }
            match &checkpoint {
                Some(content) => {
                    let sequence = content.observation.durable_checkpoint_sequence;
                    if state.checkpoint.is_some_and(|old| sequence <= old) {
                        return Err(operation("checkpoint sequence did not advance"));
                    }
                    if let CheckpointContentIdentity::Delta {
                        after_checkpoint_sequence,
                        ..
                    } = content.observation.content
                    {
                        if after_checkpoint_sequence != state.checkpoint.unwrap_or(0) {
                            return Err(operation(
                                "checkpoint delta has a delivery gap; fresh snapshot required",
                            ));
                        }
                    }
                }
                None if generation.durable_checkpoint_sequence != state.checkpoint => {
                    return Err(operation(
                        "status cannot claim undistributed checkpoint content",
                    ));
                }
                None => {}
            }
        }
        // A first fact without recipients may seed an existing durable position.
        // Late join still requires its authoritative full prefix, never this delta.
        let state = self.streams.entry(key.clone()).or_insert(StreamState {
            cursor: 0,
            checkpoint: None,
            revision: 0,
            status: GenerationStatus::Pending,
        });
        let cursor = state
            .cursor
            .checked_add(1)
            .ok_or_else(|| operation("stream cursor exhausted; new epoch required"))?;
        state.cursor = cursor;
        state.checkpoint = generation.durable_checkpoint_sequence;
        state.revision = generation.transition_revision;
        state.status = generation.status;
        let payload = match checkpoint {
            Some(content) => StreamPayload::Checkpoint(content),
            None => StreamPayload::Status(generation),
        };
        let event = Arc::new(StreamEvent {
            key: key.clone(),
            cursor,
            payload,
        });
        let text_bytes = match &event.payload {
            StreamPayload::Checkpoint(content) => content.text.len(),
            StreamPayload::Status(_) => 0,
        };
        for r in self.recipients.values_mut().filter(|r| r.key == key) {
            if r.resync.is_some()
                || r.pending.len() >= r.capacity
                || r.pending_text_bytes + text_bytes > MAX_TEXT_BYTES
            {
                r.pending.clear();
                r.pending_text_bytes = 0;
                r.resync = Some(cursor);
            } else {
                r.pending_text_bytes += text_bytes;
                r.pending.push_back(Arc::clone(&event));
            }
        }
        Ok(event)
    }
    fn bound(
        &mut self,
        id: StreamSubscriptionId,
        key: &StreamKey,
        token: &OwnerAccessToken,
    ) -> Result<&mut Recipient, ReliquaryRuntimeHostError> {
        key.validate_token(token)?;
        let r = self
            .recipients
            .get_mut(&id)
            .ok_or_else(|| operation("stream recipient is no longer registered"))?;
        let b = &r.token;
        if r.key != *key
            || b.instance_id != token.instance_id
            || b.authorization != token.authorization
            || b.owner_id != token.owner_id
            || b.owner_grant_generation != token.owner_grant_generation
            || b.mount_epoch != token.mount_epoch
            || b.fence_generation != token.fence_generation
        {
            return Err(operation(
                "stream belongs to another scope or attachment/grant generation",
            ));
        }
        Ok(r)
    }
    pub(in super::super) fn take_next(
        &mut self,
        id: StreamSubscriptionId,
        key: &StreamKey,
        token: &OwnerAccessToken,
    ) -> Result<Option<StreamDelivery>, ReliquaryRuntimeHostError> {
        let r = self.bound(id, key, token)?;
        if let Some(latest_cursor) = r.resync {
            return Ok(Some(StreamDelivery::ResyncRequired {
                key: key.clone(),
                latest_cursor,
            }));
        }
        let event = r.pending.pop_front();
        if let Some(event) = &event {
            if let StreamPayload::Checkpoint(content) = &event.payload {
                r.pending_text_bytes -= content.text.len();
            }
        }
        Ok(event.map(StreamDelivery::Event))
    }
    pub(in super::super) fn disconnect(
        &mut self,
        id: StreamSubscriptionId,
        key: &StreamKey,
        token: &OwnerAccessToken,
    ) -> Result<(), ReliquaryRuntimeHostError> {
        self.bound(id, key, token)?;
        self.recipients.remove(&id);
        Ok(())
    }
    pub(in super::super) fn remove_instance(&mut self, id: super::super::InstanceId) {
        self.recipients.retain(|_, r| r.token.instance_id != id);
    }
    pub(in super::super) fn clear(&mut self) {
        self.streams.clear();
        self.recipients.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exhausted_stream_cursor_never_wraps_or_delivers_another_event() {
        let key = StreamKey {
            rel_id: RelOwnerId::from_uuid(Uuid::new_v4()),
            mount_epoch: 1,
            conversation_id: ConversationId::new("c").unwrap(),
            operation_id: "op".into(),
            generation_id: "gen".into(),
            branch_id: "b".into(),
        };
        let mut hub = StreamHub::default();
        let mut generation = GenerationObservation {
            identity: GenerationIdentity {
                operation_id: "op".into(),
                generation_id: "gen".into(),
                branch_id: "b".into(),
            },
            status: GenerationStatus::Running,
            transition_revision: 1,
            durable_checkpoint_sequence: None,
        };
        hub.publish(key.clone(), generation.clone(), None).unwrap();
        hub.streams.get_mut(&key).unwrap().cursor = u64::MAX;
        generation.transition_revision = 2;
        assert!(hub.publish(key.clone(), generation, None).is_err());
        let state = hub.streams.get(&key).unwrap();
        assert_eq!(state.cursor, u64::MAX);
        assert_eq!(state.revision, 1);
    }
}
