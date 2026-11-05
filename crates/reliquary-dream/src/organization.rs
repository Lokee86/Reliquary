//! Complete bounded duplicate/canonical/lifecycle policy over an exact snapshot.
use super::*;
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DreamOrganizationError {
    Limit,
    MissingFact,
    ConflictingLifecycle,
}
pub fn plan_organization(
    source: MemoryId,
    decisions: &[PairDecision],
    memories: &[MemoryFact],
    relations: &[Relation],
) -> Result<EvaluationPlan, DreamOrganizationError> {
    if memories.len() > 128 || decisions.len() > 16 || relations.len() > 1024 {
        return Err(DreamOrganizationError::Limit);
    }
    let facts: BTreeMap<_, _> = memories.iter().map(|m| (m.id, m)).collect();
    if facts.len() != memories.len()
        || !facts.contains_key(&source)
        || decisions.iter().any(|d| {
            !facts.contains_key(&d.a)
                || !facts.contains_key(&d.b)
                || (d.a != source && d.b != source)
        })
    {
        return Err(DreamOrganizationError::MissingFact);
    }
    let mut plan = evaluate_snapshot(decisions, memories, relations);
    let mut current: BTreeMap<_, _> = relations
        .iter()
        .map(|r| ((r.source, r.target, r.kind), *r))
        .collect();
    let protected: BTreeSet<_> = current
        .iter()
        .filter(|(_, r)| r.origin == RelationOrigin::User)
        .map(|(k, _)| *k)
        .collect();
    for c in &plan.relation_changes {
        if !protected_pair(&protected, c.source, c.target, c.kind) {
            current.insert(
                (c.source, c.target, c.kind),
                Relation {
                    source: c.source,
                    target: c.target,
                    kind: c.kind,
                    active: c.active,
                    origin: c.origin,
                    graph_version: 0,
                },
            );
        }
    }
    let merged: Vec<_> = current.values().copied().collect();
    let mut touched = BTreeSet::new();
    let mut components = Vec::new();
    for d in decisions
        .iter()
        .filter(|d| d.relation == PairRelation::DuplicateOf)
    {
        let ids = duplicate_component(d.a, &merged);
        if ids.iter().any(|id| !facts.contains_key(id)) {
            return Err(DreamOrganizationError::MissingFact);
        }
        if ids.iter().any(|id| touched.contains(id)) {
            continue;
        }
        touched.extend(ids.iter().copied());
        components.push(ids);
    }
    plan.relation_changes
        .retain(|c| c.kind != RelationKind::DuplicateOf);
    for mut ids in components {
        ids.sort_by_key(|id| (facts[id].source_time_ns.unwrap_or(i64::MAX), *id));
        let desired: BTreeSet<_> = ids
            .windows(2)
            .map(|w| {
                (
                    NodeRef::Memory(w[1].0),
                    NodeRef::Memory(w[0].0),
                    RelationKind::DuplicateOf,
                )
            })
            .collect();
        for r in relations.iter().filter(|r| {
            r.active && r.origin == RelationOrigin::Dream && r.kind == RelationKind::DuplicateOf
        }) {
            if matches!((r.source,r.target),(NodeRef::Memory(a),NodeRef::Memory(b)) if ids.contains(&MemoryId(a))&&ids.contains(&MemoryId(b)))
                && !desired.contains(&(r.source, r.target, r.kind))
            {
                plan.relation_changes.push(RelationChange {
                    source: r.source,
                    target: r.target,
                    kind: r.kind,
                    active: false,
                    origin: RelationOrigin::Dream,
                });
            }
        }
        for (a, b, kind) in desired {
            if !relations
                .iter()
                .any(|r| r.source == a && r.target == b && r.kind == kind && r.active)
                && !protected_pair(&protected, a, b, kind)
            {
                plan.relation_changes.push(RelationChange {
                    source: a,
                    target: b,
                    kind,
                    active: true,
                    origin: RelationOrigin::Dream,
                });
            }
        }
    }
    let source_fact = facts[&source];
    let mut effective = memories.to_vec();
    for intent in &plan.lifecycle_intents {
        if let Some(fact) = effective.iter_mut().find(|m| m.id == intent.memory_id) {
            fact.archived = intent.archived;
            fact.lifecycle_state = intent.lifecycle_state.clone();
            fact.superseded_by = intent.superseded_by;
        }
    }
    let component = duplicate_component(source, &merged);
    let peer = component
        .iter()
        .any(|id| *id != source && effective.iter().any(|m| m.id == *id && !m.archived));
    if peer && source_fact.lifecycle_state == "extracted" {
        if let Some(fact) = effective.iter_mut().find(|m| m.id == source) {
            fact.archived = true;
        }
    }
    let members: Vec<_> = effective
        .iter()
        .filter(|m| component.contains(&m.id))
        .cloned()
        .collect();
    let representative = duplicate_representative(&members);
    let canonical: Vec<_> = representative
        .and_then(|id| members.iter().find(|m| m.id == id))
        .filter(|m| explicit_authority_promotes(&m.authority_kind, &m.category, &m.memory_type))
        .cloned()
        .into_iter()
        .collect();
    let superseded: Vec<_> = plan
        .lifecycle_intents
        .iter()
        .filter(|i| i.archived)
        .map(|i| (i.memory_id, i.superseded_by))
        .collect();
    plan.lifecycle_intents = plan_lifecycle(
        source_fact,
        &superseded,
        peer,
        &canonical,
        explicit_authority_promotes(
            &source_fact.authority_kind,
            &source_fact.category,
            &source_fact.memory_type,
        ),
    );
    let mut intents = BTreeMap::new();
    for intent in plan.lifecycle_intents {
        if let Some(old) = intents.insert(intent.memory_id, intent.clone()) {
            if old != intent {
                return Err(DreamOrganizationError::ConflictingLifecycle);
            }
        }
    }
    if intents.values().any(|i| {
        i.superseded_by
            .is_some_and(|target| intents.get(&target).is_some_and(|t| t.archived))
    }) {
        return Err(DreamOrganizationError::ConflictingLifecycle);
    }
    plan.lifecycle_intents = intents.into_values().collect();
    plan.relation_changes
        .retain(|c| !protected_pair(&protected, c.source, c.target, c.kind));
    plan.relation_changes
        .sort_by_key(|c| (c.source, c.target, c.kind, c.active));
    plan.relation_changes.dedup();
    Ok(plan)
}

fn protected_pair(
    keys: &BTreeSet<(NodeRef, NodeRef, RelationKind)>,
    a: NodeRef,
    b: NodeRef,
    kind: RelationKind,
) -> bool {
    keys.contains(&(a, b, kind)) || keys.contains(&(b, a, kind))
}
