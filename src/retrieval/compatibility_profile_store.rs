use crate::{
    CompatibilityProfile, CompatibilityProfileError, CompatibilityProfileId,
    CompatibilityProfileStats,
};
use reliquary_container::Container;
use reliquary_vectors::compatibility_profile_probe::compare_profiles;

/// Root-side adapter for provider compatibility decisions. Persisted profile state
/// is held exclusively by reliquary-vector-store.
#[derive(Default)]
pub(crate) struct CompatibilityProfileStore {
    inner: reliquary_vector_store::CompatibilityProfileStore,
}

impl CompatibilityProfileStore {
    pub(crate) fn lower(&self) -> &reliquary_vector_store::CompatibilityProfileStore {
        &self.inner
    }
    pub(crate) fn from_store(inner: reliquary_vector_store::CompatibilityProfileStore) -> Self {
        Self { inner }
    }
    pub(crate) fn initialize(
        &self,
        container: &mut Container,
    ) -> Result<(), CompatibilityProfileError> {
        self.inner.initialize(container).map_err(Into::into)
    }
    pub(crate) fn put(
        &mut self,
        container: &mut Container,
        profile: CompatibilityProfile,
    ) -> Result<CompatibilityProfileId, CompatibilityProfileError> {
        self.inner.put(container, profile).map_err(Into::into)
    }
    pub(crate) fn insert_rebuilt(
        &mut self,
        profile: CompatibilityProfile,
    ) -> Result<(), CompatibilityProfileError> {
        self.inner.insert_rebuilt(profile).map_err(Into::into)
    }
    pub(crate) fn get(&self, id: CompatibilityProfileId) -> Option<&CompatibilityProfile> {
        self.inner.get(id)
    }
    pub(crate) fn get_active(&self, id: CompatibilityProfileId) -> Option<&CompatibilityProfile> {
        self.inner.get_active(id)
    }
    pub(crate) fn retire(
        &mut self,
        container: &mut Container,
        id: CompatibilityProfileId,
    ) -> Result<bool, CompatibilityProfileError> {
        self.inner.retire(container, id).map_err(Into::into)
    }
    pub(crate) fn retire_rebuilt(
        &mut self,
        id: CompatibilityProfileId,
    ) -> Result<(), CompatibilityProfileError> {
        self.inner.retire_rebuilt(id).map_err(Into::into)
    }
    pub(crate) fn is_retired(&self, id: CompatibilityProfileId) -> bool {
        self.inner.is_retired(id)
    }
    pub(crate) fn profiles(&self) -> Vec<CompatibilityProfile> {
        self.inner.profiles()
    }
    pub(crate) fn stats(&self) -> CompatibilityProfileStats {
        self.inner.stats()
    }
    pub(crate) fn compatible_with(
        &self,
        candidate: &CompatibilityProfile,
    ) -> Option<&CompatibilityProfile> {
        self.inner
            .iter()
            .filter(|profile| !self.inner.is_retired(profile.id))
            .filter(|profile| compare_profiles(profile, candidate).compatible)
            .min_by_key(|profile| profile.id.0)
    }
}
