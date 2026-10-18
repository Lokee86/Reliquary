use crate::cva_reconcile::with_replayed_transaction_time;
use crate::relationship_codec::{decode_record, decode_version};
use crate::{Cva, CvaReconcileError, Relationship, RelationshipDraft};

pub(crate) struct RelationshipTail {
    revisions: Vec<RelationshipReplayRevision>,
}

struct RelationshipReplayRevision {
    relationship: Relationship,
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
        let Some(version) = decode_version(&payload)? else {
            continue;
        };
        let record_payload = cva.container.read(version.record)?;
        let Some(mut record) = decode_record(&record_payload)? else {
            return Err(CvaReconcileError::UnsupportedSemanticOwner(
                "invalid Relationship version record",
            ));
        };
        record.global_version = version.global_version;
        record.relationship_version = version.relationship_version;
        revisions.push(RelationshipReplayRevision {
            relationship: Relationship {
                id: record.id,
                revision: record.revision,
                kind: record.kind,
                participants: record.participants,
                evidence: record.evidence,
                summary: record.summary,
                mutation_id: record.mutation_id,
                created_at_ns: record.created_at_ns,
                updated_at_ns: record.updated_at_ns,
                global_version: record.global_version,
                relationship_version: record.relationship_version,
            },
            transaction_time_ns: cva.transaction_time_ns(version.global_version),
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
        let relationship = revision.relationship;
        let id = relationship.id;
        let expected_revision = relationship.revision.saturating_sub(1);
        let draft = RelationshipDraft {
            kind: relationship.kind,
            participants: relationship.participants,
            evidence: relationship.evidence,
            summary: relationship.summary,
            mutation_id: relationship.mutation_id,
            created_at_ns: relationship.created_at_ns,
            updated_at_ns: relationship.updated_at_ns,
        };
        let created = with_replayed_transaction_time(
            destination,
            revision.transaction_time_ns,
            |destination| {
                destination.replay_relationship_revision(Some(id), expected_revision, draft)
            },
        )?
        .1;
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
