//! Test-only semantic evidence selection using the owner's canonical cut rules.
use super::{
    FreshnessReplayEntry, FreshnessStore, HistoricalImportFreshnessSettlement, JournalEntry,
    ordering,
};
use crate::freshness::{FreshnessPolicy, FreshnessRecord};
use crate::{FreshnessEventCut, MemoryId};
use std::collections::BTreeMap;

pub(crate) struct FreshnessOracleState {
    pub(crate) policy: FreshnessPolicy,
    pub(crate) records: BTreeMap<MemoryId, FreshnessRecord>,
    pub(crate) entries: Vec<FreshnessReplayEntry>,
    pub(crate) settlements: Vec<HistoricalImportFreshnessSettlement>,
    pub(crate) activation: serde_json::Value,
}

impl FreshnessStore {
    /// Read-only fixture oracle: scan the exact accepted journal once, then use
    /// the existing in-memory scan path for repeated cuts. Never publish from
    /// this disposable clone. Semantic scope reconstruction explicitly rebinds
    /// its own original physical prefix and therefore bypasses this cache.
    pub(crate) fn with_cached_oracle_journal(&self) -> Result<Self, String> {
        let mut entries = std::collections::VecDeque::new();
        self.scan_entries(|entry| {
            entries.push_back((entry, 0));
            Ok(())
        })?;
        let mut cached = self.clone();
        cached.history_path = None;
        cached.recent = entries;
        cached.recent_bytes = 0;
        Ok(cached)
    }

    pub(crate) fn historical_oracle_state(
        &self,
        cut: &FreshnessEventCut,
    ) -> Result<FreshnessOracleState, String> {
        // This owns fail-closed import/graph/intra-settlement admission as well as
        // exact score settlement. The oracle must not invent another replay.
        let records = self.records_at_cut(cut)?;
        let mut entries: Vec<_> = self
            .retained_history_entries()?
            .into_iter()
            .filter(|entry| {
                !matches!(entry, FreshnessReplayEntry::Event { .. })
                    && entry.source_turn() <= cut.rel_turn
            })
            .collect();
        ordering::ordered_events(
            self,
            cut.rel_turn,
            cut.event_id_inclusive.as_deref().map(|id| (id, true)),
            cut.graph_version,
            &[],
            |event| {
                let JournalEntry::Event {
                    event_id,
                    turn,
                    provenance,
                    proof,
                    effects,
                    records: _,
                    semantic,
                } = event
                else {
                    unreachable!()
                };
                entries.push(FreshnessReplayEntry::Event {
                    event_id: event_id.clone(),
                    source_turn: *turn,
                    effects: effects.clone(),
                    proof: *proof,
                    provenance: Some(provenance.clone()),
                    semantic: semantic.clone(),
                });
                Ok(())
            },
        )?;
        Ok(FreshnessOracleState {
            policy: self.policy(),
            records,
            entries,
            settlements: self
                .import_settlements
                .iter()
                .filter(|s| s.settlement_cut <= cut.rel_turn)
                .cloned()
                .collect(),
            // There is no historical activation clock. This is current owner
            // policy/acceptance metadata, like clock-neutral Memory routing.
            activation: serde_json::json!({"seen": self.semantic_activation_seen, "activation": &self.semantic_activation}),
        })
    }
}
