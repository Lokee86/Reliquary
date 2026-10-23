//! Freshness-owned staged replacement and durable settlement publication.
use crate::freshness_storage::{FreshnessStore, HistoricalImportFreshnessSettlement};
use crate::{Container, ContainerError, FreshnessDueDispatcher};
#[derive(Debug)]
pub(crate) enum SettlementPublicationError {
    Freshness(String),
    Container(ContainerError),
}
impl From<ContainerError> for SettlementPublicationError {
    fn from(error: ContainerError) -> Self {
        Self::Container(error)
    }
}
/// Fully checked Freshness replacement, publishable only through lifecycle transport.
pub(crate) struct SettlementPublication {
    staged: FreshnessStore,
    due: FreshnessDueDispatcher,
    payload: Vec<u8>,
    owner: [u8; 16],
}
pub(crate) fn stage_settlement(
    container: &Container,
    freshness: &FreshnessStore,
    settlement: &HistoricalImportFreshnessSettlement,
    source_bytes: u64,
    epoch: bool,
) -> Result<SettlementPublication, SettlementPublicationError> {
    let payload = settlement
        .try_encode()
        .map_err(SettlementPublicationError::Freshness)?;
    let mut staged = freshness.clone();
    if epoch {
        staged
            .bind_history_source(container, source_bytes)
            .map_err(SettlementPublicationError::Freshness)?;
        staged.freeze_derived_birth_policy();
    }
    staged
        .ingest_import_settlement(&payload)
        .map_err(SettlementPublicationError::Freshness)?;
    let mut due = crate::FreshnessDueDispatcher::default();
    due.rebuild(
        staged.records().iter().map(|(&id, &r)| (id, r)),
        settlement.settlement_cut,
        &freshness.policy(),
    )
    .map_err(|e| SettlementPublicationError::Freshness(e.to_string()))?;
    due.configure_initial_decay_carries(staged.initial_decay_carries())
        .map_err(|e| SettlementPublicationError::Freshness(e.to_string()))?;
    Ok(SettlementPublication {
        staged,
        due,
        payload,
        owner: settlement.owner,
    })
}
impl SettlementPublication {
    pub(crate) fn persist_policy(
        &mut self,
        container: &mut Container,
    ) -> Result<(), SettlementPublicationError> {
        self.staged
            .persist_policy_if_needed(container, self.owner)
            .map_err(SettlementPublicationError::Freshness)
    }
    pub(crate) fn payload(&self) -> &[u8] {
        &self.payload
    }
    /// Lifecycle calls this only after append and sync have both succeeded.
    pub(crate) fn acknowledge(
        mut self,
    ) -> Result<(FreshnessStore, FreshnessDueDispatcher), SettlementPublicationError> {
        self.staged
            .ingest_appended(&self.payload)
            .map_err(SettlementPublicationError::Freshness)?;
        Ok((self.staged, self.due))
    }
}
