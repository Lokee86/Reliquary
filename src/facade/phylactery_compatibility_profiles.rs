use crate::compatibility_profile_probe::{profile_from_endpoint, verify_endpoint};
use crate::{
    CompatibilityProfile, CompatibilityProfileError, CompatibilityProfileId,
    CompatibilityProfileStats, CompatibilityReport, EmbeddingEndpoint, Phylactery,
};

impl Phylactery {
    pub fn establish_compatibility_profile(
        &mut self,
        endpoint: &impl EmbeddingEndpoint,
    ) -> Result<CompatibilityProfile, CompatibilityProfileError> {
        let candidate = profile_from_endpoint(endpoint)?;
        if let Some(existing) = self.compatibility_profiles.compatible_with(&candidate) {
            return Ok(existing.clone());
        }
        self.compatibility_profiles
            .put(&mut self.container, candidate.clone())?;
        Ok(candidate)
    }

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
        self.compatibility_profiles.retire(&mut self.container, id)
    }

    pub fn compatibility_profile_is_retired(&self, id: CompatibilityProfileId) -> bool {
        self.compatibility_profiles.is_retired(id)
    }

    pub fn active_memory_compatibility_profile(
        &self,
        id: CompatibilityProfileId,
    ) -> Option<CompatibilityProfile> {
        let profile = self.compatibility_profiles.get_active(id)?;
        self.memory_vectors
            .has_complete_coverage(id, &self.memories.current_body_ids())
            .then(|| profile.clone())
    }

    pub fn active_memory_compatibility_profiles(&self) -> Vec<CompatibilityProfile> {
        self.compatibility_profiles
            .profiles()
            .into_iter()
            .filter_map(|profile| self.active_memory_compatibility_profile(profile.id))
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
