use super::WorkspaceManagement;
use crate::{EntityId, OwnerGrant, OwnerId, ProductError};

pub use reliquary_owner_session::model::identity::{
    IdentityCitation, IdentityEndpointProof, IdentityEntityRevision, IdentityEvidenceRef,
    IdentityLimit, IdentityMemoryRef, IdentityMentionRef, IdentityOutcome, IdentityPair,
    IdentityReason, IdentityReceipt, IdentityRelationshipRef, IdentitySeparationAction,
    IdentitySeparationEvent, IdentitySide, IdentitySpan,
};
pub use reliquary_runtime_gateway::{
    EntityIdentityInspection, IdentityComparisonState, IdentityEffectiveDecision,
    IdentitySeparationInspection, IdentitySeparationPreview, IdentitySeparationRequest,
    IdentitySeparationReview, IdentitySeparationState, PreparedIdentitySeparation,
};

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

    /// List pairs recorded in this REL for a readable peer, including automatic discoveries.
    /// For the next page pass the last pair's (local Entity ID, peer Entity ID).
    pub fn list_entity_identity_pairs(
        &self,
        peer_owner: OwnerId,
        after: Option<(EntityId, EntityId)>,
        limit: usize,
    ) -> Result<Vec<IdentityPair>, ProductError> {
        self.instance.require_rel_read(self.owner)?;
        self.instance.require_rel_read(peer_owner)?;
        Ok(self.instance.gateway.identity.list_pairs(
            &self.instance.inner,
            self.owner.into(),
            peer_owner.into(),
            after,
            limit,
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
        Ok(self.instance.gateway.identity.inspect(
            &self.instance.inner,
            self.identity_pair(local, peer_owner, peer),
            revision,
        )?)
    }

    /// Inspect the latest or a numbered operator decision for this directional pair.
    pub fn inspect_entity_separation(
        &self,
        local: EntityId,
        peer_owner: OwnerId,
        peer: EntityId,
        revision: Option<u64>,
    ) -> Result<Option<IdentitySeparationInspection>, ProductError> {
        self.instance.require_rel_read(self.owner)?;
        self.instance.require_rel_read(peer_owner)?;
        Ok(self.instance.gateway.identity.inspect_separation(
            &self.instance.inner,
            self.identity_pair(local, peer_owner, peer),
            revision,
        )?)
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
