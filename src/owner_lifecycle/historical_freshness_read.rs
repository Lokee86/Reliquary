//! Immutable bounded Freshness reads bound to the held owner file.
use super::OwnerError;
use crate::freshness::historical_scorer::HistoricalScoreEvent;
use crate::freshness_storage::{FreshnessStore, ImportEventScope, ImportSemanticScope};
use crate::{Container, MemoryId};
use std::collections::BTreeSet;
/// This view exposes accepted evidence values and no mutable store or physical writer.
pub(crate) struct HistoricalFreshnessReadAccess<'a> {
    container: &'a Container,
    freshness: &'a FreshnessStore,
}
impl<'a> HistoricalFreshnessReadAccess<'a> {
    pub(crate) fn new(container: &'a Container, freshness: &'a FreshnessStore) -> Self {
        Self {
            container,
            freshness,
        }
    }
    pub(crate) fn policy(&self) -> crate::freshness::FreshnessPolicy {
        self.freshness.policy()
    }
    pub(crate) fn accepted_settlement(
        &self,
        open_import_id: Option<&str>,
    ) -> Option<crate::freshness_storage::HistoricalImportFreshnessSettlement> {
        match open_import_id {
            Some(import_id) => self
                .freshness
                .import_settlements()
                .iter()
                .find(|settlement| {
                    settlement
                        .import_id
                        .iter()
                        .map(|byte| format!("{byte:02x}"))
                        .collect::<String>()
                        == import_id
                })
                .cloned(),
            None => self.freshness.latest_import_settlement().cloned(),
        }
    }
    pub(crate) fn accepted_by_id(
        &self,
        import_id: [u8; 32],
    ) -> Option<&crate::freshness_storage::HistoricalImportFreshnessSettlement> {
        self.freshness.import_settlement(import_id)
    }
    pub(crate) fn has_accepted_settlements(&self) -> bool {
        !self.freshness.import_settlements().is_empty()
    }
    pub(crate) fn has_accepted_settlement_hex(&self, import_id: &str) -> bool {
        self.freshness
            .import_settlements()
            .iter()
            .any(|settlement| {
                settlement
                    .import_id
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect::<String>()
                    == import_id
            })
    }
    pub(crate) fn provisional_events(
        &self,
        recipients: &BTreeSet<MemoryId>,
        start: u64,
        cut: u64,
    ) -> Result<BTreeSet<String>, OwnerError> {
        self.freshness
            .provisional_import_events(recipients, start, cut)
            .map_err(OwnerError::Freshness)
    }
    pub(crate) fn replay_entries(
        &self,
    ) -> Result<Vec<crate::freshness_storage::FreshnessReplayEntry>, OwnerError> {
        self.freshness
            .replay_entries()
            .map_err(OwnerError::Freshness)
    }
    pub(crate) fn scoped_event_evidence(
        &self,
        scope: &ImportEventScope,
    ) -> Result<crate::freshness_storage::ImportEventEvidence, OwnerError> {
        let mut frozen = self.freshness.clone();
        frozen
            .bind_history_source(self.container, scope.freeze_prefix)
            .map_err(OwnerError::Freshness)?;
        frozen
            .import_event_evidence_in_scope(scope)
            .map_err(OwnerError::Freshness)
    }
    /// Physical synchronization remains on the lifecycle capability, outside History.
    pub(crate) fn synchronize_source(&self) -> Result<(), OwnerError> {
        self.container.sync().map_err(OwnerError::Container)
    }
    pub(crate) fn settlement_evidence(
        &self,
        replay: &ImportEventScope,
        cohort: &BTreeSet<MemoryId>,
        superseded: &BTreeSet<String>,
        retain_scores: bool,
    ) -> Result<(Vec<HistoricalScoreEvent>, ImportSemanticScope), OwnerError> {
        let mut history = self.freshness.clone();
        history
            .bind_history_source(self.container, replay.freeze_prefix)
            .map_err(OwnerError::Freshness)?;
        let retained = if retain_scores {
            history
                .retained_import_scores(replay, cohort, superseded)
                .map_err(OwnerError::Freshness)?
        } else {
            Vec::new()
        };
        let semantic = history
            .semantic_import_evidence(replay, cohort, superseded)
            .map_err(OwnerError::Freshness)?;
        Ok((retained, semantic))
    }
}
