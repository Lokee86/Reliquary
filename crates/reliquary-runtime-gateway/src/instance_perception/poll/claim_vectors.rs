use super::super::{OwnerWork, PerceptionRoutes, RETRY_INTERVALS, phy::PhyWork};
use crate::{GatewayError, PhyWriter, RelSession};
use reliquary_application::observation_drafts::{
    MAX_CLAIM_VECTOR_BATCH_BYTES, evaluate_claim_vectors, select_claim_vector_profile,
};
use reliquary_application::{SwitchboardEmbeddingEndpoint, prepare_compatibility_profile};
use reliquary_owner_session::model::{CompatibilityProfile, CompatibilityProfileId};
use reliquary_owner_session::observation::{
    MAX_OBSERVATION_CLAIM_VECTOR_ROWS, ObservationClaimVector, ObservationId, ObservationStanding,
};
use std::time::Instant;

/// Each poll advances at most one page and leaves verification and review on their own path.
#[derive(Default)]
pub(crate) struct ClaimVectorIndex {
    observed: Option<CompatibilityProfile>,
    scan: Option<ClaimVectorScan>,
    retry_at: Option<Instant>,
}

struct ClaimVectorScan {
    profile: CompatibilityProfileId,
    cut: (u64, u64, u64, u64),
    after: Option<ObservationId>,
    exhausted: bool,
}

enum Owner<'a> {
    Rel(&'a OwnerWork<'a>),
    Phy(&'a PhyWork<'a>, &'a PerceptionRoutes),
}

enum Read<'a> {
    Rel(&'a RelSession),
    Phy(&'a PhyWriter),
}

impl Owner<'_> {
    fn routes(&self) -> &PerceptionRoutes {
        match self {
            Self::Rel(owner) => owner.routes,
            Self::Phy(_, routes) => routes,
        }
    }

    fn read<T>(&self, action: impl FnOnce(Read<'_>) -> T) -> Result<T, GatewayError> {
        match self {
            Self::Rel(owner) => owner.read(|rel| action(Read::Rel(rel))),
            Self::Phy(owner, _) => owner.read(|phy| action(Read::Phy(phy))),
        }
    }

    fn publish(
        &self,
        profile: CompatibilityProfileId,
        rows: Vec<ObservationClaimVector>,
    ) -> Result<(), GatewayError> {
        match self {
            Self::Rel(owner) => owner
                .with_writer(|writer| Ok(writer.publish_observation_claim_vectors(profile, rows)?)),
            Self::Phy(owner, _) => owner
                .with_writer(|writer| Ok(writer.publish_observation_claim_vectors(profile, rows)?)),
        }
    }
}

macro_rules! read_owner {
    ($owner:expr, |$read:ident| $body:expr) => {
        match $owner {
            Read::Rel($read) => $body,
            Read::Phy($read) => $body,
        }
    };
}

impl ClaimVectorIndex {
    pub(super) fn tick(&mut self, owner: &OwnerWork<'_>) -> bool {
        self.tick_owner(Owner::Rel(owner))
    }

    pub(in crate::instance_perception) fn tick_phy(
        &mut self,
        owner: &PhyWork<'_>,
        routes: &PerceptionRoutes,
    ) -> bool {
        self.tick_owner(Owner::Phy(owner, routes))
    }

    fn tick_owner(&mut self, owner: Owner<'_>) -> bool {
        if self.retry_at.is_some_and(|retry| Instant::now() < retry) {
            return false;
        }
        match self.advance(&owner) {
            Ok(changed) => {
                self.retry_at = None;
                changed
            }
            Err(_) => {
                self.retry_at = Some(Instant::now() + RETRY_INTERVALS[1]);
                false
            }
        }
    }

    pub(in crate::instance_perception) fn phy_memory_profile(
        &mut self,
        owner: &PhyWork<'_>,
        routes: &PerceptionRoutes,
    ) -> Result<Option<CompatibilityProfileId>, GatewayError> {
        if !routes.embedding.supports_bounded_embedding() {
            return Ok(None);
        }
        let profiles = owner.read(|phy| phy.observation_claim_index_profiles())?;
        if profiles.is_empty() {
            return Ok(None);
        }
        let endpoint = SwitchboardEmbeddingEndpoint::new(routes.embedding.clone());
        // Lexical and structural candidate routing can continue while vectors are unavailable.
        let observed = self.profile(&endpoint).ok();
        Ok(observed
            .and_then(|observed| select_claim_vector_profile(&profiles, observed))
            .map(|profile| profile.id))
    }

    fn profile(
        &mut self,
        endpoint: &SwitchboardEmbeddingEndpoint,
    ) -> Result<&CompatibilityProfile, GatewayError> {
        if self.observed.is_none() {
            self.observed = Some(prepare_compatibility_profile(endpoint).map_err(|_| {
                GatewayError::RetrievalUnavailable(
                    "Observation claim embedding route is unavailable",
                )
            })?);
        }
        self.observed
            .as_ref()
            .ok_or(GatewayError::RetrievalUnavailable(
                "Observation claim embedding route is unavailable",
            ))
    }

    fn advance(&mut self, owner: &Owner<'_>) -> Result<bool, GatewayError> {
        if !owner.routes().embedding.supports_bounded_embedding() {
            return Ok(false);
        }
        let (cut, profiles) = owner.read(|read| {
            read_owner!(read, |source| {
                let cut = source.perception_source_cut();
                let profiles = if cut.observation == 0 {
                    Vec::new()
                } else {
                    source.observation_claim_index_profiles()
                };
                (
                    (cut.memory, cut.graph, cut.entity, cut.observation),
                    profiles,
                )
            })
        })?;
        if profiles.is_empty() {
            self.scan = None;
            return Ok(false);
        }
        let endpoint = SwitchboardEmbeddingEndpoint::new(owner.routes().embedding.clone());
        let Some(profile) = select_claim_vector_profile(&profiles, self.profile(&endpoint)?) else {
            self.scan = None;
            return Ok(false);
        };
        if self
            .scan
            .as_ref()
            .is_none_or(|scan| scan.profile != profile.id || (scan.exhausted && scan.cut != cut))
        {
            self.scan = Some(ClaimVectorScan {
                profile: profile.id,
                cut,
                after: None,
                exhausted: false,
            });
        }
        let Some(scan) = self.scan.as_mut().filter(|scan| !scan.exhausted) else {
            return Ok(false);
        };
        let row_bytes = profile.dimensions as usize * std::mem::size_of::<f32>();
        let limit = MAX_OBSERVATION_CLAIM_VECTOR_ROWS.min(MAX_CLAIM_VECTOR_BATCH_BYTES / row_bytes);
        if limit == 0 {
            scan.exhausted = true;
            return Ok(false);
        }
        let (after, exhausted, sources) = owner.read(|read| {
            read_owner!(read, |owner| {
                let page = owner.observations_after(scan.after, limit);
                let after = page.last().map(|row| row.revision.id);
                let mut sources = Vec::new();
                for head in &page {
                    if head.standing == ObservationStanding::Current
                        && owner
                            .observation_claim_vector(profile.id, head.revision)?
                            .is_none()
                    {
                        sources.push((
                            head.revision.full_citation(),
                            head.revision.draft.proposition.clone(),
                        ));
                    }
                }
                Ok::<_, reliquary_owner_session::OwnerSessionError>((
                    after,
                    page.len() < limit,
                    sources,
                ))
            })
        })??;
        if !sources.is_empty() {
            let rows = evaluate_claim_vectors(profile, sources, &endpoint).map_err(|_| {
                GatewayError::RetrievalUnavailable("Observation claim embedding failed")
            })?;
            owner.publish(profile.id, rows)?;
        }
        scan.after = after;
        scan.exhausted = exhausted;
        Ok(after.is_some())
    }
}
