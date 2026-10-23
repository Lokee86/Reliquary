//! Narrow transport for a Freshness-validated historical settlement.
use super::OwnerError;
use crate::freshness::historical_settlement::{self, SettlementPublicationError};
use crate::freshness_storage::{FreshnessStore, HistoricalImportFreshnessSettlement};
use crate::{Container, FreshnessDueDispatcher};
fn map_error(error: SettlementPublicationError) -> OwnerError {
    match error {
        SettlementPublicationError::Freshness(message) => OwnerError::Freshness(message),
        SettlementPublicationError::Container(error) => OwnerError::Container(error),
    }
}
/// Import admits the owner/epoch and frozen proof before requesting this publisher.
pub(crate) fn publish_settlement(
    container: &mut Container,
    freshness: &FreshnessStore,
    settlement: &HistoricalImportFreshnessSettlement,
    source_bytes: u64,
    epoch: bool,
) -> Result<(FreshnessStore, FreshnessDueDispatcher), OwnerError> {
    let publication = historical_settlement::stage_settlement(
        container,
        freshness,
        settlement,
        source_bytes,
        epoch,
    )
    .map_err(map_error)?;
    publish_validated(container, publication, epoch)
}
fn publish_validated(
    container: &mut Container,
    mut publication: historical_settlement::SettlementPublication,
    epoch: bool,
) -> Result<(FreshnessStore, FreshnessDueDispatcher), OwnerError> {
    container.set_owner_write_scope(epoch);
    let append_result = (|| {
        publication.persist_policy(container).map_err(map_error)?;
        container.append(publication.payload())?;
        container.sync()?;
        Ok::<(), OwnerError>(())
    })();
    container.set_owner_write_scope(false);
    append_result?;
    publication.acknowledge().map_err(map_error)
}

/// Exclusive physical authority for the existing Freshness settlement publisher.
/// It exposes only immutable evidence and one validated domain publication.
pub(crate) struct HistoricalSettlementWriteAccess<'a> {
    container: &'a mut Container,
    freshness: &'a FreshnessStore,
}
impl<'a> HistoricalSettlementWriteAccess<'a> {
    pub(crate) fn new(container: &'a mut Container, freshness: &'a FreshnessStore) -> Self {
        Self {
            container,
            freshness,
        }
    }
    pub(crate) fn history_access(&self) -> super::HistoryReadAccess<'_> {
        super::HistoryReadAccess::new(self.container)
    }
    pub(crate) fn accepted(&self, id: [u8; 32]) -> Option<&HistoricalImportFreshnessSettlement> {
        self.freshness.import_settlement(id)
    }
    pub(crate) fn publish(
        &mut self,
        settlement: &HistoricalImportFreshnessSettlement,
        source_bytes: u64,
        epoch: bool,
    ) -> Result<HistoricalSettlementPublicationResult, OwnerError> {
        let (staged, due) = publish_settlement(
            self.container,
            self.freshness,
            settlement,
            source_bytes,
            epoch,
        )?;
        Ok(HistoricalSettlementPublicationResult { staged, due })
    }
}

/// Accepted publisher output; installation stays after successful sync and acknowledgement.
pub(crate) struct HistoricalSettlementPublicationResult {
    staged: FreshnessStore,
    due: FreshnessDueDispatcher,
}
impl HistoricalSettlementPublicationResult {
    pub(crate) fn install(self, freshness: &mut FreshnessStore) -> FreshnessDueDispatcher {
        *freshness = self.staged;
        self.due
    }
}
