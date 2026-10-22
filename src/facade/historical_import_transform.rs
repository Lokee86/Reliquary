//! Checkpoint-preserving physical maintenance, without numerical cut conversion.
use crate::{Cva, CvaError};

impl Cva {
    /// Physical maintenance may relocate objects, but must retain the accepted
    /// import and every later receipt/proof, even if score saturation hides a loss.
    pub(crate) fn validate_historical_import_copy(&self, copy: &Self) -> Result<(), CvaError> {
        if self.freshness.import_settlements().is_empty()
            && copy.freshness.import_settlements().is_empty()
        {
            return Ok(());
        }
        if self.freshness.import_settlements() != copy.freshness.import_settlements()
            || self.import_epoch_statuses() != copy.import_epoch_statuses()
            || self.freshness_policy() != copy.freshness_policy()
            || self.rel_turn_count() != copy.rel_turn_count()
            || self.latest_global_version() != copy.latest_global_version()
            || self.archive_version() != copy.archive_version()
            || self.graph_version() != copy.graph_version()
            || self.memory_graph_version() != copy.memory_graph_version()
            || self
                .freshness
                .records_at(self.rel_turn_count())
                .map_err(CvaError::Freshness)?
                != copy
                    .freshness
                    .records_at(copy.rel_turn_count())
                    .map_err(CvaError::Freshness)?
            || crate::history::historical_owner_evidence_digest(
                &crate::owner_lifecycle::HistorySource::new(&self.container)
                    .map_err(|error| CvaError::Container(error.0))?,
            )
            .map_err(|error| CvaError::Container(error.0))?
                != crate::history::historical_owner_evidence_digest(
                    &crate::owner_lifecycle::HistorySource::new(&copy.container)
                        .map_err(|error| CvaError::Container(error.0))?,
                )
                .map_err(|error| CvaError::Container(error.0))?
        {
            return Err(CvaError::Repack(
                "physical transformation changed historical checkpoint scoring or receipt/proof authority".into(),
            ));
        }
        Ok(())
    }

    pub(crate) fn validate_historical_principal_repack(
        &self,
        principal: &str,
    ) -> Result<(), CvaError> {
        if !self.freshness.import_settlements().is_empty()
            && self
                .archive
                .nodes
                .iter()
                .any(|node| node.role == "user" && node.principal_id.as_deref() != Some(principal))
        {
            return Err(CvaError::Repack(
                "historical checkpoint source-identity conversion is unsupported; principal backfill would rewrite accepted sources".into(),
            ));
        }
        Ok(())
    }
}
