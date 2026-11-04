use super::*;

impl Instance {
    /// Attempt routed publication through this Instance's exact REL and PHY
    /// grants. The owner-session boundary currently rejects publication until
    /// semantic candidate evidence can be sealed across the application API.
    pub fn publish_user_owned_ingest(
        &self,
        source_rel_owner: OwnerId,
        destination_phy_owner: OwnerId,
        prepared: &PreparedIngest,
    ) -> Result<Vec<GroupedExternalMemoryRef>, GatewayError> {
        self.authorize_routed_owners(source_rel_owner, destination_phy_owner)?;
        if prepared.owner_id() != source_rel_owner.as_bytes() {
            return Err(GatewayError::RoutedMemory(
                "prepared ingest source owner differs from its REL grant".into(),
            ));
        }
        let (source, destination) = {
            let owners = self
                .owners
                .read()
                .map_err(|_| GatewayError::RegistryPoisoned)?;
            let source = match owners.get(&source_rel_owner) {
                Some(MountedOwner::Rel(session)) => RoutedSource::Snapshot(Arc::clone(session)),
                Some(MountedOwner::RelWriter(writer)) => RoutedSource::Writer(Arc::clone(writer)),
                Some(_) => return Err(GatewayError::WrongOwnerKind(source_rel_owner)),
                None => return Err(GatewayError::UnknownOwner(source_rel_owner)),
            };
            let destination = match owners.get(&destination_phy_owner) {
                Some(MountedOwner::PhyWriter(writer)) => Arc::clone(writer),
                Some(MountedOwner::Phy(_)) => return Err(GatewayError::WriterUnavailable),
                Some(_) => return Err(GatewayError::WrongOwnerKind(destination_phy_owner)),
                None => return Err(GatewayError::UnknownOwner(destination_phy_owner)),
            };
            (source, destination)
        };

        let mut destination = destination
            .writer
            .lock()
            .map_err(|_| GatewayError::RegistryPoisoned)?;
        let proofs = match source {
            RoutedSource::Snapshot(session) => {
                route_user_proofs(&session, &destination, prepared, &self.principal)?
            }
            RoutedSource::Writer(writer) => {
                let writer = writer
                    .writer
                    .lock()
                    .map_err(|_| GatewayError::RegistryPoisoned)?;
                route_user_proofs(
                    writer.read_session(),
                    &destination,
                    prepared,
                    &self.principal,
                )?
            }
        };
        self.authorize_routed_owners(source_rel_owner, destination_phy_owner)?;
        let destination_id = hex(&destination_phy_owner.0);
        let mut refs = Vec::with_capacity(proofs.len());
        let mut seen = BTreeSet::new();
        for proof in proofs {
            self.authorize_routed_owners(source_rel_owner, destination_phy_owner)?;
            let revision = destination.publish_routed_user_memory(proof)?;
            let id = revision.record.id.0;
            if !seen.insert(id) {
                return Err(GatewayError::RoutedMemory(
                    "duplicate routed Memory identity".into(),
                ));
            }
            let source = revision.record.source_ref.as_ref().ok_or_else(|| {
                GatewayError::RoutedMemory("PHY revision lost source proof".into())
            })?;
            if source.owner_id != hex(&source_rel_owner.0)
                || source.principal_id.as_deref() != Some(self.principal.as_ref())
            {
                return Err(GatewayError::RoutedMemory(
                    "published PHY revision differs from the active source grant".into(),
                ));
            }
            refs.push(GroupedExternalMemoryRef {
                owner_id: destination_id.clone(),
                memory_id: id,
            });
        }
        Ok(refs)
    }

    fn authorize_routed_owners(
        &self,
        source_rel_owner: OwnerId,
        destination_phy_owner: OwnerId,
    ) -> Result<(), GatewayError> {
        self.ensure_credential_active()?;
        if !self.rel_grants.contains(&source_rel_owner) {
            return Err(GatewayError::UnauthorizedRel {
                principal: self.principal.to_string(),
                owner: source_rel_owner,
            });
        }
        if !self.phy_grants.contains(&destination_phy_owner) {
            return Err(GatewayError::UnauthorizedPhy {
                principal: self.principal.to_string(),
                owner: destination_phy_owner,
            });
        }
        Ok(())
    }
}
