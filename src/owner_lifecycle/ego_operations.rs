//! Concrete Ego publication and read contracts over the original mounted owners.
use crate::ego_store::validate_source_memory_version;
use crate::{
    EgoAnchor, EgoAnchorId, EgoAnchorPriority, EgoError, EgoIdentity, EgoIdentityId,
    EgoPersonality, EgoWebSynthesis,
};

impl crate::owner_lifecycle::ReliquaryOwner {
    pub(crate) fn native_ego_anchors(&self) -> Vec<EgoAnchor> {
        self.ego.anchors()
    }
}

impl crate::owner_lifecycle::ReliquaryOwner {
    pub(crate) fn native_ego_anchor(&self, id: EgoAnchorId) -> Option<EgoAnchor> {
        self.ego.anchor(id)
    }
}

impl crate::owner_lifecycle::ReliquaryOwner {
    pub(crate) fn native_create_ego_anchor(
        &mut self,
        priority: EgoAnchorPriority,
        text: String,
    ) -> Result<EgoAnchor, EgoError> {
        Ok(self
            .ego
            .create_anchor(&mut self.container, priority, text)?)
    }
}

impl crate::owner_lifecycle::ReliquaryOwner {
    pub(crate) fn native_update_ego_anchor(
        &mut self,
        id: EgoAnchorId,
        expected_revision: u64,
        priority: EgoAnchorPriority,
        text: String,
    ) -> Result<(EgoAnchor, bool), EgoError> {
        Ok(self
            .ego
            .update_anchor(&mut self.container, id, expected_revision, priority, text)?)
    }
}

impl crate::owner_lifecycle::ReliquaryOwner {
    pub(crate) fn native_delete_ego_anchor(
        &mut self,
        id: EgoAnchorId,
        expected_revision: u64,
    ) -> Result<bool, EgoError> {
        Ok(self
            .ego
            .delete_anchor(&mut self.container, id, expected_revision)?)
    }
}

impl crate::owner_lifecycle::ReliquaryOwner {
    pub(crate) fn native_ego_web_synthesis(&self) -> Option<&EgoWebSynthesis> {
        self.ego.synthesis()
    }
}

impl crate::owner_lifecycle::ReliquaryOwner {
    pub(crate) fn native_put_ego_web_synthesis(
        &mut self,
        expected_revision: u64,
        source_memory_version: u64,
        text: String,
    ) -> Result<(EgoWebSynthesis, bool), EgoError> {
        validate_source_memory_version(source_memory_version, self.memories.memory_version())?;
        Ok(self.ego.put_synthesis(
            &mut self.container,
            expected_revision,
            source_memory_version,
            text,
        )?)
    }
}

impl crate::owner_lifecycle::ReliquaryOwner {
    pub(crate) fn native_ego_anchor_lifecycle(
        &self,
        id: EgoAnchorId,
    ) -> Option<crate::owner_lifecycle::LifecycleState> {
        self.ego.anchor_lifecycle(id)
    }
}

impl crate::owner_lifecycle::ReliquaryOwner {
    pub(crate) fn native_historical_ego_anchor(
        &self,
        id: EgoAnchorId,
        revision: u64,
    ) -> Option<EgoAnchor> {
        self.ego.historical_anchor(id, revision)
    }
}

impl crate::owner_lifecycle::PhylacteryOwner {
    pub(crate) fn native_ego_identity(&self) -> Option<EgoIdentity> {
        self.ego.active_identity()
    }
}

impl crate::owner_lifecycle::PhylacteryOwner {
    pub(crate) fn native_active_ego_identity_id(&self) -> Option<EgoIdentityId> {
        self.ego.active_identity_id()
    }
}

impl crate::owner_lifecycle::PhylacteryOwner {
    pub(crate) fn native_ego_identities(&self) -> Vec<EgoIdentity> {
        self.ego.identities()
    }
}

impl crate::owner_lifecycle::PhylacteryOwner {
    pub(crate) fn native_ego_identity_by_id(&self, id: EgoIdentityId) -> Option<EgoIdentity> {
        self.ego.identity(id)
    }
}

impl crate::owner_lifecycle::PhylacteryOwner {
    pub(crate) fn native_put_ego_identity(
        &mut self,
        expected_revision: u64,
        text: String,
    ) -> Result<(EgoIdentity, bool), EgoError> {
        Ok(self
            .ego
            .put_identity(&mut self.container, expected_revision, text)?)
    }
}

impl crate::owner_lifecycle::PhylacteryOwner {
    pub(crate) fn native_create_ego_identity(
        &mut self,
        name: String,
        text: String,
    ) -> Result<EgoIdentity, EgoError> {
        Ok(self.ego.create_identity(&mut self.container, name, text)?)
    }
}

impl crate::owner_lifecycle::PhylacteryOwner {
    pub(crate) fn native_update_ego_identity(
        &mut self,
        id: EgoIdentityId,
        expected_revision: u64,
        name: String,
        text: String,
    ) -> Result<(EgoIdentity, bool), EgoError> {
        Ok(self
            .ego
            .update_identity(&mut self.container, id, expected_revision, name, text)?)
    }
}

impl crate::owner_lifecycle::PhylacteryOwner {
    pub(crate) fn native_activate_ego_identity(
        &mut self,
        id: EgoIdentityId,
    ) -> Result<bool, EgoError> {
        Ok(self.ego.activate_identity(&mut self.container, id)?)
    }
}

impl crate::owner_lifecycle::PhylacteryOwner {
    pub(crate) fn native_delete_ego_identity(
        &mut self,
        id: EgoIdentityId,
        expected_revision: u64,
    ) -> Result<bool, EgoError> {
        Ok(self
            .ego
            .delete_identity(&mut self.container, id, expected_revision)?)
    }
}

impl crate::owner_lifecycle::PhylacteryOwner {
    pub(crate) fn native_ego_personality(&self) -> Option<&EgoPersonality> {
        self.ego.personality()
    }
}

impl crate::owner_lifecycle::PhylacteryOwner {
    pub(crate) fn native_put_ego_personality(
        &mut self,
        expected_revision: u64,
        text: String,
        source_memory_version: u64,
    ) -> Result<(EgoPersonality, bool), EgoError> {
        validate_source_memory_version(source_memory_version, self.memories.memory_version())?;
        Ok(self.ego.put_personality(
            &mut self.container,
            expected_revision,
            text,
            source_memory_version,
        )?)
    }
}

impl crate::owner_lifecycle::PhylacteryOwner {
    pub(crate) fn native_ego_anchors(&self) -> Vec<EgoAnchor> {
        self.ego.anchors()
    }
}

impl crate::owner_lifecycle::PhylacteryOwner {
    pub(crate) fn native_ego_anchor(&self, id: EgoAnchorId) -> Option<EgoAnchor> {
        self.ego.anchor(id)
    }
}

impl crate::owner_lifecycle::PhylacteryOwner {
    pub(crate) fn native_create_ego_anchor(
        &mut self,
        priority: EgoAnchorPriority,
        text: String,
    ) -> Result<EgoAnchor, EgoError> {
        Ok(self
            .ego
            .create_anchor(&mut self.container, priority, text)?)
    }
}

impl crate::owner_lifecycle::PhylacteryOwner {
    pub(crate) fn native_update_ego_anchor(
        &mut self,
        id: EgoAnchorId,
        expected_revision: u64,
        priority: EgoAnchorPriority,
        text: String,
    ) -> Result<(EgoAnchor, bool), EgoError> {
        Ok(self
            .ego
            .update_anchor(&mut self.container, id, expected_revision, priority, text)?)
    }
}

impl crate::owner_lifecycle::PhylacteryOwner {
    pub(crate) fn native_delete_ego_anchor(
        &mut self,
        id: EgoAnchorId,
        expected_revision: u64,
    ) -> Result<bool, EgoError> {
        Ok(self
            .ego
            .delete_anchor(&mut self.container, id, expected_revision)?)
    }
}

impl crate::owner_lifecycle::PhylacteryOwner {
    pub(crate) fn native_ego_web_synthesis(&self) -> Option<&EgoWebSynthesis> {
        self.ego.synthesis()
    }
}

impl crate::owner_lifecycle::PhylacteryOwner {
    pub(crate) fn native_put_ego_web_synthesis(
        &mut self,
        expected_revision: u64,
        source_memory_version: u64,
        text: String,
    ) -> Result<(EgoWebSynthesis, bool), EgoError> {
        validate_source_memory_version(source_memory_version, self.memories.memory_version())?;
        Ok(self.ego.put_synthesis(
            &mut self.container,
            expected_revision,
            source_memory_version,
            text,
        )?)
    }
}

impl crate::owner_lifecycle::PhylacteryOwner {
    pub(crate) fn native_ego_anchor_lifecycle(
        &self,
        id: EgoAnchorId,
    ) -> Option<crate::owner_lifecycle::LifecycleState> {
        self.ego.anchor_lifecycle(id)
    }
}

impl crate::owner_lifecycle::PhylacteryOwner {
    pub(crate) fn native_historical_ego_anchor(
        &self,
        id: EgoAnchorId,
        revision: u64,
    ) -> Option<EgoAnchor> {
        self.ego.historical_anchor(id, revision)
    }
}

impl crate::owner_lifecycle::PhylacteryOwner {
    pub(crate) fn native_ego_identity_lifecycle(
        &self,
        id: EgoIdentityId,
    ) -> Option<crate::owner_lifecycle::LifecycleState> {
        self.ego.identity_lifecycle(id)
    }
}

impl crate::owner_lifecycle::PhylacteryOwner {
    pub(crate) fn native_historical_ego_identity(
        &self,
        id: EgoIdentityId,
        revision: u64,
    ) -> Option<EgoIdentity> {
        self.ego.historical_identity(id, revision)
    }
}

impl crate::owner_lifecycle::PhylacteryOwner {
    pub(crate) fn native_ego_version(&self) -> u64 {
        self.ego.version()
    }
}
