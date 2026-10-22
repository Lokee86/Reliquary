use crate::ego_store::validate_source_memory_version;
use crate::{Cva, CvaError, EgoAnchor, EgoAnchorId, EgoAnchorPriority, EgoWebSynthesis};

impl Cva {
    pub fn ego_version(&self) -> u64 {
        self.ego.version()
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
    ) -> Result<EgoAnchor, CvaError> {
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
    ) -> Result<(EgoAnchor, bool), CvaError> {
        Ok(self
            .ego
            .update_anchor(&mut self.container, id, expected_revision, priority, text)?)
    }

    pub fn delete_ego_anchor(
        &mut self,
        id: EgoAnchorId,
        expected_revision: u64,
    ) -> Result<bool, CvaError> {
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
    ) -> Result<(EgoWebSynthesis, bool), CvaError> {
        validate_source_memory_version(source_memory_version, self.memory_version())?;
        Ok(self.ego.put_synthesis(
            &mut self.container,
            expected_revision,
            source_memory_version,
            text,
        )?)
    }
}

impl Cva {
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
