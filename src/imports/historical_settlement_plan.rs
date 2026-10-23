//! Pure historical settlement scoring over authenticated owner facts.
use super::historical_event_plan::NativeHistoricalEvents;
use crate::MemoryId;
use crate::freshness::historical_scorer::{HistoricalScoreEvent, prepare_import_numeric_authority};
use crate::freshness::{FreshnessPolicy, FreshnessRecord};
use crate::freshness_storage::HistoricalImportFreshnessSettlement;
use crate::owner_lifecycle::OwnerError;
use std::collections::{BTreeMap, BTreeSet};
fn fail(message: &str) -> OwnerError {
    OwnerError::Freshness(message.into())
}

#[derive(Clone, Debug)]
pub(crate) struct PropagationInputs {
    pub adjacency: crate::freshness::MemoryAdjacency,
    pub memberships: crate::freshness::CommunityMembership,
    pub authorized: BTreeSet<MemoryId>,
    pub availability: BTreeMap<MemoryId, crate::freshness::historical_planner::MemoryAvailability>,
}
#[derive(Clone, Debug)]
pub(crate) struct FrozenHistoricalImport {
    pub(crate) source_bytes: u64,
    pub(crate) source_digest: [u8; 32],
    pub(crate) retained: Vec<HistoricalScoreEvent>,
    pub(crate) epoch: bool,
    pub(crate) events: Option<NativeHistoricalEvents>,
    pub(crate) propagation: Option<PropagationInputs>,
    pub(crate) settlement: HistoricalImportFreshnessSettlement,
    pub(crate) initial: BTreeMap<MemoryId, FreshnessRecord>,
}
#[derive(Clone, Debug)]
pub(crate) struct PreparedHistoricalImport {
    pub(crate) source_bytes: u64,
    pub(crate) source_digest: [u8; 32],
    pub(crate) epoch: bool,
    pub(crate) settlement: HistoricalImportFreshnessSettlement,
}
impl FrozenHistoricalImport {
    /// Only the owner-derived batch can enter production scoring.
    pub(crate) fn prepare_native(&self) -> Result<PreparedHistoricalImport, OwnerError> {
        let Some(_) = &self.events else {
            return Ok(PreparedHistoricalImport {
                source_bytes: self.source_bytes,
                source_digest: self.source_digest,
                epoch: self.epoch,
                settlement: self.settlement.clone(),
            });
        };
        let mut scores = self.native_scores()?;
        scores.extend(self.retained.clone());
        self.prepare(&scores)
    }
    pub(crate) fn native_scores(&self) -> Result<Vec<HistoricalScoreEvent>, OwnerError> {
        let Some(batch) = &self.events else {
            return Ok(Vec::new());
        };
        let inputs = self
            .propagation
            .as_ref()
            .ok_or_else(|| fail("missing frozen propagation proof"))?;
        if batch
            .ordered
            .iter()
            .any(|(_, event)| event.position > self.settlement.settlement_cut)
        {
            return Err(fail(
                "historical source event exceeds frozen checkpoint cut",
            ));
        }
        let events: Vec<_> = batch
            .ordered
            .iter()
            .map(|(_, event)| event.clone())
            .collect();
        let outcomes = crate::freshness::historical_propagation::propagate_historical_events(
            &events,
            &inputs.adjacency,
            &inputs.memberships,
            &inputs.authorized,
            &inputs.availability,
            1,
            &FreshnessPolicy::default_v1(),
        )
        .map_err(|e| fail(&e.to_string()))?;
        let scores: Vec<_> = batch
            .ordered
            .iter()
            .zip(outcomes)
            .flat_map(|((id, event), outcome)| {
                outcome
                    .candidates
                    .into_iter()
                    .map(move |(recipient, amount)| HistoricalScoreEvent {
                        recipient,
                        position: event.position,
                        event_id: *id,
                        amount,
                    })
            })
            .collect();
        Ok(scores)
    }
    pub(crate) fn import_id(&self) -> [u8; 32] {
        self.settlement.import_id
    }
    pub(crate) fn cut(&self) -> u64 {
        self.settlement.settlement_cut
    }
    pub(crate) fn initial_records(&self) -> &BTreeMap<MemoryId, FreshnessRecord> {
        &self.initial
    }
    /// Scores only positive historical contributions for supported admitted recipients.
    /// Internal coordinator evidence must be covered by the frozen canonical manifest.
    pub(crate) fn prepare(
        &self,
        events: &[HistoricalScoreEvent],
    ) -> Result<PreparedHistoricalImport, OwnerError> {
        let mut positions = BTreeMap::new();
        for event in events {
            if event.amount <= 0
                || event.amount > 50
                || self
                    .initial
                    .get(&event.recipient)
                    .is_none_or(|r| r.admitted_at_turn.is_none())
            {
                return Err(OwnerError::Freshness(
                    "unsupported historical contribution/recipient".into(),
                ));
            }
            if positions
                .insert(event.event_id, event.position)
                .is_some_and(|old| old != event.position)
            {
                return Err(OwnerError::Freshness(
                    "one historical event has conflicting source positions".into(),
                ));
            }
        }
        let mut settlement = self.settlement.clone();
        prepare_import_numeric_authority(&mut settlement, &self.initial, events)
            .map_err(OwnerError::Freshness)?;
        settlement.validate().map_err(OwnerError::Freshness)?;
        Ok(PreparedHistoricalImport {
            source_bytes: self.source_bytes,
            source_digest: self.source_digest,
            epoch: self.epoch,
            settlement,
        })
    }
}
impl PreparedHistoricalImport {
    pub(crate) fn records(&self) -> &BTreeMap<MemoryId, FreshnessRecord> {
        &self.settlement.records
    }
}
