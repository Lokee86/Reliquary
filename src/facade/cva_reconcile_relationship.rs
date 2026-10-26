use crate::cva_reconcile::with_replayed_transaction_time;
use reliquary_perception_store::relationship_compact_codec::publication::coordinates;
use reliquary_perception_store::relationship_store::RelationshipReplayPublication;
use crate::{Cva, CvaReconcileError};

pub(crate) struct RelationshipTail {
    revisions: Vec<RelationshipReplayRevision>,
}

struct RelationshipReplayRevision {
    publication: RelationshipReplayPublication,
    transaction_time_ns: Option<i64>,
}

pub(crate) struct RelationshipReplayResult {
    pub(crate) revisions: usize,
    pub(crate) duplicate_revisions: usize,
}

pub(crate) fn read_relationship_tail(
    cva: &mut Cva,
    start_chunk: usize,
) -> Result<RelationshipTail, CvaReconcileError> {
    read_relationship_range(cva, start_chunk, usize::MAX)
}

pub(crate) fn read_relationship_range(
    cva: &mut Cva,
    start_chunk: usize,
    end_chunk: usize,
) -> Result<RelationshipTail, CvaReconcileError> {
    let chunks = cva.container.chunks()?;
    let mut revisions = Vec::new();
    for chunk in chunks.iter().take(end_chunk).skip(start_chunk) {
        let payload = cva.container.read(*chunk)?;
        let Some((global, local)) = coordinates(&payload)? else {
            continue;
        };
        let mutation = cva
            .relationships
            .publication_at(global, local)?
            .ok_or(crate::RelationshipError::InvalidVersion)?;
        revisions.push(RelationshipReplayRevision {
            publication: mutation,
            transaction_time_ns: cva.transaction_time_ns(global),
        });
    }
    Ok(RelationshipTail { revisions })
}

pub(crate) fn replay_relationship_tail(
    destination: &mut Cva,
    tail: RelationshipTail,
) -> Result<RelationshipReplayResult, CvaReconcileError> {
    let mut revisions = 0;
    let mut duplicate_revisions = 0;
    for revision in tail.revisions {
        let created = with_replayed_transaction_time(
            destination,
            revision.transaction_time_ns,
            |destination| {
                destination
                    .relationships
                    .replay_publication(&mut destination.container, revision.publication)
            },
        )?;
        if created {
            revisions += 1;
        } else {
            duplicate_revisions += 1;
        }
    }
    Ok(RelationshipReplayResult {
        revisions,
        duplicate_revisions,
    })
}

#[cfg(test)]
#[path = "cva_reconcile_relationship_compact_tests.rs"]
mod compact_tests;

#[cfg(test)]
#[path = "cva_reconcile_relationship_envelope_tests.rs"]
mod envelope_tests;
