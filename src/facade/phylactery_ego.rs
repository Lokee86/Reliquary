use crate::ego_store::validate_source_memory_version;
use crate::{
    EgoAnchor, EgoAnchorId, EgoAnchorPriority, EgoIdentity, EgoIdentityId, EgoPersonality,
    EgoWebSynthesis, Phylactery, PhylacteryError,
};

impl Phylactery {
    pub fn ego_version(&self) -> u64 {
        self.ego.version()
    }

    /// Compatibility accessor for the currently active Identity.
    pub fn ego_identity(&self) -> Option<EgoIdentity> {
        self.ego.active_identity()
    }

    pub fn active_ego_identity_id(&self) -> Option<EgoIdentityId> {
        self.ego.active_identity_id()
    }

    pub fn ego_identities(&self) -> Vec<EgoIdentity> {
        self.ego.identities()
    }

    pub fn ego_identity_by_id(&self, id: EgoIdentityId) -> Option<EgoIdentity> {
        self.ego.identity(id)
    }

    /// Compatibility mutator: updates the active Identity, or creates the first `Default` Identity.
    pub fn put_ego_identity(
        &mut self,
        expected_revision: u64,
        text: String,
    ) -> Result<(EgoIdentity, bool), PhylacteryError> {
        Ok(self
            .ego
            .put_identity(&mut self.container, expected_revision, text)?)
    }

    pub fn create_ego_identity(
        &mut self,
        name: String,
        text: String,
    ) -> Result<EgoIdentity, PhylacteryError> {
        Ok(self.ego.create_identity(&mut self.container, name, text)?)
    }

    pub fn update_ego_identity(
        &mut self,
        id: EgoIdentityId,
        expected_revision: u64,
        name: String,
        text: String,
    ) -> Result<(EgoIdentity, bool), PhylacteryError> {
        Ok(self
            .ego
            .update_identity(&mut self.container, id, expected_revision, name, text)?)
    }

    pub fn activate_ego_identity(&mut self, id: EgoIdentityId) -> Result<bool, PhylacteryError> {
        Ok(self.ego.activate_identity(&mut self.container, id)?)
    }

    pub fn delete_ego_identity(
        &mut self,
        id: EgoIdentityId,
        expected_revision: u64,
    ) -> Result<bool, PhylacteryError> {
        Ok(self
            .ego
            .delete_identity(&mut self.container, id, expected_revision)?)
    }

    pub fn ego_personality(&self) -> Option<&EgoPersonality> {
        self.ego.personality()
    }

    pub fn put_ego_personality(
        &mut self,
        expected_revision: u64,
        text: String,
        source_memory_version: u64,
    ) -> Result<(EgoPersonality, bool), PhylacteryError> {
        validate_source_memory_version(source_memory_version, self.memory_version())?;
        Ok(self.ego.put_personality(
            &mut self.container,
            expected_revision,
            text,
            source_memory_version,
        )?)
    }

    pub fn ego_anchors(&self) -> Vec<EgoAnchor> {
        self.ego.anchors()
    }
    pub fn ego_anchor(&self, id: EgoAnchorId) -> Option<EgoAnchor> {
        self.ego.anchor(id)
    }

    pub fn create_ego_anchor(
        &mut self,
        priority: EgoAnchorPriority,
        text: String,
    ) -> Result<EgoAnchor, PhylacteryError> {
        Ok(self
            .ego
            .create_anchor(&mut self.container, priority, text)?)
    }

    pub fn update_ego_anchor(
        &mut self,
        id: EgoAnchorId,
        expected_revision: u64,
        priority: EgoAnchorPriority,
        text: String,
    ) -> Result<(EgoAnchor, bool), PhylacteryError> {
        Ok(self
            .ego
            .update_anchor(&mut self.container, id, expected_revision, priority, text)?)
    }

    pub fn delete_ego_anchor(
        &mut self,
        id: EgoAnchorId,
        expected_revision: u64,
    ) -> Result<bool, PhylacteryError> {
        Ok(self
            .ego
            .delete_anchor(&mut self.container, id, expected_revision)?)
    }

    pub fn ego_web_synthesis(&self) -> Option<&EgoWebSynthesis> {
        self.ego.synthesis()
    }

    pub fn put_ego_web_synthesis(
        &mut self,
        expected_revision: u64,
        source_memory_version: u64,
        text: String,
    ) -> Result<(EgoWebSynthesis, bool), PhylacteryError> {
        validate_source_memory_version(source_memory_version, self.memory_version())?;
        Ok(self.ego.put_synthesis(
            &mut self.container,
            expected_revision,
            source_memory_version,
            text,
        )?)
    }
}

impl Phylactery {
    pub fn ego_anchor_lifecycle(
        &self,
        id: EgoAnchorId,
    ) -> Option<crate::owner_lifecycle::LifecycleState> {
        self.ego.anchor_lifecycle(id)
    }
    pub fn historical_ego_anchor(&self, id: EgoAnchorId, revision: u64) -> Option<EgoAnchor> {
        self.ego.historical_anchor(id, revision)
    }
}

impl Phylactery {
    pub fn ego_identity_lifecycle(
        &self,
        id: EgoIdentityId,
    ) -> Option<crate::owner_lifecycle::LifecycleState> {
        self.ego.identity_lifecycle(id)
    }
    pub fn historical_ego_identity(&self, id: EgoIdentityId, revision: u64) -> Option<EgoIdentity> {
        self.ego.historical_identity(id, revision)
    }
}
