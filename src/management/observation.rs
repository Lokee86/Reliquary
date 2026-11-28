use super::WorkspaceManagement;
use crate::{Instance, OwnerId, OwnerKind, ProductError};

pub use reliquary_owner_session::observation::{
    ObservationCitation, ObservationId, ObservationMemoryLineage, ObservationMention,
    ObservationParticipant, ObservationPendingReason, ObservationProfile, ObservationSource,
    ObservationStanding, ObservationStatus, ObservationTimeScope, ObservationValidity,
    ObservationWithdrawalReason,
};
pub use reliquary_runtime_gateway::{
    InspectedObservation, ObservationClaim, ObservationCursor, ObservationPage,
};

impl WorkspaceManagement {
    /// Inspect the current head or a retained numbered Observation revision in this REL.
    pub fn inspect_observation(
        &self,
        id: ObservationId,
        revision: Option<u64>,
    ) -> Result<Option<InspectedObservation>, ProductError> {
        self.instance
            .inspect_observation(self.owner, OwnerKind::Rel, id, revision)
    }

    /// List Observation heads, including pending and withdrawn claims, at one owner cut.
    pub fn list_observations(
        &self,
        cursor: Option<&ObservationCursor<ObservationId>>,
        limit: usize,
    ) -> Result<ObservationPage<ObservationId>, ProductError> {
        self.instance
            .list_observations(self.owner, OwnerKind::Rel, cursor, limit)
    }

    /// Page retained revisions of one Observation in revision order.
    pub fn observation_history(
        &self,
        id: ObservationId,
        cursor: Option<&ObservationCursor<(ObservationId, u64)>>,
        limit: usize,
    ) -> Result<ObservationPage<(ObservationId, u64)>, ProductError> {
        self.instance
            .observation_history(self.owner, OwnerKind::Rel, id, cursor, limit)
    }

    /// Resolve a retained citation attached to the inspected revision if its text is readable.
    pub fn inspect_observation_evidence(
        &self,
        id: ObservationId,
        revision: u64,
        citation: ObservationCitation,
    ) -> Result<Option<String>, ProductError> {
        self.instance.inspect_observation_evidence(
            self.owner,
            OwnerKind::Rel,
            id,
            revision,
            citation,
        )
    }
}

impl Instance {
    /// Inspect the current head or a retained revision in an authorized owner.
    pub fn inspect_observation(
        &self,
        owner: OwnerId,
        kind: OwnerKind,
        id: ObservationId,
        revision: Option<u64>,
    ) -> Result<Option<InspectedObservation>, ProductError> {
        self.require_observation_read(owner, kind)?;
        Ok(self.inner.inspect_observation(owner.into(), id, revision)?)
    }

    /// List Observation heads at one owner cut.
    pub fn list_observations(
        &self,
        owner: OwnerId,
        kind: OwnerKind,
        cursor: Option<&ObservationCursor<ObservationId>>,
        limit: usize,
    ) -> Result<ObservationPage<ObservationId>, ProductError> {
        self.require_observation_read(owner, kind)?;
        Ok(self.inner.list_observations(owner.into(), cursor, limit)?)
    }

    /// Page retained revisions of one Observation in revision order.
    pub fn observation_history(
        &self,
        owner: OwnerId,
        kind: OwnerKind,
        id: ObservationId,
        cursor: Option<&ObservationCursor<(ObservationId, u64)>>,
        limit: usize,
    ) -> Result<ObservationPage<(ObservationId, u64)>, ProductError> {
        self.require_observation_read(owner, kind)?;
        Ok(self
            .inner
            .observation_history(owner.into(), id, cursor, limit)?)
    }

    /// Resolve a readable passage attached to this exact retained revision.
    pub fn inspect_observation_evidence(
        &self,
        owner: OwnerId,
        kind: OwnerKind,
        id: ObservationId,
        revision: u64,
        citation: ObservationCitation,
    ) -> Result<Option<String>, ProductError> {
        self.require_observation_read(owner, kind)?;
        Ok(self
            .inner
            .inspect_observation_evidence(owner.into(), id, revision, citation)?)
    }

    fn require_observation_read(
        &self,
        owner: OwnerId,
        kind: OwnerKind,
    ) -> Result<(), ProductError> {
        match kind {
            OwnerKind::Rel => self.require_rel_read(owner),
            OwnerKind::Phylactery => self.require_phy_read(owner),
        }
    }
}
