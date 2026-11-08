use super::*;
use reliquary_application::{FreshnessTransition, SwitchboardEmbeddingEndpoint};
use reliquary_owner_session::model::{CompatibilityProfileId, MemoryId, PayloadState};
use serde::Serialize;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct VectorCoverage {
    pub profile_id: String,
    pub memory_version: u64,
    pub eligible: usize,
    pub covered: usize,
    pub pending: usize,
}

impl Instance {
    pub fn rel_vector_coverage(
        &self,
        owner: OwnerId,
        profile: CompatibilityProfileId,
    ) -> Result<VectorCoverage, GatewayError> {
        self.vector_work(owner, profile, 0)
            .map(|(coverage, _)| coverage)
    }

    pub fn build_rel_vectors(
        &self,
        owner: OwnerId,
        endpoint: Arc<dyn reliquary_switchboard::EmbeddingEndpoint>,
        limit: usize,
    ) -> Result<VectorCoverage, GatewayError> {
        if !(1..=1024).contains(&limit) {
            return Err(GatewayError::HostAdapter(
                "vector work bound must be 1 through 1024".into(),
            ));
        }
        let endpoint = SwitchboardEmbeddingEndpoint::new(endpoint);
        let profile = self.publish_embedding_profile(owner, &endpoint)?;
        let (_, missing) = self.vector_work(owner, profile, limit)?;
        for batch in missing.chunks(32) {
            let prepared = self.prepare_owner_memory_vectors(owner, profile, batch, &endpoint)?;
            self.publish_owner_memory_vectors(&prepared)?;
        }
        self.rel_vector_coverage(owner, profile)
    }

    fn vector_work(
        &self,
        owner: OwnerId,
        profile: CompatibilityProfileId,
        limit: usize,
    ) -> Result<(VectorCoverage, Vec<MemoryId>), GatewayError> {
        self.with_reliquary(owner, |rel| {
            if rel.active_compatibility_profile(profile).is_none() {
                return Err(GatewayError::Retrieval(
                    "embedding profile is not active".into(),
                ));
            }
            let snapshot = rel.retrieval_snapshot(Some(profile))?;
            let mut eligible = 0;
            let mut covered = 0;
            let mut missing = Vec::new();
            for item in snapshot.memories {
                if item.revision.record.archived
                    || !matches!(item.revision.payload, PayloadState::Available(_))
                {
                    continue;
                }
                eligible += 1;
                if snapshot.vectors.contains_key(&item.revision.record.body_id) {
                    covered += 1;
                } else if missing.len() < limit {
                    missing.push(item.revision.record.id);
                }
            }
            Ok((
                VectorCoverage {
                    profile_id: crate::hex(&profile.0),
                    memory_version: snapshot.memory_version,
                    eligible,
                    covered,
                    pending: eligible - covered,
                },
                missing,
            ))
        })?
    }

    pub fn rel_freshness_due(
        &self,
        owner: OwnerId,
        limit: usize,
    ) -> Result<Vec<FreshnessTransition>, GatewayError> {
        self.due_freshness_transitions(owner, limit)
    }
}
