//! Immutable numeric authority for the current semantic settlement grammar.
use super::historical_contract::HistoricalScoreEvent;
use super::historical_record_scoring::score_historical_records;
use super::policy::FreshnessPolicy;
use super::score::FreshnessRecord;
use crate::MemoryId;
use std::collections::BTreeMap;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CausalContribution {
    pub(crate) position: u64,
    pub(crate) effects: BTreeMap<MemoryId, i16>,
}

/// Records before scoring and canonical chronological contributions, plus exact
/// accepted scalar authority that cannot be derived from those inputs.
/// Construction validates before exposing this value; there are no output
/// record postimages or physical source coordinates in it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct HistoricalCausalBasis {
    initial: BTreeMap<MemoryId, FreshnessRecord>,
    contributions: Vec<CausalContribution>,
    corrections: BTreeMap<MemoryId, i16>,
}

impl HistoricalCausalBasis {
    /// Assign ranks only after checking the original source event identities.
    /// One source may affect several recipients at exactly one position.
    pub(crate) fn from_events(
        initial: &BTreeMap<MemoryId, FreshnessRecord>,
        events: &[HistoricalScoreEvent],
        cut: u64,
    ) -> Result<Self, String> {
        let mut positions = BTreeMap::new();
        let mut grouped = BTreeMap::<(u64, [u8; 32]), BTreeMap<MemoryId, i16>>::new();
        for event in events {
            if positions
                .insert(event.event_id, event.position)
                .is_some_and(|old| old != event.position)
            {
                return Err("conflicting causal source positions".into());
            }
            if grouped
                .entry((event.position, event.event_id))
                .or_default()
                .insert(event.recipient, event.amount)
                .is_some()
            {
                return Err("duplicate causal recipient/source identity".into());
            }
        }
        let contributions = grouped
            .into_iter()
            .map(|((position, _), effects)| CausalContribution { position, effects })
            .collect();
        Self::from_canonical(initial.clone(), contributions, BTreeMap::new(), cut)
    }

    /// The decoder provides already ordered rows. Never sort malformed input
    /// into validity or invent anchors for an unknown recipient.
    pub(crate) fn from_canonical(
        initial: BTreeMap<MemoryId, FreshnessRecord>,
        contributions: Vec<CausalContribution>,
        corrections: BTreeMap<MemoryId, i16>,
        cut: u64,
    ) -> Result<Self, String> {
        let basis = Self {
            initial,
            contributions,
            corrections,
        };
        basis.reconstruct(cut)?;
        Ok(basis)
    }

    pub(crate) fn initial(&self) -> &BTreeMap<MemoryId, FreshnessRecord> {
        &self.initial
    }
    pub(crate) fn contributions(&self) -> &[CausalContribution] {
        &self.contributions
    }
    pub(crate) fn corrections(&self) -> &BTreeMap<MemoryId, i16> {
        &self.corrections
    }

    pub(crate) fn reconstruct(
        &self,
        cut: u64,
    ) -> Result<BTreeMap<MemoryId, FreshnessRecord>, String> {
        let mut rows = Vec::new();
        let mut previous = 0;
        for (rank, contribution) in self.contributions.iter().enumerate() {
            if contribution.position == 0
                || contribution.position < previous
                || contribution.position > cut
                || contribution.effects.is_empty()
            {
                return Err("invalid causal contribution order/extent".into());
            }
            previous = contribution.position;
            let rank = u64::try_from(rank).map_err(|_| "too many causal contributions")?;
            // Big-endian rank preserves the original canonical same-turn order
            // when the single canonical scorer compares event identities.
            let mut event_id = [0; 32];
            event_id[24..].copy_from_slice(&rank.to_be_bytes());
            for (&recipient, &amount) in &contribution.effects {
                rows.push(HistoricalScoreEvent {
                    recipient,
                    position: contribution.position,
                    event_id,
                    amount,
                });
            }
        }
        let mut records = if cut == 0 && rows.is_empty() {
            let mut records = self.initial.clone();
            for record in records.values_mut() {
                record
                    .settle_with_policy(0, &FreshnessPolicy::default_v1())
                    .map_err(|e| e.to_string())?;
            }
            records
        } else {
            score_historical_records(&self.initial, &rows, cut, &FreshnessPolicy::default_v1())
                .map_err(|e| e.to_string())?
        };
        for (&recipient, &correction) in &self.corrections {
            if correction == 0 || !(-200..=200).contains(&correction) {
                return Err("invalid causal score correction".into());
            }
            let record = records
                .get_mut(&recipient)
                .ok_or("unknown causal correction recipient")?;
            let score = record
                .score
                .checked_add(correction)
                .ok_or("causal score overflow")?;
            if !(-100..=100).contains(&score) {
                return Err("causal corrected score outside policy bounds".into());
            }
            record.score = score;
        }
        Ok(records)
    }
}

#[cfg(test)]
#[path = "historical_causal_basis_tests.rs"]
mod tests;
