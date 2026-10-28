use crate::{Entity, EntityAuditCollision, EntityAuditEntity, EntityId};
use std::collections::{BTreeMap, BTreeSet, HashMap};

pub fn exact_surface_collisions(
    entities: &[Entity],
    audit_entities: &[EntityAuditEntity],
) -> Vec<EntityAuditCollision> {
    cluster_entities(entities, audit_entities, |entity| {
        vec![format!(
            "{}:{}",
            entity.canonical_name.trim().to_lowercase(),
            entity.kind
        )]
    })
}

pub fn normalized_surface_collisions(
    entities: &[Entity],
    audit_entities: &[EntityAuditEntity],
) -> Vec<EntityAuditCollision> {
    cluster_entities(entities, audit_entities, |entity| {
        vec![format!(
            "{}:{}",
            normalized_surface(&entity.canonical_name),
            entity.kind
        )]
    })
}

pub fn alias_collisions(
    entities: &[Entity],
    audit_entities: &[EntityAuditEntity],
) -> Vec<EntityAuditCollision> {
    cluster_entities(entities, audit_entities, |entity| {
        std::iter::once(entity.canonical_name.as_str())
            .chain(entity.aliases.iter().map(String::as_str))
            .map(|surface| format!("{}:{}", normalized_surface(surface), entity.kind))
            .filter(|key| !key.starts_with(':'))
            .collect()
    })
}

pub fn cross_kind_alias_shadows(
    entities: &[Entity],
    audit_entities: &[EntityAuditEntity],
) -> Vec<EntityAuditCollision> {
    let audit_by_id = audit_entities
        .iter()
        .map(|entity| (entity.id, entity.clone()))
        .collect::<HashMap<_, _>>();
    let mut claims = BTreeMap::<String, Vec<(EntityId, String, bool)>>::new();

    for entity in entities {
        for (surface, is_alias) in std::iter::once((entity.canonical_name.as_str(), false))
            .chain(entity.aliases.iter().map(|alias| (alias.as_str(), true)))
        {
            let key = normalized_surface(surface);
            if key.is_empty() {
                continue;
            }
            let values = claims.entry(key).or_default();
            if let Some(existing) = values.iter_mut().find(|value| value.0 == entity.id) {
                existing.2 |= is_alias;
            } else {
                values.push((entity.id, entity.kind.clone(), is_alias));
            }
        }
    }

    claims
        .into_iter()
        .filter_map(|(key, values)| {
            if values.len() < 2 || !values.iter().any(|value| value.2) {
                return None;
            }
            let kinds = values
                .iter()
                .map(|value| value.1.as_str())
                .collect::<BTreeSet<_>>();
            if kinds.len() < 2 {
                return None;
            }
            let mut found = values
                .into_iter()
                .filter_map(|value| audit_by_id.get(&value.0).cloned())
                .collect::<Vec<_>>();
            found.sort_by(|left, right| {
                left.canonical_name
                    .cmp(&right.canonical_name)
                    .then_with(|| left.kind.cmp(&right.kind))
                    .then_with(|| left.id.cmp(&right.id))
            });
            Some(EntityAuditCollision {
                key,
                entities: found,
            })
        })
        .collect()
}

fn cluster_entities<F>(
    entities: &[Entity],
    audit_entities: &[EntityAuditEntity],
    keys: F,
) -> Vec<EntityAuditCollision>
where
    F: Fn(&Entity) -> Vec<String>,
{
    let audit_by_id = audit_entities
        .iter()
        .map(|entity| (entity.id, entity.clone()))
        .collect::<HashMap<_, _>>();
    let mut clusters = BTreeMap::<String, BTreeSet<EntityId>>::new();

    for entity in entities {
        for key in keys(entity).into_iter().filter(|key| !key.is_empty()) {
            clusters.entry(key).or_default().insert(entity.id);
        }
    }

    clusters
        .into_iter()
        .filter_map(|(key, ids)| {
            if ids.len() < 2 {
                return None;
            }
            let entities = ids
                .into_iter()
                .filter_map(|id| audit_by_id.get(&id).cloned())
                .collect();
            Some(EntityAuditCollision { key, entities })
        })
        .collect()
}

fn normalized_surface(value: &str) -> String {
    value
        .chars()
        .flat_map(char::to_lowercase)
        .filter(|value| value.is_alphanumeric())
        .collect()
}
