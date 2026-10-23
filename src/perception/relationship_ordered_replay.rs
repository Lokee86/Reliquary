//! Local ordered relationship revision replay under the held physical owner lease.
use crate::RelationshipError;
use crate::relationship_compact_codec::publication::coordinates;
use crate::relationship_store::RelationshipReplayPublication;

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

pub(crate) fn read_relationship_range(
    container: &crate::Container,
    owner: &crate::relationship_store::RelationshipStore,
    start_chunk: usize,
    end_chunk: usize,
) -> Result<RelationshipTail, RelationshipError> {
    let chunks = container.chunk_refs()?;
    let mut revisions = Vec::new();
    for chunk in chunks.iter().take(end_chunk).skip(start_chunk) {
        let payload = container.read(*chunk)?;
        let Some((global, local)) = coordinates(&payload)? else {
            continue;
        };
        let mutation = owner
            .publication_at(global, local)?
            .ok_or(crate::RelationshipError::InvalidVersion)?;
        revisions.push(RelationshipReplayRevision {
            publication: mutation,
            transaction_time_ns: container.transaction_time_ns(global),
        });
    }
    Ok(RelationshipTail { revisions })
}

pub(crate) fn replay_relationship_tail(
    container: &mut crate::Container,
    owner: &mut crate::relationship_store::RelationshipStore,
    tail: RelationshipTail,
) -> Result<RelationshipReplayResult, RelationshipError> {
    let mut revisions = 0;
    let mut duplicate_revisions = 0;
    for revision in tail.revisions {
        container.set_next_transaction_time_override(revision.transaction_time_ns);
        let created = owner.replay_publication(container, revision.publication);
        container.clear_next_transaction_time_override();
        let created = created?;
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
