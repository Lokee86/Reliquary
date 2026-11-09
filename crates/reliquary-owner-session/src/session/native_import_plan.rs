//! Named owner operations for a resumable native-import processing plan.
use super::*;
use crate::records::ImportProcessingBinding;

impl RelWriter {
    /// Bind selected routes and stage policy before claims or provider calls.
    /// A held, unpaired import may attach its first authorized destination
    /// before reaching the vector stage.
    pub fn bind_native_import_processing(
        &mut self,
        import_id: &str,
        policy: [u8; 32],
        destination: Option<[u8; 16]>,
    ) -> Result<(), OwnerSessionError> {
        if destination.is_some()
            && self
                .session
                .import_status(import_id)
                .is_some_and(|status| match status {
                    ImportEpochStatus::Open(start) | ImportEpochStatus::Completed { start, .. } => {
                        start.source_format == reliquary_archive::HERMES_COPIED_SOURCE_FORMAT
                    }
                })
        {
            // Copied author metadata carries no authority to publish into a PHY.
            return Err(OwnerSessionError::ManagementConflict);
        }
        if let Some(previous) = self.session.imports.processing_binding(import_id) {
            if previous.policy != policy
                || previous.destination.is_some() && previous.destination != destination
                || previous.profile.is_some() && previous.destination != destination
            {
                return Err(OwnerSessionError::ManagementConflict);
            }
            if previous.destination == destination {
                return Ok(());
            }
            return self.record_import_processing_binding(
                import_id,
                ImportProcessingBinding {
                    destination,
                    ..previous
                },
            );
        }
        if matches!(
            self.session.import_status(import_id),
            Some(ImportEpochStatus::Completed { .. })
        ) || self
            .session
            .has_native_import_historical_freshness(import_id)
        {
            // Completed or sealed imports written before this marker retain
            // their original receipt and Freshness prefix.
            return Ok(());
        }
        self.record_import_processing_binding(
            import_id,
            ImportProcessingBinding {
                policy,
                destination,
                profile: None,
            },
        )
    }

    pub fn bind_native_import_profile(
        &mut self,
        import_id: &str,
        profile: CompatibilityProfileId,
    ) -> Result<(), OwnerSessionError> {
        let Some(binding) = self.session.imports.processing_binding(import_id) else {
            return Ok(());
        };
        match binding.profile {
            Some(current) if current == profile.0 => Ok(()),
            Some(_) => Err(OwnerSessionError::ManagementConflict),
            None => self.record_import_processing_binding(
                import_id,
                ImportProcessingBinding {
                    profile: Some(profile.0),
                    ..binding
                },
            ),
        }
    }

    pub fn require_native_import_profile(
        &self,
        import_id: &str,
        profile: CompatibilityProfileId,
    ) -> Result<(), OwnerSessionError> {
        if self
            .session
            .imports
            .processing_binding(import_id)
            .is_some_and(|binding| binding.profile != Some(profile.0))
        {
            return Err(OwnerSessionError::ManagementConflict);
        }
        Ok(())
    }

    pub(crate) fn record_import_processing_binding(
        &mut self,
        import_id: &str,
        binding: ImportProcessingBinding,
    ) -> Result<(), OwnerSessionError> {
        self.ensure_import_scope(import_id)?;
        let start = self
            .session
            .imports
            .open_epoch()
            .expect("validated open epoch");
        if super::decode_import_key(&start.import_key)
            .and_then(|key| self.session.native_import_receipt(&key))
            .is_none()
        {
            return Err(OwnerSessionError::Record(
                "processing requires the accepted Archive import receipt".into(),
            ));
        }
        if let Err(error) =
            self.session
                .imports
                .append_processing_binding(&mut self.container, import_id, binding)
        {
            self.session.write_poisoned = true;
            return Err(OwnerSessionError::Record(error));
        }
        self.session.writer_lease_proof = self.container.writer_lease_proof();
        Ok(())
    }
}

impl RelSession {
    /// Read the active profile pinned to the import, including after settlement.
    pub fn native_import_processing_profile(
        &self,
        import_id: &str,
    ) -> Result<Option<CompatibilityProfile>, OwnerSessionError> {
        let Some(binding) = self.imports.processing_binding(import_id) else {
            return Ok(None);
        };
        let Some(profile) = binding.profile else {
            if self.imports.settlement(import_id).is_some() {
                return Err(OwnerSessionError::ManagementConflict);
            }
            return Ok(None);
        };
        self.active_compatibility_profile(CompatibilityProfileId(profile))
            .cloned()
            .map(Some)
            .ok_or_else(|| {
                OwnerSessionError::Record("import processing profile is missing or retired".into())
            })
    }
}
