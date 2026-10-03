//! G1 executable reference model. This intentionally has no production-crate imports:
//! it is an oracle for journal/Archive behavior, not evidence that runtime code complies.
use std::collections::{HashMap, HashSet};

#[derive(Clone, Debug, PartialEq, Eq)]
enum State {
    Pending,
    Cancelled,
    Finalizing { parent: String, branch: String },
    Committed { parent: String, branch: String },
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Item {
    id: String,
    principal: String,
    conversation: String,
    payload_hash: u64,
    verified_ns: i64,
    tie: u128,
    admission_seq: u64,
    state: State,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Turn {
    conversation: String,
    id: String,
    parent: String,
    branch: String,
    principal: String,
    payload_hash: u64,
}

#[derive(Clone, Debug)]
enum Event {
    Enqueued(Item),
    CancelWon(String),
    FinalizeWon {
        id: String,
        parent: String,
        branch: String,
    },
    SubmissionCommitted(String),
    PostCommitScheduled(String),
}

#[derive(Default, Clone)]
struct Model {
    journal: Vec<Event>,
    items: HashMap<String, Item>,
    archive: HashMap<String, Turn>,
    canonical_path: Vec<String>,
    activity_receipts: HashSet<String>,
    episode_inputs: HashSet<String>,
    next_seq: u64,
    owner_epoch: u64,
    grant_epoch: u64,
    quiesced: bool,
    viewers: HashMap<String, HashSet<String>>,
    generation_running: HashSet<String>,
}

impl Model {
    fn enqueue(
        &mut self,
        id: &str,
        principal: &str,
        payload_hash: u64,
        verified_ns: i64,
        tie: u128,
    ) {
        if let Some(old) = self.items.get(id) {
            assert_eq!(
                (old.principal.as_str(), old.payload_hash),
                (principal, payload_hash),
                "same ID with different payload/principal must fail closed"
            );
            return;
        }
        self.next_seq += 1;
        let item = Item {
            id: id.into(),
            principal: principal.into(),
            conversation: "relA/convA".into(),
            payload_hash,
            verified_ns,
            tie,
            admission_seq: self.next_seq,
            state: State::Pending,
        };
        self.journal.push(Event::Enqueued(item.clone()));
        self.items.insert(id.into(), item);
    }

    fn pending_order(&self) -> Vec<String> {
        let mut rows: Vec<_> = self
            .items
            .values()
            .filter(|i| i.state == State::Pending)
            .collect();
        rows.sort_by_key(|i| (i.verified_ns, i.tie, i.admission_seq));
        rows.into_iter().map(|i| i.id.clone()).collect()
    }

    fn cancel(&mut self, id: &str) -> bool {
        let Some(item) = self.items.get(id) else {
            return false;
        };
        if item.state != State::Pending {
            return false;
        }
        self.journal.push(Event::CancelWon(id.into()));
        self.items.get_mut(id).unwrap().state = State::Cancelled;
        true
    }

    fn claim(&mut self, id: &str, parent: &str, branch: &str) -> bool {
        if self.quiesced {
            return false;
        }
        let Some(item) = self.items.get(id) else {
            return false;
        };
        if item.state != State::Pending {
            return false;
        }
        let (parent, branch) = (parent.to_owned(), branch.to_owned());
        self.journal.push(Event::FinalizeWon {
            id: id.into(),
            parent: parent.clone(),
            branch: branch.clone(),
        });
        self.items.get_mut(id).unwrap().state = State::Finalizing { parent, branch };
        true
    }

    fn archive_commit(&mut self, id: &str) -> bool {
        let item = self.items.get(id).unwrap().clone();
        let State::Finalizing { parent, branch } = item.state.clone() else {
            return false;
        };
        let turn = Turn {
            conversation: item.conversation.clone(),
            id: item.id.clone(),
            parent: parent.clone(),
            branch: branch.clone(),
            principal: item.principal.clone(),
            payload_hash: item.payload_hash,
        };
        if let Some(existing) = self.archive.get(id) {
            assert_eq!(
                existing, &turn,
                "stable Archive ID with mismatched fields is corruption"
            );
            return false;
        }
        self.archive.insert(id.into(), turn);
        self.canonical_path.push(id.into());
        self.activity_receipts.insert(id.into());
        true
    }

    fn schedule_post_commit_once(&mut self, id: &str) {
        // Models the independently durable Episode/Insomnia receipt, deduplicated by turn ID.
        if self.episode_inputs.insert(id.into()) {
            self.journal.push(Event::PostCommitScheduled(id.into()));
        }
    }

    fn mark_committed(&mut self, id: &str) {
        let item = self.items.get(id).unwrap().clone();
        let State::Finalizing { parent, branch } = item.state else {
            assert!(matches!(item.state, State::Committed { .. }));
            return;
        };
        assert!(self.archive.contains_key(id));
        self.journal.push(Event::SubmissionCommitted(id.into()));
        self.items.get_mut(id).unwrap().state = State::Committed { parent, branch };
    }

    fn crash_recover(self) -> Self {
        // Rebuild derived queue state only from checksummed journal events; Archive is
        // independently durable and reconciled by stable submission/node ID.
        let events = self.journal.clone();
        let archive = self.archive.clone();
        let path = self.canonical_path.clone();
        let activity = self.activity_receipts.clone();
        let episodes = self.episode_inputs.clone();
        let mut rebuilt = Model {
            journal: events.clone(),
            archive,
            canonical_path: path,
            activity_receipts: activity,
            episode_inputs: episodes,
            ..Model::default()
        };
        for event in events {
            match event {
                Event::Enqueued(item) => {
                    rebuilt.next_seq = rebuilt.next_seq.max(item.admission_seq);
                    rebuilt.items.insert(item.id.clone(), item);
                }
                Event::CancelWon(id) => {
                    assert_eq!(rebuilt.items[&id].state, State::Pending);
                    rebuilt.items.get_mut(&id).unwrap().state = State::Cancelled;
                }
                Event::FinalizeWon { id, parent, branch } => {
                    assert_eq!(rebuilt.items[&id].state, State::Pending);
                    rebuilt.items.get_mut(&id).unwrap().state =
                        State::Finalizing { parent, branch };
                }
                Event::SubmissionCommitted(id) => {
                    let item = rebuilt.items.get_mut(&id).unwrap();
                    let State::Finalizing { parent, branch } = item.state.clone() else {
                        panic!("bad journal")
                    };
                    item.state = State::Committed { parent, branch };
                }
                Event::PostCommitScheduled(id) => {
                    rebuilt.episode_inputs.insert(id);
                }
            }
        }
        let finalizing: Vec<String> = rebuilt
            .items
            .values()
            .filter(|i| matches!(i.state, State::Finalizing { .. }))
            .map(|i| i.id.clone())
            .collect();
        for id in finalizing {
            rebuilt.archive_commit(&id); // exact existing Archive turn is idempotent
            rebuilt.mark_committed(&id);
        }
        let committed: Vec<String> = rebuilt
            .items
            .values()
            .filter(|i| matches!(i.state, State::Committed { .. }))
            .map(|i| i.id.clone())
            .collect();
        for id in committed {
            rebuilt.schedule_post_commit_once(&id);
        }
        rebuilt
    }

    fn claim_with_fence(
        &mut self,
        id: &str,
        parent: &str,
        branch: &str,
        expected_owner_epoch: u64,
        expected_grant_epoch: u64,
    ) -> bool {
        if self.owner_epoch != expected_owner_epoch || self.grant_epoch != expected_grant_epoch {
            return false;
        }
        self.claim(id, parent, branch)
    }

    fn archive_commit_with_fence(
        &mut self,
        id: &str,
        expected_owner_epoch: u64,
        expected_grant_epoch: u64,
    ) -> bool {
        if self.owner_epoch != expected_owner_epoch || self.grant_epoch != expected_grant_epoch {
            return false;
        }
        self.archive_commit(id)
    }

    fn attach(&mut self, operation: &str, viewer: &str) {
        self.viewers
            .entry(operation.into())
            .or_default()
            .insert(viewer.into());
        self.generation_running.insert(operation.into());
    }

    fn detach(&mut self, operation: &str, viewer: &str) {
        if let Some(viewers) = self.viewers.get_mut(operation) {
            viewers.remove(viewer);
        }
        // Deliberately no implication from zero viewers to generation cancellation.
    }
}

#[test]
fn pending_order_tie_key_is_durable_and_never_rewrites_committed_ancestry() {
    let mut m = Model::default();
    m.enqueue("later", "p1", 1, 20, 7);
    m.enqueue("tie-b", "p2", 2, 10, 90);
    m.enqueue("tie-a", "p3", 3, 10, 12);
    assert_eq!(m.pending_order(), ["tie-a", "tie-b", "later"]);
    let recovered = m.clone().crash_recover();
    assert_eq!(recovered.pending_order(), ["tie-a", "tie-b", "later"]);

    assert!(m.claim("tie-a", "root", "main"));
    m.archive_commit("tie-a");
    m.mark_committed("tie-a");
    // This older request may order ahead of remaining pending work, never before
    // the already committed Archive ancestry. Transcript ancestry follows parent IDs,
    // even if timestamp-based UI summaries sort its rows differently.
    // Client's older wall time is not accepted as the ordering time. Gateway time
    // remains monotonic; a same-tick random tie key may reorder only pending work.
    m.enqueue("late-client", "p4", 4, 20, 5);
    assert_eq!(m.pending_order(), ["tie-b", "late-client", "later"]);
    assert_eq!(m.canonical_path, ["tie-a"]);
    assert_eq!(m.archive["tie-a"].parent, "root");
    assert!(m.claim("late-client", "tie-a", "main"));
    m.archive_commit("late-client");
    m.mark_committed("late-client");
    assert_eq!(m.canonical_path, ["tie-a", "late-client"]);
    assert_eq!(m.archive["late-client"].parent, "tie-a");
}

#[test]
fn cancel_and_finalize_race_has_one_durable_winner() {
    let mut cancel_first = Model::default();
    cancel_first.enqueue("s", "p", 55, 1, 2);
    assert!(cancel_first.cancel("s"));
    assert!(!cancel_first.claim("s", "root", "main"));
    let cancel_first = cancel_first.crash_recover();
    assert_eq!(cancel_first.items["s"].state, State::Cancelled);
    assert!(cancel_first.archive.is_empty());

    let mut finalize_first = Model::default();
    finalize_first.enqueue("s", "p", 55, 1, 2);
    assert!(finalize_first.claim("s", "root", "main"));
    assert!(!finalize_first.cancel("s"));
    let finalize_first = finalize_first.crash_recover();
    assert_eq!(
        finalize_first.items["s"].state,
        State::Committed {
            parent: "root".into(),
            branch: "main".into()
        }
    );
    assert_eq!(finalize_first.archive.len(), 1);
}

#[test]
fn every_finalize_crash_boundary_recovers_once_and_preserves_activity_identity() {
    // Crash after FinalizeWon, before Archive append.
    let mut before_archive = Model::default();
    before_archive.enqueue("s1", "p", 8, 10, 1);
    before_archive.claim("s1", "root", "main");
    let before_archive = before_archive.crash_recover();
    assert_eq!(before_archive.archive.len(), 1);
    assert_eq!(before_archive.activity_receipts.len(), 1);
    assert_eq!(before_archive.episode_inputs.len(), 1);

    // Crash after Archive sync, before terminal journal marker.
    let mut after_archive = Model::default();
    after_archive.enqueue("s2", "p", 9, 11, 2);
    after_archive.claim("s2", "root", "main");
    assert!(after_archive.archive_commit("s2"));
    assert!(!after_archive.archive_commit("s2"));
    let after_archive = after_archive.crash_recover();
    assert_eq!(after_archive.archive.len(), 1);
    assert_eq!(after_archive.canonical_path, ["s2"]);
    assert_eq!(
        after_archive.activity_receipts,
        HashSet::from(["s2".into()])
    );
    assert_eq!(after_archive.episode_inputs, HashSet::from(["s2".into()]));

    // Crash after terminal journal marker and a repeated recovery.
    let twice = after_archive.clone().crash_recover().crash_recover();
    assert_eq!(twice.archive.len(), 1);
    assert_eq!(
        twice.items["s2"].state,
        State::Committed {
            parent: "root".into(),
            branch: "main".into()
        }
    );
}

#[test]
fn committed_turn_recovers_missing_episode_schedule_marker_idempotently() {
    let mut m = Model::default();
    m.enqueue("s", "p", 33, 1, 1);
    m.claim("s", "root", "main");
    m.archive_commit("s"); // Archive is durable
    m.mark_committed("s"); // terminal queue marker is durable; scheduler marker is not
    assert!(m.episode_inputs.is_empty());
    let recovered = m.crash_recover();
    assert_eq!(recovered.episode_inputs, HashSet::from(["s".into()]));
    assert_eq!(
        recovered
            .journal
            .iter()
            .filter(|e| matches!(e, Event::PostCommitScheduled(id) if id == "s"))
            .count(),
        1
    );
    let recovered_again = recovered.crash_recover();
    assert_eq!(recovered_again.episode_inputs.len(), 1);
    assert_eq!(recovered_again.archive.len(), 1);
}

#[test]
fn retries_are_idempotent_and_id_reuse_with_different_payload_fails_closed() {
    let mut m = Model::default();
    m.enqueue("stable-id", "p", 0xabc, 100, 3);
    let event_count = m.journal.len();
    m.enqueue("stable-id", "p", 0xabc, 100, 3);
    assert_eq!(m.journal.len(), event_count);
    let rejected = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        m.enqueue("stable-id", "other-principal", 0xdef, 100, 3);
    }));
    assert!(rejected.is_err());
    assert!(m.cancel("stable-id"));
    let recovered = m.crash_recover();
    assert_eq!(recovered.items["stable-id"].state, State::Cancelled);
    assert!(recovered.archive.is_empty());
}

#[test]
fn shared_generation_survives_switch_detach_and_zero_viewers() {
    let mut m = Model::default();
    m.attach("op-1", "instance-A");
    m.attach("op-1", "instance-B");
    m.detach("op-1", "instance-A"); // A switches conversations
    assert_eq!(m.viewers["op-1"], HashSet::from(["instance-B".into()]));
    assert!(m.generation_running.contains("op-1"));
    m.detach("op-1", "instance-B"); // last viewer leaves
    assert!(m.viewers["op-1"].is_empty());
    assert!(m.generation_running.contains("op-1"));
    m.attach("op-1", "instance-C"); // late authorized viewer joins same operation
    assert!(m.generation_running.contains("op-1"));
    assert!(m.viewers["op-1"].contains("instance-C"));
}

#[test]
fn archive_id_mismatch_is_recovery_required_and_never_overwritten() {
    let mut m = Model::default();
    m.enqueue("stable", "p", 12, 1, 1);
    m.claim("stable", "root", "main");
    m.archive.insert(
        "stable".into(),
        Turn {
            conversation: "relA/convA".into(),
            id: "stable".into(),
            parent: "wrong-parent".into(),
            branch: "main".into(),
            principal: "p".into(),
            payload_hash: 12,
        },
    );
    let recovery = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| m.crash_recover()));
    assert!(
        recovery.is_err(),
        "mismatching stable Archive identity must fail closed"
    );
}

#[test]
fn pending_records_never_count_as_archive_activity_or_episode_input() {
    let mut m = Model::default();
    m.enqueue("pending", "p", 123, 50, 7);
    assert!(m.archive.is_empty());
    assert!(m.activity_receipts.is_empty());
    assert!(m.episode_inputs.is_empty());
    m.cancel("pending");
    let m = m.crash_recover();
    assert!(m.archive.is_empty());
    assert!(m.activity_receipts.is_empty());
    assert!(m.episode_inputs.is_empty());
}

#[test]
fn stale_grant_or_mount_fence_blocks_claim_and_quiesce_preserves_pending_work() {
    let mut m = Model::default();
    m.owner_epoch = 4;
    m.grant_epoch = 9;
    m.enqueue("held", "p", 12, 10, 2);
    assert!(!m.claim_with_fence("held", "root", "main", 3, 9));
    assert!(!m.claim_with_fence("held", "root", "main", 4, 8));
    m.quiesced = true;
    assert!(!m.claim_with_fence("held", "root", "main", 4, 9));
    let recovered = m.crash_recover();
    assert_eq!(recovered.items["held"].state, State::Pending);
    assert!(recovered.archive.is_empty());
}

#[test]
fn commit_rechecks_owner_and_grant_fence_after_finalize_claim() {
    let mut m = Model::default();
    m.owner_epoch = 2;
    m.grant_epoch = 5;
    m.enqueue("fenced", "p", 12, 20, 1);
    assert!(m.claim_with_fence("fenced", "root", "main", 2, 5));
    m.owner_epoch = 3; // quiesce/remount invalidates the previous owner token
    assert!(!m.archive_commit_with_fence("fenced", 2, 5));
    assert!(m.archive.is_empty());
    assert!(matches!(m.items["fenced"].state, State::Finalizing { .. }));
}

#[test]
fn equal_timestamp_tie_order_is_archive_ancestry_not_timestamp_or_node_id_sort() {
    let mut m = Model::default();
    m.enqueue("a-lexically-first", "p1", 1, 50, 90);
    m.enqueue("z-randomly-first", "p2", 2, 50, 12);
    assert_eq!(m.pending_order(), ["z-randomly-first", "a-lexically-first"]);
    m.claim("z-randomly-first", "root", "main");
    m.archive_commit("z-randomly-first");
    m.mark_committed("z-randomly-first");
    m.claim("a-lexically-first", "z-randomly-first", "main");
    m.archive_commit("a-lexically-first");
    m.mark_committed("a-lexically-first");
    assert_eq!(m.canonical_path, ["z-randomly-first", "a-lexically-first"]);
    assert_eq!(m.archive["a-lexically-first"].parent, "z-randomly-first");
}

#[test]
fn journal_replay_uses_durable_submission_fields_not_parent_actor_inheritance() {
    let mut m = Model::default();
    m.enqueue("user-submit", "verified-user", 0x1234, 77, 6);
    m.claim("user-submit", "agent-parent", "main");
    let recovered = m.crash_recover();
    let turn = &recovered.archive["user-submit"];
    assert_eq!(turn.principal, "verified-user");
    assert_eq!(turn.parent, "agent-parent");
}

#[test]
fn branch_policy_captures_branch_and_parent_before_crash() {
    let mut m = Model::default();
    m.enqueue("conflict-submit", "p", 71, 4, 9);
    assert!(m.claim(
        "conflict-submit",
        "stale-view-leaf",
        "branch-conflict-submit"
    ));
    let recovered = m.crash_recover();
    let turn = &recovered.archive["conflict-submit"];
    assert_eq!(turn.parent, "stale-view-leaf");
    assert_eq!(turn.branch, "branch-conflict-submit");
}
