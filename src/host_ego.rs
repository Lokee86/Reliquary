//! Authorized Reliquary Ego operations for native hosts and Inspector.

use super::{Instance, OwnerGrant, OwnerId, ProductError, runtime};

pub use runtime::{
    EgoAnchor as EgoPin, EgoAnchorId as EgoPinId, EgoAnchorPriority as EgoPinPriority, EgoContext,
    EgoContextBinding, EgoIdentity, EgoIdentityDraft, EgoIdentityId, EgoIdentitySelection,
    EgoPinPatch, EgoQueryVector, PreparedEgoIdentity,
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
