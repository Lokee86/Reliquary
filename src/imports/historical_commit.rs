//! Import admission and publication of one already prepared historical settlement.
use super::historical_freeze::{self, HistoricalOwnerCut};
use super::historical_settlement_plan::PreparedHistoricalImport;
use crate::history::HistoricalSettlementReadOwners;
use crate::owner_lifecycle::{OwnerError, historical_settlement::HistoricalSettlementWriteAccess};
/// None is an exact accepted retry; Some carries the newly rebuilt transient index.
pub(crate) fn commit(
    writer: &mut HistoricalSettlementWriteAccess<'_>,
    owners: &HistoricalSettlementReadOwners<'_>,
    prepared: &PreparedHistoricalImport,
    cut: HistoricalOwnerCut,
    open_epoch: Option<crate::imports::ImportStart>,
) -> Result<
    Option<crate::owner_lifecycle::historical_settlement::HistoricalSettlementPublicationResult>,
    OwnerError,
> {
    let source = writer.history_access();
    if historical_freeze::admit_prepared(
        source.owner_uuid(),
        prepared,
        writer.accepted(prepared.settlement.import_id),
    )?
    .is_some()
    {
        return Ok(None);
    }
    historical_freeze::validate_owner_cut(prepared, cut)?;
    historical_freeze::validate_physical_freeze(prepared, &source)?;
    if prepared.epoch {
        let start = open_epoch.ok_or_else(|| {
            OwnerError::Freshness("historical reconstruction requires an open import epoch".into())
        })?;
        let scope = owners.epoch_scope(&source, start)?;
        historical_freeze::validate_start_binding(prepared, scope.start_digest)?;
        owners.validate(&source, &prepared.settlement)?;
    }
    drop(source);
    writer
        .publish(&prepared.settlement, prepared.source_bytes, prepared.epoch)
        .map(Some)
}
