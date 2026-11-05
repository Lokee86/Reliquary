use super::*;
use reliquary_application::{
    PreparedProjectCorrelation, ProjectCorrelatedNativeCommit, ProjectCorrelationRequest,
    commit_project_correlated_native_turn, prepare_project_correlation,
    validate_project_correlated_native_commit, validate_project_correlation_retry,
};
use reliquary_owner_session::model::ProjectRepositoryRef;

/// Host-configured authority for one principal's exact project repository.
/// Keep construction and revocation in the trusted host configuration layer.
pub struct TrustedProjectSource {
    principal: String,
    owner: OwnerId,
    repository: ProjectRepositoryRef,
    root: std::path::PathBuf,
    active: Arc<AtomicBool>,
}
impl TrustedProjectSource {
    pub fn new(
        principal: impl Into<String>,
        owner: OwnerId,
        repository: ProjectRepositoryRef,
        root: impl Into<std::path::PathBuf>,
    ) -> Self {
        Self {
            principal: principal.into(),
            owner,
            repository,
            root: root.into(),
            active: Arc::new(AtomicBool::new(true)),
        }
    }
    pub fn revoke(&self) {
        self.active.store(false, Ordering::Release);
    }
    pub(crate) fn authorize(
        &self,
        instance: &Instance,
        owner: OwnerId,
        repository: &ProjectRepositoryRef,
    ) -> Result<&Path, GatewayError> {
        instance.authorize_queue_owner(owner)?;
        if !self.active.load(Ordering::Acquire)
            || self.principal != instance.principal()
            || self.owner != owner
            || &self.repository != repository
        {
            return Err(GatewayError::ProjectSource(
                "project source is not authorized".into(),
            ));
        }
        Ok(&self.root)
    }

    pub(super) fn context_root(
        &self,
        instance: &Instance,
        owner: OwnerId,
        repository: &ProjectRepositoryRef,
    ) -> Result<(std::path::PathBuf, Arc<AtomicBool>), GatewayError> {
        let root = self.authorize(instance, owner, repository)?.to_path_buf();
        Ok((root, Arc::clone(&self.active)))
    }
}

/// Source proof kept opaque through native turn admission.
#[derive(Clone)]
pub struct AuthorizedProjectCorrelation {
    prepared: PreparedProjectCorrelation,
    active: Arc<AtomicBool>,
    instance: Uuid,
    owner: OwnerId,
}
impl AuthorizedProjectCorrelation {
    fn validate(&self, instance: &Instance, owner: OwnerId) -> Result<(), GatewayError> {
        instance.authorize_queue_owner(owner)?;
        if self.instance != instance.instance_id
            || self.owner != owner
            || !self.active.load(Ordering::Acquire)
        {
            return Err(GatewayError::ProjectSource(
                "project source authorization changed".into(),
            ));
        }
        Ok(())
    }
}

impl Instance {
    /// Verify one exact VCS source outside the REL lock. REL and principal
    /// grants are checked on both sides of repository I/O.
    pub fn prepare_project_correlation(
        &self,
        owner: OwnerId,
        request: ProjectCorrelationRequest,
        source: &TrustedProjectSource,
    ) -> Result<AuthorizedProjectCorrelation, GatewayError> {
        if request.authorized_owner_id != owner.as_bytes()
            || request.authorized_principal_id != self.principal()
        {
            return Err(GatewayError::UnauthorizedRel {
                principal: self.principal().to_owned(),
                owner,
            });
        }
        self.authorize_queue_owner(owner)?;
        let repository = request.repository.clone();
        let project_dir = source.authorize(self, owner, &repository)?;
        let prepared = prepare_project_correlation(owner.as_bytes(), request, project_dir)
            .map_err(|error| OwnerSessionError::Record(error.to_string()))?;
        self.authorize_queue_owner(owner)?;
        source.authorize(self, owner, &repository)?;
        Ok(AuthorizedProjectCorrelation {
            prepared,
            active: Arc::clone(&source.active),
            instance: self.instance_id,
            owner,
        })
    }
}

impl Gateway {
    /// Finalize the existing native queue submission with exact VCS-backed
    /// source references in the same accepted Archive turn.
    pub fn finalize_project_correlated_submission(
        &self,
        instance: &Instance,
        queue: &mut ConversationQueue,
        id: &ConversationSubmissionId,
        expected_revision: u64,
        capture: FinalizeCapture,
        project_sources: Vec<AuthorizedProjectCorrelation>,
    ) -> Result<SubmissionStatus, GatewayError> {
        instance.authorize_queue_owner(queue.owner())?;
        if !Arc::ptr_eq(&self.owners, &instance.owners) {
            return Err(GatewayError::ProjectSource(
                "project correlation belongs to another Gateway".into(),
            ));
        }
        for source in &project_sources {
            source.validate(instance, queue.owner())?;
        }
        let submitter = queue
            .submitter_principal(id)
            .ok_or(conversation_queue::QueueError::MissingSubmission)?;
        if submitter != instance.principal()
            || capture.verified_principal_id != instance.principal()
        {
            return Err(GatewayError::UnauthorizedQueueSubmission {
                principal: instance.principal().to_owned(),
                owner: queue.owner(),
            });
        }

        if let Some(status) = queue.status(id)
            && status.state == QueueState::Committed
        {
            if status.finalize_capture.as_ref() != Some(&capture) {
                return Err(conversation_queue::QueueError::ArchiveIdentityConflict.into());
            }
            let validate = |rel: &RelSession| {
                validate_project_correlation_retry(
                    rel,
                    queue.owner().as_bytes(),
                    instance.principal(),
                    &id.conversation_id,
                    id.submission_id.as_str(),
                    &project_sources
                        .iter()
                        .map(|source| source.prepared.clone())
                        .collect::<Vec<_>>(),
                )
            };
            match instance.reliquary(queue.owner()) {
                Ok(rel) => validate(&rel)?,
                Err(GatewayError::WriterUnavailable) => {
                    instance.with_writable_reliquary(queue.owner(), validate)??
                }
                Err(error) => return Err(error),
            }
            instance.ensure_credential_active()?;
            return Ok(status);
        }

        let submission = queue
            .submission(id)
            .cloned()
            .ok_or(conversation_queue::QueueError::MissingSubmission)?;
        let command = ProjectCorrelatedNativeCommit {
            native: reliquary_application::NativeConversationCommit {
                node_id: submission.id.submission_id.as_str().to_owned(),
                conversation_id: submission.id.conversation_id.clone(),
                parent_node_id: capture.parent_node_id.clone(),
                branch_id: capture.branch_id.clone(),
                principal_id: submission.principal_id.clone(),
                committed_timestamp_ns: capture.committed_timestamp_ns,
                content: submission.content,
            },
            project_sources: project_sources
                .iter()
                .map(|source| source.prepared.clone())
                .collect(),
        };

        let (finalizing, node) = instance.with_rel_writer(queue.owner(), |writer| {
            instance.ensure_credential_active()?;
            for source in &project_sources {
                source.validate(instance, queue.owner())?;
            }
            validate_project_correlated_native_commit(writer, queue.owner().as_bytes(), &command)?;
            let finalizing = queue.claim_finalize(id, expected_revision, capture.clone())?;
            let node =
                commit_project_correlated_native_turn(writer, queue.owner().as_bytes(), command)?;
            Ok::<_, GatewayError>((finalizing, node))
        })??;

        if finalizing.state != QueueState::Finalizing {
            return Err(conversation_queue::QueueError::InvalidState.into());
        }
        let receipt = ArchiveReceipt {
            node_id: node.node_id,
            branch_id: capture.branch_id,
            parent_node_id: node.parent_node_id,
            principal_id: node
                .principal_id
                .ok_or(conversation_queue::QueueError::ArchiveIdentityConflict)?,
            committed_timestamp_ns: node.committed_timestamp_ns,
            payload_hash: submission.payload_hash,
        };
        queue.record_archive_commit(id, ArchiveCommitProof::from_archive(receipt))?;
        Ok(queue.status(id).expect("committed entry remains projected"))
    }
}

#[cfg(test)]
#[path = "instance_project_correlation/tests.rs"]
mod tests;
