//! A positive Dream relation and its evaluated-pair receipt share one
//! durable owner carrier. No Graph or pair projection changes before sync.
use super::*;
use crate::records::{DreamRelationCut, dream_group::GroupedDreamRelation};

fn preflight(
    common: &CommonState,
    owner: [u8; 16],
    record: GroupedDreamRelation,
) -> Result<bool, OwnerSessionError> {
    let cut = record.0;
    if cut.owner_id != owner {
        return Err(OwnerSessionError::StaleDreamCut);
    }
    let (left, right) = record.pair();
    if common.dream_pairs.contains(left, right) {
        return if common.dream_pairs.grouped_receipt(left, right) == Some(record) {
            Ok(false)
        } else {
            Err(OwnerSessionError::StaleDreamCut)
        };
    }
    if common.graph.graph_version() != cut.graph_version {
        return Err(OwnerSessionError::StaleDreamCut);
    }
    super::dream_no_link::validate_memory(common, cut.source)?;
    super::dream_no_link::validate_memory(common, cut.candidate)?;
    common.graph.validate_grouped_relation(
        &GraphAdmission::for_changes(owner, [left.0, right.0], []),
        record.relation(),
    )?;
    Ok(true)
}

fn publish(
    container: &mut Container,
    common: &mut CommonState,
    record: GroupedDreamRelation,
) -> Result<(), OwnerSessionError> {
    container.append(&record.encode())?;
    container.sync()?;
    let (left, right) = record.pair();
    common.graph.apply_grouped_relation(
        &GraphAdmission::for_changes(record.0.owner_id, [left.0, right.0], []),
        record.relation(),
    )?;
    common.dream_pairs.apply_grouped(record)?;
    Ok(())
}

impl RelWriter {
    /// Publish a single positive relation with its evaluated-pair receipt.
    /// Unsupported lifecycle changes must be refused in Application.
    pub fn commit_dream_grouped_relation(
        &mut self,
        cut: DreamRelationCut,
    ) -> Result<bool, OwnerSessionError> {
        self.ensure_write_ready()?;
        self.commit_dream_grouped_relation_after_fence(cut)
    }

    pub fn commit_native_import_dream_grouped_relation(
        &mut self,
        import_id: &str,
        cut: DreamRelationCut,
    ) -> Result<bool, OwnerSessionError> {
        self.validate_native_import_memory(import_id, cut.source.id)?;
        self.commit_dream_grouped_relation_after_fence(cut)
    }

    fn commit_dream_grouped_relation_after_fence(
        &mut self,
        cut: DreamRelationCut,
    ) -> Result<bool, OwnerSessionError> {
        let record = GroupedDreamRelation::new(cut).map_err(OwnerSessionError::Record)?;
        let before = self.container.writer_lease_proof();
        if before.is_none() || before != self.session.writer_lease_proof {
            return Err(OwnerSessionError::Record(
                "stale REL Dream writer lease".into(),
            ));
        }
        if !preflight(&self.session.common, self.owner_id(), record)? {
            return Ok(false);
        }
        if let Err(error) = publish(&mut self.container, &mut self.session.common, record) {
            self.session.write_poisoned = true;
            return Err(error);
        }
        self.session.writer_lease_proof = self.container.writer_lease_proof();
        Ok(true)
    }
}

impl PhyWriter {
    /// Private owner equivalent; no REL Archive authority is assumed.
    pub fn commit_dream_grouped_relation(
        &mut self,
        cut: DreamRelationCut,
    ) -> Result<bool, OwnerSessionError> {
        let record = GroupedDreamRelation::new(cut).map_err(OwnerSessionError::Record)?;
        let before = self.container.writer_lease_proof();
        if self.poisoned || before.is_none() || before != self.writer_lease_proof {
            return Err(OwnerSessionError::Record(
                "stale PHY Dream writer lease".into(),
            ));
        }
        if !preflight(&self.common, self.owner_id, record)? {
            return Ok(false);
        }
        if let Err(error) = publish(&mut self.container, &mut self.common, record) {
            self.poisoned = true;
            return Err(error);
        }
        self.writer_lease_proof = self.container.writer_lease_proof();
        Ok(true)
    }
}
#[cfg(test)]
#[path = "dream_group/tests.rs"]
mod tests;
