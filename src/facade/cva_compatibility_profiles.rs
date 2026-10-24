use crate::compatibility_profile_probe::{profile_from_endpoint, verify_endpoint};
use crate::{
    CompatibilityProfile, CompatibilityProfileError, CompatibilityProfileId,
    CompatibilityProfileStats, CompatibilityReport, Cva, EmbeddingEndpoint,
};

impl Cva {
    pub fn compatibility_profile(
        &self,
        id: CompatibilityProfileId,
    ) -> Result<CompatibilityProfile, CompatibilityProfileError> {
        self.compatibility_profiles
            .get(id)
            .cloned()
            .ok_or(CompatibilityProfileError::MissingProfile)
    }

    pub fn compatibility_profiles(&self) -> Vec<CompatibilityProfile> {
        self.compatibility_profiles.profiles()
    }

    pub fn retire_compatibility_profile(
        &mut self,
        id: CompatibilityProfileId,
    ) -> Result<bool, CompatibilityProfileError> {
        self.owner
            .compatibility_profiles
            .retire(&mut self.owner.container, id)
    }

    pub fn compatibility_profile_is_retired(&self, id: CompatibilityProfileId) -> bool {
        self.compatibility_profiles.is_retired(id)
    }

    pub fn active_memory_compatibility_profile(
        &self,
        id: CompatibilityProfileId,
    ) -> Option<CompatibilityProfile> {
        if self.owner_uuid().is_none() || self.open_import_epoch().is_some() {
            return None;
        }
        let profile = self.compatibility_profiles.get_active(id)?;
        self.memory_vectors
            .has_complete_coverage(id, &self.memories.current_body_ids())
            .then(|| profile.clone())
    }

    pub fn active_memory_compatibility_profiles(&self) -> Vec<CompatibilityProfile> {
        self.runtime_active_memory_compatibility_profiles()
    }

    pub fn active_archive_compatibility_profiles(&self) -> Vec<CompatibilityProfile> {
        self.compatibility_profiles
            .profiles()
            .into_iter()
            .filter(|profile| self.current_vector_generation(profile.id).is_some())
            .collect()
    }

    pub fn compatibility_profile_stats(&self) -> CompatibilityProfileStats {
        self.compatibility_profiles.stats()
    }

    pub fn verify_compatibility_endpoint(
        &self,
        id: CompatibilityProfileId,
        endpoint: &impl EmbeddingEndpoint,
    ) -> Result<CompatibilityReport, CompatibilityProfileError> {
        let profile = self
            .compatibility_profiles
            .get(id)
            .ok_or(CompatibilityProfileError::MissingProfile)?;
        verify_endpoint(profile, endpoint)
    }
}
