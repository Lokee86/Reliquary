use super::WorkspaceManagement;
use crate::ProductError;

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
        self.instance.require_rel_read(self.owner)?;
        Ok(self
            .instance
            .inner
            .inspect_observation(self.owner.into(), id, revision)?)
    }

    /// List Observation heads, including pending and withdrawn claims, at one owner cut.
    pub fn list_observations(
        &self,
        cursor: Option<&ObservationCursor<ObservationId>>,
        limit: usize,
    ) -> Result<ObservationPage<ObservationId>, ProductError> {
        self.instance.require_rel_read(self.owner)?;
        Ok(self
            .instance
            .inner
            .list_observations(self.owner.into(), cursor, limit)?)
    }

    /// Page retained revisions of one Observation in revision order.
    pub fn observation_history(
        &self,
        id: ObservationId,
        cursor: Option<&ObservationCursor<(ObservationId, u64)>>,
        limit: usize,
    ) -> Result<ObservationPage<(ObservationId, u64)>, ProductError> {
        self.instance.require_rel_read(self.owner)?;
        Ok(self
            .instance
            .inner
            .observation_history(self.owner.into(), id, cursor, limit)?)
    }

    /// Resolve a retained citation attached to the inspected revision if its text is readable.
    pub fn inspect_observation_evidence(
        &self,
        id: ObservationId,
        revision: u64,
        citation: ObservationCitation,
    ) -> Result<Option<String>, ProductError> {
        self.instance.require_rel_read(self.owner)?;
        Ok(self.instance.inner.inspect_observation_evidence(
            self.owner.into(),
            id,
            revision,
            citation,
        )?)
    }
}
