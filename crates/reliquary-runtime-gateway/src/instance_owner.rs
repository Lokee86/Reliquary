use super::*;

impl Instance {
    pub(crate) fn gateway_binding(&self) -> usize {
        Arc::as_ptr(&self.owners) as usize
    }

    pub fn principal(&self) -> &str {
        &self.principal
    }

    pub fn reliquary(&self, owner: OwnerId) -> Result<Arc<RelSession>, GatewayError> {
        self.ensure_credential_active()?;
        if !self.rel_grants.contains(&owner) {
            return Err(GatewayError::UnauthorizedRel {
                principal: self.principal.to_string(),
                owner,
            });
        }
        let owners = self
            .owners
            .read()
            .map_err(|_| GatewayError::RegistryPoisoned)?;
        match owners.get(&owner) {
            None => Err(GatewayError::UnknownOwner(owner)),
            Some(MountedOwner::Rel(session)) => Ok(Arc::clone(session)),
            Some(MountedOwner::RelWriter(_)) => Err(GatewayError::WriterUnavailable),
            Some(MountedOwner::Phy(_)) => Err(GatewayError::WrongOwnerKind(owner)),
            Some(MountedOwner::PhyWriter(_)) => Err(GatewayError::WrongOwnerKind(owner)),
        }
    }

    /// Execute bounded owner-local read work against a writable mount's current
    /// projection. Do not call providers, network APIs, or external callbacks here.
    pub(crate) fn with_writable_reliquary<T>(
        &self,
        owner: OwnerId,
        read: impl FnOnce(&RelSession) -> T,
    ) -> Result<T, GatewayError> {
        self.with_rel_writer(owner, |writer| read(writer.read_session()))
    }

    pub(crate) fn with_reliquary<T>(
        &self,
        owner: OwnerId,
        read: impl FnOnce(&RelSession) -> T,
    ) -> Result<T, GatewayError> {
        self.ensure_credential_active()?;
        if !self.rel_grants.contains(&owner) {
            return Err(GatewayError::UnauthorizedRel {
                principal: self.principal.to_string(),
                owner,
            });
        }
        let mounted = {
            let owners = self
                .owners
                .read()
                .map_err(|_| GatewayError::RegistryPoisoned)?;
            owners
                .get(&owner)
                .cloned()
                .ok_or(GatewayError::UnknownOwner(owner))?
        };
        match mounted {
            MountedOwner::Rel(session) => Ok(read(&session)),
            MountedOwner::RelWriter(writer) => {
                let writer = writer
                    .writer
                    .lock()
                    .map_err(|_| GatewayError::RegistryPoisoned)?;
                self.ensure_credential_active()?;
                Ok(read(writer.read_session()))
            }
            MountedOwner::Phy(_) | MountedOwner::PhyWriter(_) => {
                Err(GatewayError::WrongOwnerKind(owner))
            }
        }
    }

    pub fn finalize_conversation_episode(
        &self,
        owner: OwnerId,
        conversation_id: &str,
        start_node_id: &str,
        end_node_id: &str,
        boundary: reliquary_owner_session::model::EpisodeBoundary,
        finalized_at_ns: i64,
    ) -> Result<reliquary_owner_session::model::EpisodeId, GatewayError> {
        Ok(self.with_rel_writer(owner, |writer| {
            reliquary_application::finalize_native_episode(
                writer,
                conversation_id,
                start_node_id,
                end_node_id,
                boundary,
                finalized_at_ns,
            )
        })??)
    }

    pub(super) fn with_rel_writer<T>(
        &self,
        owner: OwnerId,
        operation: impl FnOnce(&mut RelWriter) -> T,
    ) -> Result<T, GatewayError> {
        self.ensure_credential_active()?;
        if !self.rel_grants.contains(&owner) {
            return Err(GatewayError::UnauthorizedRel {
                principal: self.principal.to_string(),
                owner,
            });
        }
        let mounted = {
            let owners = self
                .owners
                .read()
                .map_err(|_| GatewayError::RegistryPoisoned)?;
            match owners.get(&owner) {
                None => return Err(GatewayError::UnknownOwner(owner)),
                Some(MountedOwner::RelWriter(writer)) => Arc::clone(writer),
                Some(MountedOwner::Rel(_)) => return Err(GatewayError::WriterUnavailable),
                Some(MountedOwner::Phy(_)) => return Err(GatewayError::WrongOwnerKind(owner)),
                Some(MountedOwner::PhyWriter(_)) => {
                    return Err(GatewayError::WrongOwnerKind(owner));
                }
            }
        };
        let mut writer = mounted
            .writer
            .lock()
            .map_err(|_| GatewayError::RegistryPoisoned)?;
        self.ensure_credential_active()?;
        Ok(operation(&mut writer))
    }

    pub fn claim_next_insomnia_input(
        &self,
        owner: OwnerId,
        worker: &str,
        started_at_ns: i64,
        lease_duration_ns: i64,
        max_admissions: usize,
    ) -> Result<Option<RelInsomniaClaimInput>, GatewayError> {
        self.ensure_credential_active()?;
        if !self.rel_grants.contains(&owner) {
            return Err(GatewayError::UnauthorizedRel {
                principal: self.principal.to_string(),
                owner,
            });
        }
        let mounted = {
            let owners = self
                .owners
                .read()
                .map_err(|_| GatewayError::RegistryPoisoned)?;
            match owners.get(&owner) {
                None => return Err(GatewayError::UnknownOwner(owner)),
                Some(MountedOwner::RelWriter(writer)) => Arc::clone(writer),
                Some(MountedOwner::Rel(_)) => return Err(GatewayError::WriterUnavailable),
                Some(_) => return Err(GatewayError::WrongOwnerKind(owner)),
            }
        };
        let result = {
            let mut writer = mounted
                .writer
                .lock()
                .map_err(|_| GatewayError::RegistryPoisoned)?;
            self.ensure_credential_active()?;
            writer.claim_next_insomnia_input(
                worker,
                started_at_ns,
                lease_duration_ns,
                max_admissions,
            )?
        };
        Ok(result)
    }

    /// Persist an explicit Held state for a claimed Episode that this worker
    /// cannot safely finish. Held work is excluded from automatic claim/retry.
    pub fn hold_insomnia_claim(
        &self,
        owner: OwnerId,
        claim: &RelInsomniaClaimInput,
        held_at_ns: i64,
        reason: String,
    ) -> Result<(), GatewayError> {
        self.hold_insomnia_claim_with_payload(owner, claim, held_at_ns, reason, None)
    }

    pub(crate) fn hold_insomnia_claim_with_payload(
        &self,
        owner: OwnerId,
        claim: &RelInsomniaClaimInput,
        held_at_ns: i64,
        reason: String,
        held_payload: Option<Vec<u8>>,
    ) -> Result<(), GatewayError> {
        self.ensure_credential_active()?;
        if !self.rel_grants.contains(&owner) {
            return Err(GatewayError::UnauthorizedRel {
                principal: self.principal.to_string(),
                owner,
            });
        }
        if claim.owner_id != owner.as_bytes() {
            return Err(GatewayError::OwnerSession(OwnerSessionError::Record(
                "Insomnia claim owner mismatch".into(),
            )));
        }
        let mounted = {
            let owners = self
                .owners
                .read()
                .map_err(|_| GatewayError::RegistryPoisoned)?;
            match owners.get(&owner) {
                None => return Err(GatewayError::UnknownOwner(owner)),
                Some(MountedOwner::RelWriter(writer)) => Arc::clone(writer),
                Some(MountedOwner::Rel(_)) => return Err(GatewayError::WriterUnavailable),
                Some(_) => return Err(GatewayError::WrongOwnerKind(owner)),
            }
        };
        let mut writer = mounted
            .writer
            .lock()
            .map_err(|_| GatewayError::RegistryPoisoned)?;
        self.ensure_credential_active()?;
        writer.hold_insomnia_claim_with_payload(claim, held_at_ns, reason, held_payload)?;
        Ok(())
    }

    pub(crate) fn checkpoint_insomnia_claim(
        &self,
        owner: OwnerId,
        claim: &RelInsomniaClaimInput,
        checkpointed_at_ns: i64,
        payload: Vec<u8>,
    ) -> Result<reliquary_owner_session::model::InsomniaWork, GatewayError> {
        self.ensure_credential_active()?;
        if !self.rel_grants.contains(&owner) {
            return Err(GatewayError::UnauthorizedRel {
                principal: self.principal.to_string(),
                owner,
            });
        }
        if claim.owner_id != owner.as_bytes() {
            return Err(GatewayError::OwnerSession(OwnerSessionError::Record(
                "Insomnia claim owner mismatch".into(),
            )));
        }
        let mounted = {
            let owners = self
                .owners
                .read()
                .map_err(|_| GatewayError::RegistryPoisoned)?;
            match owners.get(&owner) {
                None => return Err(GatewayError::UnknownOwner(owner)),
                Some(MountedOwner::RelWriter(writer)) => Arc::clone(writer),
                Some(MountedOwner::Rel(_)) => return Err(GatewayError::WriterUnavailable),
                Some(_) => return Err(GatewayError::WrongOwnerKind(owner)),
            }
        };
        let mut writer = mounted
            .writer
            .lock()
            .map_err(|_| GatewayError::RegistryPoisoned)?;
        self.ensure_credential_active()?;
        Ok(writer.checkpoint_insomnia_claim(claim, checkpointed_at_ns, payload)?)
    }

    /// Commit only a prepared, same-owner Insomnia batch under its original
    /// durable lease. The application layer verifies the exact receipt.
    pub(crate) fn commit_prepared_insomnia_claim(
        &self,
        owner: OwnerId,
        claim: &RelInsomniaClaimInput,
        prepared: &PreparedIngest,
        extractor_model: &str,
        extractor_version: &str,
        completed_at_ns: i64,
    ) -> Result<CompletionCheck, GatewayError> {
        self.ensure_credential_active()?;
        if !self.rel_grants.contains(&owner) {
            return Err(GatewayError::UnauthorizedRel {
                principal: self.principal.to_string(),
                owner,
            });
        }
        if claim.owner_id != owner.as_bytes()
            || prepared.owner_id() != owner.as_bytes()
            || claim.episode.id.0 != prepared.episode_id().0
            || claim.work.episode_id.0 != prepared.episode_id().0
        {
            return Err(GatewayError::OwnerSession(OwnerSessionError::Record(
                "Insomnia prepared batch does not match its owner claim".into(),
            )));
        }
        let mounted = {
            let owners = self
                .owners
                .read()
                .map_err(|_| GatewayError::RegistryPoisoned)?;
            match owners.get(&owner) {
                None => return Err(GatewayError::UnknownOwner(owner)),
                Some(MountedOwner::RelWriter(writer)) => Arc::clone(writer),
                Some(MountedOwner::Rel(_)) => return Err(GatewayError::WriterUnavailable),
                Some(_) => return Err(GatewayError::WrongOwnerKind(owner)),
            }
        };
        let mut writer = mounted
            .writer
            .lock()
            .map_err(|_| GatewayError::RegistryPoisoned)?;
        self.ensure_credential_active()?;
        commit_prepared_native_ingest(
            &mut writer,
            prepared,
            &claim.work,
            extractor_model,
            extractor_version,
            completed_at_ns,
        )
        .map_err(|error| GatewayError::OwnerSession(OwnerSessionError::Record(error.to_string())))
    }
    pub fn phylactery(&self, owner: OwnerId) -> Result<Arc<PhySession>, GatewayError> {
        self.ensure_credential_active()?;
        if !self.phy_grants.contains(&owner) {
            return Err(GatewayError::UnauthorizedPhy {
                principal: self.principal.to_string(),
                owner,
            });
        }
        let owners = self
            .owners
            .read()
            .map_err(|_| GatewayError::RegistryPoisoned)?;
        match owners.get(&owner) {
            None => Err(GatewayError::UnknownOwner(owner)),
            Some(MountedOwner::Phy(session)) => Ok(Arc::clone(session)),
            Some(MountedOwner::Rel(_)) => Err(GatewayError::WrongOwnerKind(owner)),
            Some(MountedOwner::RelWriter(_)) => Err(GatewayError::WrongOwnerKind(owner)),
            Some(MountedOwner::PhyWriter(_)) => Err(GatewayError::WriterUnavailable),
        }
    }

    /// Recheck the credential backing this Instance without accessing an owner.
    /// Product lifecycle operations use this while an owner is quiesced.
    pub fn ensure_credential_active(&self) -> Result<(), GatewayError> {
        if self
            .credential_active
            .as_ref()
            .is_some_and(|active| !active.load(Ordering::Acquire))
        {
            Err(GatewayError::CredentialRevoked)
        } else {
            Ok(())
        }
    }
}
