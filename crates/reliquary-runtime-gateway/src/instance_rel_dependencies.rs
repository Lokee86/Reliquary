use super::*;
use reliquary_owner_session::{RelDependencyDeclaration, canonical_rel_dependency_ids};

impl Instance {
    pub fn rel_dependencies(
        &self,
        owner: OwnerId,
    ) -> Result<RelDependencyDeclaration, GatewayError> {
        let result = self.with_reliquary(owner, |session| session.rel_dependencies().clone())?;
        self.ensure_credential_active()?;
        Ok(result)
    }

    pub fn replace_rel_dependencies(
        &self,
        owner: OwnerId,
        expected_generation: u64,
        dependency_ids: Vec<[u8; 16]>,
    ) -> Result<(RelDependencyDeclaration, bool), GatewayError> {
        self.authorize_rel_access(owner)?;
        let dependency_ids = canonical_rel_dependency_ids(owner.0, dependency_ids)?;
        let _topology = self
            .rel_topology
            .write()
            .map_err(|_| GatewayError::RegistryPoisoned)?;
        let current = self.with_rel_writer(owner, |writer| {
            writer.read_session().rel_dependencies().clone()
        })?;
        if current.generation != expected_generation {
            return Err(OwnerSessionError::ManagementStale.into());
        }
        if current.owner_ids != dependency_ids {
            for &id in &dependency_ids {
                self.authorize_rel_access(OwnerId(id))?;
            }
            dependency_order(owner, |id| {
                if id == owner {
                    Ok(Some(dependency_ids.clone()))
                } else {
                    self.mounted_rel_dependency_ids(id)
                        .map(Some)
                        .map_err(dependency_mount_error)
                }
            })?;
        }
        self.with_rel_writer(owner, |writer| {
            writer.replace_rel_dependencies(expected_generation, dependency_ids)
        })?
        .map_err(Into::into)
    }

    pub fn resolve_rel_dependencies(&self, owner: OwnerId) -> Result<Vec<OwnerId>, GatewayError> {
        self.with_rel_dependency_order(owner, Ok)
    }

    /// Read the authorized graph and use its order before allowing a topology change.
    pub(super) fn with_rel_dependency_order<T>(
        &self,
        owner: OwnerId,
        read: impl FnOnce(Vec<OwnerId>) -> Result<T, GatewayError>,
    ) -> Result<T, GatewayError> {
        self.authorize_rel_access(owner)?;
        let _topology = self
            .rel_topology
            .read()
            .map_err(|_| GatewayError::RegistryPoisoned)?;
        let order = dependency_order(owner, |id| {
            if !self.rel_grants.contains(&id) {
                return Ok(None);
            }
            self.with_reliquary(id, RelSession::rel_dependency_ids)
                .map_err(dependency_mount_error)?
                .map(Some)
                .map_err(Into::into)
        })?;
        let value = read(order)?;
        self.ensure_credential_active()?;
        Ok(value)
    }
}

fn dependency_mount_error(error: GatewayError) -> GatewayError {
    match error {
        GatewayError::UnknownOwner(_) | GatewayError::WrongOwnerKind(_) => {
            GatewayError::RelDependencyUnresolved
        }
        error => error,
    }
}

fn dependency_order(
    root: OwnerId,
    mut read: impl FnMut(OwnerId) -> Result<Option<Vec<[u8; 16]>>, GatewayError>,
) -> Result<Vec<OwnerId>, GatewayError> {
    enum Visit {
        Enter(OwnerId),
        Leave(OwnerId),
    }
    let mut pending = vec![Visit::Enter(root)];
    let mut active = BTreeSet::new();
    let mut complete = BTreeSet::new();
    let mut order = Vec::new();
    while let Some(visit) = pending.pop() {
        match visit {
            Visit::Enter(owner) => {
                if complete.contains(&owner) {
                    continue;
                }
                if active.contains(&owner) {
                    return Err(GatewayError::RelDependencyCycle);
                }
                let Some(children) = read(owner)? else {
                    complete.insert(owner);
                    continue;
                };
                active.insert(owner);
                pending.push(Visit::Leave(owner));
                pending.extend(
                    children
                        .into_iter()
                        .rev()
                        .map(|id| Visit::Enter(OwnerId(id))),
                );
            }
            Visit::Leave(owner) => {
                active.remove(&owner);
                complete.insert(owner);
                order.push(owner);
            }
        }
    }
    Ok(order)
}
