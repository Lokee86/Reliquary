use super::*;
use reliquary_graph::RelationChange;
use reliquary_perception_store::{EntityDraft, RelationshipDraft, RelationshipId};

pub(super) fn replay_perception(
    target: &mut Container,
    source: &PerceptionStore,
    counts: &mut ReplayCounts,
) -> Result<(), RelReconciliationError> {
    let mut destination = PerceptionStore::open(target).map_err(conflict)?;
    let mut entity_ids = source
        .entities()
        .map(|entity| entity.id)
        .collect::<BTreeSet<_>>();
    entity_ids.extend(source.historical_entities());
    for id in entity_ids {
        let last = source
            .entity(id)
            .map(|entity| entity.revision)
            .or_else(|| source.entity_tombstone(id).map(|value| value.revision))
            .ok_or_else(|| conflict("Entity history has no terminal record"))?;
        for revision in 1..=last {
            let value = source
                .entity_revision(id, revision)
                .ok_or_else(|| conflict("Entity revision is missing"))?;
            if let Some(existing) = destination.entity_revision(id, revision) {
                if existing != value {
                    return Err(conflict("conflicting Entity revision"));
                }
                continue;
            }
            let draft = EntityDraft {
                canonical_name: value.canonical_name.clone(),
                aliases: value.aliases.clone(),
                kind: value.kind.clone(),
                summary: value.summary.clone(),
                mutation_id: value.mutation_id.clone(),
                created_at_ns: value.created_at_ns,
                updated_at_ns: value.updated_at_ns,
            };
            let published = if revision == 1 {
                destination.publish_entity(target, draft)
            } else {
                destination.update_entity(target, id, revision - 1, draft)
            }
            .map_err(conflict)?;
            if published.id != id || published.revision != revision {
                return Err(conflict("Entity stable identity changed"));
            }
            counts.entities += 1;
        }
        if source.entity_tombstone(id).is_some() && destination.entity_tombstone(id).is_none() {
            destination
                .tombstone_entity(target, id, last)
                .map_err(conflict)?;
            counts.entities += 1;
        }
    }
    replay_relationships(target, source, &mut destination, counts)?;
    for current in source.mentions() {
        for revision in 1..=current.revision {
            let value = source
                .mention_revision(current.key, revision)
                .ok_or_else(|| conflict("Entity resolution revision is missing"))?;
            if let Some(existing) = destination.mention_revision(current.key, revision) {
                if existing != value {
                    return Err(conflict("conflicting Entity resolution"));
                }
                continue;
            }
            destination
                .put_mention_resolution(
                    target,
                    value.key,
                    revision - 1,
                    value.updated_at_ns,
                    value.status.clone(),
                )
                .map_err(conflict)?;
            counts.entities += 1;
        }
    }
    Ok(())
}

fn replay_relationships(
    target: &mut Container,
    source: &PerceptionStore,
    destination: &mut PerceptionStore,
    counts: &mut ReplayCounts,
) -> Result<(), RelReconciliationError> {
    let mut ids = source
        .relationships()
        .map(|relationship| relationship.id)
        .collect::<BTreeSet<RelationshipId>>();
    ids.extend(source.historical_relationships());
    for id in ids {
        let last = source
            .relationship(id)
            .map(|value| value.revision)
            .or_else(|| {
                source
                    .relationship_tombstone(id)
                    .map(|value| value.revision)
            })
            .ok_or_else(|| conflict("Relationship history has no terminal record"))?;
        for revision in 1..=last {
            let value = source
                .relationship_revision(id, revision)
                .ok_or_else(|| conflict("Relationship revision is missing"))?;
            if let Some(existing) = destination.relationship_revision(id, revision) {
                if existing != value {
                    return Err(conflict("conflicting Relationship revision"));
                }
                continue;
            }
            let draft = RelationshipDraft {
                kind: value.kind.clone(),
                participants: value.participants.clone(),
                evidence: value.evidence.clone(),
                summary: value.summary.clone(),
                mutation_id: value.mutation_id.clone(),
                created_at_ns: value.created_at_ns,
                updated_at_ns: value.updated_at_ns,
            };
            let published = if revision == 1 {
                destination.publish_relationship(target, draft)
            } else {
                destination.update_relationship(target, id, revision - 1, draft)
            }
            .map_err(conflict)?;
            if published.id != id || published.revision != revision {
                return Err(conflict("Relationship stable identity changed"));
            }
            counts.relationships += 1;
        }
        if source.relationship_tombstone(id).is_some()
            && destination.relationship_tombstone(id).is_none()
        {
            destination
                .tombstone_relationship(target, id, last)
                .map_err(conflict)?;
            counts.relationships += 1;
        }
    }
    Ok(())
}

pub(super) fn replay_graph(
    target: &mut Container,
    destination: &mut GraphStore,
    left: &GraphStore,
    right: &GraphStore,
    admission: &GraphAdmission,
    counts: &mut ReplayCounts,
) -> Result<(), RelReconciliationError> {
    let left_history = left.history().copied().collect::<Vec<_>>();
    let right_history = right.history().copied().collect::<Vec<_>>();
    let common = left_history
        .iter()
        .zip(&right_history)
        .take_while(|(left, right)| same_relation(left, right))
        .count();
    let left_tail = &left_history[common..];
    for relation in &right_history[common..] {
        if left_tail.iter().any(|left| same_relation(left, relation)) {
            continue;
        }
        if left_tail
            .iter()
            .any(|left| same_relation_key(left, relation))
        {
            return Err(conflict("conflicting Graph mutation"));
        }
        destination
            .put(
                target,
                admission,
                &[RelationChange {
                    source: relation.source,
                    target: relation.target,
                    kind: relation.kind,
                    active: relation.active,
                    origin: relation.origin,
                }],
            )
            .map_err(conflict)?;
        counts.graph += 1;
    }
    Ok(())
}

fn same_relation(left: &Relation, right: &Relation) -> bool {
    same_relation_key(left, right) && left.active == right.active && left.origin == right.origin
}

fn same_relation_key(left: &Relation, right: &Relation) -> bool {
    left.source == right.source && left.target == right.target && left.kind == right.kind
}
