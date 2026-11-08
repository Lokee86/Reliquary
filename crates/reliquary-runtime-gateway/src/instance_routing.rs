use super::*;
use reliquary_application::{
    commit_prepared_native_import_with_routed_receipts,
    commit_prepared_native_ingest_with_routed_receipts, prepare_routed_user_proofs,
};

impl Instance {
    /// Publish to the explicitly granted PHY before recording the REL receipt.
    /// Both mounted writers stay stable through authorization and publication.
    pub(crate) fn commit_routed_insomnia_claim(
        &self,
        owner: OwnerId,
        destination: OwnerId,
        claim: &RelInsomniaClaimInput,
        prepared: &PreparedIngest,
        model: &str,
        version: &str,
        now_ns: i64,
    ) -> Result<CompletionCheck, GatewayError> {
        self.commit_scoped_routed_insomnia_claim(
            owner,
            destination,
            claim,
            prepared,
            model,
            version,
            || now_ns,
            None,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn commit_scoped_routed_insomnia_claim(
        &self,
        owner: OwnerId,
        destination: OwnerId,
        claim: &RelInsomniaClaimInput,
        prepared: &PreparedIngest,
        model: &str,
        version: &str,
        now: impl Fn() -> i64,
        import_id: Option<&str>,
    ) -> Result<CompletionCheck, GatewayError> {
        self.authorize_routed_owners(owner, destination)?;
        if owner == destination
            || claim.owner_id != owner.as_bytes()
            || prepared.owner_id() != owner.as_bytes()
            || claim.episode.id != prepared.episode_id()
        {
            return Err(GatewayError::RoutedMemory(
                "routed claim identity mismatch".into(),
            ));
        }
        let (rel, phy) = {
            let owners = self
                .owners
                .read()
                .map_err(|_| GatewayError::RegistryPoisoned)?;
            let rel = match owners.get(&owner) {
                Some(MountedOwner::RelWriter(writer)) => Arc::clone(writer),
                Some(MountedOwner::Rel(_)) => return Err(GatewayError::WriterUnavailable),
                Some(_) => return Err(GatewayError::WrongOwnerKind(owner)),
                None => return Err(GatewayError::UnknownOwner(owner)),
            };
            let phy = match owners.get(&destination) {
                Some(MountedOwner::PhyWriter(writer)) => Arc::clone(writer),
                Some(MountedOwner::Phy(_)) => return Err(GatewayError::WriterUnavailable),
                Some(_) => return Err(GatewayError::WrongOwnerKind(destination)),
                None => return Err(GatewayError::UnknownOwner(destination)),
            };
            (rel, phy)
        };
        let mut rel = rel
            .writer
            .lock()
            .map_err(|_| GatewayError::RegistryPoisoned)?;
        let mut phy = phy
            .writer
            .lock()
            .map_err(|_| GatewayError::RegistryPoisoned)?;
        let recheck = |rel: &RelWriter| -> Result<i64, GatewayError> {
            self.authorize_routed_owners(owner, destination)?;
            let timestamp = now();
            match import_id {
                Some(id) => rel.validate_native_import_claim(id, claim, timestamp)?,
                None => rel.validate_insomnia_claim(claim, timestamp)?,
            }
            Ok(timestamp)
        };
        recheck(&rel)?;
        let proofs =
            prepare_routed_user_proofs(rel.read_session(), &phy, prepared, &self.principal)
                .map_err(|error| GatewayError::RoutedMemory(error.to_string()))?;
        let mut receipts = Vec::with_capacity(proofs.len());
        for proof in proofs {
            recheck(&rel)?;
            let (_, receipt) = phy.publish_routed_user_memory(proof)?;
            receipts.push(receipt);
        }
        let now_ns = recheck(&rel)?;
        match import_id {
            Some(id) => commit_prepared_native_import_with_routed_receipts(
                &mut rel,
                id,
                prepared,
                &claim.work,
                model,
                version,
                now_ns,
                &receipts,
            ),
            None => commit_prepared_native_ingest_with_routed_receipts(
                &mut rel,
                prepared,
                &claim.work,
                model,
                version,
                now_ns,
                &receipts,
            ),
        }
        .map_err(|error| GatewayError::RoutedMemory(error.to_string()))
    }

    pub(crate) fn authorize_routed_owners(
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
