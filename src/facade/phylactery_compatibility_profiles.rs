use crate::{
    CompatibilityProfile, CompatibilityProfileError, CompatibilityProfileId,
    CompatibilityProfileStats, CompatibilityReport, EmbeddingEndpoint, Phylactery,
};

impl Phylactery {
    pub fn establish_compatibility_profile(
        &mut self,
        endpoint: &impl EmbeddingEndpoint,
    ) -> Result<CompatibilityProfile, CompatibilityProfileError> {
        self.public_establish_profile(endpoint)
    }

    pub fn compatibility_profile(
        &self,
        id: CompatibilityProfileId,
    ) -> Result<CompatibilityProfile, CompatibilityProfileError> {
        self.public_profile(id)
    }

    pub fn compatibility_profiles(&self) -> Vec<CompatibilityProfile> {
        self.public_profiles()
    }

    pub fn retire_compatibility_profile(
        &mut self,
        id: CompatibilityProfileId,
    ) -> Result<bool, CompatibilityProfileError> {
        self.public_retire_profile(id)
    }

    pub fn compatibility_profile_is_retired(&self, id: CompatibilityProfileId) -> bool {
        self.public_profile_is_retired(id)
    }

    pub fn active_memory_compatibility_profiles(&self) -> Vec<CompatibilityProfile> {
        self.public_active_memory_profiles()
    }

    pub fn compatibility_profile_stats(&self) -> CompatibilityProfileStats {
        self.public_profile_stats()
    }

    pub fn verify_compatibility_endpoint(
        &self,
        id: CompatibilityProfileId,
        endpoint: &impl EmbeddingEndpoint,
    ) -> Result<CompatibilityReport, CompatibilityProfileError> {
        self.public_verify_compatibility_endpoint(id, endpoint)
    }
}
