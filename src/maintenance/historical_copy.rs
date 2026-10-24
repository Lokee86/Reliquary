//! Checkpoint-preserving physical maintenance, without numerical cut conversion.
use crate::owner_lifecycle::{OwnerError, ReliquaryOwner};

impl ReliquaryOwner {
    /// Physical maintenance may relocate objects, but must retain the accepted
    /// import and every later receipt/proof, even if score saturation hides a loss.
    pub(crate) fn validate_historical_import_copy(&self, copy: &Self) -> Result<(), OwnerError> {
        let source_access = crate::owner_lifecycle::HistoryReadAccess::new(&self.container);
        let copy_access = crate::owner_lifecycle::HistoryReadAccess::new(&copy.container);
        if !crate::history::historical_copy_preserves_import_authority(
            crate::history::HistoricalCopyView {
                archive: &self.archive,
                graph: &self.graph,
                freshness: &self.freshness,
                import_epochs: &self.import_epochs,
                read_access: &source_access,
                accepted_global_head: self.latest_global_version(),
            },
            crate::history::HistoricalCopyView {
                archive: &copy.archive,
                graph: &copy.graph,
                freshness: &copy.freshness,
                import_epochs: &copy.import_epochs,
                read_access: &copy_access,
                accepted_global_head: copy.latest_global_version(),
            },
        )? {
            return Err(OwnerError::Repack(
                "physical transformation changed historical checkpoint scoring or receipt/proof authority".into(),
            ));
        }
        Ok(())
    }

    pub(crate) fn validate_historical_principal_repack(
        &self,
        principal: &str,
    ) -> Result<(), OwnerError> {
        if !self.freshness.import_settlements().is_empty()
            && self
                .archive
                .nodes
                .iter()
                .any(|node| node.role == "user" && node.principal_id.as_deref() != Some(principal))
        {
            return Err(OwnerError::Repack(
                "historical checkpoint source-identity conversion is unsupported; principal backfill would rewrite accepted sources".into(),
            ));
        }
        Ok(())
    }
}
