//! Canonical Gateway bindings for public Ego owner operations.
use crate::{
    CvaError, EgoAnchor, EgoAnchorId, EgoAnchorPriority, EgoIdentity, EgoIdentityId,
    EgoPersonality, EgoWebSynthesis, PhylacteryError,
};

impl crate::RelExecutionOwner {
    pub(crate) fn native_ego_anchors(&self) -> Vec<EgoAnchor> {
        self.owner.native_ego_anchors()
    }
}

impl crate::RelExecutionOwner {
    pub(crate) fn native_ego_anchor(&self, id: EgoAnchorId) -> Option<EgoAnchor> {
        self.owner.native_ego_anchor(id)
    }
}

impl crate::RelExecutionOwner {
    pub(crate) fn native_create_ego_anchor(
        &mut self,
        priority: EgoAnchorPriority,
        text: String,
    ) -> Result<EgoAnchor, CvaError> {
        self.owner
            .native_create_ego_anchor(priority, text)
            .map_err(Into::into)
    }
}

impl crate::RelExecutionOwner {
    pub(crate) fn native_update_ego_anchor(
        &mut self,
        id: EgoAnchorId,
        expected_revision: u64,
        priority: EgoAnchorPriority,
        text: String,
    ) -> Result<(EgoAnchor, bool), CvaError> {
        self.owner
            .native_update_ego_anchor(id, expected_revision, priority, text)
            .map_err(Into::into)
    }
}

impl crate::RelExecutionOwner {
    pub(crate) fn native_delete_ego_anchor(
        &mut self,
        id: EgoAnchorId,
        expected_revision: u64,
    ) -> Result<bool, CvaError> {
        self.owner
            .native_delete_ego_anchor(id, expected_revision)
            .map_err(Into::into)
    }
}

impl crate::RelExecutionOwner {
    pub(crate) fn native_ego_web_synthesis(&self) -> Option<&EgoWebSynthesis> {
        self.owner.native_ego_web_synthesis()
    }
}

impl crate::RelExecutionOwner {
    pub(crate) fn native_put_ego_web_synthesis(
        &mut self,
        expected_revision: u64,
        source_memory_version: u64,
        text: String,
    ) -> Result<(EgoWebSynthesis, bool), CvaError> {
        self.owner
            .native_put_ego_web_synthesis(expected_revision, source_memory_version, text)
            .map_err(Into::into)
    }
}

impl crate::RelExecutionOwner {
    pub(crate) fn native_ego_anchor_lifecycle(
        &self,
        id: EgoAnchorId,
    ) -> Option<crate::owner_lifecycle::LifecycleState> {
        self.owner.native_ego_anchor_lifecycle(id)
    }
}

impl crate::RelExecutionOwner {
    pub(crate) fn native_historical_ego_anchor(
        &self,
        id: EgoAnchorId,
        revision: u64,
    ) -> Option<EgoAnchor> {
        self.owner.native_historical_ego_anchor(id, revision)
    }
}

impl crate::phy_execution_owner::PhyExecutionOwner {
    pub(crate) fn native_ego_identity(&self) -> Option<EgoIdentity> {
        self.owner.native_ego_identity()
    }
}

impl crate::phy_execution_owner::PhyExecutionOwner {
    pub(crate) fn native_active_ego_identity_id(&self) -> Option<EgoIdentityId> {
        self.owner.native_active_ego_identity_id()
    }
}

impl crate::phy_execution_owner::PhyExecutionOwner {
    pub(crate) fn native_ego_identities(&self) -> Vec<EgoIdentity> {
        self.owner.native_ego_identities()
    }
}

impl crate::phy_execution_owner::PhyExecutionOwner {
    pub(crate) fn native_ego_identity_by_id(&self, id: EgoIdentityId) -> Option<EgoIdentity> {
        self.owner.native_ego_identity_by_id(id)
    }
}

impl crate::phy_execution_owner::PhyExecutionOwner {
    pub(crate) fn native_put_ego_identity(
        &mut self,
        expected_revision: u64,
        text: String,
    ) -> Result<(EgoIdentity, bool), PhylacteryError> {
        self.owner
            .native_put_ego_identity(expected_revision, text)
            .map_err(Into::into)
    }
}

impl crate::phy_execution_owner::PhyExecutionOwner {
    pub(crate) fn native_create_ego_identity(
        &mut self,
        name: String,
        text: String,
    ) -> Result<EgoIdentity, PhylacteryError> {
        self.owner
            .native_create_ego_identity(name, text)
            .map_err(Into::into)
    }
}

impl crate::phy_execution_owner::PhyExecutionOwner {
    pub(crate) fn native_update_ego_identity(
        &mut self,
        id: EgoIdentityId,
        expected_revision: u64,
        name: String,
        text: String,
    ) -> Result<(EgoIdentity, bool), PhylacteryError> {
        self.owner
            .native_update_ego_identity(id, expected_revision, name, text)
            .map_err(Into::into)
    }
}

impl crate::phy_execution_owner::PhyExecutionOwner {
    pub(crate) fn native_activate_ego_identity(
        &mut self,
        id: EgoIdentityId,
    ) -> Result<bool, PhylacteryError> {
        self.owner
            .native_activate_ego_identity(id)
            .map_err(Into::into)
    }
}

impl crate::phy_execution_owner::PhyExecutionOwner {
    pub(crate) fn native_delete_ego_identity(
        &mut self,
        id: EgoIdentityId,
        expected_revision: u64,
    ) -> Result<bool, PhylacteryError> {
        self.owner
            .native_delete_ego_identity(id, expected_revision)
            .map_err(Into::into)
    }
}

impl crate::phy_execution_owner::PhyExecutionOwner {
    pub(crate) fn native_ego_personality(&self) -> Option<&EgoPersonality> {
        self.owner.native_ego_personality()
    }
}

impl crate::phy_execution_owner::PhyExecutionOwner {
    pub(crate) fn native_put_ego_personality(
        &mut self,
        expected_revision: u64,
        text: String,
        source_memory_version: u64,
    ) -> Result<(EgoPersonality, bool), PhylacteryError> {
        self.owner
            .native_put_ego_personality(expected_revision, text, source_memory_version)
            .map_err(Into::into)
    }
}

impl crate::phy_execution_owner::PhyExecutionOwner {
    pub(crate) fn native_ego_anchors(&self) -> Vec<EgoAnchor> {
        self.owner.native_ego_anchors()
    }
}

impl crate::phy_execution_owner::PhyExecutionOwner {
    pub(crate) fn native_ego_anchor(&self, id: EgoAnchorId) -> Option<EgoAnchor> {
        self.owner.native_ego_anchor(id)
    }
}

impl crate::phy_execution_owner::PhyExecutionOwner {
    pub(crate) fn native_create_ego_anchor(
        &mut self,
        priority: EgoAnchorPriority,
        text: String,
    ) -> Result<EgoAnchor, PhylacteryError> {
        self.owner
            .native_create_ego_anchor(priority, text)
            .map_err(Into::into)
    }
}

impl crate::phy_execution_owner::PhyExecutionOwner {
    pub(crate) fn native_update_ego_anchor(
        &mut self,
        id: EgoAnchorId,
        expected_revision: u64,
        priority: EgoAnchorPriority,
        text: String,
    ) -> Result<(EgoAnchor, bool), PhylacteryError> {
        self.owner
            .native_update_ego_anchor(id, expected_revision, priority, text)
            .map_err(Into::into)
    }
}

impl crate::phy_execution_owner::PhyExecutionOwner {
    pub(crate) fn native_delete_ego_anchor(
        &mut self,
        id: EgoAnchorId,
        expected_revision: u64,
    ) -> Result<bool, PhylacteryError> {
        self.owner
            .native_delete_ego_anchor(id, expected_revision)
            .map_err(Into::into)
    }
}

impl crate::phy_execution_owner::PhyExecutionOwner {
    pub(crate) fn native_ego_web_synthesis(&self) -> Option<&EgoWebSynthesis> {
        self.owner.native_ego_web_synthesis()
    }
}

impl crate::phy_execution_owner::PhyExecutionOwner {
    pub(crate) fn native_put_ego_web_synthesis(
        &mut self,
        expected_revision: u64,
        source_memory_version: u64,
        text: String,
    ) -> Result<(EgoWebSynthesis, bool), PhylacteryError> {
        self.owner
            .native_put_ego_web_synthesis(expected_revision, source_memory_version, text)
            .map_err(Into::into)
    }
}

impl crate::phy_execution_owner::PhyExecutionOwner {
    pub(crate) fn native_ego_anchor_lifecycle(
        &self,
        id: EgoAnchorId,
    ) -> Option<crate::owner_lifecycle::LifecycleState> {
        self.owner.native_ego_anchor_lifecycle(id)
    }
}

impl crate::phy_execution_owner::PhyExecutionOwner {
    pub(crate) fn native_historical_ego_anchor(
        &self,
        id: EgoAnchorId,
        revision: u64,
    ) -> Option<EgoAnchor> {
        self.owner.native_historical_ego_anchor(id, revision)
    }
}

impl crate::phy_execution_owner::PhyExecutionOwner {
    pub(crate) fn native_ego_identity_lifecycle(
        &self,
        id: EgoIdentityId,
    ) -> Option<crate::owner_lifecycle::LifecycleState> {
        self.owner.native_ego_identity_lifecycle(id)
    }
}

impl crate::phy_execution_owner::PhyExecutionOwner {
    pub(crate) fn native_historical_ego_identity(
        &self,
        id: EgoIdentityId,
        revision: u64,
    ) -> Option<EgoIdentity> {
        self.owner.native_historical_ego_identity(id, revision)
    }
}

impl crate::phy_execution_owner::PhyExecutionOwner {
    pub(crate) fn native_ego_version(&self) -> u64 {
        self.owner.native_ego_version()
    }
}
