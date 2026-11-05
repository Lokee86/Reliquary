//! Grouped Dream Supersedes mutation through one owner writer lease.
use super::*;
use crate::records::{DreamRelationCut, dream_supersession::GroupedDreamSupersession};

fn prepare(
    common: &CommonState,
    owner: [u8; 16],
    cut: DreamRelationCut,
    updated_at_ns: i64,
) -> Result<Option<GroupedDreamSupersession>, OwnerSessionError> {
    if cut.owner_id != owner {
        return Err(OwnerSessionError::StaleDreamCut);
    }
    let (left, right) = (cut.source.id, cut.candidate.id);
    if common.dream_pairs.contains(left, right) {
        return match common.dream_pairs.supersession_receipt(left, right) {
            Some(existing) if existing.cut == cut && existing.updated_at_ns == updated_at_ns => {
                Ok(None)
            }
            _ => Err(OwnerSessionError::StaleDreamCut),
        };
    }
    if common.graph.graph_version() != cut.graph_version {
        return Err(OwnerSessionError::StaleDreamCut);
    }
    super::dream_no_link::validate_memory(common, cut.source)?;
    super::dream_no_link::validate_memory(common, cut.candidate)?;
    let NodeRef::Memory(older) = cut.change.target else {
        return Err(OwnerSessionError::StaleDreamCut);
    };
    let NodeRef::Memory(newer) = cut.change.source else {
        return Err(OwnerSessionError::StaleDreamCut);
    };
    let previous_revision = if cut.source.id.0 == older {
        cut.source.revision
    } else if cut.candidate.id.0 == older {
        cut.candidate.revision
    } else {
        return Err(OwnerSessionError::StaleDreamCut);
    };
    let delta = common.memories.prepare_grouped_supersession(
        MemoryId(older),
        previous_revision,
        MemoryId(newer),
        GroupedDreamSupersession::mutation_id(&cut),
        updated_at_ns,
    )?;
    let group = GroupedDreamSupersession::new(cut, updated_at_ns, delta)
        .map_err(OwnerSessionError::Record)?;
    common.graph.validate_grouped_relation(
        &GraphAdmission::for_changes(owner, [left.0, right.0], []),
        group.relation(),
    )?;
    Ok(Some(group))
}

fn publish(
    container: &mut Container,
    common: &mut CommonState,
    group: GroupedDreamSupersession,
) -> Result<(), OwnerSessionError> {
    // The exact Graph, Memory and pair effects are fully preflighted before
    // one physical append. An uncertain sync requires owner reopen.
    let admission = GraphAdmission::for_changes(
        group.cut.owner_id,
        [group.cut.source.id.0, group.cut.candidate.id.0],
        [],
    );
    common
        .memories
        .preflight_grouped_lifecycle_delta(&group.delta)?;
    container.append(&group.encode())?;
    container.sync()?;
    common
        .memories
        .apply_grouped_supersession(&group.memory_event())?;
    common
        .graph
        .apply_grouped_relation(&admission, group.relation())?;
    common.refresh_dream_index(&[group.memory_event().older])?;
    common.dream_pairs.apply_supersession(group)?;
    Ok(())
}

impl RelWriter {
    pub fn commit_dream_supersession(
        &mut self,
        cut: DreamRelationCut,
        updated_at_ns: i64,
    ) -> Result<bool, OwnerSessionError> {
        self.ensure_write_ready()?;
        let before = self.container.writer_lease_proof();
        if before.is_none() || before != self.session.writer_lease_proof {
            return Err(OwnerSessionError::Record(
                "stale REL Dream writer lease".into(),
            ));
        }
        let Some(group) = prepare(&self.session.common, self.owner_id(), cut, updated_at_ns)?
        else {
            return Ok(false);
        };
        if let Err(error) = publish(&mut self.container, &mut self.session.common, group) {
            self.session.write_poisoned = true;
            return Err(error);
        }
        self.session.writer_lease_proof = self.container.writer_lease_proof();
        Ok(true)
    }
}

impl PhyWriter {
    pub fn commit_dream_supersession(
        &mut self,
        cut: DreamRelationCut,
        updated_at_ns: i64,
    ) -> Result<bool, OwnerSessionError> {
        let before = self.container.writer_lease_proof();
        if self.poisoned || before.is_none() || before != self.writer_lease_proof {
            return Err(OwnerSessionError::Record(
                "stale PHY Dream writer lease".into(),
            ));
        }
        let Some(group) = prepare(&self.common, self.owner_id, cut, updated_at_ns)? else {
            return Ok(false);
        };
        if let Err(error) = publish(&mut self.container, &mut self.common, group) {
            self.poisoned = true;
            return Err(error);
        }
        self.writer_lease_proof = self.container.writer_lease_proof();
        Ok(true)
    }
}
#[cfg(test)]
#[path = "dream_supersession/tests.rs"]
mod tests;
