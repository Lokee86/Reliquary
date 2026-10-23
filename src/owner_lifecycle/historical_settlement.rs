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
