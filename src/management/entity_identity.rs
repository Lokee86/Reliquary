use super::WorkspaceManagement;
use crate::{EntityId, OwnerGrant, OwnerId, ProductError};

pub use reliquary_owner_session::model::identity::{
    IdentityCitation, IdentityEntityRevision, IdentityEvidenceRef, IdentityLimit,
    IdentityMemoryRef, IdentityOutcome, IdentityPair, IdentityReason, IdentityReceipt,
    IdentityRelationshipRef, IdentitySide, IdentitySpan,
};
pub use reliquary_runtime_gateway::{EntityIdentityInspection, IdentityComparisonState};

impl WorkspaceManagement {
    /// Compare an explicit pair and publish the evidence receipt in this REL.
    pub fn compare_entity_identity(
        &self,
        local: EntityId,
        peer_owner: OwnerId,
        peer: EntityId,
    ) -> Result<IdentityReceipt, ProductError> {
        self.instance.require(OwnerGrant::ManageRel(self.owner))?;
        self.instance.require_rel_read(peer_owner)?;
        Ok(self.instance.gateway.identity.compare(
            &self.instance.inner,
            self.identity_pair(local, peer_owner, peer),
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

    fn identity_pair(&self, local: EntityId, peer_owner: OwnerId, peer: EntityId) -> IdentityPair {
        IdentityPair {
            local_owner: self.owner.as_bytes(),
            local_entity: local,
            peer_owner: peer_owner.as_bytes(),
            peer_entity: peer,
        }
    }
}
