//! File-backed controlled adapter: durable acceptance/status; no provider effects.
use super::fixture::{attach, metric, request, seeded};
use super::*;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::{
    Arc, Barrier, Mutex,
    atomic::{AtomicBool, Ordering},
};

#[derive(Clone, Serialize, Deserialize)]
struct Entry {
    digest: [u8; 32],
    status: HostOperationStatus,
}
pub(super) struct DurableHost {
    path: PathBuf,
    entries: Mutex<BTreeMap<String, Entry>>,
    lose_ack: AtomicBool,
    calls: Mutex<Vec<String>>,
}
impl DurableHost {
    pub(super) fn at(path: &Path) -> Self {
        let entries = if path.exists() {
            serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
        } else {
            BTreeMap::new()
        };
        Self {
            path: path.into(),
            entries: Mutex::new(entries),
            lose_ack: AtomicBool::new(false),
            calls: Mutex::new(vec![]),
        }
    }
    fn persist(&self, entries: &BTreeMap<String, Entry>) -> Result<(), String> {
        let tmp = self.path.with_extension("tmp");
        let mut file = std::fs::File::create(&tmp).map_err(|e| e.to_string())?;
        std::io::Write::write_all(
            &mut file,
            &serde_json::to_vec(entries).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        file.sync_all().map_err(|e| e.to_string())?;
        drop(file);
        std::fs::rename(tmp, &self.path).map_err(|e| e.to_string())
    }
    pub(super) fn lose_ack(&self) {
        self.lose_ack.store(true, Ordering::SeqCst);
    }
    pub(super) fn operation_id(&self) -> String {
        self.entries.lock().unwrap().keys().next().unwrap().clone()
    }
    pub(super) fn set_status(&self, id: &str, status: HostOperationStatus) {
        let mut entries = self.entries.lock().unwrap();
        entries.get_mut(id).unwrap().status = status;
        self.persist(&entries).unwrap();
    }
    fn dequeue(&self, id: &str) -> bool {
        let mut entries = self.entries.lock().unwrap();
        let entry = entries.get_mut(id).unwrap();
        if entry.status != HostOperationStatus::Pending {
            return false;
        }
        entry.status = HostOperationStatus::Running;
        self.persist(&entries).unwrap();
        true
    }
    fn unique_acceptances(&self) -> usize {
        self.entries.lock().unwrap().len()
    }
    pub(super) fn calls(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }
}
impl GatewayHostAdapter for DurableHost {
    fn configured_capabilities(&self) -> BTreeSet<RuntimeCapability> {
        BTreeSet::from([RuntimeCapability::ConversationExecution])
    }
    fn connect(&self) -> Result<HostConnection, String> {
        Ok(HostConnection {
            capabilities: BTreeMap::from([(
                RuntimeCapability::ConversationExecution,
                HostCapabilityContract {
                    host_executes_stage: true,
                    queue: HostQueueGuarantees {
                        durable_acceptance: true,
                        stable_submission_ids: true,
                        linearizable_cancel_dequeue: true,
                        queryable_operation_status: true,
                        idempotent_handoff: true,
                    },
                },
            )]),
        })
    }
    fn reconcile(&self, _: RuntimeCapability) -> Result<HostReconcileStatus, String> {
        Ok(
            if self.entries.lock().unwrap().values().any(|e| {
                matches!(
                    e.status,
                    HostOperationStatus::Pending
                        | HostOperationStatus::Running
                        | HostOperationStatus::Unknown
                )
            }) {
                HostReconcileStatus::Ready
            } else {
                HostReconcileStatus::NoOutstandingWork
            },
        )
    }
    fn handoff(&self, _: RuntimeCapability, op: &HostOperation) -> Result<HandoffReceipt, String> {
        op.validate().map_err(|e| e.to_string())?;
        self.calls
            .lock()
            .unwrap()
            .push(format!("handoff:{}", op.stable_id));
        let mut entries = self.entries.lock().unwrap();
        if let Some(existing) = entries.get(&op.stable_id) {
            if existing.digest != op.payload_digest {
                return Err("digest conflict".into());
            }
        } else {
            entries.insert(
                op.stable_id.clone(),
                Entry {
                    digest: op.payload_digest,
                    status: HostOperationStatus::Pending,
                },
            );
        }
        self.persist(&entries)?;
        drop(entries);
        if self.lose_ack.swap(false, Ordering::SeqCst) {
            return Err("accepted durably; acknowledgment lost".into());
        }
        Ok(HandoffReceipt {
            stable_id: op.stable_id.clone(),
            payload_digest: op.payload_digest,
            durably_accepted: true,
        })
    }
    fn cancel(&self, _: RuntimeCapability, id: &str) -> Result<HostCancelOutcome, String> {
        self.calls.lock().unwrap().push(format!("cancel:{id}"));
        let mut entries = self.entries.lock().unwrap();
        let Some(entry) = entries.get_mut(id) else {
            return Ok(HostCancelOutcome::Unknown);
        };
        let outcome = match entry.status {
            HostOperationStatus::Pending => {
                entry.status = HostOperationStatus::Cancelled;
                HostCancelOutcome::CancelledBeforeStart
            }
            HostOperationStatus::Running => HostCancelOutcome::RunningCancellationRequested,
            HostOperationStatus::Unknown => HostCancelOutcome::Unknown,
            status => HostCancelOutcome::AlreadyTerminal(status),
        };
        self.persist(&entries)?;
        Ok(outcome)
    }
    fn status(&self, _: RuntimeCapability, id: &str) -> Result<HostOperationStatus, String> {
        self.calls.lock().unwrap().push(format!("status:{id}"));
        Ok(self
            .entries
            .lock()
            .unwrap()
            .get(id)
            .map(|e| e.status)
            .unwrap_or(HostOperationStatus::Unknown))
    }
}
pub(super) fn gateway_host(root: &Path, host: Arc<DurableHost>) -> GatewayRuntime {
    GatewayRuntime::new_with_host(
        root.join("state"),
        Default::default(),
        crate::InsomniaWorkerConfig {
            workers: 0,
            ..Default::default()
        },
        Default::default(),
        host,
    )
}
#[test]
fn file_backed_host_unknown_holds_without_redispatch_and_completion_is_not_cancel_rollback() {
    let f = super::fixture::Fixture::new("durable-host", 0, 0);
    let host = Arc::new(DurableHost::at(&f.root.join("host.json")));
    let g = gateway_host(&f.root, host.clone());
    let owner = g.mount_rel(seeded(&f.root.join("host.rel"))).unwrap();
    let writer = attach(&g, &[owner.clone()], None, "alice", true);
    host.lose_ack();
    g.submit_conversation(&writer, &owner, request(&g, &owner, "a", 400, "root-a"))
        .unwrap();
    g.process_next_conversation_submission(&writer, &owner, "a")
        .unwrap()
        .unwrap();
    let id = host.operation_id();
    host.set_status(&id, HostOperationStatus::Unknown);
    let before = host
        .calls()
        .iter()
        .filter(|s| s.starts_with("handoff:"))
        .count();
    g.resume_conversation_submission(&writer, &owner, "a", &super::fixture::id(400))
        .unwrap();
    assert_eq!(
        g.capability_authority(RuntimeCapability::ConversationExecution)
            .unwrap(),
        StageAuthority::Held
    );
    assert_eq!(
        host.calls()
            .iter()
            .filter(|s| s.starts_with("handoff:"))
            .count(),
        before
    );
    host.set_status(&id, HostOperationStatus::Running);
    let cancel = g
        .cancel_conversation_generation(&writer, &owner, "a", &id)
        .unwrap();
    assert_eq!(
        cancel.status.state,
        crate::multiplexing_journal::SubmissionState::Pending
    );
    host.set_status(&id, HostOperationStatus::Completed);
    let result = g
        .resume_conversation_submission(&writer, &owner, "a", &super::fixture::id(400))
        .unwrap();
    assert_eq!(
        result.status.state,
        crate::multiplexing_journal::SubmissionState::Committed
    );
    assert!(
        g.process_next_accepted_conversation_submission(&owner, "a")
            .unwrap()
            .is_none()
    );
    assert_eq!(host.unique_acceptances(), 1);
    metric(
        "durable_host",
        json!({"unique_acceptances":1,"provider_effects":"not modeled","calls":host.calls()}),
    );
}
#[test]
fn actual_adapter_pending_cancel_and_dequeue_share_one_serialization_gate() {
    for index in 0..8 {
        let f = super::fixture::Fixture::new("host-dequeue", 0, 0);
        let host = Arc::new(DurableHost::at(&f.root.join("host.json")));
        let g = Arc::new(gateway_host(&f.root, host.clone()));
        let owner = g.mount_rel(seeded(&f.root.join("host.rel"))).unwrap();
        let writer = attach(&g, &[owner.clone()], None, "alice", true);
        g.submit_conversation(
            &writer,
            &owner,
            request(&g, &owner, "a", 500 + index, "root-a"),
        )
        .unwrap();
        g.process_next_conversation_submission(&writer, &owner, "a")
            .unwrap()
            .unwrap();
        let id = host.operation_id();
        let start = Arc::new(Barrier::new(3));
        let cancel = {
            let g = g.clone();
            let w = writer.clone();
            let o = owner.clone();
            let id = id.clone();
            let b = start.clone();
            std::thread::spawn(move || {
                b.wait();
                g.cancel_conversation_generation(&w, &o, "a", &id).unwrap()
            })
        };
        let dequeue = {
            let h = host.clone();
            let id = id.clone();
            let b = start.clone();
            std::thread::spawn(move || {
                b.wait();
                h.dequeue(&id)
            })
        };
        start.wait();
        let cancelled = cancel.join().unwrap();
        let started = dequeue.join().unwrap();
        let status = host
            .status(RuntimeCapability::ConversationExecution, &id)
            .unwrap();
        if started {
            assert_eq!(status, HostOperationStatus::Running);
            assert_eq!(
                cancelled.status.state,
                crate::multiplexing_journal::SubmissionState::Pending
            );
        } else {
            assert_eq!(status, HostOperationStatus::Cancelled);
            assert_eq!(
                cancelled.status.state,
                crate::multiplexing_journal::SubmissionState::Cancelled
            );
        }
    }
}
