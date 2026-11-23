use super::*;

#[derive(Clone, Copy)]
pub enum OwnerMountMode {
    ReadOnlyRel,
    WritableRel,
    ReadOnlyPhy,
    WritablePhy,
}

/// An admitted owner whose state and lease have not been installed in a Gateway.
pub struct PreparedOwnerMount {
    owner: OwnerId,
    state: MountedOwner,
}

impl Gateway {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn mount_reliquary(
        &self,
        path: impl AsRef<Path>,
        graph_admission: &GraphAdmission,
        accepted_episodes: impl IntoIterator<Item = AcceptedEpisodeOrder>,
        dream_admissions: impl IntoIterator<Item = DreamBeginAdmission>,
    ) -> Result<OwnerId, GatewayError> {
        let session = Arc::new(RelSession::open_path(
            path,
            graph_admission,
            accepted_episodes,
            dream_admissions,
        )?);
        let owner = OwnerId(session.owner_id());
        self.check_admission(owner, graph_admission)?;
        self.insert(owner, MountedOwner::Rel(session))?;
        Ok(owner)
    }

    /// Mount a current-format REL using owner-session's durable admission.
    pub fn mount_reliquary_current(&self, path: impl AsRef<Path>) -> Result<OwnerId, GatewayError> {
        self.install_owner_mount(Self::prepare_owner_mount(
            path,
            OwnerMountMode::ReadOnlyRel,
            None,
        )?)
    }

    /// Mount a REL with one owner-local writer lease. The mutable Container
    /// remains inside owner-session; request code receives only concrete ops.
    pub fn mount_writable_reliquary(
        &self,
        path: impl AsRef<Path>,
        graph_admission: &GraphAdmission,
        accepted_episodes: impl IntoIterator<Item = AcceptedEpisodeOrder>,
        dream_admissions: impl IntoIterator<Item = DreamBeginAdmission>,
    ) -> Result<OwnerId, GatewayError> {
        let writer = RelWriter::open_path(
            path,
            graph_admission.clone(),
            accepted_episodes,
            dream_admissions,
        )?;
        let owner = OwnerId(writer.owner_id());
        self.check_admission(owner, graph_admission)?;
        let mounted = MountedRelWriter {
            writer: std::sync::Mutex::new(writer),
        };
        self.insert(owner, MountedOwner::RelWriter(Arc::new(mounted)))?;
        Ok(owner)
    }

    /// Mount a current-format REL with its exclusive owner-session writer.
    pub fn mount_writable_reliquary_current(
        &self,
        path: impl AsRef<Path>,
    ) -> Result<OwnerId, GatewayError> {
        self.install_owner_mount(Self::prepare_owner_mount(
            path,
            OwnerMountMode::WritableRel,
            None,
        )?)
    }

    pub fn mount_phylactery(
        &self,
        path: impl AsRef<Path>,
        graph_admission: &GraphAdmission,
        dream_admissions: impl IntoIterator<Item = DreamBeginAdmission>,
    ) -> Result<OwnerId, GatewayError> {
        let session = Arc::new(PhySession::open_path(
            path,
            graph_admission,
            dream_admissions,
        )?);
        let owner = OwnerId(session.owner_id());
        self.check_admission(owner, graph_admission)?;
        self.insert(owner, MountedOwner::Phy(session))?;
        Ok(owner)
    }

    pub fn mount_phylactery_current(
        &self,
        path: impl AsRef<Path>,
    ) -> Result<OwnerId, GatewayError> {
        self.install_owner_mount(Self::prepare_owner_mount(
            path,
            OwnerMountMode::ReadOnlyPhy,
            None,
        )?)
    }

    /// Mount a PHY with one owner-local writer lease for authenticated routed
    /// user Memory. General mutation stays inside owner-session.
    pub fn mount_writable_phylactery(
        &self,
        path: impl AsRef<Path>,
        graph_admission: &GraphAdmission,
        dream_admissions: impl IntoIterator<Item = DreamBeginAdmission>,
    ) -> Result<OwnerId, GatewayError> {
        let writer = PhyWriter::open_path(path, graph_admission, dream_admissions)?;
        let owner = OwnerId(writer.owner_id());
        self.check_admission(owner, graph_admission)?;
        self.insert(
            owner,
            MountedOwner::PhyWriter(Arc::new(MountedPhyWriter {
                writer: std::sync::Mutex::new(writer),
            })),
        )?;
        Ok(owner)
    }

    pub fn mount_writable_phylactery_current(
        &self,
        path: impl AsRef<Path>,
    ) -> Result<OwnerId, GatewayError> {
        self.install_owner_mount(Self::prepare_owner_mount(
            path,
            OwnerMountMode::WritablePhy,
            None,
        )?)
    }

    pub fn prepare_owner_mount(
        path: impl AsRef<Path>,
        mode: OwnerMountMode,
        expected: Option<OwnerId>,
    ) -> Result<PreparedOwnerMount, GatewayError> {
        use OwnerMountMode::*;
        let expected = expected.map(|owner| owner.0);
        let (owner, state) = match mode {
            ReadOnlyRel => {
                let session = RelSession::open_path_for_owner(path, expected)?;
                (session.owner_id(), MountedOwner::Rel(Arc::new(session)))
            }
            WritableRel => {
                let writer = RelWriter::open_path_for_owner(path, expected)?;
                (
                    writer.owner_id(),
                    MountedOwner::RelWriter(Arc::new(MountedRelWriter {
                        writer: std::sync::Mutex::new(writer),
                    })),
                )
            }
            ReadOnlyPhy => {
                let session = PhySession::open_path_for_owner(path, expected)?;
                (session.owner_id(), MountedOwner::Phy(Arc::new(session)))
            }
            WritablePhy => {
                let writer = PhyWriter::open_path_for_owner(path, expected)?;
                (
                    writer.owner_id(),
                    MountedOwner::PhyWriter(Arc::new(MountedPhyWriter {
                        writer: std::sync::Mutex::new(writer),
                    })),
                )
            }
        };
        Ok(PreparedOwnerMount {
            owner: OwnerId(owner),
            state,
        })
    }

    pub fn install_owner_mount(
        &self,
        prepared: PreparedOwnerMount,
    ) -> Result<OwnerId, GatewayError> {
        self.insert(prepared.owner, prepared.state)?;
        Ok(prepared.owner)
    }

    /// Remove one mounted owner after host-side quiescence.
    pub fn quiesce_owner(&self, owner: OwnerId) -> Result<(), GatewayError> {
        self.quiesce_mount(owner, false)
    }

    /// Remove a read-only PHY after the trusted host drains its requests.
    pub fn detach_read_only_phylactery(&self, owner: OwnerId) -> Result<(), GatewayError> {
        self.quiesce_mount(owner, true)
    }

    fn quiesce_mount(
        &self,
        owner: OwnerId,
        require_read_only_phy: bool,
    ) -> Result<(), GatewayError> {
        let _topology = self
            .rel_topology
            .write()
            .map_err(|_| GatewayError::RegistryPoisoned)?;
        let leases = self
            .owner_worker_leases
            .lock()
            .map_err(|_| GatewayError::RegistryPoisoned)?;
        if leases.iter().any(|(held, _)| *held == owner) {
            return Err(GatewayError::OwnerSession(
                OwnerSessionError::ManagementConflict,
            ));
        }
        let mut owners = self
            .owners
            .write()
            .map_err(|_| GatewayError::RegistryPoisoned)?;
        if require_read_only_phy {
            match owners
                .get(&owner)
                .ok_or(GatewayError::UnknownOwner(owner))?
            {
                MountedOwner::Phy(_) => {}
                MountedOwner::PhyWriter(_) => {
                    return Err(OwnerSessionError::ManagementConflict.into());
                }
                _ => return Err(GatewayError::WrongOwnerKind(owner)),
            }
        }
        owners
            .remove(&owner)
            .ok_or(GatewayError::UnknownOwner(owner))?;
        Ok(())
    }

    /// Private construction path used only after trusted host authentication.
    #[allow(dead_code)]
    pub(crate) fn instance(
        &self,
        principal: impl Into<String>,
        rel_grants: impl IntoIterator<Item = OwnerId>,
        phy_grants: impl IntoIterator<Item = OwnerId>,
    ) -> Result<Instance, GatewayError> {
        let principal = principal.into();
        validate_principal(&principal)?;
        let rel_grants: BTreeSet<_> = rel_grants.into_iter().collect();
        let phy_grants: BTreeSet<_> = phy_grants.into_iter().collect();
        self.validate_grants(&rel_grants, &phy_grants)?;
        self.instance_with_revocation(principal, rel_grants, phy_grants, None)
    }

    pub(super) fn instance_with_revocation(
        &self,
        principal: String,
        rel_grants: BTreeSet<OwnerId>,
        phy_grants: BTreeSet<OwnerId>,
        credential_active: Option<Arc<AtomicBool>>,
    ) -> Result<Instance, GatewayError> {
        Ok(Instance {
            owners: Arc::clone(&self.owners),
            rel_topology: Arc::clone(&self.rel_topology),
            principal: Arc::from(principal),
            instance_id: Uuid::new_v4(),
            rel_grants: Arc::new(rel_grants),
            phy_grants: Arc::new(phy_grants),
            credential_active,
        })
    }

    pub(super) fn validate_grants(
        &self,
        rel_grants: &BTreeSet<OwnerId>,
        phy_grants: &BTreeSet<OwnerId>,
    ) -> Result<(), GatewayError> {
        let owners = self
            .owners
            .read()
            .map_err(|_| GatewayError::RegistryPoisoned)?;
        for owner in rel_grants {
            match owners.get(owner) {
                None => return Err(GatewayError::UnknownOwner(*owner)),
                Some(mounted) if !mounted.is_rel() => {
                    return Err(GatewayError::WrongOwnerKind(*owner));
                }
                Some(_) => {}
            }
        }
        for owner in phy_grants {
            match owners.get(owner) {
                None => return Err(GatewayError::UnknownOwner(*owner)),
                Some(mounted) if mounted.is_rel() => {
                    return Err(GatewayError::WrongOwnerKind(*owner));
                }
                Some(_) => {}
            }
        }
        Ok(())
    }

    pub fn mounted_owner_count(&self) -> Result<usize, GatewayError> {
        self.owners
            .read()
            .map(|owners| owners.len())
            .map_err(|_| GatewayError::RegistryPoisoned)
    }

    fn check_admission(
        &self,
        owner: OwnerId,
        admission: &GraphAdmission,
    ) -> Result<(), GatewayError> {
        let actual = admission.owner_id().map(OwnerId);
        if actual != Some(owner) {
            return Err(GatewayError::AdmissionOwnerMismatch {
                expected: owner,
                actual,
            });
        }
        Ok(())
    }
    fn insert(&self, owner: OwnerId, mounted: MountedOwner) -> Result<(), GatewayError> {
        let _topology = self
            .rel_topology
            .write()
            .map_err(|_| GatewayError::RegistryPoisoned)?;
        let mut owners = self
            .owners
            .write()
            .map_err(|_| GatewayError::RegistryPoisoned)?;
        if owners.contains_key(&owner) {
            return Err(GatewayError::DuplicateOwner(owner));
        }
        owners.insert(owner, mounted);
        Ok(())
    }
}
