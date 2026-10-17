use crate::compatibility_profile_codec::{encode_format, encode_profile, encode_retirement};
use crate::compatibility_profile_probe::{
    compare_profiles, compatibility_profile_id, validate_profile,
};
use crate::{
    CompatibilityProfile, CompatibilityProfileError, CompatibilityProfileId,
    CompatibilityProfileStats, Container,
};
use std::collections::{HashMap, HashSet};

#[derive(Default)]
pub(crate) struct CompatibilityProfileStore {
    profiles: HashMap<CompatibilityProfileId, CompatibilityProfile>,
    retired: HashSet<CompatibilityProfileId>,
}

impl CompatibilityProfileStore {
    pub(crate) fn initialize(
        &self,
        container: &mut Container,
    ) -> Result<(), CompatibilityProfileError> {
        container.append(&encode_format())?;
        Ok(())
    }

    pub(crate) fn put(
        &mut self,
        container: &mut Container,
        profile: CompatibilityProfile,
    ) -> Result<CompatibilityProfileId, CompatibilityProfileError> {
        validate_profile(&profile)?;
        if profile.id != compatibility_profile_id(&profile) {
            return Err(CompatibilityProfileError::InvalidProfile(
                "profile identity",
            ));
        }
        if let Some(existing) = self.profiles.get(&profile.id) {
            return if existing == &profile {
                Ok(profile.id)
            } else {
                Err(CompatibilityProfileError::HashCollision)
            };
        }
        container.append(&encode_profile(&profile)?)?;
        let id = profile.id;
        self.profiles.insert(id, profile);
        Ok(id)
    }

    pub(crate) fn insert_rebuilt(
        &mut self,
        profile: CompatibilityProfile,
    ) -> Result<(), CompatibilityProfileError> {
        validate_profile(&profile)?;
        if profile.id != compatibility_profile_id(&profile) {
            return Err(CompatibilityProfileError::HashCollision);
        }
        if let Some(existing) = self.profiles.get(&profile.id) {
            return if existing == &profile {
                Ok(())
            } else {
                Err(CompatibilityProfileError::HashCollision)
            };
        }
        self.profiles.insert(profile.id, profile);
        Ok(())
    }

    pub(crate) fn compatible_with(
        &self,
        candidate: &CompatibilityProfile,
    ) -> Option<&CompatibilityProfile> {
        self.profiles
            .values()
            .filter(|profile| !self.retired.contains(&profile.id))
            .filter(|profile| compare_profiles(profile, candidate).compatible)
            .min_by_key(|profile| profile.id.0)
    }

    pub(crate) fn get(&self, id: CompatibilityProfileId) -> Option<&CompatibilityProfile> {
        self.profiles.get(&id)
    }

    pub(crate) fn get_active(&self, id: CompatibilityProfileId) -> Option<&CompatibilityProfile> {
        (!self.retired.contains(&id))
            .then(|| self.profiles.get(&id))
            .flatten()
    }

    pub(crate) fn retire(
        &mut self,
        container: &mut Container,
        id: CompatibilityProfileId,
    ) -> Result<bool, CompatibilityProfileError> {
        if !self.profiles.contains_key(&id) {
            return Err(CompatibilityProfileError::MissingProfile);
        }
        if self.retired.contains(&id) {
            return Ok(false);
        }
        container.append(&encode_retirement(id))?;
        self.retired.insert(id);
        Ok(true)
    }

    pub(crate) fn retire_rebuilt(
        &mut self,
        id: CompatibilityProfileId,
    ) -> Result<(), CompatibilityProfileError> {
        if !self.profiles.contains_key(&id) {
            return Err(CompatibilityProfileError::MissingProfile);
        }
        self.retired.insert(id);
        Ok(())
    }

    pub(crate) fn is_retired(&self, id: CompatibilityProfileId) -> bool {
        self.retired.contains(&id)
    }

    pub(crate) fn profiles(&self) -> Vec<CompatibilityProfile> {
        let mut profiles: Vec<_> = self.profiles.values().cloned().collect();
        profiles.sort_by_key(|profile| profile.id.0);
        profiles
    }

    pub(crate) fn stats(&self) -> CompatibilityProfileStats {
        CompatibilityProfileStats {
            profiles: self.profiles.len(),
            active_profiles: self.profiles.len() - self.retired.len(),
            retired_profiles: self.retired.len(),
        }
    }
}
