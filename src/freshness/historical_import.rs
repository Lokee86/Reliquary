//! Experimental composition of caller-supplied historical planning stages.
//!
//! This in-memory model neither validates the canonical H1 manifest/Archive
//! owner proofs nor commits the durable Cva Freshness checkpoint. Importer and
//! runtime integration remain unimplemented; callers must not treat its result
//! as accepted REL authority or an authorization decision.
use super::historical_checkpoint::{
    HistoricalCheckpoint, HistoricalCheckpointCommit, HistoricalCheckpointError,
    HistoricalCheckpointStore,
};
use super::historical_dream::HistoricalDreamPlan;
use super::historical_planner::{HistoricalProvenancePlan, HistoricalTimeline, MemoryAvailability};
use super::historical_propagation::{
    HistoricalPropagationError, HistoricalPropagationEvent, propagate_historical_events,
};
use super::historical_scorer::{
    HistoricalScoreError, HistoricalScoreEvent, score_historical_records,
};
use super::historical_semantic::HistoricalSemanticPlan;
use super::{
    CommunityMembership, FreshnessError, FreshnessPolicy, FreshnessRecord, MemoryAdjacency,
};
use crate::MemoryId;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug)]
pub struct HistoricalImportInput {
    pub import_id: [u8; 32],
    pub manifest_digest: [u8; 32],
    pub as_of_position: u64,
    pub timeline: HistoricalTimeline,
    pub provenance: HistoricalProvenancePlan,
    pub semantic_plan: HistoricalSemanticPlan,
    pub dream_plan: HistoricalDreamPlan,
    pub initial_records: BTreeMap<MemoryId, FreshnessRecord>,
    pub adjacency: MemoryAdjacency,
    pub memberships: CommunityMembership,
    pub authorized: BTreeSet<MemoryId>,
    pub availability: BTreeMap<MemoryId, MemoryAvailability>,
    pub workers: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HistoricalImportResult {
    pub checkpoint: HistoricalCheckpoint,
    pub semantic_events: usize,
    pub dream_events: usize,
    pub propagated_events: usize,
    pub commit: HistoricalCheckpointCommit,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HistoricalImportError {
    InvalidInput,
    Propagation(HistoricalPropagationError),
    Scoring(HistoricalScoreError),
    Checkpoint(HistoricalCheckpointError),
    Freshness(FreshnessError),
}
impl std::fmt::Display for HistoricalImportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "historical import: {self:?}")
    }
}
impl std::error::Error for HistoricalImportError {}
impl From<HistoricalPropagationError> for HistoricalImportError {
    fn from(error: HistoricalPropagationError) -> Self {
        Self::Propagation(error)
    }
}
impl From<HistoricalScoreError> for HistoricalImportError {
    fn from(error: HistoricalScoreError) -> Self {
        Self::Scoring(error)
    }
}
impl From<HistoricalCheckpointError> for HistoricalImportError {
    fn from(error: HistoricalCheckpointError) -> Self {
        Self::Checkpoint(error)
    }
}
impl From<FreshnessError> for HistoricalImportError {
    fn from(error: FreshnessError) -> Self {
        Self::Freshness(error)
    }
}

/// Computes a caller-supplied projection in an experimental in-memory model.
/// It does not freeze a REL, verify source/graph/Community/vector/cohort proofs,
/// quiesce workers, or call `Cva::commit_historical_import`. Its event identities
/// are not the canonical H1 durable source-event identities. No model result is
/// evidence that a real import has been accepted.
pub fn execute_historical_import(
    input: HistoricalImportInput,
    store: &mut HistoricalCheckpointStore,
    policy: &FreshnessPolicy,
) -> Result<HistoricalImportResult, HistoricalImportError> {
    if input.import_id == [0; 32]
        || input.manifest_digest == [0; 32]
        || input.as_of_position == 0
        || input.timeline.activity_order.is_empty()
        || input.provenance.memories.is_empty()
    {
        return Err(HistoricalImportError::InvalidInput);
    }
    let mut propagation_events = Vec::new();
    let mut event_ids = Vec::new();
    for event in &input.semantic_plan.events {
        let propagation = HistoricalPropagationEvent::from_semantic(event, BTreeSet::new())?;
        event_ids.push(hash_event_id(event.event_id.as_bytes()));
        propagation_events.push(propagation);
    }
    for event in &input.dream_plan.events {
        event_ids.push(event.event_id);
        propagation_events.push(event.propagation.clone());
    }
    let outcomes = propagate_historical_events(
        &propagation_events,
        &input.adjacency,
        &input.memberships,
        &input.authorized,
        &input.availability,
        input.workers,
        policy,
    )?;
    let mut score_events = Vec::new();
    for (event_index, outcome) in outcomes.iter().enumerate() {
        for (&recipient, &amount) in &outcome.candidates {
            score_events.push(HistoricalScoreEvent {
                recipient,
                position: propagation_events[event_index].position,
                event_id: contribution_id(event_ids[event_index], recipient),
                amount,
            });
        }
    }
    let records = score_historical_records(
        &input.initial_records,
        &score_events,
        input.as_of_position,
        policy,
    )?;
    let applied_event_ids = score_events.iter().map(|event| event.event_id).collect();
    let candidate = HistoricalCheckpoint {
        import_id: input.import_id,
        manifest_digest: input.manifest_digest,
        position: input.as_of_position,
        records,
        applied_event_ids,
    };
    let commit = store.publish(candidate)?;
    let checkpoint = store
        .committed()
        .cloned()
        .ok_or(HistoricalImportError::InvalidInput)?;
    Ok(HistoricalImportResult {
        checkpoint,
        semantic_events: input.semantic_plan.events.len(),
        dream_events: input.dream_plan.events.len(),
        propagated_events: propagation_events.len(),
        commit,
    })
}

fn hash_event_id(event_id: &[u8]) -> [u8; 32] {
    Sha256::digest(event_id).into()
}
fn contribution_id(event_id: [u8; 32], recipient: MemoryId) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"reliquary/historical-contribution/v1\0");
    hash.update(event_id);
    hash.update(recipient.0);
    hash.finalize().into()
}

#[cfg(test)]
#[path = "historical_import_tests.rs"]
mod tests;
