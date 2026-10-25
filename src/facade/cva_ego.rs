use crate::{Cva, CvaError, EgoAnchor, EgoAnchorId, EgoAnchorPriority, EgoWebSynthesis};

impl Cva {
    pub fn ego_version(&self) -> u64 {
        self.runtime_ego_version()
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
    ) -> Result<EgoAnchor, CvaError> {
        self.native_create_ego_anchor(priority, text)
    }

    pub fn update_ego_anchor(
        &mut self,
        id: EgoAnchorId,
        expected_revision: u64,
        priority: EgoAnchorPriority,
        text: String,
    ) -> Result<(EgoAnchor, bool), CvaError> {
        self.native_update_ego_anchor(id, expected_revision, priority, text)
    }

    pub fn delete_ego_anchor(
        &mut self,
        id: EgoAnchorId,
        expected_revision: u64,
    ) -> Result<bool, CvaError> {
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
    ) -> Result<(EgoWebSynthesis, bool), CvaError> {
        self.native_put_ego_web_synthesis(expected_revision, source_memory_version, text)
    }
}

impl Cva {
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
