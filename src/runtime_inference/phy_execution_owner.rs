use crate::dream_duplicate_index::DuplicateIndex;
use crate::lexical_index::LexicalIndex;
use crate::owner_lifecycle::PhylacteryOwner;
use crate::{
    CompatibilityProfile, CompatibilityProfileId, Memory, MemoryBodyId, MemoryError, MemoryId,
    MemoryVectorLocation, PhylacteryError,
};
use std::ops::{Deref, DerefMut};
use std::path::Path;

/// Mounted mutable PHY runtime aggregate. Durable store state remains in the
/// owner-lifecycle type; Retrieval and Dream caches are runtime projections.
pub struct PhyExecutionOwner {
    pub(crate) owner: PhylacteryOwner,
    pub(crate) lexical_index: LexicalIndex,
    pub(crate) duplicate_index: DuplicateIndex,
}

impl Deref for PhyExecutionOwner {
    type Target = PhylacteryOwner;
    fn deref(&self) -> &Self::Target {
        &self.owner
    }
}
impl DerefMut for PhyExecutionOwner {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.owner
    }
}

impl PhyExecutionOwner {
    pub fn memory(&mut self, id: MemoryId) -> Result<Memory, MemoryError> {
        self.owner.memories.memory(&mut self.owner.container, id)
    }

    pub fn memory_ids(&self) -> Vec<MemoryId> {
        self.owner.memories.current_ids()
    }

    pub fn memory_body_id(&self, id: MemoryId) -> Result<MemoryBodyId, MemoryError> {
        self.owner.memories.current_body_id(id)
    }

    pub fn sync(&self) -> Result<(), PhylacteryError> {
        self.owner.sync()?;
        Ok(())
    }

    pub fn graph_version(&self) -> u64 {
        self.owner.graph.graph_version()
    }

    pub fn memory_vector_location(
        &self,
        compatibility_profile_id: CompatibilityProfileId,
        memory_body_id: MemoryBodyId,
    ) -> Option<MemoryVectorLocation> {
        self.owner
            .memory_vectors
            .location(compatibility_profile_id, memory_body_id)
    }

    pub fn active_memory_compatibility_profile(
        &self,
        id: CompatibilityProfileId,
    ) -> Option<CompatibilityProfile> {
        let profile = self.owner.compatibility_profiles.get_active(id)?;
        self.owner
            .memory_vectors
            .has_complete_coverage(id, &self.owner.memories.current_body_ids())
            .then(|| profile.clone())
    }

    pub fn create(path: impl AsRef<Path>) -> Result<Self, PhylacteryError> {
        Self::from_replay(crate::owner_lifecycle::create_phylactery(path)?)
    }

    pub(crate) fn create_with_uuid(
        path: impl AsRef<Path>,
        owner_uuid: [u8; 16],
    ) -> Result<Self, PhylacteryError> {
        Self::from_replay(crate::owner_lifecycle::create_phylactery_with_uuid(
            path, owner_uuid,
        )?)
    }

    fn from_replay(owner: PhylacteryOwner) -> Result<Self, PhylacteryError> {
        Ok(Self {
            owner,
            lexical_index: LexicalIndex::default(),
            duplicate_index: DuplicateIndex::empty(),
        })
    }

    pub(crate) fn open_unvalidated(path: impl AsRef<Path>) -> Result<Self, PhylacteryError> {
        Self::from_replay(crate::owner_lifecycle::open_phylactery(path)?)
    }

    pub(crate) fn open_read_only_unvalidated(
        path: impl AsRef<Path>,
    ) -> Result<Self, PhylacteryError> {
        Self::from_replay(crate::owner_lifecycle::open_phylactery_read_only(path)?)
    }

    pub(crate) fn validate_open_owner(&mut self) -> Result<(), PhylacteryError> {
        let result = self;
        result.entity_resolutions.validate_local()?;
        crate::owner_lifecycle::validate_entity_resolution_references(
            &result.memories,
            &result.entities,
            &result.entity_resolutions.records(),
        )?;
        Ok(())
    }
}
