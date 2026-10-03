//! Executable reference contract for host/native queue dispatch.
//!
//! This is deliberately a std-only model, not Reliquary production code or proof
//! of production durability. Run with:
//!   cargo test --test multiplexing_phase0b_host_contract

use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum Stage {
    Conversation,
    DurableQueue,
    EventIngest,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Authority {
    Native,
    Host,
}

#[derive(Clone, Copy, Debug, Default)]
struct HostProof {
    durable_queue: bool,
    cancel_by_stable_id: bool,
    status_reconciliation: bool,
    stable_job_ids: bool,
}

impl HostProof {
    fn may_own_queue(self) -> bool {
        self.durable_queue
            && self.cancel_by_stable_id
            && self.status_reconciliation
            && self.stable_job_ids
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Link {
    Offline,
    Connecting,
    Online,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum JobState {
    NativePending,
    NativeRunning,
    HandoffIntent,
    HostAcceptedUnacked,
    HostPending,
    HostRunning,
    CancelRequested,
    Cancelled,
    Completed,
    HeldUnknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum HostStatus {
    Queued,
    Running,
    Completed,
    Cancelled,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Resolution {
    KeepHostPending,
    HostRunning,
    ArchiveCompleted,
    RecordCancelledReceipt,
    HoldNoReplay,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Dispatch {
    RunNative,
    SendHost,
    Hold,
}

#[derive(Clone, Debug)]
struct Job {
    id: String,
    stage: Stage,
    state: JobState,
    cancellation_durable: bool,
    cancel_requested_durable: bool,
    archive_count: u8,
    terminal_receipt_count: u8,
    native_execution_count: u8,
    host_submission_count: u8,
    host_receipt_count: u8,
    host_execution_count: u8,
}

impl Job {
    fn new(id: &str, stage: Stage) -> Self {
        Self {
            id: id.to_owned(),
            stage,
            state: JobState::NativePending,
            cancellation_durable: false,
            cancel_requested_durable: false,
            archive_count: 0,
            terminal_receipt_count: 0,
            native_execution_count: 0,
            host_submission_count: 0,
            host_receipt_count: 0,
            host_execution_count: 0,
        }
    }
}

#[derive(Clone, Debug)]
struct Runtime {
    link: Link,
    proof: HostProof,
    authorities: BTreeMap<Stage, Authority>,
    startup_connect_attempts: u8,
    trace: Vec<String>,
    dispatches: Vec<(String, Dispatch)>,
    admission_fenced: bool,
    jobs: BTreeMap<String, Job>,
    durable_cancel_log: BTreeSet<String>,
}

impl Runtime {
    fn new(proof: HostProof, authorities: &[(Stage, Authority)]) -> Self {
        Self {
            link: Link::Offline,
            proof,
            authorities: authorities.iter().copied().collect(),
            startup_connect_attempts: 0,
            trace: Vec::new(),
            dispatches: Vec::new(),
            admission_fenced: false,
            jobs: BTreeMap::new(),
            durable_cancel_log: BTreeSet::new(),
        }
    }

    /// Startup attempts the configured host connection before queue-dependent work.
    fn startup(&mut self, connection_succeeds: bool) {
        self.link = Link::Connecting;
        self.startup_connect_attempts += 1;
        self.trace.push("configured-connect-attempt".to_owned());
        self.link = if connection_succeeds {
            Link::Online
        } else {
            Link::Offline
        };
    }

    fn reconnect(&mut self, connection_succeeds: bool) {
        self.link = Link::Connecting;
        self.link = if connection_succeeds {
            Link::Online
        } else {
            Link::Offline
        };
    }

    fn add_job(&mut self, job: Job) -> bool {
        if self.admission_fenced || self.jobs.contains_key(&job.id) {
            return false;
        }
        self.jobs.insert(job.id.clone(), job);
        true
    }

    fn authority(&self, stage: Stage) -> Authority {
        self.authorities
            .get(&stage)
            .copied()
            .unwrap_or(Authority::Native)
    }

    fn dispatch(&mut self, id: &str) -> Dispatch {
        let Some(stage) = self.jobs.get(id).map(|job| job.stage) else {
            return Dispatch::Hold;
        };
        let authority = self.authority(stage);
        let link = self.link;
        let host_proof = self.proof.may_own_queue();
        let admission_fenced = self.admission_fenced;
        let startup_attempted = self.startup_connect_attempts > 0;
        let Some(job) = self.jobs.get_mut(id) else {
            return Dispatch::Hold;
        };
        if job.cancellation_durable
            || matches!(job.state, JobState::Cancelled | JobState::Completed)
        {
            return Dispatch::Hold;
        }
        let result = match authority {
            Authority::Native => {
                if job.state == JobState::NativePending {
                    job.state = JobState::NativeRunning;
                    job.native_execution_count += 1;
                    Dispatch::RunNative
                } else {
                    Dispatch::Hold
                }
            }
            Authority::Host => {
                if !startup_attempted || !host_proof || link != Link::Online || admission_fenced {
                    Dispatch::Hold
                } else if job.state == JobState::NativePending {
                    // Persist intent before sending. Ownership changes only after the host
                    // returns a durable acceptance receipt for this same stable ID.
                    job.state = JobState::HandoffIntent;
                    job.host_submission_count += 1;
                    Dispatch::SendHost
                } else {
                    Dispatch::Hold
                }
            }
        };
        self.dispatches.push((id.to_owned(), result));
        self.trace.push(format!("dispatch:{id}:{result:?}"));
        result
    }

    fn accept_host_handoff(&mut self, id: &str, receipt_has_same_stable_id: bool) -> bool {
        let Some(job) = self.jobs.get_mut(id) else {
            return false;
        };
        if !receipt_has_same_stable_id
            || job.host_receipt_count != 0
            || !matches!(
                job.state,
                JobState::HandoffIntent | JobState::CancelRequested
            )
        {
            return false;
        }
        job.host_receipt_count += 1;
        if job.state == JobState::HandoffIntent {
            job.state = JobState::HostAcceptedUnacked;
        }
        true
    }

    /// Sender records its acknowledgement only after the durable receiver receipt.
    fn acknowledge_transfer(&mut self, id: &str) -> bool {
        let Some(job) = self.jobs.get_mut(id) else {
            return false;
        };
        if job.state != JobState::HostAcceptedUnacked || job.host_receipt_count != 1 {
            return false;
        }
        job.state = JobState::HostPending;
        true
    }

    /// Host pending cancellation and host dequeue share one atomic pending-state gate.
    fn dequeue_host(&mut self, id: &str) -> bool {
        let Some(job) = self.jobs.get_mut(id) else {
            return false;
        };
        if job.state != JobState::HostPending || job.cancel_requested_durable {
            return false;
        }
        job.state = JobState::HostRunning;
        job.host_execution_count += 1;
        true
    }

    /// Native claim and host handoff compete on the same pending-state transition.
    fn claim_native(&mut self, id: &str) -> bool {
        let authority = self.jobs.get(id).map(|job| self.authority(job.stage));
        let Some(job) = self.jobs.get_mut(id) else {
            return false;
        };
        if job.stage != Stage::DurableQueue
            || job.state != JobState::NativePending
            || authority != Some(Authority::Native)
            || job.cancellation_durable
        {
            return false;
        }
        job.state = JobState::NativeRunning;
        job.native_execution_count += 1;
        true
    }

    /// Cancellation is recorded durably before a restart can replay pending work.
    fn cancel(&mut self, id: &str) -> bool {
        let Some(job) = self.jobs.get_mut(id) else {
            return false;
        };
        if matches!(job.state, JobState::Cancelled | JobState::Completed) {
            return false;
        }
        if job.state == JobState::NativeRunning
            || job.state == JobState::HostRunning
            || job.state == JobState::HostPending
            || job.state == JobState::HostAcceptedUnacked
            || job.state == JobState::HandoffIntent
        {
            // Honest cooperative result: this requests stop; it does not claim the work stopped.
            self.durable_cancel_log.insert(id.to_owned());
            job.cancel_requested_durable = true;
            job.state = JobState::CancelRequested;
            return true;
        }
        self.durable_cancel_log.insert(id.to_owned());
        job.cancellation_durable = true;
        job.state = JobState::Cancelled;
        true
    }

    /// A cancellation command may only mutate work owned by the caller's stage.
    fn cancel_owned_stage(&mut self, caller_stage: Stage, id: &str) -> bool {
        if self.jobs.get(id).map(|job| job.stage) != Some(caller_stage) {
            return false;
        }
        self.cancel(id)
    }

    /// Cross-stage cancellation requests the receiving authority by stable ID.
    fn propagate_cancel(&mut self, id: &str) -> bool {
        let Some(receiver_stage) = self.jobs.get(id).map(|job| job.stage) else {
            return false;
        };
        self.cancel_owned_stage(receiver_stage, id)
    }

    /// A restart restores the durable cancellation ledger before examining/replaying work.
    fn restart_restore(&mut self) {
        for (id, job) in self.jobs.iter_mut() {
            if self.durable_cancel_log.contains(id) {
                if job.host_submission_count > 0 || job.native_execution_count > 0 {
                    job.cancel_requested_durable = true;
                    if !matches!(job.state, JobState::Completed | JobState::Cancelled) {
                        job.state = JobState::CancelRequested;
                    }
                } else {
                    job.cancellation_durable = true;
                    if !matches!(job.state, JobState::Completed) {
                        job.state = JobState::Cancelled;
                    }
                }
            }
        }
    }

    fn reconcile_host(&mut self, id: &str, status: HostStatus) -> Resolution {
        let Some(job) = self.jobs.get_mut(id) else {
            return Resolution::HoldNoReplay;
        };
        if !matches!(
            job.state,
            JobState::HandoffIntent
                | JobState::HostAcceptedUnacked
                | JobState::HostPending
                | JobState::HostRunning
                | JobState::HeldUnknown
                | JobState::CancelRequested
        ) {
            return Resolution::HoldNoReplay;
        }
        match status {
            HostStatus::Queued => {
                job.state = if job.cancel_requested_durable {
                    JobState::CancelRequested
                } else if job.host_receipt_count > 0 {
                    JobState::HostAcceptedUnacked
                } else {
                    JobState::HostPending
                };
                Resolution::KeepHostPending
            }
            HostStatus::Running => {
                job.state = if job.cancel_requested_durable {
                    JobState::CancelRequested
                } else {
                    JobState::HostRunning
                };
                if job.host_execution_count == 0 {
                    job.host_execution_count = 1;
                }
                Resolution::HostRunning
            }
            HostStatus::Completed => {
                job.state = JobState::Completed;
                job.archive_count += 1;
                job.terminal_receipt_count += 1;
                Resolution::ArchiveCompleted
            }
            HostStatus::Cancelled => {
                job.state = JobState::Cancelled;
                job.cancellation_durable = true;
                self.durable_cancel_log.insert(id.to_owned());
                job.terminal_receipt_count += 1;
                Resolution::RecordCancelledReceipt
            }
            HostStatus::Unknown => {
                job.state = JobState::HeldUnknown;
                Resolution::HoldNoReplay
            }
        }
    }

    fn request_cancel_for_running_work(&mut self, id: &str) -> bool {
        let Some(job) = self.jobs.get_mut(id) else {
            return false;
        };
        if matches!(job.state, JobState::NativeRunning | JobState::HostRunning) {
            self.durable_cancel_log.insert(id.to_owned());
            job.cancel_requested_durable = true;
            job.state = JobState::CancelRequested;
            true
        } else {
            false
        }
    }

    fn acknowledge_cooperative_cancel(&mut self, id: &str, stopped: bool) -> bool {
        let Some(job) = self.jobs.get_mut(id) else {
            return false;
        };
        if job.state != JobState::CancelRequested {
            return false;
        }
        if stopped {
            self.durable_cancel_log.insert(id.to_owned());
            job.cancellation_durable = true;
            job.cancel_requested_durable = true;
            job.state = JobState::Cancelled;
        }
        true
    }

    /// Downgrade first fences new admission. Host-owned work remains assigned to host
    /// until status is known; there is no implicit replay under native authority.
    fn downgrade_fence(&mut self) {
        self.admission_fenced = true;
    }
}

fn complete_authorities() -> HostProof {
    HostProof {
        durable_queue: true,
        cancel_by_stable_id: true,
        status_reconciliation: true,
        stable_job_ids: true,
    }
}

#[test]
fn startup_attempt_precedes_dispatch_and_native_work_runs_offline() {
    let mut rt = Runtime::new(
        complete_authorities(),
        &[
            (Stage::DurableQueue, Authority::Host),
            (Stage::EventIngest, Authority::Native),
        ],
    );
    rt.add_job(Job::new("q-1", Stage::DurableQueue));
    rt.add_job(Job::new("e-1", Stage::EventIngest));
    assert_eq!(rt.dispatch("q-1"), Dispatch::Hold); // dispatch itself enforces startup attempt
    rt.startup(false);
    assert_eq!(rt.startup_connect_attempts, 1);
    assert_eq!(rt.dispatch("q-1"), Dispatch::Hold);
    assert_eq!(rt.dispatch("e-1"), Dispatch::RunNative);
    assert_eq!(rt.jobs["q-1"].state, JobState::NativePending);
    assert_eq!(rt.dispatches[0], ("q-1".into(), Dispatch::Hold));
    assert_eq!(rt.dispatches[1], ("q-1".into(), Dispatch::Hold));
    assert_eq!(rt.dispatches[2], ("e-1".into(), Dispatch::RunNative));
    assert_eq!(rt.trace[1], "configured-connect-attempt");
    assert!(rt.trace[2].starts_with("dispatch:q-1:"));
}

#[test]
fn host_dispatch_itself_requires_startup_attempt_then_connection() {
    let mut rt = Runtime::new(
        complete_authorities(),
        &[(Stage::DurableQueue, Authority::Host)],
    );
    rt.add_job(Job::new("startup-gated", Stage::DurableQueue));
    assert_eq!(rt.dispatch("startup-gated"), Dispatch::Hold);
    assert_eq!(rt.startup_connect_attempts, 0);
    rt.startup(true);
    assert_eq!(rt.dispatch("startup-gated"), Dispatch::SendHost);
    let connect = rt
        .trace
        .iter()
        .position(|e| e == "configured-connect-attempt")
        .unwrap();
    let host_send = rt
        .trace
        .iter()
        .position(|e| e == "dispatch:startup-gated:SendHost")
        .unwrap();
    assert!(connect < host_send);
}

#[test]
fn host_queue_is_selected_only_after_every_capability_is_proven() {
    for missing in 0..4 {
        let mut proof = complete_authorities();
        match missing {
            0 => proof.durable_queue = false,
            1 => proof.cancel_by_stable_id = false,
            2 => proof.status_reconciliation = false,
            _ => proof.stable_job_ids = false,
        }
        let mut rt = Runtime::new(proof, &[(Stage::DurableQueue, Authority::Host)]);
        rt.startup(true);
        rt.add_job(Job::new("stable-1", Stage::DurableQueue));
        assert_eq!(
            rt.dispatch("stable-1"),
            Dispatch::Hold,
            "missing proof index {missing}"
        );
        assert_eq!(rt.jobs["stable-1"].host_submission_count, 0);
    }
}

#[test]
fn stage_authorities_are_independent_and_do_not_share_one_global_queue() {
    let mut rt = Runtime::new(
        complete_authorities(),
        &[
            (Stage::DurableQueue, Authority::Host),
            (Stage::Conversation, Authority::Native),
        ],
    );
    rt.startup(true);
    rt.add_job(Job::new("q", Stage::DurableQueue));
    rt.add_job(Job::new("c", Stage::Conversation));
    assert_eq!(rt.dispatch("q"), Dispatch::SendHost);
    assert_eq!(rt.dispatch("c"), Dispatch::RunNative);
    assert_eq!(rt.jobs["q"].host_submission_count, 1);
    assert_eq!(rt.jobs["c"].native_execution_count, 1);
}

#[test]
fn upstream_cancellation_cannot_cross_stage_ownership() {
    let mut rt = Runtime::new(
        complete_authorities(),
        &[
            (Stage::DurableQueue, Authority::Host),
            (Stage::Conversation, Authority::Native),
        ],
    );
    rt.startup(true);
    rt.add_job(Job::new("host-stage", Stage::DurableQueue));
    rt.add_job(Job::new("conversation-stage", Stage::Conversation));
    assert_eq!(rt.dispatch("host-stage"), Dispatch::SendHost);
    assert!(rt.accept_host_handoff("host-stage", true));
    assert!(rt.acknowledge_transfer("host-stage"));
    assert!(!rt.cancel_owned_stage(Stage::Conversation, "host-stage"));
    assert_eq!(rt.jobs["host-stage"].state, JobState::HostPending);
    assert_eq!(rt.dispatch("conversation-stage"), Dispatch::RunNative);
    assert!(rt.cancel_owned_stage(Stage::Conversation, "conversation-stage"));
}

#[test]
fn completed_commit_wins_over_a_late_pending_cancel() {
    let mut rt = Runtime::new(
        HostProof::default(),
        &[(Stage::EventIngest, Authority::Native)],
    );
    let mut job = Job::new("finalize-won", Stage::EventIngest);
    job.state = JobState::Completed; // finalization commit crossed its linearization point
    rt.add_job(job);
    assert!(!rt.cancel_owned_stage(Stage::EventIngest, "finalize-won"));
    assert_eq!(rt.jobs["finalize-won"].state, JobState::Completed);
}

#[test]
fn reconnect_reconciles_unknown_without_duplicate_execution_or_archive() {
    let mut rt = Runtime::new(
        complete_authorities(),
        &[(Stage::DurableQueue, Authority::Host)],
    );
    rt.startup(true);
    rt.add_job(Job::new("stable-2", Stage::DurableQueue));
    assert_eq!(rt.dispatch("stable-2"), Dispatch::SendHost);
    assert!(rt.accept_host_handoff("stable-2", true));
    assert!(rt.acknowledge_transfer("stable-2"));
    rt.reconnect(true);
    assert_eq!(
        rt.reconcile_host("stable-2", HostStatus::Unknown),
        Resolution::HoldNoReplay
    );
    assert_eq!(rt.jobs["stable-2"].state, JobState::HeldUnknown);
    assert_eq!(rt.dispatch("stable-2"), Dispatch::Hold);
    assert_eq!(rt.jobs["stable-2"].native_execution_count, 0);
    assert_eq!(rt.jobs["stable-2"].host_submission_count, 1);
    assert_eq!(rt.jobs["stable-2"].archive_count, 0);
}

#[test]
fn durable_cancel_restores_before_restart_replay() {
    let mut rt = Runtime::new(
        complete_authorities(),
        &[(Stage::DurableQueue, Authority::Host)],
    );
    rt.startup(false);
    rt.add_job(Job::new("cancel-first", Stage::DurableQueue));
    assert!(rt.cancel("cancel-first")); // persisted while host is unavailable
    rt.restart_restore();
    rt.reconnect(true);
    assert_eq!(rt.dispatch("cancel-first"), Dispatch::Hold);
    assert_eq!(rt.jobs["cancel-first"].state, JobState::Cancelled);
    assert_eq!(rt.jobs["cancel-first"].host_submission_count, 0);
    assert_eq!(rt.jobs["cancel-first"].native_execution_count, 0);
}

#[test]
fn cancellation_after_host_acceptance_survives_restart_as_request_not_false_completion() {
    let mut rt = Runtime::new(
        complete_authorities(),
        &[(Stage::DurableQueue, Authority::Host)],
    );
    rt.startup(true);
    rt.add_job(Job::new("host-cancel", Stage::DurableQueue));
    assert_eq!(rt.dispatch("host-cancel"), Dispatch::SendHost);
    assert!(rt.accept_host_handoff("host-cancel", true));
    assert!(rt.acknowledge_transfer("host-cancel"));
    assert!(rt.cancel("host-cancel"));
    rt.restart_restore();
    assert_eq!(rt.jobs["host-cancel"].state, JobState::CancelRequested);
    assert!(!rt.jobs["host-cancel"].cancellation_durable);
    assert_eq!(rt.dispatch("host-cancel"), Dispatch::Hold);
    assert_eq!(rt.jobs["host-cancel"].host_submission_count, 1);
}

#[test]
fn cancel_and_handoff_interleavings_have_exactly_one_winner() {
    // Both operation orders are adversarial schedules around the shared pending-state CAS.
    for cancel_first in [true, false] {
        let mut rt = Runtime::new(
            complete_authorities(),
            &[(Stage::DurableQueue, Authority::Host)],
        );
        rt.startup(true);
        rt.add_job(Job::new("race", Stage::DurableQueue));
        let (cancelled, handoff) = if cancel_first {
            let c = rt.cancel("race");
            let h = rt.dispatch("race");
            (c, h)
        } else {
            let h = rt.dispatch("race");
            assert!(rt.accept_host_handoff("race", true));
            assert!(rt.acknowledge_transfer("race"));
            let c = rt.cancel("race");
            (c, h)
        };
        let job = &rt.jobs["race"];
        let cancellation_won = job.state == JobState::Cancelled;
        let handoff_won = job.host_receipt_count == 1;
        assert_ne!(
            cancellation_won, handoff_won,
            "exactly one winner in order {cancel_first}"
        );
        assert_eq!(job.native_execution_count, 0);
        assert_eq!(job.host_receipt_count, u8::from(handoff_won));
        if cancel_first {
            assert!(cancelled);
            assert_eq!(handoff, Dispatch::Hold);
        } else {
            assert_eq!(handoff, Dispatch::SendHost);
            assert!(cancelled); // later cancellation becomes an explicit host cancel request
            assert_eq!(job.state, JobState::CancelRequested);
        }
    }
}

#[test]
fn native_claim_and_cancel_race_also_have_one_winner() {
    for cancel_first in [true, false] {
        let mut rt = Runtime::new(
            HostProof::default(),
            &[(Stage::DurableQueue, Authority::Native)],
        );
        rt.add_job(Job::new("native-race", Stage::DurableQueue));
        if cancel_first {
            assert!(rt.cancel("native-race"));
            assert!(!rt.claim_native("native-race"));
        } else {
            assert!(rt.claim_native("native-race"));
            assert!(rt.cancel("native-race")); // transitions to cooperative cancellation request
        }
        let j = &rt.jobs["native-race"];
        assert!(j.cancellation_durable || j.state == JobState::CancelRequested);
        assert_eq!(j.native_execution_count, u8::from(!cancel_first));
    }
}

#[test]
fn running_cancellation_reports_cooperative_request_until_acknowledged() {
    let mut rt = Runtime::new(
        complete_authorities(),
        &[(Stage::DurableQueue, Authority::Host)],
    );
    rt.startup(true);
    rt.add_job(Job::new("running", Stage::DurableQueue));
    assert_eq!(rt.dispatch("running"), Dispatch::SendHost);
    assert!(rt.accept_host_handoff("running", true));
    assert!(rt.acknowledge_transfer("running"));
    assert_eq!(
        rt.reconcile_host("running", HostStatus::Running),
        Resolution::HostRunning
    );
    assert!(rt.request_cancel_for_running_work("running"));
    assert_eq!(rt.jobs["running"].state, JobState::CancelRequested);
    assert!(!rt.jobs["running"].cancellation_durable);
    assert!(rt.acknowledge_cooperative_cancel("running", false));
    assert_eq!(rt.jobs["running"].state, JobState::CancelRequested);
    assert!(rt.acknowledge_cooperative_cancel("running", true));
    assert_eq!(rt.jobs["running"].state, JobState::Cancelled);
    assert!(rt.jobs["running"].cancellation_durable);
}

#[test]
fn downgrade_fences_admission_and_never_replays_unknown_host_work_natively() {
    let mut rt = Runtime::new(
        complete_authorities(),
        &[(Stage::DurableQueue, Authority::Host)],
    );
    rt.startup(true);
    rt.add_job(Job::new("old-host", Stage::DurableQueue));
    assert_eq!(rt.dispatch("old-host"), Dispatch::SendHost);
    assert!(rt.accept_host_handoff("old-host", true));
    assert!(rt.acknowledge_transfer("old-host"));
    rt.downgrade_fence();
    assert!(!rt.add_job(Job::new("new", Stage::DurableQueue)));
    rt.reconnect(true);
    assert_eq!(
        rt.reconcile_host("old-host", HostStatus::Unknown),
        Resolution::HoldNoReplay
    );
    assert_eq!(rt.dispatch("old-host"), Dispatch::Hold);
    assert_eq!(rt.jobs["old-host"].native_execution_count, 0);
    assert_eq!(rt.jobs["old-host"].host_submission_count, 1);
}

#[test]
fn host_cancel_and_dequeue_interleavings_have_one_winner_and_no_false_archive() {
    // Cancellation wins the host pending-state gate: dequeue/provider execution is forbidden.
    let mut cancel_wins = Runtime::new(
        complete_authorities(),
        &[(Stage::DurableQueue, Authority::Host)],
    );
    cancel_wins.startup(true);
    cancel_wins.add_job(Job::new("host-cancel-wins", Stage::DurableQueue));
    assert_eq!(cancel_wins.dispatch("host-cancel-wins"), Dispatch::SendHost);
    assert!(cancel_wins.accept_host_handoff("host-cancel-wins", true));
    assert!(cancel_wins.acknowledge_transfer("host-cancel-wins"));
    assert!(cancel_wins.cancel("host-cancel-wins"));
    assert!(!cancel_wins.dequeue_host("host-cancel-wins"));
    assert_eq!(cancel_wins.jobs["host-cancel-wins"].host_execution_count, 0);
    assert_eq!(
        cancel_wins.reconcile_host("host-cancel-wins", HostStatus::Cancelled),
        Resolution::RecordCancelledReceipt
    );
    assert_eq!(cancel_wins.jobs["host-cancel-wins"].archive_count, 0);
    assert_eq!(
        cancel_wins.jobs["host-cancel-wins"].terminal_receipt_count,
        1
    );

    // Dequeue wins first: execution began, so cancellation is a cooperative request.
    let mut dequeue_wins = Runtime::new(
        complete_authorities(),
        &[(Stage::DurableQueue, Authority::Host)],
    );
    dequeue_wins.startup(true);
    dequeue_wins.add_job(Job::new("host-dequeue-wins", Stage::DurableQueue));
    assert_eq!(
        dequeue_wins.dispatch("host-dequeue-wins"),
        Dispatch::SendHost
    );
    assert!(dequeue_wins.accept_host_handoff("host-dequeue-wins", true));
    assert!(dequeue_wins.acknowledge_transfer("host-dequeue-wins"));
    assert!(dequeue_wins.dequeue_host("host-dequeue-wins"));
    assert!(dequeue_wins.cancel("host-dequeue-wins"));
    assert_eq!(
        dequeue_wins.jobs["host-dequeue-wins"].state,
        JobState::CancelRequested
    );
    assert!(!dequeue_wins.jobs["host-dequeue-wins"].cancellation_durable);
    assert_eq!(
        dequeue_wins.jobs["host-dequeue-wins"].host_execution_count,
        1
    );
    assert!(dequeue_wins.acknowledge_cooperative_cancel("host-dequeue-wins", true));
    assert_eq!(
        dequeue_wins.jobs["host-dequeue-wins"].state,
        JobState::Cancelled
    );
    assert_eq!(dequeue_wins.jobs["host-dequeue-wins"].archive_count, 0);
}

#[test]
fn receiver_receipt_without_sender_ack_is_reconciled_by_same_id() {
    let mut rt = Runtime::new(
        complete_authorities(),
        &[(Stage::DurableQueue, Authority::Host)],
    );
    rt.startup(true);
    rt.add_job(Job::new("ack-gap", Stage::DurableQueue));
    assert_eq!(rt.dispatch("ack-gap"), Dispatch::SendHost);
    assert!(rt.accept_host_handoff("ack-gap", true)); // crash point: receipt persisted, sender ack absent
    assert_eq!(rt.jobs["ack-gap"].state, JobState::HostAcceptedUnacked);
    assert_eq!(
        rt.reconcile_host("ack-gap", HostStatus::Queued),
        Resolution::KeepHostPending
    );
    assert!(rt.acknowledge_transfer("ack-gap"));
    assert_eq!(rt.jobs["ack-gap"].state, JobState::HostPending);
    assert_eq!(rt.dispatch("ack-gap"), Dispatch::Hold); // never submit a second copy
    assert_eq!(rt.jobs["ack-gap"].host_submission_count, 1);
}

#[test]
fn handoff_intent_with_unknown_receipt_holds_after_crash() {
    let mut rt = Runtime::new(
        complete_authorities(),
        &[(Stage::DurableQueue, Authority::Host)],
    );
    rt.startup(true);
    rt.add_job(Job::new("intent-gap", Stage::DurableQueue));
    assert_eq!(rt.dispatch("intent-gap"), Dispatch::SendHost); // durable intent, crash before receipt
    assert_eq!(
        rt.reconcile_host("intent-gap", HostStatus::Unknown),
        Resolution::HoldNoReplay
    );
    assert_eq!(rt.dispatch("intent-gap"), Dispatch::Hold);
    assert_eq!(rt.jobs["intent-gap"].host_submission_count, 1);
    assert_eq!(rt.jobs["intent-gap"].native_execution_count, 0);
}

#[test]
fn cancellation_reaches_receiver_between_acceptance_and_sender_ack() {
    let mut rt = Runtime::new(
        complete_authorities(),
        &[(Stage::DurableQueue, Authority::Host)],
    );
    rt.startup(true);
    rt.add_job(Job::new("handoff-cancel", Stage::DurableQueue));
    assert_eq!(rt.dispatch("handoff-cancel"), Dispatch::SendHost);
    assert!(rt.accept_host_handoff("handoff-cancel", true));
    assert!(rt.propagate_cancel("handoff-cancel"));
    assert_eq!(rt.jobs["handoff-cancel"].state, JobState::CancelRequested);
    assert!(!rt.jobs["handoff-cancel"].cancellation_durable);
    rt.restart_restore();
    assert_eq!(
        rt.reconcile_host("handoff-cancel", HostStatus::Cancelled),
        Resolution::RecordCancelledReceipt
    );
    assert!(!rt.dequeue_host("handoff-cancel"));
    assert_eq!(rt.jobs["handoff-cancel"].host_execution_count, 0);
    assert_eq!(rt.jobs["handoff-cancel"].archive_count, 0);
}

#[test]
fn recovered_host_completion_is_published_once_despite_late_cancel_and_replay() {
    let mut rt = Runtime::new(
        complete_authorities(),
        &[(Stage::DurableQueue, Authority::Host)],
    );
    rt.startup(true);
    rt.add_job(Job::new("completed-offline", Stage::DurableQueue));
    assert_eq!(rt.dispatch("completed-offline"), Dispatch::SendHost);
    assert!(rt.accept_host_handoff("completed-offline", true));
    rt.restart_restore();
    assert_eq!(
        rt.reconcile_host("completed-offline", HostStatus::Completed),
        Resolution::ArchiveCompleted
    );
    assert!(!rt.cancel("completed-offline"));
    assert_eq!(
        rt.reconcile_host("completed-offline", HostStatus::Completed),
        Resolution::HoldNoReplay
    );
    assert_eq!(rt.jobs["completed-offline"].archive_count, 1);
    assert_eq!(rt.jobs["completed-offline"].terminal_receipt_count, 1);
}
