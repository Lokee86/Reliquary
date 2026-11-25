use super::super::{OwnerWork, RETRY_INTERVALS};
use crate::GatewayError;
use reliquary_application::observation_drafts::{
    MAX_CLAIM_VECTOR_BATCH_BYTES, evaluate_claim_vectors, select_claim_vector_profile,
};
use reliquary_application::{SwitchboardEmbeddingEndpoint, prepare_compatibility_profile};
use reliquary_owner_session::model::{CompatibilityProfile, CompatibilityProfileId};
use reliquary_owner_session::observation::{
    MAX_OBSERVATION_CLAIM_VECTOR_ROWS, ObservationId, ObservationStanding,
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

impl ClaimVectorIndex {
    pub(super) fn tick(&mut self, owner: &OwnerWork<'_>) -> bool {
        if self.retry_at.is_some_and(|retry| Instant::now() < retry) {
            return false;
        }
        match self.advance(owner) {
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

    fn advance(&mut self, owner: &OwnerWork<'_>) -> Result<bool, GatewayError> {
        if !owner.routes.embedding.supports_bounded_embedding() {
            return Ok(false);
        }
        let (cut, profiles) = owner.read(|rel| {
            let cut = rel.perception_source_cut();
            let profiles = if cut.observation == 0 {
                Vec::new()
            } else {
                rel.observation_claim_index_profiles()
            };
            (
                (cut.memory, cut.graph, cut.entity, cut.observation),
                profiles,
            )
        })?;
        if profiles.is_empty() {
            self.scan = None;
            return Ok(false);
        }
        let endpoint = SwitchboardEmbeddingEndpoint::new(owner.routes.embedding.clone());
        if self.observed.is_none() {
            self.observed = Some(prepare_compatibility_profile(&endpoint).map_err(|_| {
                GatewayError::RetrievalUnavailable(
                    "Observation claim embedding route is unavailable",
                )
            })?);
        }
        let Some(profile) = self
            .observed
            .as_ref()
            .and_then(|observed| select_claim_vector_profile(&profiles, observed))
        else {
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
        let (after, exhausted, sources) = owner.read(|rel| {
            let page = rel.observations_after(scan.after, limit);
            let after = page.last().map(|row| row.revision.id);
            let mut sources = Vec::new();
            for head in &page {
                if head.standing == ObservationStanding::Current
                    && rel
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
        })??;
        if !sources.is_empty() {
            let rows = evaluate_claim_vectors(profile, sources, &endpoint).map_err(|_| {
                GatewayError::RetrievalUnavailable("Observation claim embedding failed")
            })?;
            owner.with_writer(|writer| {
                writer.publish_observation_claim_vectors(profile.id, rows)?;
                Ok(())
            })?;
        }
        scan.after = after;
        scan.exhausted = exhausted;
        Ok(after.is_some())
    }
}
