//! Adversarial, std-only contract model for Phase 0B multiplexing G3/G4.
//! This is a specification fixture, not production implementation or proof.
//! Run: rustc --edition=2024 --test tests/multiplexing_phase0b_events_auth_contract.rs -o events_auth_contract.exe && events_auth_contract.exe

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Arc, Barrier, Condvar, Mutex};
use std::thread;

#[derive(Clone, Debug, PartialEq, Eq)]
struct QueueStatus {
    submission_id: String,
    state: String,
    gateway_order: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Snapshot {
    archive_revision: u64,
    inactive_conversations: Vec<String>,
    queue_status: Vec<QueueStatus>,
    operation_status: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum EventBody {
    RelInvalidation {
        key: String,
        revision: u64,
    },
    OperationStatus {
        conversation: String,
        operation: String,
        status: String,
    },
    StreamCheckpoint {
        conversation: String,
        operation: String,
        sequence: u64,
        text: String,
    },
    ResyncRequired {
        latest_cursor: u64,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Event {
    epoch: u64,
    // REL events use the REL feed cursor. Stream events live on a separate
    // per-operation queue and use the stream checkpoint sequence.
    cursor: u64,
    body: EventBody,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct WatermarkedSnapshot {
    epoch: u64,
    cursor: u64,
    snapshot: Snapshot,
}

#[derive(Clone, Debug)]
struct Subscriber {
    view_rel: bool,
    active_conversations: HashSet<String>,
    rel_queue: VecDeque<Event>,
    stream_queues: HashMap<String, VecDeque<Event>>,
    capacity: usize,
    overflowed: bool,
}

#[derive(Debug)]
struct RelFeed {
    rel_id: String,
    epoch: u64,
    rel_cursor: u64,
    state: Snapshot,
    subscribers: HashMap<String, Subscriber>,
}

impl RelFeed {
    fn new(rel_id: &str) -> Self {
        Self {
            rel_id: rel_id.to_owned(),
            epoch: 1,
            rel_cursor: 0,
            state: Snapshot {
                archive_revision: 0,
                inactive_conversations: vec!["inactive".to_owned()],
                queue_status: Vec::new(),
                operation_status: Vec::new(),
            },
            subscribers: HashMap::new(),
        }
    }

    // Snapshot creation, watermark capture, and registration are one critical section
    // in the gateway contract. This model's &mut borrow stands in for that sequencer.
    fn subscribe(&mut self, instance: &str, capacity: usize) -> WatermarkedSnapshot {
        let result = WatermarkedSnapshot {
            epoch: self.epoch,
            cursor: self.rel_cursor,
            snapshot: self.state.clone(),
        };
        self.subscribers.insert(
            instance.to_owned(),
            Subscriber {
                view_rel: true,
                active_conversations: HashSet::new(),
                rel_queue: VecDeque::new(),
                stream_queues: HashMap::new(),
                capacity,
                overflowed: false,
            },
        );
        result
    }

    fn join_stream(
        &mut self,
        instance: &str,
        conversation: &str,
    ) -> Result<WatermarkedSnapshot, &'static str> {
        let subscriber = self.subscribers.get_mut(instance).ok_or("not subscribed")?;
        if !subscriber.view_rel {
            return Err("not authorized");
        }
        subscriber
            .active_conversations
            .insert(conversation.to_owned());
        subscriber
            .stream_queues
            .entry(conversation.to_owned())
            .or_default();
        Ok(WatermarkedSnapshot {
            epoch: self.epoch,
            cursor: self.rel_cursor,
            snapshot: self.state.clone(),
        })
    }

    fn commit_rel_change(&mut self, key: &str) -> Event {
        // Durable owner change precedes publication.
        self.state.archive_revision += 1;
        self.publish_rel(EventBody::RelInvalidation {
            key: key.to_owned(),
            revision: self.state.archive_revision,
        })
    }

    fn publish_operation_status(
        &mut self,
        conversation: &str,
        operation: &str,
        status: &str,
    ) -> Event {
        self.state.operation_status = vec![format!("{operation}:{status}")];
        self.publish_rel(EventBody::OperationStatus {
            conversation: conversation.to_owned(),
            operation: operation.to_owned(),
            status: status.to_owned(),
        })
    }

    fn publish_checkpoint(
        &mut self,
        conversation: &str,
        operation: &str,
        sequence: u64,
        durable: bool,
        text: &str,
    ) -> Result<Event, &'static str> {
        if !durable {
            return Err("checkpoint is not durable");
        }
        let event = Event {
            epoch: self.epoch,
            cursor: sequence,
            body: EventBody::StreamCheckpoint {
                conversation: conversation.to_owned(),
                operation: operation.to_owned(),
                sequence,
                text: text.to_owned(),
            },
        };
        // Stream fan-out is isolated from the REL update cursor/feed. Only instances
        // with an active subscription for this conversation receive checkpoint text.
        for subscriber in self.subscribers.values_mut() {
            if subscriber.view_rel && subscriber.active_conversations.contains(conversation) {
                let queue = subscriber
                    .stream_queues
                    .entry(conversation.to_owned())
                    .or_default();
                if queue.len() < subscriber.capacity {
                    queue.push_back(event.clone());
                } else {
                    queue.clear();
                    queue.push_back(Event {
                        epoch: self.epoch,
                        cursor: sequence,
                        body: EventBody::ResyncRequired {
                            latest_cursor: sequence,
                        },
                    });
                }
            }
        }
        Ok(event)
    }

    fn publish_rel(&mut self, body: EventBody) -> Event {
        self.rel_cursor += 1;
        let event = Event {
            epoch: self.epoch,
            cursor: self.rel_cursor,
            body,
        };
        for subscriber in self.subscribers.values_mut() {
            if !subscriber.view_rel || subscriber.overflowed {
                continue;
            }
            if subscriber.rel_queue.len() >= subscriber.capacity {
                subscriber.rel_queue.clear();
                subscriber.rel_queue.push_back(Event {
                    epoch: self.epoch,
                    cursor: self.rel_cursor,
                    body: EventBody::ResyncRequired {
                        latest_cursor: self.rel_cursor,
                    },
                });
                subscriber.overflowed = true;
            } else {
                subscriber.rel_queue.push_back(event.clone());
            }
        }
        event
    }

    fn poll_rel(&mut self, instance: &str) -> Vec<Event> {
        self.subscribers
            .get_mut(instance)
            .map(|subscriber| subscriber.rel_queue.drain(..).collect())
            .unwrap_or_default()
    }

    fn poll_stream(&mut self, instance: &str, conversation: &str) -> Vec<Event> {
        self.subscribers
            .get_mut(instance)
            .and_then(|subscriber| subscriber.stream_queues.get_mut(conversation))
            .map(|queue| queue.drain(..).collect())
            .unwrap_or_default()
    }

    fn acknowledge_resync(&mut self, instance: &str) -> WatermarkedSnapshot {
        let snapshot = WatermarkedSnapshot {
            epoch: self.epoch,
            cursor: self.rel_cursor,
            snapshot: self.state.clone(),
        };
        if let Some(subscriber) = self.subscribers.get_mut(instance) {
            subscriber.rel_queue.clear();
            subscriber.overflowed = false;
        }
        snapshot
    }

    // A lost mount continuity invalidates every old cursor and stream subscription.
    fn remount(&mut self) {
        self.epoch += 1;
        self.rel_cursor = 0;
        self.subscribers.clear();
    }

    fn reconnect(
        &mut self,
        instance: &str,
        _old_cursor: u64,
        capacity: usize,
    ) -> WatermarkedSnapshot {
        // Cursor is merely an optimization. This API always returns an authoritative snapshot.
        self.subscribe(instance, capacity)
    }
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
enum Capability {
    ViewRel,
    Submit,
    CancelOwnPending,
    CancelGeneration,
    ManageProject,
}

#[derive(Clone, Debug)]
struct Attachment {
    principal: String,
    phy: String,
    attached: bool,
    grant_generation: u64,
    attachment_incarnation: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Token {
    instance: String,
    principal: String,
    phy: String,
    grant_generation: u64,
    attachment_incarnation: u64,
    rel_epoch: u64,
}

#[derive(Default)]
struct Authorization {
    mounted_rels: HashSet<String>,
    instances: HashMap<String, Attachment>,
    rel_grants: HashSet<(String, String, Capability)>,
    next_incarnation: u64,
}

impl Authorization {
    fn attach(&mut self, instance: &str, principal: &str, phy: &str, rel_epoch: u64) -> Token {
        self.next_incarnation += 1;
        let attachment_incarnation = self.next_incarnation;
        self.instances.insert(
            instance.to_owned(),
            Attachment {
                principal: principal.to_owned(),
                phy: phy.to_owned(),
                attached: true,
                grant_generation: 1,
                attachment_incarnation,
            },
        );
        Token {
            instance: instance.to_owned(),
            principal: principal.to_owned(),
            phy: phy.to_owned(),
            grant_generation: 1,
            attachment_incarnation,
            rel_epoch,
        }
    }

    fn grant(&mut self, principal: &str, rel: &str, capability: Capability) {
        self.mounted_rels.insert(rel.to_owned());
        self.rel_grants
            .insert((principal.to_owned(), rel.to_owned(), capability));
        for attachment in self
            .instances
            .values_mut()
            .filter(|a| a.principal == principal)
        {
            attachment.grant_generation += 1;
        }
    }

    fn validate(
        &self,
        token: &Token,
        rel: &str,
        epoch: u64,
        capability: Capability,
    ) -> Result<(), &'static str> {
        if !self.mounted_rels.contains(rel) {
            return Err("owner unavailable");
        }
        let attachment = self
            .instances
            .get(&token.instance)
            .ok_or("unknown instance")?;
        if !attachment.attached
            || attachment.principal != token.principal
            || attachment.phy != token.phy
            || attachment.grant_generation != token.grant_generation
            || attachment.attachment_incarnation != token.attachment_incarnation
        {
            return Err("stale attachment/token");
        }
        if token.rel_epoch != epoch {
            return Err("stale REL epoch");
        }
        if !self
            .rel_grants
            .contains(&(token.principal.clone(), rel.to_owned(), capability))
        {
            return Err("missing capability");
        }
        Ok(())
    }

    fn read_phy(&self, token: &Token, requested_phy: &str) -> Result<(), &'static str> {
        let attachment = self
            .instances
            .get(&token.instance)
            .ok_or("unknown instance")?;
        if !attachment.attached
            || attachment.principal != token.principal
            || attachment.phy != token.phy
            || attachment.grant_generation != token.grant_generation
            || attachment.attachment_incarnation != token.attachment_incarnation
        {
            return Err("stale attachment/token");
        }
        if requested_phy != attachment.phy {
            return Err("foreign PHY");
        }
        Ok(())
    }

    fn revoke(&mut self, instance: &str, rel: &str, capability: Capability) {
        if let Some(attachment) = self.instances.get_mut(instance) {
            let principal = attachment.principal.clone();
            self.rel_grants
                .remove(&(principal, rel.to_owned(), capability));
            attachment.grant_generation += 1;
        }
    }

    fn detach(&mut self, instance: &str) {
        if let Some(attachment) = self.instances.get_mut(instance) {
            attachment.attached = false;
            attachment.grant_generation += 1;
        }
    }
}

// Small concurrency model for the G5-compatible fence: the global authority gate
// grants bounded leases, while the owner mutation runs outside the gate/owner locks
// are never nested with it. Revocation closes admission, then drains admitted leases.
struct LeaseGate {
    state: Mutex<LeaseState>,
    idle: Condvar,
}

struct LeaseState {
    open: bool,
    generation: u64,
    active: usize,
}

impl LeaseGate {
    fn new(generation: u64) -> Self {
        Self {
            state: Mutex::new(LeaseState {
                open: true,
                generation,
                active: 0,
            }),
            idle: Condvar::new(),
        }
    }

    fn acquire(&self, token_generation: u64) -> Result<Lease<'_>, &'static str> {
        let mut state = self.state.lock().unwrap();
        if !state.open || state.generation != token_generation {
            return Err("lease fenced");
        }
        state.active += 1;
        Ok(Lease { gate: self })
    }

    fn close_and_revoke(&self) {
        let mut state = self.state.lock().unwrap();
        state.open = false;
        while state.active != 0 {
            state = self.idle.wait(state).unwrap();
        }
        state.generation += 1;
    }

    fn generation(&self) -> u64 {
        self.state.lock().unwrap().generation
    }
}

struct Lease<'a> {
    gate: &'a LeaseGate,
}

impl Drop for Lease<'_> {
    fn drop(&mut self) {
        let mut state = self.gate.state.lock().unwrap();
        state.active -= 1;
        if state.active == 0 {
            self.gate.idle.notify_all();
        }
    }
}

#[test]
fn mutation_racing_atomic_subscribe_is_in_snapshot_or_after_watermark() {
    let feed = Arc::new(Mutex::new(RelFeed::new("rel-a")));
    let gate = Arc::new(Barrier::new(3));
    let mut joins = Vec::new();

    for name in ["i1", "i2"] {
        let feed = Arc::clone(&feed);
        let gate = Arc::clone(&gate);
        let name = name.to_owned();
        joins.push(thread::spawn(move || {
            gate.wait();
            feed.lock().unwrap().subscribe(&name, 8)
        }));
    }
    let writer_feed = Arc::clone(&feed);
    let writer_gate = Arc::clone(&gate);
    let writer = thread::spawn(move || {
        writer_gate.wait();
        writer_feed.lock().unwrap().commit_rel_change("memory:42")
    });

    let snapshots: Vec<_> = joins.into_iter().map(|h| h.join().unwrap()).collect();
    let event = writer.join().unwrap();
    let feed = feed.lock().unwrap();
    for (i, snapshot) in ["i1", "i2"].iter().zip(snapshots) {
        let delivered = feed
            .subscribers
            .get(*i)
            .unwrap()
            .rel_queue
            .iter()
            .any(|e| e.cursor == event.cursor);
        let in_snapshot = snapshot.snapshot.archive_revision >= 1;
        assert!(in_snapshot || delivered, "mutation missed by {i}");
        if delivered {
            assert!(event.cursor > snapshot.cursor);
        }
    }
}

#[test]
fn durable_stream_checkpoint_has_own_sequence_not_rel_cursor() {
    let mut feed = RelFeed::new("rel-a");
    let snapshot = feed.subscribe("viewer", 8);
    feed.join_stream("viewer", "chat").unwrap();
    let checkpoint = feed
        .publish_checkpoint("chat", "op-1", 7, true, "delta")
        .unwrap();
    assert_eq!(
        feed.rel_cursor, snapshot.cursor,
        "stream text must not create REL feed cursor gaps"
    );
    assert_eq!(checkpoint.cursor, 7);
    assert_eq!(feed.state.archive_revision, 0);
    assert!(matches!(feed.poll_rel("viewer").as_slice(), []));
    assert!(
        feed.poll_stream("viewer", "chat")
            .iter()
            .any(|e| e.cursor == 7)
    );
    assert!(
        feed.publish_checkpoint("chat", "op-1", 8, false, "undurable")
            .is_err()
    );
}

#[test]
fn inactive_conversation_status_is_rel_wide_but_stream_text_is_selected_only() {
    let mut feed = RelFeed::new("rel-a");
    feed.subscribe("alice", 8);
    feed.subscribe("bob", 8);
    feed.join_stream("alice", "chat-a").unwrap();
    feed.publish_operation_status("chat-b", "op-b", "running");
    feed.publish_checkpoint("chat-b", "op-b", 1, true, "private delta")
        .unwrap();
    let alice = feed.poll_rel("alice");
    let bob = feed.poll_rel("bob");
    assert!(alice.iter().any(|e| matches!(&e.body, EventBody::OperationStatus { conversation, .. } if conversation == "chat-b")));
    assert!(bob.iter().any(|e| matches!(&e.body, EventBody::OperationStatus { conversation, .. } if conversation == "chat-b")));
    assert!(feed.poll_stream("alice", "chat-a").is_empty());
    assert!(feed.poll_stream("bob", "chat-b").is_empty());
}

#[test]
fn overflow_latches_resync_until_snapshot_acknowledgement() {
    let mut feed = RelFeed::new("rel-a");
    feed.subscribe("slow", 1);
    feed.commit_rel_change("memory:1");
    feed.commit_rel_change("memory:2");
    let overflow = feed.poll_rel("slow");
    assert_eq!(overflow.len(), 1);
    assert!(matches!(overflow[0].body, EventBody::ResyncRequired { .. }));
    feed.commit_rel_change("memory:3");
    assert!(
        feed.poll_rel("slow").is_empty(),
        "incremental events must remain latched"
    );
    let snapshot = feed.acknowledge_resync("slow");
    assert_eq!(snapshot.snapshot.archive_revision, 3);
    feed.commit_rel_change("memory:4");
    assert_eq!(feed.poll_rel("slow").len(), 1);
}

#[test]
fn remount_invalidates_old_epoch_and_reconnect_always_snapshots() {
    let mut feed = RelFeed::new("rel-a");
    let initial = feed.subscribe("alice", 8);
    feed.commit_rel_change("archive");
    feed.remount();
    assert_ne!(initial.epoch, feed.epoch);
    assert!(feed.poll_rel("alice").is_empty());
    feed.state.archive_revision = 9;
    let reconnected = feed.reconnect("alice", initial.cursor, 8);
    assert_eq!(reconnected.epoch, feed.epoch);
    assert_eq!(reconnected.snapshot.archive_revision, 9);
    assert_eq!(reconnected.cursor, 0);
}

#[test]
fn rel_wide_grants_do_not_imply_dependencies_or_mutation_capabilities() {
    let mut auth = Authorization::default();
    auth.grant("alice", "rel-project", Capability::ViewRel);
    auth.grant("alice", "rel-project", Capability::Submit);
    auth.grant("alice", "rel-project", Capability::CancelOwnPending);
    auth.grant("alice", "rel-sibling", Capability::Submit); // mounted, but no view grant
    let token = auth.attach("inst-a", "alice", "phy-alice", 4);
    assert!(
        auth.validate(&token, "rel-project", 4, Capability::ViewRel)
            .is_ok()
    );
    assert_eq!(
        auth.validate(&token, "rel-sibling", 4, Capability::ViewRel),
        Err("missing capability")
    );
    assert!(
        auth.validate(&token, "rel-project", 4, Capability::ManageProject)
            .is_err()
    );
    assert!(
        auth.validate(&token, "rel-project", 4, Capability::CancelGeneration)
            .is_err()
    );
    assert!(
        auth.validate(&token, "rel-project", 4, Capability::CancelOwnPending)
            .is_ok()
    );
}

#[test]
fn principal_binding_prevents_foreign_phy_selection() {
    let mut auth = Authorization::default();
    let token = auth.attach("inst-a", "alice", "phy-alice", 1);
    assert!(auth.read_phy(&token, "phy-alice").is_ok());
    assert_eq!(auth.read_phy(&token, "phy-bob"), Err("foreign PHY"));
}

#[test]
fn authorization_revoke_detach_and_reattach_invalidate_attachment_tokens() {
    let mut auth = Authorization::default();
    auth.grant("alice", "rel-a", Capability::ViewRel);
    auth.grant("alice", "rel-a", Capability::CancelGeneration);
    let stale = auth.attach("inst-a", "alice", "phy-alice", 9);
    assert!(
        auth.validate(&stale, "rel-a", 9, Capability::ViewRel)
            .is_ok()
    );
    auth.revoke("inst-a", "rel-a", Capability::CancelGeneration);
    assert_eq!(
        auth.validate(&stale, "rel-a", 9, Capability::ViewRel),
        Err("stale attachment/token")
    );
    let detached = auth.attach("inst-a", "alice", "phy-alice", 9);
    auth.detach("inst-a");
    assert_eq!(
        auth.validate(&detached, "rel-a", 9, Capability::ViewRel),
        Err("stale attachment/token")
    );
    let reattached = auth.attach("inst-a", "alice", "phy-alice", 9);
    assert_ne!(
        stale.attachment_incarnation,
        reattached.attachment_incarnation
    );
    assert_eq!(
        auth.validate(&stale, "rel-a", 9, Capability::ViewRel),
        Err("stale attachment/token")
    );
}

#[test]
fn revoke_closes_lease_admission_and_drains_an_inflight_owner_stage() {
    let gate = Arc::new(LeaseGate::new(1));
    let admitted = gate.acquire(1).unwrap();

    let revoker_gate = Arc::clone(&gate);
    let revoker = thread::spawn(move || revoker_gate.close_and_revoke());
    while gate.state.lock().unwrap().open {
        thread::yield_now();
    }
    assert_eq!(gate.acquire(1).err(), Some("lease fenced"));
    assert_eq!(
        gate.generation(),
        1,
        "revocation waits for admitted stage to finish"
    );

    // The owner commit is represented by the admitted bounded stage completing.
    drop(admitted);
    revoker.join().unwrap();
    assert_eq!(gate.generation(), 2);
    assert_eq!(gate.acquire(1).err(), Some("lease fenced"));
}

#[test]
fn rel_feed_queue_status_shape_has_no_submission_body_or_phy_payload() {
    let status = QueueStatus {
        submission_id: "submit-1".into(),
        state: "pending".into(),
        gateway_order: 3,
    };
    assert_eq!(status.submission_id, "submit-1");
    assert_eq!(status.state, "pending");
    assert_eq!(status.gateway_order, 3);
    // Deliberately no content/PHY memory field exists in QueueStatus or RelInvalidation.
    let event = EventBody::RelInvalidation {
        key: "memory:42".into(),
        revision: 5,
    };
    assert!(matches!(event, EventBody::RelInvalidation { .. }));
    assert_eq!("rel-a", RelFeed::new("rel-a").rel_id);
}
