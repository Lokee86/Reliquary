use super::*;

/// Durable native conversation queue. Reads and mutations require an authenticated Instance.
pub struct ConversationQueue {
    pub(super) inner: runtime::ConversationQueue,
}
impl ConversationQueue {
    pub fn owner_id(&self) -> OwnerId {
        self.inner.owner().into()
    }
}

#[derive(Clone)]
pub struct Instance {
    pub(crate) inner: Arc<runtime::Instance>,
    pub(super) grants: Arc<BTreeSet<OwnerGrant>>,
    pub(crate) gateway: Gateway,
}
impl Instance {
    pub fn principal(&self) -> &str {
        self.inner.principal()
    }
    pub(crate) fn require(&self, grant: OwnerGrant) -> Result<(), ProductError> {
        self.inner.ensure_credential_active()?;
        if self.grants.contains(&grant) {
            Ok(())
        } else {
            Err(ProductError::new(
                ProductErrorKind::Unauthorized,
                format!(
                    "principal {:?} lacks required owner grant",
                    self.principal()
                ),
            ))
        }
    }
    pub(crate) fn require_rel_read(&self, owner: OwnerId) -> Result<(), ProductError> {
        self.inner.ensure_credential_active()?;
        if self.grants.contains(&OwnerGrant::ViewRel(owner))
            || self.grants.contains(&OwnerGrant::OperateRel(owner))
            || self.grants.contains(&OwnerGrant::ManageRel(owner))
        {
            Ok(())
        } else {
            Err(ProductError::new(
                ProductErrorKind::Unauthorized,
                "REL view grant is required",
            ))
        }
    }
    pub(crate) fn require_phy_read(&self, owner: OwnerId) -> Result<(), ProductError> {
        self.inner.ensure_credential_active()?;
        if self.grants.contains(&OwnerGrant::ViewPhylactery(owner))
            || self.grants.contains(&OwnerGrant::OperatePhylactery(owner))
        {
            Ok(())
        } else {
            Err(ProductError::new(
                ProductErrorKind::Unauthorized,
                "Phylactery view grant is required",
            ))
        }
    }
    pub fn rel_info(&self, owner: OwnerId) -> Result<crate::OwnerInfo, ProductError> {
        self.require_rel_read(owner)?;
        let (path, kind, mounted) = self.gateway.record(owner)?;
        if kind != OwnerKind::Rel {
            return Err(ProductError::new(
                ProductErrorKind::WrongFormat,
                "owner is not REL",
            ));
        }
        if mounted {
            self.mounted_owner_info(owner, kind)
        } else {
            Rel::open(path)?.info()
        }
    }
    pub fn phylactery_info(&self, owner: OwnerId) -> Result<crate::OwnerInfo, ProductError> {
        self.require_phy_read(owner)?;
        let (path, kind, mounted) = self.gateway.record(owner)?;
        if kind != OwnerKind::Phylactery {
            return Err(ProductError::new(
                ProductErrorKind::WrongFormat,
                "owner is not a Phylactery",
            ));
        }
        if mounted {
            self.mounted_owner_info(owner, kind)
        } else {
            Phylactery::open(path)?.info()
        }
    }
    pub fn workspace_management(
        &self,
        owner: OwnerId,
    ) -> Result<WorkspaceManagement, ProductError> {
        self.require_rel_read(owner)?;
        Ok(WorkspaceManagement::new(self.clone(), owner))
    }
    pub fn open_conversation_queue(
        &self,
        state_dir: impl AsRef<Path>,
        owner: OwnerId,
    ) -> Result<ConversationQueue, ProductError> {
        self.require(OwnerGrant::OperateRel(owner))?;
        self.inner
            .open_conversation_queue(state_dir, owner.into())
            .map(|inner| ConversationQueue { inner })
            .map_err(Into::into)
    }
    pub fn open_conversation_queue_read(
        &self,
        state_dir: impl AsRef<Path>,
        owner: OwnerId,
    ) -> Result<ConversationQueue, ProductError> {
        self.require_rel_read(owner)?;
        self.inner
            .open_conversation_queue_read(state_dir, owner.into())
            .map(|inner| ConversationQueue { inner })
            .map_err(Into::into)
    }
    pub fn submit_conversation(
        &self,
        queue: &mut ConversationQueue,
        conversation_id: impl Into<String>,
        submission_id: runtime::SubmissionId,
        content: impl Into<String>,
        expected_revision: u64,
    ) -> Result<runtime::EnqueueOutcome, ProductError> {
        self.require(OwnerGrant::OperateRel(queue.owner_id()))?;
        self.inner
            .submit_conversation(
                &mut queue.inner,
                conversation_id,
                submission_id,
                content,
                expected_revision,
            )
            .map_err(Into::into)
    }
    pub fn finalize_conversation_submission(
        &self,
        queue: &mut ConversationQueue,
        id: &runtime::ConversationSubmissionId,
        expected_revision: u64,
        capture: runtime::FinalizeCapture,
    ) -> Result<runtime::SubmissionStatus, ProductError> {
        self.require(OwnerGrant::OperateRel(queue.owner_id()))?;
        self.inner
            .finalize_conversation_submission(&mut queue.inner, id, expected_revision, capture)
            .map_err(Into::into)
    }
    pub fn conversation_submission_status(
        &self,
        queue: &ConversationQueue,
        id: &runtime::ConversationSubmissionId,
    ) -> Result<Option<runtime::SubmissionStatus>, ProductError> {
        self.require_rel_read(queue.owner_id())?;
        self.inner
            .conversation_submission_status(&queue.inner, id)
            .map_err(Into::into)
    }
    pub fn conversation_revision(
        &self,
        queue: &ConversationQueue,
        conversation_id: &str,
    ) -> Result<u64, ProductError> {
        self.require_rel_read(queue.owner_id())?;
        Ok(queue.inner.conversation_revision(conversation_id))
    }
    pub fn finalize_conversation_episode(
        &self,
        owner: OwnerId,
        conversation_id: &str,
        start_node_id: &str,
        end_node_id: &str,
        boundary: crate::EpisodeBoundary,
        finalized_at_ns: i64,
    ) -> Result<crate::EpisodeId, ProductError> {
        self.require(OwnerGrant::OperateRel(owner))?;
        self.inner
            .finalize_conversation_episode(
                owner.into(),
                conversation_id,
                start_node_id,
                end_node_id,
                boundary,
                finalized_at_ns,
            )
            .map_err(Into::into)
    }
    pub fn cancel_own_pending_submission(
        &self,
        queue: &mut ConversationQueue,
        id: &runtime::ConversationSubmissionId,
        expected_revision: u64,
    ) -> Result<runtime::SubmissionStatus, ProductError> {
        self.require(OwnerGrant::OperateRel(queue.owner_id()))?;
        self.inner
            .cancel_own_pending_submission(&mut queue.inner, id, expected_revision)
            .map_err(Into::into)
    }
    pub fn host_operation_status(
        &self,
        queue: &ConversationQueue,
        id: &runtime::ConversationSubmissionId,
        capability: runtime::RuntimeCapability,
    ) -> Result<Option<runtime::DurableHostOperation>, ProductError> {
        self.require_rel_read(queue.owner_id())?;
        self.inner
            .host_operation_status(&queue.inner, id, capability)
            .map_err(Into::into)
    }
    pub fn generation_checkpoint_snapshot(
        &self,
        queue: &ConversationQueue,
        id: &runtime::ConversationSubmissionId,
    ) -> Result<Option<runtime::DurableGenerationCheckpoint>, ProductError> {
        self.require_rel_read(queue.owner_id())?;
        self.inner
            .generation_checkpoint_snapshot(&queue.inner, id)
            .map_err(Into::into)
    }
    pub fn replay_generation_checkpoints(
        &self,
        queue: &ConversationQueue,
        id: &runtime::ConversationSubmissionId,
        after_sequence: u64,
        max_items: usize,
    ) -> Result<runtime::GenerationReplay, ProductError> {
        self.require_rel_read(queue.owner_id())?;
        self.inner
            .replay_generation_checkpoints(&queue.inner, id, after_sequence, max_items)
            .map_err(Into::into)
    }
    pub fn subscribe_generation_stream(
        &self,
        queue: &ConversationQueue,
        id: &runtime::ConversationSubmissionId,
    ) -> Result<
        (
            runtime::GenerationStreamSnapshot,
            runtime::GenerationStreamSubscription,
        ),
        ProductError,
    > {
        self.require_rel_read(queue.owner_id())?;
        self.inner
            .subscribe_generation_stream(&queue.inner, id)
            .map_err(Into::into)
    }
    pub fn next_generation_stream_delivery(
        &self,
        queue: &ConversationQueue,
        subscription: &mut runtime::GenerationStreamSubscription,
    ) -> Result<Option<runtime::GenerationStreamDelivery>, ProductError> {
        self.require_rel_read(queue.owner_id())?;
        self.inner
            .next_generation_stream_delivery(&queue.inner, subscription)
            .map_err(Into::into)
    }
    pub fn reconnect_generation_stream(
        &self,
        queue: &ConversationQueue,
        previous: runtime::GenerationStreamSubscription,
    ) -> Result<
        (
            runtime::GenerationStreamSnapshot,
            runtime::GenerationStreamSubscription,
        ),
        ProductError,
    > {
        self.require_rel_read(queue.owner_id())?;
        self.inner
            .reconnect_generation_stream(&queue.inner, previous)
            .map_err(Into::into)
    }
    pub fn dispatch_conversation_submission(
        &self,
        queue: &mut ConversationQueue,
        id: &runtime::ConversationSubmissionId,
        capability: runtime::RuntimeCapability,
        router: &runtime::CapabilityRouter,
    ) -> Result<runtime::HostDispatchOutcome, ProductError> {
        self.require(OwnerGrant::OperateRel(queue.owner_id()))?;
        self.inner
            .dispatch_conversation_submission(&mut queue.inner, id, capability, router)
            .map_err(Into::into)
    }
    pub fn reconcile_host_operation(
        &self,
        queue: &mut ConversationQueue,
        id: &runtime::ConversationSubmissionId,
        capability: runtime::RuntimeCapability,
        router: &runtime::CapabilityRouter,
    ) -> Result<runtime::HostDispatchOutcome, ProductError> {
        self.require(OwnerGrant::OperateRel(queue.owner_id()))?;
        self.inner
            .reconcile_host_operation(&mut queue.inner, id, capability, router)
            .map_err(Into::into)
    }
    pub fn cancel_host_operation(
        &self,
        queue: &mut ConversationQueue,
        id: &runtime::ConversationSubmissionId,
        capability: runtime::RuntimeCapability,
        router: &runtime::CapabilityRouter,
    ) -> Result<runtime::SubmissionStatus, ProductError> {
        self.require(OwnerGrant::OperateRel(queue.owner_id()))?;
        self.inner
            .cancel_host_operation(&mut queue.inner, id, capability, router)
            .map_err(Into::into)
    }
}
