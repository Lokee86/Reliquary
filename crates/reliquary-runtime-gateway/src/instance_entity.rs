use super::*;
use reliquary_application::{
    EntityMemoryMention, EntityOwnerSession, EntityResolutionProposal, EntityWorkflowError,
    prepare_memory_mention, prepare_phy_writer_mention,
};
use reliquary_owner_session::model::MemoryEntityResolution;
use reliquary_switchboard::GeneralEndpoint;

impl Instance {
    /// Capture owner-local evidence, release the owner lock for inference,
    /// then reauthorize and publish the outcome against the current owner.
    pub fn resolve_and_publish_entity_mention<E: GeneralEndpoint>(
        &self,
        input: EntityMemoryMention,
        endpoint: E,
        updated_at_ns: i64,
    ) -> Result<MemoryEntityResolution, GatewayError> {
        self.ensure_credential_active()?;
        let owner = OwnerId::from_bytes(input.owner_id);
        let prepared = if self.rel_grants.contains(&owner) {
            self.with_writable_reliquary(owner, |session| {
                prepare_memory_mention(EntityOwnerSession::Rel(session), input)
            })?
        } else {
            self.with_entity_phy_writer(owner, |writer| prepare_phy_writer_mention(writer, input))?
        }
        .map_err(mention_error)?;
        if let Some(existing) = prepared.existing_resolution() {
            return Ok(existing.clone());
        }
        let proposal = prepared
            .evaluate_with_endpoint(endpoint)
            .map_err(mention_error)?;
        self.publish_entity_mention(proposal, updated_at_ns)
    }

    /// Publish an evaluated mention through this Instance's grant to the same
    /// owner. Owner-session rechecks the current body, span and target revisions.
    pub fn publish_entity_mention(
        &self,
        proposal: EntityResolutionProposal,
        updated_at_ns: i64,
    ) -> Result<MemoryEntityResolution, GatewayError> {
        self.ensure_credential_active()?;
        let command = proposal
            .into_mention_commit(updated_at_ns)
            .map_err(mention_error)?;
        let owner = OwnerId::from_bytes(command.owner_id);
        if self.rel_grants.contains(&owner) {
            return Ok(
                self.with_rel_writer(owner, |writer| writer.commit_entity_mention(command))??
            );
        }
        Ok(self.with_entity_phy_writer(owner, |writer| writer.commit_entity_mention(command))??)
    }

    pub(super) fn with_entity_phy_writer<T>(
        &self,
        owner: OwnerId,
        operation: impl FnOnce(&mut PhyWriter) -> T,
    ) -> Result<T, GatewayError> {
        self.ensure_credential_active()?;
        if !self.phy_grants.contains(&owner) {
            return Err(GatewayError::UnauthorizedPhy {
                principal: self.principal.to_string(),
                owner,
            });
        }
        let mount = {
            let owners = self
                .owners
                .read()
                .map_err(|_| GatewayError::RegistryPoisoned)?;
            match owners.get(&owner) {
                Some(MountedOwner::PhyWriter(mount)) => Arc::clone(mount),
                Some(MountedOwner::Phy(_)) => return Err(GatewayError::WriterUnavailable),
                Some(_) => return Err(GatewayError::WrongOwnerKind(owner)),
                None => return Err(GatewayError::UnknownOwner(owner)),
            }
        };
        let mut writer = mount
            .writer
            .lock()
            .map_err(|_| GatewayError::RegistryPoisoned)?;
        self.ensure_credential_active()?;
        Ok(operation(&mut writer))
    }
}

fn mention_error(error: EntityWorkflowError) -> GatewayError {
    OwnerSessionError::Record(error.to_string()).into()
}
