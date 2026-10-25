use crate::{
    EgoAnchor, EgoAnchorId, EgoAnchorPriority, EgoIdentity, EgoIdentityId, EgoPersonality,
    EgoWebSynthesis, Phylactery, PhylacteryError,
};

impl Phylactery {
    pub fn ego_version(&self) -> u64 {
        self.native_ego_version()
    }

    /// Compatibility accessor for the currently active Identity.
    pub fn ego_identity(&self) -> Option<EgoIdentity> {
        self.native_ego_identity()
    }

    pub fn active_ego_identity_id(&self) -> Option<EgoIdentityId> {
        self.native_active_ego_identity_id()
    }

    pub fn ego_identities(&self) -> Vec<EgoIdentity> {
        self.native_ego_identities()
    }

    pub fn ego_identity_by_id(&self, id: EgoIdentityId) -> Option<EgoIdentity> {
        self.native_ego_identity_by_id(id)
    }

    /// Compatibility mutator: updates the active Identity, or creates the first `Default` Identity.
    pub fn put_ego_identity(
        &mut self,
        expected_revision: u64,
        text: String,
    ) -> Result<(EgoIdentity, bool), PhylacteryError> {
        self.native_put_ego_identity(expected_revision, text)
    }

    pub fn create_ego_identity(
        &mut self,
        name: String,
        text: String,
    ) -> Result<EgoIdentity, PhylacteryError> {
        self.native_create_ego_identity(name, text)
    }

    pub fn update_ego_identity(
        &mut self,
        id: EgoIdentityId,
        expected_revision: u64,
        name: String,
        text: String,
    ) -> Result<(EgoIdentity, bool), PhylacteryError> {
        self.native_update_ego_identity(id, expected_revision, name, text)
    }

    pub fn activate_ego_identity(&mut self, id: EgoIdentityId) -> Result<bool, PhylacteryError> {
        self.native_activate_ego_identity(id)
    }

    pub fn delete_ego_identity(
        &mut self,
        id: EgoIdentityId,
        expected_revision: u64,
    ) -> Result<bool, PhylacteryError> {
        self.native_delete_ego_identity(id, expected_revision)
    }

    pub fn ego_personality(&self) -> Option<&EgoPersonality> {
        self.native_ego_personality()
    }

    pub fn put_ego_personality(
        &mut self,
        expected_revision: u64,
        text: String,
        source_memory_version: u64,
    ) -> Result<(EgoPersonality, bool), PhylacteryError> {
        self.native_put_ego_personality(expected_revision, text, source_memory_version)
    }

    pub fn ego_anchors(&self) -> Vec<EgoAnchor> {
        self.native_ego_anchors()
    }
    pub fn ego_anchor(&self, id: EgoAnchorId) -> Option<EgoAnchor> {
        self.native_ego_anchor(id)
    }

    pub fn create_ego_anchor(
        &mut self,
        priority: EgoAnchorPriority,
        text: String,
    ) -> Result<EgoAnchor, PhylacteryError> {
        self.native_create_ego_anchor(priority, text)
    }

    pub fn update_ego_anchor(
        &mut self,
        id: EgoAnchorId,
        expected_revision: u64,
        priority: EgoAnchorPriority,
        text: String,
    ) -> Result<(EgoAnchor, bool), PhylacteryError> {
        self.native_update_ego_anchor(id, expected_revision, priority, text)
    }

    pub fn delete_ego_anchor(
        &mut self,
        id: EgoAnchorId,
        expected_revision: u64,
    ) -> Result<bool, PhylacteryError> {
        self.native_delete_ego_anchor(id, expected_revision)
    }

    pub fn ego_web_synthesis(&self) -> Option<&EgoWebSynthesis> {
        self.native_ego_web_synthesis()
    }

    pub fn put_ego_web_synthesis(
        &mut self,
        expected_revision: u64,
        source_memory_version: u64,
        text: String,
    ) -> Result<(EgoWebSynthesis, bool), PhylacteryError> {
        self.native_put_ego_web_synthesis(expected_revision, source_memory_version, text)
    }
}

impl Phylactery {
    pub fn ego_anchor_lifecycle(
        &self,
        id: EgoAnchorId,
    ) -> Option<crate::owner_lifecycle::LifecycleState> {
        self.native_ego_anchor_lifecycle(id)
    }
    pub fn historical_ego_anchor(&self, id: EgoAnchorId, revision: u64) -> Option<EgoAnchor> {
        self.native_historical_ego_anchor(id, revision)
    }
}

impl Phylactery {
    pub fn ego_identity_lifecycle(
        &self,
        id: EgoIdentityId,
    ) -> Option<crate::owner_lifecycle::LifecycleState> {
        self.native_ego_identity_lifecycle(id)
    }
    pub fn historical_ego_identity(&self, id: EgoIdentityId, revision: u64) -> Option<EgoIdentity> {
        self.native_historical_ego_identity(id, revision)
    }
}
