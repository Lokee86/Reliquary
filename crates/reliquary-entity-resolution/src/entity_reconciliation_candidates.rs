use crate::{Entity, EntityId, EntityReconciliationCandidate};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

const GENERIC_TOKENS: &[&str] = &[
    "application",
    "component",
    "directory",
    "file",
    "module",
    "project",
    "repository",
    "repo",
    "server",
    "service",
    "subsystem",
    "system",
    "tool",
];

pub fn reconciliation_candidates(
    entities: &[Entity],
    pair_limit: usize,
) -> Vec<EntityReconciliationCandidate> {
    let by_id = entities
        .iter()
        .map(|entity| (entity.id, entity))
        .collect::<HashMap<_, _>>();
    let mut pairs = BTreeMap::<(EntityId, EntityId), EntityReconciliationCandidate>::new();

    add_exact_claim_pairs(entities, &mut pairs);
    add_lexical_pairs(entities, &mut pairs);

    let mut values = pairs.into_values().collect::<Vec<_>>();
    values.sort_by(|left, right| {
        right
            .deterministic
            .cmp(&left.deterministic)
            .then_with(|| right.score.cmp(&left.score))
            .then_with(|| {
                let l = by_id
                    .get(&left.left)
                    .map(|e| e.canonical_name.as_str())
                    .unwrap_or("");
                let r = by_id
                    .get(&right.left)
                    .map(|e| e.canonical_name.as_str())
                    .unwrap_or("");
                l.cmp(r)
            })
            .then_with(|| left.left.cmp(&right.left))
            .then_with(|| left.right.cmp(&right.right))
    });
    values.truncate(pair_limit);
    values
}

fn add_exact_claim_pairs(
    entities: &[Entity],
    pairs: &mut BTreeMap<(EntityId, EntityId), EntityReconciliationCandidate>,
) {
    let mut claims = BTreeMap::<(String, String), BTreeSet<EntityId>>::new();
    for entity in entities {
        if entity.kind == "principal" {
            continue;
        }
        for surface in std::iter::once(entity.canonical_name.as_str())
            .chain(entity.aliases.iter().map(String::as_str))
        {
            let key = surface.trim().to_lowercase();
            if !key.is_empty() {
                claims
                    .entry((key, entity.kind.clone()))
                    .or_default()
                    .insert(entity.id);
            }
        }
    }
    for ids in claims.into_values() {
        let values = ids.into_iter().collect::<Vec<_>>();
        for i in 0..values.len() {
            for j in (i + 1)..values.len() {
                insert_pair(pairs, values[i], values[j], false, u16::MAX);
            }
        }
    }
}

fn add_lexical_pairs(
    entities: &[Entity],
    pairs: &mut BTreeMap<(EntityId, EntityId), EntityReconciliationCandidate>,
) {
    let mut by_token = BTreeMap::<String, Vec<EntityId>>::new();
    let mut tokens_by_id = HashMap::<EntityId, BTreeSet<String>>::new();
    let kind_by_id = entities
        .iter()
        .map(|entity| (entity.id, entity.kind.as_str()))
        .collect::<HashMap<_, _>>();

    for entity in entities {
        if entity.kind == "principal" {
            continue;
        }
        let tokens = identity_tokens(&entity.canonical_name);
        for token in &tokens {
            by_token.entry(token.clone()).or_default().push(entity.id);
        }
        tokens_by_id.insert(entity.id, tokens);
    }

    let mut seen = HashSet::new();
    for ids in by_token.into_values() {
        if ids.len() > 32 {
            continue;
        }
        for i in 0..ids.len() {
            for j in (i + 1)..ids.len() {
                let (left, right) = ordered(ids[i], ids[j]);
                if !seen.insert((left, right)) || kind_by_id.get(&left) != kind_by_id.get(&right) {
                    continue;
                }
                let Some(a) = tokens_by_id.get(&left) else {
                    continue;
                };
                let Some(b) = tokens_by_id.get(&right) else {
                    continue;
                };
                let shared = a.intersection(b).count();
                let union = a.union(b).count().max(1);
                let subset = a.is_subset(b) || b.is_subset(a);
                let score = ((shared * 1000) / union) as u16;
                if subset || score >= 330 {
                    insert_pair(pairs, left, right, false, score);
                }
            }
        }
    }
}

fn insert_pair(
    pairs: &mut BTreeMap<(EntityId, EntityId), EntityReconciliationCandidate>,
    a: EntityId,
    b: EntityId,
    deterministic: bool,
    score: u16,
) {
    let (left, right) = ordered(a, b);
    pairs
        .entry((left, right))
        .and_modify(|candidate| {
            candidate.deterministic |= deterministic;
            candidate.score = candidate.score.max(score);
        })
        .or_insert(EntityReconciliationCandidate {
            left,
            right,
            deterministic,
            score,
        });
}

fn ordered(a: EntityId, b: EntityId) -> (EntityId, EntityId) {
    if a <= b { (a, b) } else { (b, a) }
}

fn identity_tokens(value: &str) -> BTreeSet<String> {
    value
        .split(|ch: char| !ch.is_alphanumeric())
        .map(str::to_lowercase)
        .filter(|token| token.len() >= 4 && !GENERIC_TOKENS.contains(&token.as_str()))
        .collect()
}
