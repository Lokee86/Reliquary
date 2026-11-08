//! Host-configured project source authorization and native turn correlation.

use super::{ConversationQueue, Gateway, Instance, OwnerGrant, OwnerId, ProductError, runtime};
use crate::{
    ConversationSubmissionId, FinalizeCapture, OwnerKind, ProductErrorKind, SubmissionStatus,
};
use std::{path::PathBuf, sync::Arc};

pub use reliquary_application::{ProjectCorrelationRequest, VerifiedExternalSource};
pub use reliquary_archive::{
    ContentId, ProjectFileRef, ProjectRepositoryKind, ProjectRepositoryRef, ProjectRevisionRef,
};
pub use runtime::AuthorizedProjectCorrelation;

/// Revocable project source configured by the trusted Gateway holder.
pub struct TrustedProjectSource {
    gateway: Gateway,
    inner: runtime::TrustedProjectSource,
}

impl TrustedProjectSource {
    pub fn revoke(&self) {
        self.inner.revoke();
    }

    pub(super) fn for_gateway(
        &self,
        gateway: &Gateway,
    ) -> Result<&runtime::TrustedProjectSource, ProductError> {
        if !Arc::ptr_eq(&self.gateway.mounted, &gateway.mounted) {
            return Err(ProductError::new(
                ProductErrorKind::Unauthorized,
                "project source belongs to another Gateway",
            ));
        }
        Ok(&self.inner)
    }
}

impl Gateway {
    pub fn bind_project_source(
        &self,
        principal: impl Into<String>,
        owner: OwnerId,
        repository: ProjectRepositoryRef,
        root: impl Into<PathBuf>,
    ) -> Result<TrustedProjectSource, ProductError> {
        let (_, kind, mounted) = self.record(owner)?;
        if kind != OwnerKind::Rel {
            return Err(ProductError::new(
                ProductErrorKind::WrongFormat,
                "owner is not REL",
            ));
        }
        if !mounted {
            return Err(ProductError::new(
                ProductErrorKind::Unavailable,
                "REL is not mounted",
            ));
        }
        Ok(TrustedProjectSource {
            gateway: self.clone(),
            inner: runtime::TrustedProjectSource::new(principal, owner.into(), repository, root),
        })
    }
}

impl Instance {
    pub fn prepare_project_correlation(
        &self,
        owner: OwnerId,
        request: ProjectCorrelationRequest,
        source: &TrustedProjectSource,
    ) -> Result<AuthorizedProjectCorrelation, ProductError> {
        self.require(OwnerGrant::OperateRel(owner))?;
        self.inner
            .prepare_project_correlation(owner.into(), request, source.for_gateway(&self.gateway)?)
            .map_err(Into::into)
    }

    pub fn finalize_project_correlated_submission(
        &self,
        queue: &mut ConversationQueue,
        id: &ConversationSubmissionId,
        expected_revision: u64,
        capture: FinalizeCapture,
        project_sources: Vec<AuthorizedProjectCorrelation>,
    ) -> Result<SubmissionStatus, ProductError> {
        self.require(OwnerGrant::OperateRel(queue.owner_id()))?;
        self.gateway
            .inner
            .finalize_project_correlated_submission(
                &self.inner,
                &mut queue.inner,
                id,
                expected_revision,
                capture,
                project_sources,
            )
            .map_err(Into::into)
    }

    pub fn verify_project_attachment(
        &self,
        owner: OwnerId,
        conversation_id: &str,
        node_id: &str,
        attachment_id: [u8; 32],
        source: &TrustedProjectSource,
    ) -> Result<VerifiedExternalSource, ProductError> {
        self.require_rel_read(owner)?;
        self.inner
            .verify_project_attachment(
                owner.into(),
                conversation_id,
                node_id,
                attachment_id,
                source.for_gateway(&self.gateway)?,
            )
            .map_err(Into::into)
    }
}
