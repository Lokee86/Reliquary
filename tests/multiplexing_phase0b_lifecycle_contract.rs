//! Std-only executable model for the Phase 0B owner-fencing/recovery contract.
//!
//! Run from the Reliquary root:
//!   rustc --edition=2021 --test tests/multiplexing_phase0b_lifecycle_contract.rs -o target/multiplexing_phase0b_lifecycle_contract
//!   target/multiplexing_phase0b_lifecycle_contract

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;

#[derive(Clone, Debug, Eq, PartialEq)]
enum MountError {
    DuplicateOwnerDifferentFile,
    OneFileDifferentOwner,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum MountResult {
    Mounted { epoch: u64 },
    ReusedExisting { epoch: u64 },
}

#[derive(Default)]
struct MountRegistry {
    by_owner: HashMap<String, (String, u64)>,
    by_file: HashMap<String, String>,
    next_epoch: u64,
}

impl MountRegistry {
    fn mount(&mut self, owner: &str, file_key: &str) -> Result<MountResult, MountError> {
        if let Some((existing_file, epoch)) = self.by_owner.get(owner) {
            return if existing_file == file_key {
                Ok(MountResult::ReusedExisting { epoch: *epoch })
            } else {
                Err(MountError::DuplicateOwnerDifferentFile)
            };
        }
        if self
            .by_file
            .get(file_key)
            .is_some_and(|existing| existing != owner)
        {
            return Err(MountError::OneFileDifferentOwner);
        }

        self.next_epoch += 1;
        let epoch = self.next_epoch;
        self.by_owner
            .insert(owner.to_owned(), (file_key.to_owned(), epoch));
        self.by_file.insert(file_key.to_owned(), owner.to_owned());
        Ok(MountResult::Mounted { epoch })
    }
}

#[derive(Clone, Debug)]
struct WorkToken {
    owner: String,
    mount_epoch: u64,
    grant_generation: u64,
    operation_id: String,
}

#[derive(Default)]
struct GateState {
    accepting: bool,
    quiescing: bool,
    mount_epoch: u64,
    grant_generation: u64,
    active_commits: usize,
    pending: Vec<String>,
    frozen_pending: Vec<String>,
}

#[derive(Default)]
struct DurableOwner {
    applied_stages: HashSet<String>,
}

#[derive(Default)]
struct OwnerRuntime {
    gate: Arc<(Mutex<GateState>, Condvar)>,
    durable: Arc<Mutex<DurableOwner>>,
}

struct CommitLease {
    gate: Arc<(Mutex<GateState>, Condvar)>,
    durable: Arc<Mutex<DurableOwner>>,
    token: WorkToken,
    finished: bool,
}

impl OwnerRuntime {
    fn start_at_epoch(epoch: u64, grant_generation: u64) -> Self {
        let gate = GateState {
            accepting: true,
            mount_epoch: epoch,
            grant_generation,
            ..GateState::default()
        };
        Self {
            gate: Arc::new((Mutex::new(gate), Condvar::new())),
            durable: Arc::new(Mutex::new(DurableOwner::default())),
        }
    }

    fn token(&self, owner: &str, operation_id: &str) -> WorkToken {
        let (lock, _) = &*self.gate;
        let state = lock.lock().unwrap();
        WorkToken {
            owner: owner.to_owned(),
            mount_epoch: state.mount_epoch,
            grant_generation: state.grant_generation,
            operation_id: operation_id.to_owned(),
        }
    }

    // This models atomic authorization + attachment/mount/incarnation admission.
    // The returned short lease is the linearization point before the owner write.
    fn begin_commit(&self, token: WorkToken, expected_owner: &str) -> Option<CommitLease> {
        let (lock, _) = &*self.gate;
        let mut state = lock.lock().unwrap();
        if !state.accepting
            || state.quiescing
            || token.owner != expected_owner
            || token.mount_epoch != state.mount_epoch
            || token.grant_generation != state.grant_generation
        {
            return None;
        }
        state.active_commits += 1;
        Some(CommitLease {
            gate: Arc::clone(&self.gate),
            durable: Arc::clone(&self.durable),
            token,
            finished: false,
        })
    }

    // Close admission first, then drain commit leases already admitted. A queued
    // item is frozen for later reconciliation; it is never silently discarded.
    fn quiesce_and_reopen(&self) -> u64 {
        let (lock, changed) = &*self.gate;
        let mut state = lock.lock().unwrap();
        state.accepting = false;
        state.quiescing = true;
        let pending = std::mem::take(&mut state.pending);
        state.frozen_pending.extend(pending);
        changed.notify_all();
        while state.active_commits != 0 {
            state = changed.wait(state).unwrap();
        }
        state.mount_epoch += 1;
        state.quiescing = false;
        state.accepting = true;
        state.mount_epoch
    }

    fn wait_until_admission_closed(&self) {
        let (lock, changed) = &*self.gate;
        let mut state = lock.lock().unwrap();
        while state.accepting {
            state = changed.wait(state).unwrap();
        }
    }

    fn enqueue_pending(&self, item: &str) {
        let (lock, _) = &*self.gate;
        let mut state = lock.lock().unwrap();
        assert!(state.accepting, "no enqueue during quiesce");
        state.pending.push(item.to_owned());
    }

    fn pending(&self) -> (Vec<String>, Vec<String>) {
        let (lock, _) = &*self.gate;
        let state = lock.lock().unwrap();
        (state.pending.clone(), state.frozen_pending.clone())
    }

    fn revoke_and_drain(&self) -> u64 {
        let (lock, changed) = &*self.gate;
        let mut state = lock.lock().unwrap();
        state.accepting = false;
        changed.notify_all();
        while state.active_commits != 0 {
            state = changed.wait(state).unwrap();
        }
        state.grant_generation += 1;
        state.grant_generation
    }

    fn applied_count(&self) -> usize {
        self.durable.lock().unwrap().applied_stages.len()
    }
}

impl CommitLease {
    // The production analogue performs one bounded owner-local durable mutation
    // while this lease is alive. Stage identity makes a retry idempotent.
    fn apply_stage(&mut self) -> bool {
        self.durable
            .lock()
            .unwrap()
            .applied_stages
            .insert(self.token.operation_id.clone())
    }
}

impl Drop for CommitLease {
    fn drop(&mut self) {
        if self.finished {
            return;
        }
        let (lock, changed) = &*self.gate;
        let mut state = lock.lock().unwrap();
        state.active_commits -= 1;
        self.finished = true;
        if state.active_commits == 0 {
            changed.notify_all();
        }
    }
}

#[test]
fn path_alias_reuses_one_execution_but_conflicted_copy_fails_closed() {
    let mut registry = MountRegistry::default();
    let first = registry.mount("rel-a", "volume-1:file-9").unwrap();
    let alias = registry.mount("rel-a", "volume-1:file-9").unwrap();
    assert_eq!(first, MountResult::Mounted { epoch: 1 });
    assert_eq!(alias, MountResult::ReusedExisting { epoch: 1 });
    assert_eq!(
        registry.mount("rel-a", "volume-1:file-10"),
        Err(MountError::DuplicateOwnerDifferentFile)
    );
    assert_eq!(
        registry.mount("rel-b", "volume-1:file-9"),
        Err(MountError::OneFileDifferentOwner)
    );
}

#[test]
fn stale_work_token_cannot_commit_after_owner_remount() {
    let runtime = OwnerRuntime::start_at_epoch(12, 4);
    let old = runtime.token("rel-a", "submission-1");
    let new_epoch = runtime.quiesce_and_reopen();
    assert_eq!(new_epoch, 13);
    assert!(runtime.begin_commit(old, "rel-a").is_none());

    let current = runtime.token("rel-a", "submission-2");
    let mut lease = runtime.begin_commit(current, "rel-a").unwrap();
    assert!(lease.apply_stage());
    drop(lease);
    assert_eq!(runtime.applied_count(), 1);
}

#[test]
fn quiesce_freezes_pending_items_and_drains_an_admitted_commit() {
    let runtime = Arc::new(OwnerRuntime::start_at_epoch(2, 7));
    runtime.enqueue_pending("queued-accepted");
    let token = runtime.token("rel-a", "commit-a");
    let lease = runtime.begin_commit(token, "rel-a").unwrap();

    let quiescing = Arc::clone(&runtime);
    let thread = thread::spawn(move || quiescing.quiesce_and_reopen());
    runtime.wait_until_admission_closed();
    assert!(
        runtime
            .begin_commit(runtime.token("rel-a", "too-late"), "rel-a")
            .is_none()
    );
    assert_eq!(
        runtime.pending(),
        (Vec::new(), vec!["queued-accepted".to_owned()])
    );
    drop(lease);
    assert_eq!(thread.join().unwrap(), 3);
}

#[test]
fn revocation_closes_new_commit_admission_then_waits_for_prior_lease() {
    let runtime = Arc::new(OwnerRuntime::start_at_epoch(5, 11));
    let token = runtime.token("rel-a", "write-before-revoke");
    let mut lease = runtime.begin_commit(token.clone(), "rel-a").unwrap();

    let revoking = Arc::clone(&runtime);
    let thread = thread::spawn(move || revoking.revoke_and_drain());
    runtime.wait_until_admission_closed();
    assert!(runtime.begin_commit(token.clone(), "rel-a").is_none());
    assert!(
        lease.apply_stage(),
        "a lease admitted before revocation may finish"
    );
    drop(lease);
    assert_eq!(thread.join().unwrap(), 12);
    assert_eq!(runtime.applied_count(), 1);
    assert!(runtime.begin_commit(token, "rel-a").is_none());
}

#[test]
fn retry_after_partial_cross_owner_publication_is_idempotent_by_stage_id() {
    let phy = OwnerRuntime::start_at_epoch(1, 3);
    let token = phy.token("phy-user-a", "insomnia-op-88:phy-stage");
    let mut first = phy.begin_commit(token.clone(), "phy-user-a").unwrap();
    assert!(first.apply_stage());
    drop(first); // Simulated crash before the REL receipt stage.

    let mut retry = phy.begin_commit(token, "phy-user-a").unwrap();
    assert!(
        !retry.apply_stage(),
        "same durable stage ID must not publish twice"
    );
    drop(retry);
    assert_eq!(phy.applied_count(), 1);
}

#[test]
fn owner_quiesce_does_not_block_an_unrelated_owner() {
    let rel_a = Arc::new(OwnerRuntime::start_at_epoch(1, 2));
    let rel_b = OwnerRuntime::start_at_epoch(9, 2);
    let held = rel_a
        .begin_commit(rel_a.token("rel-a", "slow-commit"), "rel-a")
        .unwrap();

    let quiescing = Arc::clone(&rel_a);
    let thread = thread::spawn(move || quiescing.quiesce_and_reopen());
    rel_a.wait_until_admission_closed();

    let mut independent = rel_b
        .begin_commit(rel_b.token("rel-b", "other-owner-write"), "rel-b")
        .unwrap();
    assert!(independent.apply_stage());
    drop(independent);
    drop(held);
    assert_eq!(thread.join().unwrap(), 2);
    assert_eq!(rel_b.applied_count(), 1);
}

#[test]
fn revocation_wins_when_it_closes_admission_before_commit_lease() {
    let runtime = OwnerRuntime::start_at_epoch(3, 20);
    let stale = runtime.token("rel-a", "write-after-revoke");
    assert_eq!(runtime.revoke_and_drain(), 21);
    assert!(runtime.begin_commit(stale, "rel-a").is_none());
}
