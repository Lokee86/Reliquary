use super::*;

impl RelWriter {
    pub fn publish_compatibility_profile(
        &mut self,
        profile: reliquary_vector_store::CompatibilityProfile,
    ) -> Result<CompatibilityProfileId, OwnerSessionError> {
        self.ensure_write_ready()?;
        self.publish_compatibility_profile_after_fence(profile)
    }

    pub fn publish_native_import_profile(
        &mut self,
        import_id: &str,
        profile: reliquary_vector_store::CompatibilityProfile,
    ) -> Result<CompatibilityProfileId, OwnerSessionError> {
        self.validate_native_import_postprocessing(import_id)?;
        self.bind_native_import_profile(import_id, profile.id)?;
        self.publish_compatibility_profile_after_fence(profile)
    }

    fn publish_compatibility_profile_after_fence(
        &mut self,
        profile: reliquary_vector_store::CompatibilityProfile,
    ) -> Result<CompatibilityProfileId, OwnerSessionError> {
        self.check_vector_lease()?;
        if self.session.common.profiles.get(profile.id).is_some()
            && self
                .session
                .common
                .profiles
                .get_active(profile.id)
                .is_none()
        {
            return Err(invalid("embedding profile is retired"));
        }
        let result = (|| {
            let id = self
                .session
                .common
                .profiles
                .put(&mut self.container, profile)?;
            self.container.sync()?;
            Ok(id)
        })();
        self.finish_vector_write(result)
    }
    pub fn publish_memory_vectors(
        &mut self,
        profile: CompatibilityProfileId,
        rows: &[MemoryVectorRow],
    ) -> Result<bool, OwnerSessionError> {
        self.ensure_write_ready()?;
        self.publish_memory_vectors_after_fence(profile, rows)
    }

    pub fn native_import_memory_vector_input(
        &self,
        import_id: &str,
        profile: CompatibilityProfileId,
        ids: &[MemoryId],
    ) -> Result<MemoryVectorInput, OwnerSessionError> {
        self.ensure_import_scope(import_id)?;
        self.require_native_import_profile(import_id, profile)?;
        for &id in ids {
            self.validate_native_import_memory(import_id, id)?;
        }
        self.session.memory_vector_input(profile, ids)
    }

    pub fn publish_native_import_memory_vectors(
        &mut self,
        import_id: &str,
        profile: CompatibilityProfileId,
        rows: &[MemoryVectorRow],
    ) -> Result<bool, OwnerSessionError> {
        self.ensure_import_scope(import_id)?;
        self.require_native_import_profile(import_id, profile)?;
        for row in rows {
            self.validate_native_import_memory(import_id, row.memory_id)?;
        }
        self.publish_memory_vectors_after_fence(profile, rows)
    }

    fn publish_memory_vectors_after_fence(
        &mut self,
        profile: CompatibilityProfileId,
        rows: &[MemoryVectorRow],
    ) -> Result<bool, OwnerSessionError> {
        self.check_vector_lease()?;
        let result = self
            .session
            .common
            .publish_vector_rows(&mut self.container, profile, rows);
        self.finish_vector_write(result)
    }
    fn check_vector_lease(&self) -> Result<(), OwnerSessionError> {
        if self.container.writer_lease_proof().is_none()
            || self.container.writer_lease_proof() != self.session.writer_lease_proof
        {
            return Err(invalid("REL vector writer is stale"));
        }
        Ok(())
    }
    fn finish_vector_write<T>(
        &mut self,
        result: Result<T, OwnerSessionError>,
    ) -> Result<T, OwnerSessionError> {
        if result.is_ok() {
            self.session.writer_lease_proof = self.container.writer_lease_proof();
        } else if uncertain(&result)
            || self.session.writer_lease_proof != self.container.writer_lease_proof()
        {
            self.session.write_poisoned = true;
        }
        result
    }
}
impl PhyWriter {
    pub fn publish_compatibility_profile(
        &mut self,
        profile: reliquary_vector_store::CompatibilityProfile,
    ) -> Result<CompatibilityProfileId, OwnerSessionError> {
        self.check_vector_lease()?;
        if self.common.profiles.get(profile.id).is_some()
            && self.common.profiles.get_active(profile.id).is_none()
        {
            return Err(invalid("embedding profile is retired"));
        }
        let result = (|| {
            let id = self.common.profiles.put(&mut self.container, profile)?;
            self.container.sync()?;
            Ok(id)
        })();
        self.finish_vector_write(result)
    }
    pub fn publish_memory_vectors(
        &mut self,
        profile: CompatibilityProfileId,
        rows: &[MemoryVectorRow],
    ) -> Result<bool, OwnerSessionError> {
        self.check_vector_lease()?;
        let result = self
            .common
            .publish_vector_rows(&mut self.container, profile, rows);
        self.finish_vector_write(result)
    }
    fn check_vector_lease(&self) -> Result<(), OwnerSessionError> {
        if self.poisoned
            || self.container.writer_lease_proof().is_none()
            || self.container.writer_lease_proof() != self.writer_lease_proof
        {
            return Err(invalid("PHY vector writer is stale or poisoned"));
        }
        Ok(())
    }
    fn finish_vector_write<T>(
        &mut self,
        result: Result<T, OwnerSessionError>,
    ) -> Result<T, OwnerSessionError> {
        if result.is_ok() {
            self.writer_lease_proof = self.container.writer_lease_proof();
        } else if uncertain(&result)
            || self.writer_lease_proof != self.container.writer_lease_proof()
        {
            self.poisoned = true;
        }
        result
    }
}

fn uncertain<T>(result: &Result<T, OwnerSessionError>) -> bool {
    result.as_ref().is_err_and(|error| {
        matches!(
            error,
            OwnerSessionError::Container(_)
                | OwnerSessionError::PackedVector(PackedVectorError::Container(_))
                | OwnerSessionError::Profile(ProfileStoreError::Container(_))
                | OwnerSessionError::MemoryVector(MemoryVectorError::Container(_))
        )
    })
}
