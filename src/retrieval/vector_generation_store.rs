use crate::vector_generation_codec::{
    GenerationPayload, GenerationVersion, encode_format, encode_generation, encode_version,
};
use crate::vector_generation_validation::vector_generation_id;
use crate::{
    CompatibilityProfileId, Container, VectorGeneration, VectorGenerationError, VectorGenerationId,
    VectorGenerationStats,
};
use std::collections::HashMap;

#[derive(Default)]
pub(crate) struct VectorGenerationStore {
    generations: Vec<VectorGeneration>,
    by_id: HashMap<VectorGenerationId, usize>,
    current_by_profile: HashMap<CompatibilityProfileId, usize>,
    next_vector_version: u64,
}

impl VectorGenerationStore {
    pub(crate) fn empty() -> Self {
        Self {
            generations: Vec::new(),
            by_id: HashMap::new(),
            current_by_profile: HashMap::new(),
            next_vector_version: 1,
        }
    }

    pub(crate) fn initialize(
        &self,
        container: &mut Container,
    ) -> Result<(), VectorGenerationError> {
        container.append(&encode_format())?;
        Ok(())
    }

    pub(crate) fn publish(
        &mut self,
        container: &mut Container,
        compatibility_profile_id: CompatibilityProfileId,
        archive_vector_id: crate::ArchiveVectorId,
        source_archive_version: u64,
    ) -> Result<VectorGeneration, VectorGenerationError> {
        if let Some(current) = self.current_generation(compatibility_profile_id)
            && source_archive_version < current.source_archive_version
        {
            return Err(VectorGenerationError::SourceVersionRegression);
        }
        let id = vector_generation_id(
            compatibility_profile_id,
            archive_vector_id,
            source_archive_version,
        );
        if let Some(index) = self.by_id.get(&id).copied() {
            return Ok(self.generations[index]);
        }
        let payload = GenerationPayload {
            id,
            compatibility_profile_id,
            archive_vector_id,
            source_archive_version,
        };
        let record = container.append(&encode_generation(payload))?;
        let vector_version = self.next_vector_version;
        let next = vector_version
            .checked_add(1)
            .ok_or(VectorGenerationError::VectorVersionExhausted)?;
        let global_version = container.allocate_version()?;
        container.append(&encode_version(GenerationVersion {
            global_version,
            vector_version,
            record,
        }))?;
        let generation = VectorGeneration {
            id,
            compatibility_profile_id,
            archive_vector_id,
            source_archive_version,
            global_version,
            vector_version,
        };
        self.insert(generation)?;
        self.next_vector_version = next;
        Ok(generation)
    }

    pub(crate) fn insert_rebuilt(
        &mut self,
        generation: VectorGeneration,
    ) -> Result<(), VectorGenerationError> {
        if generation.vector_version != self.next_vector_version {
            return Err(VectorGenerationError::InvalidGenerationVersion);
        }
        self.insert(generation)?;
        self.next_vector_version = self
            .next_vector_version
            .checked_add(1)
            .ok_or(VectorGenerationError::VectorVersionExhausted)?;
        Ok(())
    }

    pub(crate) fn generation(&self, id: VectorGenerationId) -> Option<VectorGeneration> {
        self.by_id.get(&id).map(|index| self.generations[*index])
    }

    pub(crate) fn current_generation(
        &self,
        compatibility_profile_id: CompatibilityProfileId,
    ) -> Option<VectorGeneration> {
        self.current_by_profile
            .get(&compatibility_profile_id)
            .map(|index| self.generations[*index])
    }

    pub(crate) fn generation_at(
        &self,
        compatibility_profile_id: CompatibilityProfileId,
        vector_version: u64,
    ) -> Option<VectorGeneration> {
        self.generations
            .iter()
            .rev()
            .find(|generation| {
                generation.compatibility_profile_id == compatibility_profile_id
                    && generation.vector_version <= vector_version
            })
            .copied()
    }

    pub(crate) fn generations(&self) -> &[VectorGeneration] {
        &self.generations
    }

    pub(crate) fn vector_version(&self) -> u64 {
        self.next_vector_version.saturating_sub(1)
    }

    pub(crate) fn stats(&self) -> VectorGenerationStats {
        VectorGenerationStats {
            generations: self.generations.len(),
            active_profiles: self.current_by_profile.len(),
            vector_version: self.vector_version(),
        }
    }

    fn insert(&mut self, generation: VectorGeneration) -> Result<(), VectorGenerationError> {
        if vector_generation_id(
            generation.compatibility_profile_id,
            generation.archive_vector_id,
            generation.source_archive_version,
        ) != generation.id
        {
            return Err(VectorGenerationError::HashCollision);
        }
        if let Some(index) = self.by_id.get(&generation.id).copied() {
            return if self.generations[index] == generation {
                Ok(())
            } else {
                Err(VectorGenerationError::HashCollision)
            };
        }
        if let Some(current) = self.current_generation(generation.compatibility_profile_id)
            && generation.source_archive_version < current.source_archive_version
        {
            return Err(VectorGenerationError::SourceVersionRegression);
        }
        let index = self.generations.len();
        self.generations.push(generation);
        self.by_id.insert(generation.id, index);
        self.current_by_profile
            .insert(generation.compatibility_profile_id, index);
        Ok(())
    }
}
