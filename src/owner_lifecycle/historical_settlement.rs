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
    let scope = SettlementWriteScope(container);
    let append_result = (|| {
        publication.persist_policy(scope.0).map_err(map_error)?;
        scope.0.append(publication.payload())?;
        scope.0.sync()?;
        Ok::<(), OwnerError>(())
    })();
    drop(scope);
    append_result?;
    publication.acknowledge().map_err(map_error)
}

/// Exclusive physical authority for the existing Freshness settlement publisher.
/// It exposes only immutable evidence and one validated domain publication.
pub(crate) struct HistoricalSettlementWriteAccess<'a> {
    container: &'a mut Container,
    freshness: &'a FreshnessStore,
}

impl super::ReliquaryOwner {
    /// Construct the owner-bound read view used by historical Import policy.
    pub(crate) fn historical_freshness_read_access(
        &self,
    ) -> super::historical_freshness_read::HistoricalFreshnessReadAccess<'_> {
        super::historical_freshness_read::HistoricalFreshnessReadAccess::new(
            &self.container,
            &self.freshness,
        )
    }

    /// Construct vector evidence access over the stores mounted by this REL.
    pub(crate) fn historical_vector_read_access(
        &self,
    ) -> super::semantic_source::HistoricalVectorReadAccess<'_> {
        super::semantic_source::HistoricalVectorReadAccess::new(
            super::semantic_source::SemanticSourceReadParts {
                container: &self.container,
                archive: &self.archive,
                freshness: &self.freshness,
                generations: &self.vector_generations,
                compatibility_profiles: &self.compatibility_profiles,
                archive_vectors: &self.archive_vectors,
                packed_vectors: &self.packed_vectors,
            },
            &self.memory_vectors,
        )
    }

    /// Bind the historical settlement writer to this owner's physical session.
    pub(crate) fn historical_settlement_write_access<'a>(
        container: &'a mut Container,
        freshness: &'a FreshnessStore,
    ) -> HistoricalSettlementWriteAccess<'a> {
        HistoricalSettlementWriteAccess::new(container, freshness)
    }

    /// Install a synced settlement publication into the durable Freshness owner.
    pub(crate) fn install_historical_settlement(
        &mut self,
        publication: HistoricalSettlementPublicationResult,
    ) -> FreshnessDueDispatcher {
        publication.install(&mut self.freshness)
    }
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

/// Clear only this bounded physical bypass on error or unwind.
struct SettlementWriteScope<'a>(&'a mut Container);
impl Drop for SettlementWriteScope<'_> {
    fn drop(&mut self) {
        self.0.set_owner_write_scope(false);
    }
}
