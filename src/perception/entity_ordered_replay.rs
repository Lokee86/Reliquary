//! Local ordered entity revision replay under the held physical owner lease.
use crate::EntityError;
use crate::entity_compact_codec::publication::coordinates;
use crate::entity_store::EntityReplayPublication;

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

pub(crate) fn read_entity_range(
    container: &crate::Container,
    owner: &crate::entity_store::EntityStore,
    start_chunk: usize,
    end_chunk: usize,
) -> Result<EntityTail, EntityError> {
    let chunks = container.chunk_refs()?;
    let mut revisions = Vec::new();
    for chunk in chunks.iter().take(end_chunk).skip(start_chunk) {
        let payload = container.read(*chunk)?;
        let Some((global, local)) = coordinates(&payload)? else {
            continue;
        };
        let mutation = owner
            .publication_at(global, local)?
            .ok_or(crate::EntityError::InvalidVersion)?;
        revisions.push(EntityReplayRevision {
            publication: mutation,
            transaction_time_ns: container.transaction_time_ns(global),
        });
    }
    Ok(EntityTail { revisions })
}

pub(crate) fn replay_entity_tail(
    container: &mut crate::Container,
    owner: &mut crate::entity_store::EntityStore,
    tail: EntityTail,
) -> Result<EntityReplayResult, EntityError> {
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
    Ok(EntityReplayResult {
        revisions,
        duplicate_revisions,
    })
}
