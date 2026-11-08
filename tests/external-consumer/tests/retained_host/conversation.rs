use super::{FixtureDir, OPERATOR, VIEWER, admission_config};
use reliquary_memory::{
    ConversationQueue, ConversationSubmissionId, DurableHostOperationPhase, EnqueueOutcome,
    EpisodeBoundary, EpisodeId, FinalizeCapture, Gateway, GenerationReplay,
    GenerationStreamDelivery, GenerationStreamSnapshot, GenerationStreamSubscription,
    HandoffReceipt, HostCancelOutcome, HostCapabilityContract, HostCapabilityState, HostConnection,
    HostDispatchOutcome, HostExecutionMode, HostOperation, HostOperationStatus,
    HostQueueGuarantees, HostReconcileStatus, Instance, MemoryDeliveryOperation,
    MemoryDeliveryReceipt, MemoryDeliveryStatus, ProductError, ProductErrorKind, QueueError,
    QueueState, RuntimeCapability, SubmissionId, TrustedHostAdapter,
};
use std::{
    collections::BTreeSet,
    sync::{Arc, Mutex},
};

const EXECUTION: RuntimeCapability = RuntimeCapability::ConversationExecution;

#[derive(Default)]
struct Host {
    accepted: Mutex<Option<String>>,
}

impl TrustedHostAdapter for Host {
    fn configured_capabilities(&self) -> BTreeSet<RuntimeCapability> {
        BTreeSet::from([EXECUTION])
    }

    fn connect(&self) -> Result<HostConnection, String> {
        let contract = HostCapabilityContract::pending_queue(self.queue_guarantees(EXECUTION));
        assert_eq!(contract.execution_mode, HostExecutionMode::PendingQueue);
        Ok(HostConnection {
            capabilities: [(EXECUTION, contract)].into(),
        })
    }

    fn queue_guarantees(&self, _: RuntimeCapability) -> HostQueueGuarantees {
        HostQueueGuarantees {
            durable_acceptance: true,
            stable_submission_ids: true,
            linearizable_cancel_dequeue: true,
            queryable_operation_status: true,
            idempotent_handoff: true,
        }
    }

    fn reconcile(&self, _: RuntimeCapability) -> Result<HostReconcileStatus, String> {
        Ok(HostReconcileStatus::Ready)
    }

    fn handoff(
        &self,
        _: RuntimeCapability,
        operation: &HostOperation,
    ) -> Result<HandoffReceipt, String> {
        let mut accepted = self.accepted.lock().unwrap();
        assert!(accepted.replace(operation.stable_id.clone()).is_none());
        Err("fixture transport failed after acceptance".into())
    }

    fn status(&self, _: RuntimeCapability, stable_id: &str) -> Result<HostOperationStatus, String> {
        assert_eq!(self.accepted.lock().unwrap().as_deref(), Some(stable_id));
        Ok(HostOperationStatus::Unknown)
    }

    fn cancel(&self, _: RuntimeCapability, stable_id: &str) -> Result<HostCancelOutcome, String> {
        assert_eq!(
            self.accepted.lock().unwrap().take().as_deref(),
            Some(stable_id)
        );
        Ok(HostCancelOutcome::CancelledBeforeStart)
    }

    fn deliver_memory(&self, _: &MemoryDeliveryOperation) -> Result<MemoryDeliveryReceipt, String> {
        Err("fixture does not consume Memories".into())
    }

    fn memory_delivery_status(&self, _: &str) -> Result<MemoryDeliveryStatus, String> {
        Ok(MemoryDeliveryStatus::NotFound)
    }
}

#[test]
fn queue_finalizes_episode_and_recovers_an_ambiguous_handoff_after_reopen() {
    let dir = FixtureDir::new();
    let rel = dir.create_rel();
    let queue_dir = dir.0.join("queue");
    let host = Arc::new(Host::default());
    let submit_id = |value| -> Result<SubmissionId, QueueError> { SubmissionId::new(value) };
    let first = ConversationSubmissionId::new("chat", submit_id("one").unwrap()).unwrap();
    let second = ConversationSubmissionId::new("chat", submit_id("two").unwrap()).unwrap();

    {
        let gateway = Gateway::new();
        let owner = gateway.mount_rel(&rel, true).unwrap();
        let (admission, _) = admission_config(owner).install(&gateway).unwrap();
        let instance = admission.authenticate(OPERATOR).unwrap();
        let viewer = admission.authenticate(VIEWER).unwrap();
        let router = admission.connect_runtime_host(host.clone());
        assert_eq!(router.state(EXECUTION), HostCapabilityState::Ready);
        let mut queue = instance.open_conversation_queue(&queue_dir, owner).unwrap();
        assert!(matches!(
            instance
                .submit_conversation(
                    &mut queue,
                    "chat",
                    first.submission_id.clone(),
                    "first turn",
                    0
                )
                .unwrap(),
            EnqueueOutcome::Durable(_)
        ));
        let revision = viewer.conversation_revision(&queue, "chat").unwrap();
        let mut snapshot = viewer
            .open_conversation_queue_read(&queue_dir, owner)
            .unwrap();
        assert_eq!(
            viewer.conversation_revision(&snapshot, "chat").unwrap(),
            revision
        );
        assert!(
            instance
                .submit_conversation(
                    &mut snapshot,
                    "chat",
                    second.submission_id.clone(),
                    "ignored",
                    revision,
                )
                .is_err()
        );
        assert_eq!(
            viewer.conversation_revision(&queue, "chat").unwrap(),
            revision
        );
        drop(snapshot);
        let capture = FinalizeCapture {
            branch_id: "main".into(),
            parent_node_id: None,
            committed_timestamp_ns: 10,
            verified_principal_id: "alice".into(),
        };
        let committed = instance
            .finalize_conversation_submission(&mut queue, &first, revision, capture)
            .unwrap();
        assert_eq!(committed.state, QueueState::Committed);
        assert_eq!(committed.archive_receipt.unwrap().node_id, "one");
        let denied = viewer
            .finalize_conversation_episode(
                owner,
                "chat",
                "one",
                "one",
                EpisodeBoundary::Explicit,
                11,
            )
            .unwrap_err();
        assert_eq!(denied.kind(), ProductErrorKind::Unauthorized);
        let _: EpisodeId = instance
            .finalize_conversation_episode(
                owner,
                "chat",
                "one",
                "one",
                EpisodeBoundary::Explicit,
                11,
            )
            .unwrap();
        let revision = instance.conversation_revision(&queue, "chat").unwrap();
        instance
            .submit_conversation(
                &mut queue,
                "chat",
                second.submission_id.clone(),
                "second turn",
                revision,
            )
            .unwrap();
        assert!(
            instance
                .generation_checkpoint_snapshot(&queue, &second)
                .unwrap()
                .is_none()
        );
        assert!(
            instance
                .dispatch_conversation_submission(&mut queue, &second, EXECUTION, &router)
                .is_err()
        );
    }

    let gateway = Gateway::new();
    let owner = gateway.mount_rel(&rel, true).unwrap();
    let (admission, _) = admission_config(owner).install(&gateway).unwrap();
    let instance = admission.authenticate(OPERATOR).unwrap();
    let router = admission.connect_runtime_host(host);
    let mut queue = instance.open_conversation_queue(&queue_dir, owner).unwrap();
    assert_eq!(
        instance
            .conversation_submission_status(&queue, &first)
            .unwrap()
            .unwrap()
            .state,
        QueueState::Committed
    );
    let outcome = instance
        .dispatch_conversation_submission(&mut queue, &second, EXECUTION, &router)
        .unwrap();
    assert!(matches!(outcome, HostDispatchOutcome::Reconciled(_)));
    let held = instance
        .host_operation_status(&queue, &second, EXECUTION)
        .unwrap()
        .unwrap();
    assert_eq!(held.phase, DurableHostOperationPhase::Held);
    let cancelled = instance
        .cancel_host_operation(&mut queue, &second, EXECUTION, &router)
        .unwrap();
    assert_eq!(
        cancelled.host_operations[&EXECUTION].stable_id,
        held.stable_id
    );
    assert_eq!(
        cancelled.host_operations[&EXECUTION].phase,
        DurableHostOperationPhase::Cancelled
    );
}

// Type-check the recipient API without creating private generation journal entries.
#[allow(dead_code)]
fn stream_contract(
    instance: &Instance,
    queue: &ConversationQueue,
    id: &ConversationSubmissionId,
) -> Result<(), ProductError> {
    let _: GenerationReplay = instance.replay_generation_checkpoints(queue, id, 0, 16)?;
    let (_, mut subscription): (GenerationStreamSnapshot, GenerationStreamSubscription) =
        instance.subscribe_generation_stream(queue, id)?;
    let _: Option<GenerationStreamDelivery> =
        instance.next_generation_stream_delivery(queue, &mut subscription)?;
    let _ = instance.reconnect_generation_stream(queue, subscription)?;
    Ok(())
}
