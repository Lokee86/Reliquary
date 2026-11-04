//! Recipient pulls over the durable generation checkpoint journal.
//!
//! This module publishes no provider buffers. A delivery exists only after its
//! full-prefix checkpoint has been appended and synced by the queue journal.

use super::*;
use crate::conversation_queue::GenerationEventReplay;

/// Authoritative durable generation prefix and its independent recipient watermark.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GenerationStreamSnapshot {
    pub submission_id: ConversationSubmissionId,
    pub generation: DurableGeneration,
    pub watermark: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GenerationStreamEvent {
    pub cursor: u64,
    pub payload: GenerationStreamPayload,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GenerationStreamPayload {
    Checkpoint(DurableGenerationCheckpoint),
    Status(DurableGenerationPhase),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GenerationStreamDelivery {
    Event(GenerationStreamEvent),
    ResyncRequired { latest_cursor: u64 },
}

/// Opaque recipient handle bound to one authenticated Instance and generation.
pub struct GenerationStreamSubscription {
    instance_id: Uuid,
    owner: OwnerId,
    submission_id: ConversationSubmissionId,
    operation_id: String,
    generation_id: String,
    branch_id: String,
    cursor: u64,
}

impl Instance {
    /// Attach at the current synced prefix. The snapshot watermark is a delivery
    /// cursor, independent from journal sequence and checkpoint sequence.
    pub fn subscribe_generation_stream(
        &self,
        queue: &ConversationQueue,
        id: &ConversationSubmissionId,
    ) -> Result<(GenerationStreamSnapshot, GenerationStreamSubscription), GatewayError> {
        self.authorize_queue_owner(queue.owner())?;
        let generation = queue
            .status(id)
            .ok_or(conversation_queue::QueueError::MissingSubmission)?
            .generation
            .ok_or(conversation_queue::QueueError::InvalidState)?;
        let cursor = generation.stream_cursor;
        let snapshot = GenerationStreamSnapshot {
            submission_id: id.clone(),
            generation: generation.clone(),
            watermark: cursor,
        };
        let subscription = GenerationStreamSubscription {
            instance_id: self.instance_id,
            owner: queue.owner(),
            submission_id: id.clone(),
            operation_id: generation.operation_id,
            generation_id: generation.generation_id,
            branch_id: generation.branch_id,
            cursor,
        };
        Ok((snapshot, subscription))
    }

    /// Deliver at most one synced generation event. A slow recipient whose
    /// cursor has fallen outside bounded replay receives an explicit resync marker.
    pub fn next_generation_stream_delivery(
        &self,
        queue: &ConversationQueue,
        subscription: &mut GenerationStreamSubscription,
    ) -> Result<Option<GenerationStreamDelivery>, GatewayError> {
        self.validate_generation_subscription(queue, subscription)?;
        match queue.replay_generation_events(&subscription.submission_id, subscription.cursor, 1)? {
            GenerationEventReplay::ResyncRequired { latest_cursor } => {
                Ok(Some(GenerationStreamDelivery::ResyncRequired {
                    latest_cursor,
                }))
            }
            GenerationEventReplay::Events {
                latest_cursor,
                mut items,
            } => {
                if latest_cursor < subscription.cursor {
                    return Ok(Some(GenerationStreamDelivery::ResyncRequired {
                        latest_cursor,
                    }));
                }
                let Some(item) = items.pop() else {
                    if latest_cursor > subscription.cursor {
                        return Ok(Some(GenerationStreamDelivery::ResyncRequired {
                            latest_cursor,
                        }));
                    }
                    return Ok(None);
                };
                if Some(item.cursor) != subscription.cursor.checked_add(1) {
                    return Ok(Some(GenerationStreamDelivery::ResyncRequired {
                        latest_cursor,
                    }));
                }
                subscription.cursor = item.cursor;
                let payload = match item.checkpoint {
                    Some(checkpoint) => GenerationStreamPayload::Checkpoint(checkpoint),
                    None => GenerationStreamPayload::Status(item.phase),
                };
                Ok(Some(GenerationStreamDelivery::Event(
                    GenerationStreamEvent {
                        cursor: item.cursor,
                        payload,
                    },
                )))
            }
        }
    }

    /// Replace the recipient with a fresh authoritative snapshot and watermark.
    /// Consuming the old handle prevents replay of its stale cursor.
    pub fn reconnect_generation_stream(
        &self,
        queue: &ConversationQueue,
        previous: GenerationStreamSubscription,
    ) -> Result<(GenerationStreamSnapshot, GenerationStreamSubscription), GatewayError> {
        self.validate_generation_subscription(queue, &previous)?;
        self.subscribe_generation_stream(queue, &previous.submission_id)
    }

    fn validate_generation_subscription(
        &self,
        queue: &ConversationQueue,
        subscription: &GenerationStreamSubscription,
    ) -> Result<(), GatewayError> {
        self.authorize_queue_owner(queue.owner())?;
        if subscription.instance_id != self.instance_id || subscription.owner != queue.owner() {
            return Err(GatewayError::InvalidGenerationSubscription);
        }
        let generation = queue
            .status(&subscription.submission_id)
            .ok_or(conversation_queue::QueueError::MissingSubmission)?
            .generation
            .ok_or(conversation_queue::QueueError::InvalidState)?;
        if generation.operation_id != subscription.operation_id
            || generation.generation_id != subscription.generation_id
            || generation.branch_id != subscription.branch_id
        {
            return Err(GatewayError::InvalidGenerationSubscription);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use reliquary_container::{Container, ContainerIdentity, ReliquaryScope};
    use reliquary_vector_store::{
        ArchiveVectorStore, CompatibilityProfileStore, MemoryVectorStore, PackedVectorStore,
        VectorGenerationStore,
    };
    use std::path::PathBuf;

    struct Temp(PathBuf);
    impl Temp {
        fn new() -> Self {
            let root = std::env::current_dir().unwrap().join(".test-tmp");
            std::fs::create_dir_all(&root).unwrap();
            Self(root.join(format!(
                "generation-stream-{}-{}",
                std::process::id(),
                Uuid::new_v4()
            )))
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    fn initialize_rel(path: &Path, owner: [u8; 16]) {
        let mut container = Container::create(
            path,
            ContainerIdentity::reliquary(ReliquaryScope::Project),
            owner,
        )
        .unwrap();
        PackedVectorStore::default()
            .initialize(&mut container)
            .unwrap();
        CompatibilityProfileStore::default()
            .initialize(&mut container)
            .unwrap();
        ArchiveVectorStore::default()
            .initialize(&mut container)
            .unwrap();
        MemoryVectorStore::default()
            .initialize(&mut container)
            .unwrap();
        VectorGenerationStore::empty()
            .initialize(&mut container)
            .unwrap();
        container.sync().unwrap();
    }
    fn setup() -> (
        Temp,
        Gateway,
        Instance,
        OwnerId,
        ConversationQueue,
        ConversationSubmissionId,
    ) {
        let temp = Temp::new();
        std::fs::create_dir_all(&temp.0).unwrap();
        let owner_bytes = [74; 16];
        initialize_rel(&temp.0.join("owner.rel"), owner_bytes);
        let gateway = Gateway::new();
        let admission = GraphAdmission::for_replay(owner_bytes, [], [], [], [], []);
        let owner = gateway
            .mount_writable_reliquary(&temp.0.join("owner.rel"), &admission, [], [])
            .unwrap();
        let instance = gateway.instance("alice", [owner], []).unwrap();
        let id =
            ConversationSubmissionId::new("conversation", SubmissionId::new("submission").unwrap())
                .unwrap();
        let mut queue = instance
            .open_conversation_queue(temp.0.join("queue"), owner)
            .unwrap();
        instance
            .submit_conversation(
                &mut queue,
                "conversation",
                id.submission_id.clone(),
                "input",
                0,
            )
            .unwrap();
        queue
            .begin_generation(
                &id,
                "operation".into(),
                "generation".into(),
                "main".into(),
                "parent".into(),
                1,
            )
            .unwrap();
        (temp, gateway, instance, owner, queue, id)
    }

    #[test]
    fn stream_delivers_only_synced_checkpoints_and_reconnects_from_fresh_watermark() {
        let (_temp, gateway, instance, owner, mut queue, id) = setup();
        let (initial, mut subscription) =
            instance.subscribe_generation_stream(&queue, &id).unwrap();
        assert_eq!(initial.watermark, 0);
        assert!(initial.generation.checkpoint.is_none());
        assert_eq!(
            instance
                .next_generation_stream_delivery(&queue, &mut subscription)
                .unwrap(),
            None
        );

        queue
            .record_generation_checkpoint(&id, 1, "durable prefix".into())
            .unwrap();
        let Some(GenerationStreamDelivery::Event(first)) = instance
            .next_generation_stream_delivery(&queue, &mut subscription)
            .unwrap()
        else {
            panic!("synced checkpoint was not delivered");
        };
        assert_eq!(first.cursor, 1);
        assert!(matches!(
            first.payload,
            GenerationStreamPayload::Checkpoint(checkpoint)
                if checkpoint.content_prefix == "durable prefix"
        ));

        queue
            .record_generation_checkpoint(&id, 2, "durable prefix plus".into())
            .unwrap();
        let Some(GenerationStreamDelivery::Event(second)) = instance
            .next_generation_stream_delivery(&queue, &mut subscription)
            .unwrap()
        else {
            panic!("second synced checkpoint was not delivered");
        };
        assert_eq!(second.cursor, 2);

        queue
            .set_generation_phase(&id, DurableGenerationPhase::Completed)
            .unwrap();
        assert!(matches!(
            instance
                .next_generation_stream_delivery(&queue, &mut subscription)
                .unwrap(),
            Some(GenerationStreamDelivery::Event(GenerationStreamEvent {
                cursor: 3,
                payload: GenerationStreamPayload::Status(DurableGenerationPhase::Completed),
            }))
        ));

        let (snapshot, mut reconnected) = instance
            .reconnect_generation_stream(&queue, subscription)
            .unwrap();
        assert_eq!(snapshot.watermark, 3);
        assert_eq!(
            snapshot.generation.checkpoint.unwrap().content_prefix,
            "durable prefix plus"
        );
        assert_eq!(
            instance
                .next_generation_stream_delivery(&queue, &mut reconnected)
                .unwrap(),
            None
        );

        assert_eq!(
            instance
                .next_generation_stream_delivery(&queue, &mut reconnected)
                .unwrap(),
            None
        );

        let other = gateway.instance("alice", [owner], []).unwrap();
        let (_, mut foreign_subscription) = other.subscribe_generation_stream(&queue, &id).unwrap();
        assert!(matches!(
            instance.next_generation_stream_delivery(&queue, &mut foreign_subscription),
            Err(GatewayError::InvalidGenerationSubscription)
        ));
    }

    #[test]
    fn bounded_recipient_replay_requires_fresh_snapshot_and_current_grant() {
        let (_temp, gateway, instance, owner, mut queue, id) = setup();
        let (initial, mut stale) = instance.subscribe_generation_stream(&queue, &id).unwrap();
        assert_eq!(initial.watermark, 0);
        let mut prefix = String::new();
        for sequence in 1..=70 {
            prefix.push('x');
            queue
                .record_generation_checkpoint(&id, sequence, prefix.clone())
                .unwrap();
        }
        assert!(matches!(
            instance
                .next_generation_stream_delivery(&queue, &mut stale)
                .unwrap(),
            Some(GenerationStreamDelivery::ResyncRequired { latest_cursor: 70 })
        ));
        let (snapshot, fresh) = instance.reconnect_generation_stream(&queue, stale).unwrap();
        assert_eq!(snapshot.watermark, 70);
        assert_eq!(
            snapshot.generation.checkpoint.unwrap().content_prefix.len(),
            70
        );
        drop(fresh);

        let token = "stream revocation credential with enough bytes";
        let host =
            TrustedHostAdmission::new([HostCredentialBinding::new("alice", token, [owner], [])]);
        let (adapter, control) = host.install(&gateway).unwrap();
        let revoked_instance = adapter.authenticate(token).unwrap();
        let (_, mut revoked_subscription) = revoked_instance
            .subscribe_generation_stream(&queue, &id)
            .unwrap();
        control.revoke(token).unwrap();
        assert!(matches!(
            revoked_instance.next_generation_stream_delivery(&queue, &mut revoked_subscription),
            Err(GatewayError::CredentialRevoked)
        ));
    }

    #[test]
    fn journal_reopen_recovers_stream_watermark_and_deliveries() {
        let (temp, _gateway, instance, owner, mut queue, id) = setup();
        let (initial, mut subscription) =
            instance.subscribe_generation_stream(&queue, &id).unwrap();
        assert_eq!(initial.watermark, 0);

        queue
            .record_generation_checkpoint(&id, 1, "checkpoint survives reopen".into())
            .unwrap();
        drop(queue);

        let queue = instance
            .open_conversation_queue(temp.0.join("queue"), owner)
            .unwrap();
        let (snapshot, _) = instance.subscribe_generation_stream(&queue, &id).unwrap();
        assert_eq!(snapshot.watermark, 1);
        assert_eq!(
            snapshot.generation.checkpoint.unwrap().content_prefix,
            "checkpoint survives reopen"
        );
        assert!(matches!(
            instance
                .next_generation_stream_delivery(&queue, &mut subscription)
                .unwrap(),
            Some(GenerationStreamDelivery::Event(GenerationStreamEvent {
                cursor: 1,
                payload: GenerationStreamPayload::Checkpoint(DurableGenerationCheckpoint {
                    content_prefix,
                    ..
                }),
            })) if content_prefix == "checkpoint survives reopen"
        ));
    }
}
