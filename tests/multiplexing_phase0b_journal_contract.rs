//! G1 REL-wide provisional journal and compaction reference oracle.
//! This is std-only model code, not a production filesystem/durability test.
use std::collections::BTreeMap;

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
struct Key {
    conversation: &'static str,
    submission: &'static str,
}
fn key(conversation: &'static str, submission: &'static str) -> Key {
    Key {
        conversation,
        submission,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum State {
    Pending,
    Cancelled,
    Finalizing,
    Committed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Entry {
    payload_hash: u64,
    state: State,
    verified_timestamp: i64,
    admission_seq: u64,
    payload_retained: bool,
    archive_receipt: bool,
    postcommit_receipt: bool,
}

#[derive(Clone, Debug)]
enum Action {
    Enqueue(Key, u64, i64),
    Cancel(Key),
    Finalize(Key),
    ArchiveCommitted(Key),
    PostCommitScheduled(Key),
}

#[derive(Clone, Debug)]
struct Frame {
    sequence: u64,
    action: Action,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct Projection {
    entries: BTreeMap<Key, Entry>,
    per_conversation_revision: BTreeMap<&'static str, u64>,
    last_sequence: u64,
    last_verified_timestamp: i64,
}

impl Projection {
    fn apply(&mut self, frame: &Frame) -> Result<(), &'static str> {
        if frame.sequence
            != self
                .last_sequence
                .checked_add(1)
                .ok_or("sequence overflow")?
        {
            return Err("journal sequence must advance across all conversations");
        }
        match &frame.action {
            Action::Enqueue(id, hash, timestamp) => {
                if self.entries.contains_key(id) {
                    return Err("duplicate durable enqueue");
                }
                if *timestamp < self.last_verified_timestamp {
                    return Err("verified timestamp moved backwards");
                }
                self.last_verified_timestamp = *timestamp;
                self.entries.insert(
                    id.clone(),
                    Entry {
                        payload_hash: *hash,
                        state: State::Pending,
                        verified_timestamp: *timestamp,
                        admission_seq: frame.sequence,
                        payload_retained: true,
                        archive_receipt: false,
                        postcommit_receipt: false,
                    },
                );
            }
            Action::Cancel(id) => {
                let entry = self.entries.get_mut(id).ok_or("missing pending item")?;
                if entry.state != State::Pending {
                    return Err("cancellation lost to finalization");
                }
                entry.state = State::Cancelled;
            }
            Action::Finalize(id) => {
                let entry = self.entries.get_mut(id).ok_or("missing pending item")?;
                if entry.state != State::Pending {
                    return Err("finalization lost to cancellation");
                }
                entry.state = State::Finalizing;
            }
            Action::ArchiveCommitted(id) => {
                let entry = self.entries.get_mut(id).ok_or("missing finalizing item")?;
                if entry.state != State::Finalizing {
                    return Err("not finalizing");
                }
                entry.archive_receipt = true;
                entry.state = State::Committed;
            }
            Action::PostCommitScheduled(id) => {
                let entry = self.entries.get_mut(id).ok_or("missing committed item")?;
                if entry.state != State::Committed || !entry.archive_receipt {
                    return Err("postcommit before Archive durability");
                }
                entry.postcommit_receipt = true;
            }
        }
        let conversation = match &frame.action {
            Action::Enqueue(id, ..)
            | Action::Cancel(id)
            | Action::Finalize(id)
            | Action::ArchiveCommitted(id)
            | Action::PostCommitScheduled(id) => id.conversation,
        };
        *self
            .per_conversation_revision
            .entry(conversation)
            .or_default() += 1;
        self.last_sequence = frame.sequence;
        Ok(())
    }

    fn compacted(&self) -> Self {
        let mut compacted = self.clone();
        for entry in compacted.entries.values_mut() {
            if entry.state == State::Cancelled
                || (entry.state == State::Committed && entry.postcommit_receipt)
            {
                entry.payload_retained = false;
            }
        }
        compacted
    }
}

#[derive(Clone, Debug, Default)]
struct Disk {
    snapshot: Projection,
    tail: Vec<Frame>,
}
impl Disk {
    fn reopen(&self) -> Projection {
        let mut state = self.snapshot.clone();
        for frame in &self.tail {
            state
                .apply(frame)
                .expect("durable journal must replay exactly");
        }
        state
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Outcome {
    Durable,
    Idempotent,
    QueuedWithoutAcknowledgment,
}

// A single owner is the only REL journal writer; a production actor/mutex would
// serialize its calls. Snapshot replacement and appends share this same gate.
#[derive(Default)]
struct JournalOwner {
    disk: Disk,
    live: Projection,
    compact_candidate: Option<Projection>,
    deferred: Vec<(Action, Option<u64>)>,
}
impl JournalOwner {
    fn append(&mut self, action: Action) -> Result<Outcome, &'static str> {
        self.append_with_revision(action, None)
    }

    // A coordinator may supply the revision it observed before releasing its
    // local lock; the REL writer validates that revision at durable transition.
    fn append_with_revision(
        &mut self,
        action: Action,
        expected_revision: Option<u64>,
    ) -> Result<Outcome, &'static str> {
        if self.compact_candidate.is_some() {
            self.deferred.push((action, expected_revision));
            return Ok(Outcome::QueuedWithoutAcknowledgment);
        }
        let conversation = match &action {
            Action::Enqueue(id, ..)
            | Action::Cancel(id)
            | Action::Finalize(id)
            | Action::ArchiveCommitted(id)
            | Action::PostCommitScheduled(id) => id.conversation,
        };
        if expected_revision.is_some_and(|expected| {
            *self
                .live
                .per_conversation_revision
                .get(conversation)
                .unwrap_or(&0)
                != expected
        }) {
            return Err("stale conversation transition revision");
        }
        if let Action::Enqueue(id, hash, _) = &action {
            if let Some(old) = self.live.entries.get(id) {
                return if old.payload_hash == *hash {
                    Ok(Outcome::Idempotent)
                } else {
                    Err("same composite key, different payload")
                };
            }
        }
        if let Action::Cancel(id) = &action {
            if self
                .live
                .entries
                .get(id)
                .is_some_and(|e| e.state == State::Cancelled)
            {
                return Ok(Outcome::Idempotent);
            }
        }
        let action = match action {
            Action::Enqueue(id, hash, sample) => {
                Action::Enqueue(id, hash, sample.max(self.live.last_verified_timestamp))
            }
            action => action,
        };
        let frame = Frame {
            sequence: self
                .live
                .last_sequence
                .checked_add(1)
                .ok_or("sequence overflow")?,
            action,
        };
        let mut proposed = self.live.clone();
        proposed.apply(&frame)?;
        // Models a successfully synchronized append. Never acknowledge before it.
        self.disk.tail.push(frame);
        self.live = proposed;
        Ok(Outcome::Durable)
    }

    fn begin_compaction(&mut self) {
        assert!(self.compact_candidate.is_none());
        self.compact_candidate = Some(self.live.compacted());
    }

    // The candidate is validated and synchronized before installing it.
    // The replacement is atomic: recovery sees exactly the old or new disk.
    fn install_compaction(&mut self) -> Vec<Result<Outcome, &'static str>> {
        let candidate = self
            .compact_candidate
            .take()
            .expect("compaction in progress");
        assert_eq!(candidate.entries.len(), self.live.entries.len());
        assert_eq!(candidate.last_sequence, self.live.last_sequence);
        self.disk = Disk {
            snapshot: candidate,
            tail: Vec::new(),
        };
        self.live = self.disk.reopen();
        let held = std::mem::take(&mut self.deferred);
        held.into_iter()
            .map(|(cmd, rev)| self.append_with_revision(cmd, rev))
            .collect()
    }

    fn crash(self) -> Self {
        // All unacknowledged requests and any unfinished candidate are lost.
        let live = self.disk.reopen();
        Self {
            live,
            disk: self.disk,
            ..Self::default()
        }
    }
}

#[test]
fn two_conversations_share_one_monotonic_journal_but_have_scoped_ids() {
    let mut owner = JournalOwner::default();
    let a = key("A", "same-id");
    let b = key("B", "same-id");
    assert_eq!(
        owner.append(Action::Enqueue(a.clone(), 10, 50)),
        Ok(Outcome::Durable)
    );
    assert_eq!(
        owner.append(Action::Enqueue(b.clone(), 20, 49)),
        Ok(Outcome::Durable)
    );
    assert_eq!(
        owner.append(Action::Cancel(a.clone())),
        Ok(Outcome::Durable)
    );
    assert_eq!(
        owner.append(Action::Finalize(b.clone())),
        Ok(Outcome::Durable)
    );
    assert_eq!(owner.live.last_sequence, 4);
    assert_eq!(owner.live.entries[&a].state, State::Cancelled);
    assert_eq!(owner.live.entries[&b].state, State::Finalizing);
    assert_eq!(owner.live.entries[&b].verified_timestamp, 50);
    assert_eq!(owner.live.per_conversation_revision["A"], 2);
    assert_eq!(owner.live.per_conversation_revision["B"], 2);
    let recovered = owner.crash();
    assert_eq!(recovered.live.entries[&a].state, State::Cancelled);
    assert_eq!(recovered.live.entries[&b].state, State::Finalizing);
}

#[test]
fn compaction_fences_all_conversations_and_never_acknowledges_a_lost_tail() {
    let mut owner = JournalOwner::default();
    let a = key("A", "pending");
    let b = key("B", "pending");
    let c = key("C", "arrived-during-compaction");
    owner.append(Action::Enqueue(a.clone(), 1, 10)).unwrap();
    owner.append(Action::Enqueue(b.clone(), 2, 11)).unwrap();
    owner.begin_compaction();
    assert_eq!(
        owner.append(Action::Cancel(a.clone())),
        Ok(Outcome::QueuedWithoutAcknowledgment)
    );
    assert_eq!(
        owner.append(Action::Finalize(b.clone())),
        Ok(Outcome::QueuedWithoutAcknowledgment)
    );
    assert_eq!(
        owner.append(Action::Enqueue(c.clone(), 3, 12)),
        Ok(Outcome::QueuedWithoutAcknowledgment)
    );
    assert_eq!(owner.live.last_sequence, 2);
    assert_eq!(owner.disk.reopen().entries[&a].state, State::Pending);
    assert!(!owner.disk.reopen().entries.contains_key(&c));
    assert_eq!(owner.install_compaction(), [Ok(Outcome::Durable); 3]);
    assert_eq!(owner.live.last_sequence, 5);
    assert_eq!(owner.live.entries[&a].state, State::Cancelled);
    assert_eq!(owner.live.entries[&b].state, State::Finalizing);
    assert_eq!(owner.live.entries[&c].state, State::Pending);
    assert_eq!(owner.crash().live.last_sequence, 5);
}

#[test]
fn crash_on_either_side_of_replace_recovers_old_or_new_complete_rel() {
    let mut before = JournalOwner::default();
    let a = key("A", "cancelled");
    let b = key("B", "finalizing");
    before.append(Action::Enqueue(a.clone(), 1, 10)).unwrap();
    before.append(Action::Cancel(a.clone())).unwrap();
    before.append(Action::Enqueue(b.clone(), 2, 11)).unwrap();
    before.append(Action::Finalize(b.clone())).unwrap();
    let mut candidate = before;
    candidate.begin_compaction();
    candidate
        .append(Action::Enqueue(key("C", "unacked"), 3, 12))
        .unwrap();
    let old = JournalOwner {
        disk: candidate.disk.clone(),
        ..JournalOwner::default()
    }
    .crash();
    assert_eq!(old.live.last_sequence, 4);
    assert!(!old.live.entries.contains_key(&key("C", "unacked")));
    assert_eq!(old.live.entries[&a].state, State::Cancelled);
    assert_eq!(old.live.entries[&b].state, State::Finalizing);

    // Crash after replacing the full-REL file, before processing queued calls.
    let installed = candidate.compact_candidate.take().unwrap();
    candidate.disk = Disk {
        snapshot: installed,
        tail: vec![],
    };
    let after = candidate.crash();
    assert_eq!(after.live.entries.len(), 2);
    assert_eq!(after.live.entries[&a].state, State::Cancelled);
    assert!(!after.live.entries[&a].payload_retained);
    assert_eq!(after.live.entries[&b].state, State::Finalizing);
    assert_eq!(after.live.last_sequence, 4);
    // Clients retry unacknowledged work by its full composite identity.
    let mut after = after;
    assert_eq!(
        after.append(Action::Enqueue(key("C", "unacked"), 3, 12)),
        Ok(Outcome::Durable)
    );
    assert_eq!(after.live.last_sequence, 5);
}

#[test]
fn compaction_keeps_terminal_tombstones_and_unresolved_completion_receipts() {
    let mut owner = JournalOwner::default();
    let a = key("A", "same");
    let b = key("B", "same");
    owner.append(Action::Enqueue(a.clone(), 1, 50)).unwrap();
    owner.append(Action::Cancel(a.clone())).unwrap();
    owner.append(Action::Enqueue(b.clone(), 2, 50)).unwrap();
    owner.append(Action::Finalize(b.clone())).unwrap();
    owner.append(Action::ArchiveCommitted(b.clone())).unwrap();
    owner.begin_compaction();
    assert!(owner.install_compaction().is_empty());
    assert_eq!(owner.live.entries[&a].state, State::Cancelled);
    assert!(!owner.live.entries[&a].payload_retained);
    assert_eq!(owner.live.entries[&b].state, State::Committed);
    assert!(owner.live.entries[&b].archive_receipt);
    assert!(!owner.live.entries[&b].postcommit_receipt);
    assert!(owner.live.entries[&b].payload_retained);
    assert_eq!(
        owner.append(Action::Enqueue(a.clone(), 999, 60)),
        Err("same composite key, different payload")
    );
    assert_eq!(
        owner.append(Action::Enqueue(a.clone(), 1, 50)),
        Ok(Outcome::Idempotent)
    );
    assert_eq!(
        owner.append(Action::PostCommitScheduled(b.clone())),
        Ok(Outcome::Durable)
    );
    owner.begin_compaction();
    owner.install_compaction();
    let recovered = owner.crash();
    assert!(!recovered.live.entries[&b].payload_retained);
    assert_eq!(recovered.live.last_sequence, 6);
    assert_eq!(recovered.live.last_verified_timestamp, 50);
    assert_eq!(recovered.live.per_conversation_revision["B"], 4);
}

#[test]
fn stale_coordinator_revision_cannot_override_another_transition_after_compaction() {
    let mut owner = JournalOwner::default();
    let item = key("A", "racing");
    let independent = key("B", "unrelated");
    owner.append(Action::Enqueue(item.clone(), 1, 1)).unwrap();
    let observed_revision = owner.live.per_conversation_revision["A"];
    owner.begin_compaction();
    assert_eq!(
        owner.append_with_revision(Action::Cancel(item.clone()), Some(observed_revision)),
        Ok(Outcome::QueuedWithoutAcknowledgment)
    );
    assert_eq!(
        owner.append_with_revision(Action::Finalize(item.clone()), Some(observed_revision)),
        Ok(Outcome::QueuedWithoutAcknowledgment)
    );
    assert_eq!(
        owner.append_with_revision(Action::Enqueue(independent.clone(), 2, 2), Some(0)),
        Ok(Outcome::QueuedWithoutAcknowledgment)
    );
    assert_eq!(
        owner.install_compaction(),
        [
            Ok(Outcome::Durable),
            Err("stale conversation transition revision"),
            Ok(Outcome::Durable),
        ]
    );
    assert_eq!(owner.crash().live.entries[&item].state, State::Cancelled);
}

#[test]
fn cancellation_waiting_on_compaction_is_not_durably_acknowledged_before_sync() {
    let mut owner = JournalOwner::default();
    let item = key("A", "cancel-me");
    owner.append(Action::Enqueue(item.clone(), 1, 10)).unwrap();
    owner.begin_compaction();
    assert_eq!(
        owner.append(Action::Cancel(item.clone())),
        Ok(Outcome::QueuedWithoutAcknowledgment)
    );
    let mut crashed = owner.crash();
    assert_eq!(crashed.live.entries[&item].state, State::Pending);
    // After reconnect the caller retries cancellation, rather than assuming success.
    assert_eq!(
        crashed.append(Action::Cancel(item.clone())),
        Ok(Outcome::Durable)
    );
    assert_eq!(crashed.crash().live.entries[&item].state, State::Cancelled);
}
