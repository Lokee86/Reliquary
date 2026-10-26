use crate::cva_reconcile::with_replayed_transaction_time;
use reliquary_perception_store::entity_compact_codec::publication::coordinates;
use reliquary_perception_store::EntityReplayPublication;
use crate::{Cva, CvaReconcileError};

pub(crate) struct EntityTail {
    revisions: Vec<EntityReplayRevision>,
}

struct EntityReplayRevision {
    publication: EntityReplayPublication,
    transaction_time_ns: Option<i64>,
}

pub(crate) struct EntityReplayResult {
    pub(crate) revisions: usize,
    pub(crate) duplicate_revisions: usize,
}

pub(crate) fn read_entity_tail(
    cva: &mut Cva,
    start_chunk: usize,
) -> Result<EntityTail, CvaReconcileError> {
    read_entity_range(cva, start_chunk, usize::MAX)
}

pub(crate) fn read_entity_range(
    cva: &mut Cva,
    start_chunk: usize,
    end_chunk: usize,
) -> Result<EntityTail, CvaReconcileError> {
    let chunks = cva.container.chunks()?;
    let mut revisions = Vec::new();
    for chunk in chunks.iter().take(end_chunk).skip(start_chunk) {
        let payload = cva.container.read(*chunk)?;
        let Some((global, local)) = coordinates(&payload)? else {
            continue;
        };
        let mutation = cva
            .entities
            .publication_at(global, local)?
            .ok_or(crate::EntityError::InvalidVersion)?;
        revisions.push(EntityReplayRevision {
            publication: mutation,
            transaction_time_ns: cva.transaction_time_ns(global),
        });
    }
    Ok(EntityTail { revisions })
}

pub(crate) fn replay_entity_tail(
    destination: &mut Cva,
    tail: EntityTail,
) -> Result<EntityReplayResult, CvaReconcileError> {
    let mut revisions = 0;
    let mut duplicate_revisions = 0;
    for revision in tail.revisions {
        let created = with_replayed_transaction_time(
            destination,
            revision.transaction_time_ns,
            |destination| {
                destination
                    .entities
                    .replay_publication(&mut destination.container, revision.publication)
            },
        )?;
        if created {
            revisions += 1;
        } else {
            duplicate_revisions += 1;
        }
    }
    Ok(EntityReplayResult {
        revisions,
        duplicate_revisions,
    })
}

#[cfg(test)]
#[path = "cva_reconcile_entity_compact_tests.rs"]
mod compact_tests;

#[cfg(test)]
#[path = "cva_reconcile_entity_envelope_tests.rs"]
mod envelope_tests;
