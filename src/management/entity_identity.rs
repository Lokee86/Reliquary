use super::WorkspaceManagement;
use crate::{ConfiguredRuntime, EntityId, OwnerGrant, OwnerId, ProductError};
use reliquary_runtime_gateway::OwnerWorkerCoordinator;

pub use reliquary_owner_session::model::identity::{
    IdentityCitation, IdentityEntityRevision, IdentityEvidenceRef, IdentityLimit,
    IdentityMemoryRef, IdentityOutcome, IdentityPair, IdentityReason, IdentityReceipt,
    IdentityRelationshipRef, IdentitySide, IdentitySpan,
};
pub use reliquary_runtime_gateway::{EntityIdentityInspection, IdentityComparisonState};

pub struct EntityIdentityProcessing {
    coordinator: OwnerWorkerCoordinator,
}

impl EntityIdentityProcessing {
    /// Stop accepting work and join any evaluation already running.
    pub fn stop(&mut self) -> Result<(), ProductError> {
        self.coordinator.stop().map_err(Into::into)
    }
}

impl WorkspaceManagement {
    /// Compare an explicit pair and publish the evidence receipt in this REL.
    pub fn compare_entity_identity(
        &self,
        local: EntityId,
        peer_owner: OwnerId,
        peer: EntityId,
        runtime: &ConfiguredRuntime,
    ) -> Result<IdentityReceipt, ProductError> {
        self.instance.require(OwnerGrant::ManageRel(self.owner))?;
        self.instance.require_rel_read(peer_owner)?;
        let endpoint = runtime.entity_endpoint();
        Ok(self.instance.inner.compare_entity_identity(
            self.identity_pair(local, peer_owner, peer),
            endpoint.as_deref(),
            &identity_route(runtime),
        )?)
    }

    /// Read a receipt and check whether its evidence still supports a current judgment.
    pub fn inspect_entity_identity(
        &self,
        local: EntityId,
        peer_owner: OwnerId,
        peer: EntityId,
        revision: Option<u64>,
    ) -> Result<Option<EntityIdentityInspection>, ProductError> {
        self.instance.require_rel_read(self.owner)?;
        self.instance.require_rel_read(peer_owner)?;
        Ok(self
            .instance
            .inner
            .inspect_entity_identity(self.identity_pair(local, peer_owner, peer), revision)?)
    }

    /// Recheck previously compared pairs with the active Instance's independent peer grants.
    pub fn start_entity_identity_processing(
        &self,
        runtime: &ConfiguredRuntime,
    ) -> Result<EntityIdentityProcessing, ProductError> {
        self.instance.require(OwnerGrant::ManageRel(self.owner))?;
        let coordinator = self
            .instance
            .gateway
            .inner
            .start_entity_identity_processing(
                self.instance.inner.clone(),
                self.owner.into(),
                runtime.entity_endpoint(),
                identity_route(runtime),
            )?;
        Ok(EntityIdentityProcessing { coordinator })
    }

    fn identity_pair(&self, local: EntityId, peer_owner: OwnerId, peer: EntityId) -> IdentityPair {
        IdentityPair {
            local_owner: self.owner.as_bytes(),
            local_entity: local,
            peer_owner: peer_owner.as_bytes(),
            peer_entity: peer,
        }
    }
}

fn identity_route(runtime: &ConfiguredRuntime) -> String {
    runtime
        .processing_route_identity()
        .entity_identity
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
