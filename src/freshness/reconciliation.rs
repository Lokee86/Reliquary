//! Native Freshness journal replay writes used by owner reconciliation.

use crate::freshness_storage::{FreshnessStore, SemanticEventEvidence};
use crate::{Container, ContainerError, MemoryId};
use std::collections::BTreeMap;

#[derive(Debug)]
pub(crate) enum ReplayError {
    Freshness(String),
    Container(ContainerError),
}

impl From<ContainerError> for ReplayError {
    fn from(error: ContainerError) -> Self {
        Self::Container(error)
    }
}

pub(crate) fn replay_semantic_event(
    container: &mut Container,
    owner: [u8; 16],
    freshness: &mut FreshnessStore,
    event_id: String,
    evidence: SemanticEventEvidence,
    effects: BTreeMap<MemoryId, i16>,
) -> Result<bool, ReplayError> {
    if event_id != evidence.event_id()
        || evidence.base_policy_digest
            != crate::freshness::semantic_policy::freshness_policy_digest(freshness.policy())
        || evidence.semantic_policy_version
            != crate::freshness::semantic_policy::SemanticPolicy::default().version
        || evidence.semantic_policy_digest
            != crate::freshness::semantic_policy::SemanticPolicy::default()
                .digest()
                .map_err(|error| ReplayError::Freshness(error.to_string()))?
    {
        return Err(ReplayError::Freshness(
            "unsupported or conflicting semantic replay evidence".into(),
        ));
    }
    if freshness
        .contains_event(&event_id)
        .map_err(ReplayError::Freshness)?
    {
        return if freshness
            .semantic_event_matches(&event_id, &evidence, &effects)
            .map_err(ReplayError::Freshness)?
        {
            Ok(false)
        } else {
            Err(ReplayError::Freshness(
                "semantic replay identity conflicts".into(),
            ))
        };
    }
    let payload = freshness
        .encode_semantic_event(owner, evidence, effects)
        .map_err(ReplayError::Freshness)?;
    freshness
        .validate_semantic_preappend(owner, &payload)
        .map_err(ReplayError::Freshness)?;
    freshness
        .persist_policy_if_needed(container, owner)
        .map_err(ReplayError::Freshness)?;
    container.append(&payload)?;
    container.sync()?;
    freshness
        .ingest_appended(&payload)
        .map_err(ReplayError::Freshness)?;
    Ok(true)
}
