//! Authorized Reliquary Ego operations for native hosts and Inspector.

use super::{Instance, OwnerGrant, OwnerId, ProductError, ProductErrorKind, runtime};
use crate::ConfiguredRuntime;
use std::{path::Path, sync::Arc};

pub use runtime::{
    EgoAnchor as EgoPin, EgoAnchorId as EgoPinId, EgoAnchorPriority as EgoPinPriority, EgoCitation,
    EgoContext, EgoContextBinding, EgoIdentity, EgoIdentityDraft, EgoIdentityId,
    EgoIdentitySelection, EgoPhySummary, EgoPinPatch, EgoPointId, EgoPointInspection,
    EgoPointStatus, EgoQueryVector, EgoRelInspection, EgoSummaryEdit, EgoSummaryItemId,
    EgoTextField, EgoTimeScope, EgoTopicId, EgoTopicInspection, EgoWorkerHandle,
    PreparedEgoIdentity,
};

#[derive(Clone, Copy)]
pub enum EgoScope {
    Rel(OwnerId),
    Phy(OwnerId),
}

impl From<EgoScope> for runtime::EgoScope {
    fn from(scope: EgoScope) -> Self {
        match scope {
            EgoScope::Rel(owner) => Self::Rel(owner.into()),
            EgoScope::Phy(owner) => Self::Phy(owner.into()),
        }
    }
}

impl Instance {
    pub fn inspect_ego_phy_summary(
        &self,
        phy: OwnerId,
        revision: Option<u64>,
    ) -> Result<Option<EgoPhySummary>, ProductError> {
        self.require_phy_read(phy)?;
        Ok(self.inner.inspect_ego_phy_summary(phy.into(), revision)?)
    }

    pub fn inspect_ego_rel_summary(
        &self,
        phy: OwnerId,
        source: OwnerId,
        revision: Option<u64>,
    ) -> Result<Option<EgoRelInspection>, ProductError> {
        self.require_phy_read(phy)?;
        self.require_rel_read(source)?;
        Ok(self
            .inner
            .inspect_ego_rel_summary(phy.into(), source.into(), revision)?)
    }

    pub fn edit_ego_rel_summary(
        &self,
        phy: OwnerId,
        source: OwnerId,
        expected_summary_revision: u64,
        edit: EgoSummaryEdit,
    ) -> Result<u64, ProductError> {
        self.require(OwnerGrant::OperatePhylactery(phy))?;
        self.require_rel_read(source)?;
        Ok(self.inner.edit_ego_rel_summary(
            phy.into(),
            source.into(),
            expected_summary_revision,
            edit,
        )?)
    }

    /// Bind the explicitly active PHY and managed source RELs during normal
    /// profile startup. Dropping the handle drains the worker before switching
    /// owners or replacing the configured routes.
    pub fn start_ego_worker(
        &self,
        phy: OwnerId,
        source_rels: &[OwnerId],
        protected_profile_dir: &Path,
        runtime: &ConfiguredRuntime,
        count_tokens: impl Fn(&str) -> u64 + Send + Sync + 'static,
    ) -> Result<EgoWorkerHandle, ProductError> {
        self.require(OwnerGrant::OperatePhylactery(phy))?;
        for &owner in source_rels {
            self.require_rel_read(owner)?;
        }
        let general = runtime.general_endpoint().ok_or_else(|| {
            ProductError::new(
                ProductErrorKind::Unavailable,
                "Ego General route is unavailable",
            )
        })?;
        let binding = runtime.processing_route_identity();
        Ok(self.gateway.inner.start_ego_worker(
            self.inner.clone(),
            phy.into(),
            source_rels.iter().copied().map(Into::into).collect(),
            protected_profile_dir,
            runtime::EgoRoutes {
                general,
                embedding: runtime.embedding_endpoint(),
                identity: binding.perception,
                embedding_profile: binding.perception_embedding,
                count_tokens: Arc::new(count_tokens),
            },
        )?)
    }

    pub fn ego_pins(&self, scope: EgoScope) -> Result<Vec<EgoPin>, ProductError> {
        match scope {
            EgoScope::Rel(owner) => self.require_rel_read(owner)?,
            EgoScope::Phy(owner) => self.require_phy_read(owner)?,
        }
        Ok(self.inner.ego_pins(scope.into())?)
    }

    pub fn pin_anchor(
        &self,
        scope: EgoScope,
        text: String,
        priority: Option<EgoPinPriority>,
    ) -> Result<EgoPin, ProductError> {
        self.require_ego_write(scope)?;
        Ok(self
            .inner
            .create_ego_pin(scope.into(), text, priority.unwrap_or_default())?)
    }

    pub fn modify_anchor(
        &self,
        scope: EgoScope,
        id: EgoPinId,
        expected_revision: u64,
        patch: EgoPinPatch,
    ) -> Result<EgoPin, ProductError> {
        self.require_ego_write(scope)?;
        Ok(self
            .inner
            .modify_ego_pin(scope.into(), id, expected_revision, patch)?)
    }

    pub fn delete_anchor(
        &self,
        scope: EgoScope,
        id: EgoPinId,
        expected_revision: u64,
    ) -> Result<(), ProductError> {
        self.require_ego_write(scope)?;
        Ok(self
            .inner
            .delete_ego_pin(scope.into(), id, expected_revision)?)
    }

    fn require_ego_write(&self, scope: EgoScope) -> Result<(), ProductError> {
        match scope {
            EgoScope::Rel(owner) => self
                .require(OwnerGrant::OperateRel(owner))
                .or_else(|_| self.require(OwnerGrant::ManageRel(owner))),
            EgoScope::Phy(owner) => self.require(OwnerGrant::OperatePhylactery(owner)),
        }
    }

    pub fn ego_identities(
        &self,
        phy: OwnerId,
    ) -> Result<(Option<EgoIdentitySelection>, Vec<EgoIdentity>), ProductError> {
        self.require_phy_read(phy)?;
        Ok(self.inner.ego_identities(phy.into())?)
    }

    pub fn create_ego_identity(
        &self,
        phy: OwnerId,
        name: String,
        text: String,
    ) -> Result<EgoIdentity, ProductError> {
        self.require_ego_write(EgoScope::Phy(phy))?;
        Ok(self.inner.create_ego_identity(phy.into(), name, text)?)
    }

    pub fn edit_ego_identity(
        &self,
        phy: OwnerId,
        draft: EgoIdentityDraft,
    ) -> Result<EgoIdentity, ProductError> {
        self.require_ego_write(EgoScope::Phy(phy))?;
        Ok(self.inner.edit_ego_identity(phy.into(), draft)?)
    }

    pub fn select_ego_identity(
        &self,
        phy: OwnerId,
        expected_selection: u64,
        target: Option<(EgoIdentityId, u64)>,
    ) -> Result<EgoIdentitySelection, ProductError> {
        self.require_ego_write(EgoScope::Phy(phy))?;
        Ok(self
            .inner
            .select_ego_identity(phy.into(), expected_selection, target)?)
    }

    pub fn prepare_ego_identity(
        &self,
        phy: OwnerId,
        draft: EgoIdentityDraft,
    ) -> Result<PreparedEgoIdentity, ProductError> {
        self.require_ego_write(EgoScope::Phy(phy))?;
        Ok(self.inner.prepare_ego_identity(phy.into(), draft)?)
    }

    /// Apply the exact draft the trusted host showed the user for approval.
    pub fn approve_ego_identity(
        &self,
        phy: OwnerId,
        prepared: PreparedEgoIdentity,
    ) -> Result<EgoIdentity, ProductError> {
        self.require_ego_write(EgoScope::Phy(phy))?;
        Ok(self.inner.approve_ego_identity(phy.into(), prepared)?)
    }

    /// Call at request assembly. The counter is owned by the destination host.
    pub fn ego_context(
        &self,
        rel: Option<OwnerId>,
        phy: OwnerId,
        query: &str,
        vector: Option<&EgoQueryVector>,
        budget: usize,
        count: impl Fn(&str) -> usize,
    ) -> Result<EgoContext, ProductError> {
        self.require_phy_read(phy)?;
        if let Some(owner) = rel {
            self.require_rel_read(owner)?;
        }
        Ok(self.inner.ego_context(
            rel.map(Into::into),
            phy.into(),
            query,
            vector,
            budget,
            count,
        )?)
    }
}
