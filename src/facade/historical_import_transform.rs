//! Checkpoint-preserving physical maintenance, without numerical cut conversion.
use crate::{Container, Cva, CvaError};
use sha2::{Digest, Sha256};

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
            || self.legacy_import_epochs() != copy.legacy_import_epochs()
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
            || authority_digest(&self.container)? != authority_digest(&copy.container)?
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

fn authority_digest(container: &Container) -> Result<[u8; 32], CvaError> {
    let mut scan = Container::open_read_only_for_scan(container.path())?;
    let mut hash = Sha256::new();
    hash.update(b"reliquary:historical-physical-authority:v1\0");
    scan.visit_payloads::<CvaError>(|_, payload| {
        // These concrete owner families have no relocatable ObjectRef fields.
        // Insomnia completion includes the native grouped publication proof;
        // Dream includes publication intents and original first-cycle roots.
        if payload.starts_with(b"CVAFR")
            || payload.starts_with(b"CVAFSA")
            || payload.starts_with(b"CVDREAM")
            || payload.starts_with(b"CVAINSC")
            || payload.starts_with(b"CVAIEPS1")
            || payload.starts_with(b"CVAIEPC1")
            || payload.starts_with(b"CVAILEG1")
        {
            hash.update((payload.len() as u64).to_le_bytes());
            hash.update(payload);
        }
        Ok(())
    })?;
    Ok(hash.finalize().into())
}
