use super::*;
use reliquary_perception_store::RelationshipDraft;

pub(super) fn relation_touches(closure: &SemanticClosure, relation: &Relation) -> bool {
    [relation.source, relation.target]
        .into_iter()
        .any(|node| closure.contains_node(node))
}

pub(super) fn require_memory(source: &CommonState, id: MemoryId) -> Result<(), String> {
    source
        .memories
        .history(id)
        .map(|_| ())
        .map_err(|_| "selected semantic closure references a missing local Memory".into())
}

pub(super) fn require_entity(source: &CommonState, id: EntityId) -> Result<(), String> {
    entity_revision_count(&source.perception, id)
        .map(|_| ())
        .map_err(|_| "selected semantic closure references a missing local Entity".into())
}

pub(super) fn all_relationship_ids(perception: &PerceptionStore) -> BTreeSet<RelationshipId> {
    perception
        .relationships()
        .map(|value| value.id)
        .chain(perception.historical_relationships())
        .collect()
}

pub(super) fn latest_relationship(
    perception: &PerceptionStore,
    id: RelationshipId,
) -> Result<&reliquary_perception_store::Relationship, String> {
    let revision = relationship_revision_count(perception, id)?;
    perception
        .relationship_revision(id, revision)
        .ok_or_else(|| "selected Relationship history is incomplete".into())
}

pub(super) fn entity_revision_count(
    perception: &PerceptionStore,
    id: EntityId,
) -> Result<u64, String> {
    perception
        .entity(id)
        .map(|value| value.revision)
        .or_else(|| perception.entity_tombstone(id).map(|value| value.revision))
        .ok_or_else(|| "selected Entity history is missing".into())
}

pub(super) fn relationship_revision_count(
    perception: &PerceptionStore,
    id: RelationshipId,
) -> Result<u64, String> {
    perception
        .relationship(id)
        .map(|value| value.revision)
        .or_else(|| {
            perception
                .relationship_tombstone(id)
                .map(|value| value.revision)
        })
        .ok_or_else(|| "selected Relationship history is missing".into())
}

pub(super) fn owner_ref_matches(value: &str, owner: [u8; 16]) -> bool {
    let compact = value.replace('-', "");
    if compact.len() != 32 {
        return false;
    }
    let mut candidate = [0u8; 16];
    for (index, byte) in candidate.iter_mut().enumerate() {
        let Some(value) = u8::from_str_radix(&compact[index * 2..index * 2 + 2], 16).ok() else {
            return false;
        };
        *byte = value;
    }
    candidate == owner
}

fn rewrite_owner_ref(value: &mut String, source: [u8; 16], destination: [u8; 16]) {
    if !owner_ref_matches(value, source) {
        return;
    }
    let compact = hex(&destination);
    *value = if value.contains('-') {
        format!(
            "{}-{}-{}-{}-{}",
            &compact[0..8],
            &compact[8..12],
            &compact[12..16],
            &compact[16..20],
            &compact[20..32]
        )
    } else {
        compact
    };
}

pub(super) fn relationship_draft(
    relationship: &reliquary_perception_store::Relationship,
    source_owner: [u8; 16],
    destination_owner: [u8; 16],
) -> RelationshipDraft {
    let mut participants = relationship.participants.clone();
    for participant in &mut participants {
        rewrite_owner_ref(
            &mut participant.entity.owner_id,
            source_owner,
            destination_owner,
        );
    }
    participants.sort_by(|left, right| {
        (&left.entity.owner_id, left.entity.entity_id, &left.role).cmp(&(
            &right.entity.owner_id,
            right.entity.entity_id,
            &right.role,
        ))
    });
    let mut evidence = relationship.evidence.clone();
    for reference in &mut evidence {
        rewrite_owner_ref(&mut reference.owner_id, source_owner, destination_owner);
    }
    evidence.sort_by(|left, right| {
        (&left.owner_id, left.memory_id).cmp(&(&right.owner_id, right.memory_id))
    });
    RelationshipDraft {
        kind: relationship.kind.clone(),
        participants,
        evidence,
        summary: relationship.summary.clone(),
        mutation_id: relationship.mutation_id.clone(),
        created_at_ns: relationship.created_at_ns,
        updated_at_ns: relationship.updated_at_ns,
    }
}
