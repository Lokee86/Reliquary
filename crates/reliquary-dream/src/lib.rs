//! Pure Dream evaluation over caller-authorized Memory and Graph snapshots.
//!
//! This crate proposes relation and lifecycle changes. Owners remain
//! responsible for admission, revalidation, and durable writes.

use reliquary_graph::{NodeRef, Relation, RelationChange, RelationKind, RelationOrigin};
use reliquary_memory_core::{MemoryBodyId, MemoryId};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

mod candidate_config;
mod candidate_frontier;
mod candidate_ranking;
mod classifier;
mod classifier_error;
mod classifier_model;
mod classifier_schema;
mod evaluator;
mod evaluator_frontier;
mod pair_context;
mod verifier;
mod verifier_error;
mod verifier_model;
mod verifier_schema;

pub use candidate_config::*;
pub use candidate_frontier::DreamCandidateFrontier;
pub use candidate_ranking::{
    RankedCandidate, candidate_cosine, lexical_score, rank_lanes, select_candidates,
    validate_candidate_config,
};
pub use classifier::DreamClassifier;
pub use classifier_error::DreamClassificationError;
pub use classifier_model::*;
pub use classifier_schema::{DREAM_CLASSIFIER_SYSTEM_PROMPT, dream_classifier_schema};
pub use evaluator::{DreamEvaluationError, DreamPairEvaluator};
pub use pair_context::{DreamCandidateContexts, DreamContext, DreamMemory};
pub use reliquary_switchboard::{GeneralEndpoint, GeneralEndpointError};
pub use verifier::DreamVerifier;
pub use verifier_error::DreamVerificationError;
pub use verifier_model::*;
pub use verifier_schema::{DREAM_VERIFIER_SYSTEM_PROMPT, dream_verifier_schema};

#[cfg(test)]
mod acceptance_tests;
#[cfg(test)]
mod evaluator_tests;
#[cfg(test)]
mod inference_tests;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PairRelation {
    None,
    Topical,
    Factual,
    Causal,
    Recurrent,
    DuplicateOf,
    Supersedes,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PairDirection {
    None,
    Undirected,
    AToB,
    BToA,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PairDecision {
    pub a: MemoryId,
    pub b: MemoryId,
    pub relation: PairRelation,
    pub direction: PairDirection,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemoryFact {
    pub id: MemoryId,
    pub lifecycle_state: String,
    pub archived: bool,
    pub superseded_by: Option<MemoryId>,
    pub source_time_ns: Option<i64>,
    pub authority_kind: String,
    pub category: String,
    pub memory_type: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LifecycleIntent {
    pub memory_id: MemoryId,
    pub lifecycle_state: String,
    pub archived: bool,
    pub superseded_by: Option<MemoryId>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct EvaluationPlan {
    pub relation_changes: Vec<RelationChange>,
    pub lifecycle_intents: Vec<LifecycleIntent>,
}

/// Plans only changes justified by the supplied pair decisions and owner facts.
/// No store is opened and no owner state is mutated.
pub fn evaluate_snapshot(
    decisions: &[PairDecision],
    memories: &[MemoryFact],
    relations: &[Relation],
) -> EvaluationPlan {
    let facts: BTreeMap<_, _> = memories.iter().map(|m| (m.id, m)).collect();
    let mut desired = BTreeSet::<(MemoryId, MemoryId, RelationKind)>::new();
    let mut evaluated_pairs = BTreeSet::new();
    let mut intents = BTreeMap::new();
    for decision in decisions {
        if decision.a == decision.b
            || !facts.contains_key(&decision.a)
            || !facts.contains_key(&decision.b)
        {
            continue;
        }
        evaluated_pairs.insert(ordered_pair(decision.a, decision.b));
        match decision.relation {
            PairRelation::None => {}
            PairRelation::DuplicateOf => {
                for edge in duplicate_chain(decision.a, decision.b, &facts) {
                    desired.insert(edge);
                }
            }
            PairRelation::Supersedes => {
                let (newer, older) = match decision.direction {
                    PairDirection::AToB => (decision.a, decision.b),
                    PairDirection::BToA => (decision.b, decision.a),
                    _ => continue,
                };
                desired.insert((newer, older, RelationKind::Supersedes));
                intents.insert(
                    older,
                    LifecycleIntent {
                        memory_id: older,
                        lifecycle_state: "archived".into(),
                        archived: true,
                        superseded_by: Some(newer),
                    },
                );
            }
            kind => {
                let graph_kind = match kind {
                    PairRelation::Topical => RelationKind::Topical,
                    PairRelation::Factual => RelationKind::Factual,
                    PairRelation::Causal => RelationKind::Causal,
                    PairRelation::Recurrent => RelationKind::Recurrent,
                    _ => unreachable!(),
                };
                let (a, b) = match decision.direction {
                    PairDirection::AToB => (decision.a, decision.b),
                    PairDirection::BToA => (decision.b, decision.a),
                    PairDirection::None | PairDirection::Undirected => (decision.a, decision.b),
                };
                desired.insert((a, b, graph_kind));
            }
        }
    }
    let existing: BTreeSet<_> = relations
        .iter()
        .filter(|r| r.active && r.origin == RelationOrigin::Dream)
        .filter_map(|r| match (r.source, r.target) {
            (NodeRef::Memory(a), NodeRef::Memory(b)) => Some((MemoryId(a), MemoryId(b), r.kind)),
            _ => None,
        })
        .collect();
    let mut changes = Vec::new();
    for (a, b, kind) in existing.difference(&desired) {
        if is_dream_managed_kind(*kind) && evaluated_pairs.contains(&ordered_pair(*a, *b)) {
            changes.push(relation_change(*a, *b, *kind, false));
        }
    }
    for (a, b, kind) in desired.difference(&existing) {
        changes.push(relation_change(*a, *b, *kind, true));
    }
    changes.sort_by_key(|c| (c.source, c.target, c.kind, c.active));
    EvaluationPlan {
        relation_changes: changes,
        lifecycle_intents: intents.into_values().collect(),
    }
}

fn ordered_pair(a: MemoryId, b: MemoryId) -> (MemoryId, MemoryId) {
    if a <= b { (a, b) } else { (b, a) }
}

fn duplicate_chain(
    a: MemoryId,
    b: MemoryId,
    facts: &BTreeMap<MemoryId, &MemoryFact>,
) -> Vec<(MemoryId, MemoryId, RelationKind)> {
    let mut ids = vec![a, b];
    ids.sort_by_key(|id| {
        let m = facts[id];
        (m.source_time_ns.unwrap_or(i64::MAX), m.id)
    });
    vec![(ids[1], ids[0], RelationKind::DuplicateOf)]
}

fn relation_change(a: MemoryId, b: MemoryId, kind: RelationKind, active: bool) -> RelationChange {
    RelationChange {
        source: NodeRef::Memory(a.0),
        target: NodeRef::Memory(b.0),
        kind,
        active,
        origin: RelationOrigin::Dream,
    }
}

fn is_dream_managed_kind(kind: RelationKind) -> bool {
    matches!(
        kind,
        RelationKind::Topical
            | RelationKind::Factual
            | RelationKind::Causal
            | RelationKind::Recurrent
            | RelationKind::DuplicateOf
            | RelationKind::Supersedes
    )
}

/// Stable representative preference used when the complete duplicate component is supplied.
pub fn duplicate_representative(memories: &[MemoryFact]) -> Option<MemoryId> {
    memories
        .iter()
        .filter(|m| !m.archived)
        .min_by_key(|m| {
            let state_rank = match m.lifecycle_state.as_str() {
                "canonical" => 0,
                "knowledge" => 1,
                "extracted" => 2,
                _ => 3,
            };
            (state_rank, m.source_time_ns.unwrap_or(i64::MAX), m.id)
        })
        .map(|m| m.id)
}

/// Walks only caller-supplied active duplicate relations and returns sorted IDs.
pub fn duplicate_component(source: MemoryId, relations: &[Relation]) -> Vec<MemoryId> {
    let mut adjacency: BTreeMap<MemoryId, Vec<MemoryId>> = BTreeMap::new();
    for r in relations
        .iter()
        .filter(|r| r.active && r.kind == RelationKind::DuplicateOf)
    {
        if let (NodeRef::Memory(a), NodeRef::Memory(b)) = (r.source, r.target) {
            adjacency.entry(MemoryId(a)).or_default().push(MemoryId(b));
            adjacency.entry(MemoryId(b)).or_default().push(MemoryId(a));
        }
    }
    let mut seen = BTreeSet::from([source]);
    let mut queue = VecDeque::from([source]);
    while let Some(id) = queue.pop_front() {
        for next in adjacency.get(&id).into_iter().flatten() {
            if seen.insert(*next) {
                queue.push_back(*next);
            }
        }
    }
    seen.into_iter().collect()
}

pub fn explicit_authority_promotes(
    authority_kind: &str,
    category: &str,
    memory_type: &str,
) -> bool {
    matches!(authority_kind, "direct" | "correction")
        && (matches!(
            category,
            "decision" | "preference" | "instruction" | "constraint" | "correction" | "commitment"
        ) || (category == "fact"
            && matches!(memory_type, "project" | "process" | "product" | "schedule")))
}

/// Derives deterministic lifecycle proposals from an already-authorized set of
/// superseded targets, duplicate status, and canonical candidates.
pub fn plan_lifecycle(
    source: &MemoryFact,
    superseded_targets: &[(MemoryId, Option<MemoryId>)],
    has_active_duplicate_peer: bool,
    canonical_candidates: &[MemoryFact],
    source_has_explicit_authority: bool,
) -> Vec<LifecycleIntent> {
    let mut intents = Vec::new();
    let mut archived = BTreeSet::new();
    let mut source_archived = source.archived;
    for (target, superseded_by) in superseded_targets {
        intents.push(LifecycleIntent {
            memory_id: *target,
            lifecycle_state: "archived".into(),
            archived: true,
            superseded_by: *superseded_by,
        });
        archived.insert(*target);
        if *target == source.id {
            source_archived = true;
        }
    }
    if !source_archived && source.lifecycle_state == "extracted" && has_active_duplicate_peer {
        intents.push(LifecycleIntent {
            memory_id: source.id,
            lifecycle_state: "archived".into(),
            archived: true,
            superseded_by: source.superseded_by,
        });
        archived.insert(source.id);
        source_archived = true;
    }
    let mut candidates: BTreeMap<_, _> = canonical_candidates.iter().map(|m| (m.id, m)).collect();
    if !source_archived && source_has_explicit_authority {
        candidates.insert(source.id, source);
    }
    let mut source_promoted = false;
    for m in candidates.into_values() {
        if m.archived || m.lifecycle_state == "canonical" || archived.contains(&m.id) {
            continue;
        }
        intents.push(LifecycleIntent {
            memory_id: m.id,
            lifecycle_state: "canonical".into(),
            archived: false,
            superseded_by: m.superseded_by,
        });
        source_promoted |= m.id == source.id;
    }
    if !source_promoted && !source_archived && source.lifecycle_state == "extracted" {
        intents.push(LifecycleIntent {
            memory_id: source.id,
            lifecycle_state: "knowledge".into(),
            archived: false,
            superseded_by: source.superseded_by,
        });
    }
    intents
}

pub const COMMUNITY_NAMING_CONTRACT_VERSION: u32 = 2;
pub const COMMUNITY_NAMING_REPRESENTATIVES: usize = 8;
pub const COMMUNITY_NAME_MAX_BYTES: usize = 96;
const COMMUNITY_TEXT_LIMIT: usize = 2048;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommunityNamingMemory {
    pub id: MemoryId,
    pub title: String,
    pub content: String,
}

/// Select representatives using centroid similarity and stable identity ties.
pub fn community_representative_ids(
    members: &[MemoryId],
    centroids: &[Vec<f32>],
    vectors: &BTreeMap<MemoryId, Vec<f32>>,
) -> Vec<MemoryId> {
    let mut selected = Vec::new();
    for centroid in centroids {
        let mut ranked: Vec<_> = members
            .iter()
            .filter_map(|id| Some((*id, reliquary_vectors::cosine(centroid, vectors.get(id)?)?)))
            .collect();
        ranked.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        let mut admitted = 0;
        for (id, _) in ranked {
            if !selected.contains(&id) {
                selected.push(id);
                admitted += 1;
            }
            if admitted == 2 || selected.len() == COMMUNITY_NAMING_REPRESENTATIVES {
                break;
            }
        }
        if selected.len() == COMMUNITY_NAMING_REPRESENTATIVES {
            break;
        }
    }
    let mut remaining: Vec<_> = members
        .iter()
        .filter_map(|id| {
            if selected.contains(id) {
                return None;
            }
            let vector = vectors.get(id)?;
            let score = centroids
                .iter()
                .filter_map(|centroid| reliquary_vectors::cosine(centroid, vector))
                .max_by(f64::total_cmp)?;
            Some((*id, score))
        })
        .collect();
    remaining.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    for (id, _) in remaining
        .into_iter()
        .take(COMMUNITY_NAMING_REPRESENTATIVES.saturating_sub(selected.len()))
    {
        selected.push(id);
    }
    for id in members {
        if selected.len() == COMMUNITY_NAMING_REPRESENTATIVES {
            break;
        }
        if !selected.contains(id) {
            selected.push(*id);
        }
    }
    selected.sort();
    selected
}

pub fn build_community_naming_payload(
    memories: &[CommunityNamingMemory],
) -> Result<String, String> {
    serde_json::to_string(&json!({
        "contract_version": COMMUNITY_NAMING_CONTRACT_VERSION,
        "representative_memories": memories.iter().enumerate().map(|(i, m)| json!({
            "ordinal": i + 1,
            "title": truncate_utf8(&m.title, COMMUNITY_TEXT_LIMIT / 4),
            "content": truncate_utf8(&m.content, COMMUNITY_TEXT_LIMIT),
        })).collect::<Vec<_>>()
    }))
    .map_err(|error| error.to_string())
}

pub fn parse_community_name(output: &Value) -> Result<String, String> {
    let raw = output
        .get("name")
        .and_then(Value::as_str)
        .ok_or("missing string field name")?;
    let name = raw.trim();
    if name.is_empty() {
        return Err("name is empty".into());
    }
    if name.len() > COMMUNITY_NAME_MAX_BYTES {
        return Err("name is too long".into());
    }
    if name.chars().any(char::is_control) {
        return Err("name contains control characters".into());
    }
    Ok(name.to_owned())
}

pub fn community_name_schema() -> Value {
    json!({ "type":"object", "additionalProperties":false, "required":["name"],
        "properties":{"name":{"type":"string", "minLength":1, "maxLength":COMMUNITY_NAME_MAX_BYTES}} })
}

fn truncate_utf8(value: &str, max_bytes: usize) -> &str {
    if value.len() <= max_bytes {
        return value;
    }
    let mut end = max_bytes;
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    &value[..end]
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fact(n: u8, state: &str, time: i64) -> MemoryFact {
        MemoryFact {
            id: MemoryId([n; 32]),
            lifecycle_state: state.into(),
            archived: false,
            superseded_by: None,
            source_time_ns: Some(time),
            authority_kind: "inferred".into(),
            category: "fact".into(),
            memory_type: "observation".into(),
        }
    }
    #[test]
    fn snapshot_plans_duplicate_chain_and_supersession_lifecycle_without_writes() {
        let memories = [fact(1, "knowledge", 10), fact(2, "extracted", 20)];
        let plan = evaluate_snapshot(
            &[
                PairDecision {
                    a: MemoryId([1; 32]),
                    b: MemoryId([2; 32]),
                    relation: PairRelation::DuplicateOf,
                    direction: PairDirection::Undirected,
                },
                PairDecision {
                    a: MemoryId([2; 32]),
                    b: MemoryId([1; 32]),
                    relation: PairRelation::Supersedes,
                    direction: PairDirection::AToB,
                },
            ],
            &memories,
            &[],
        );
        assert!(
            plan.relation_changes
                .iter()
                .any(|c| c.kind == RelationKind::DuplicateOf && c.active)
        );
        assert_eq!(
            plan.lifecycle_intents,
            vec![LifecycleIntent {
                memory_id: MemoryId([1; 32]),
                lifecycle_state: "archived".into(),
                archived: true,
                superseded_by: Some(MemoryId([2; 32]))
            }]
        );
    }
    #[test]
    fn component_and_representative_use_only_supplied_snapshot() {
        let rel = Relation {
            source: NodeRef::Memory([1; 32]),
            target: NodeRef::Memory([2; 32]),
            kind: RelationKind::DuplicateOf,
            active: true,
            origin: RelationOrigin::Dream,
            graph_version: 1,
        };
        assert_eq!(
            duplicate_component(MemoryId([2; 32]), &[rel]),
            vec![MemoryId([1; 32]), MemoryId([2; 32])]
        );
        assert_eq!(
            duplicate_representative(&[fact(2, "extracted", 1), fact(1, "knowledge", 99)]),
            Some(MemoryId([1; 32]))
        );
    }
    #[test]
    fn explicit_authority_gate_matches_pinned_policy() {
        assert!(explicit_authority_promotes(
            "direct",
            "decision",
            "observation"
        ));
        assert!(explicit_authority_promotes("correction", "fact", "project"));
        assert!(!explicit_authority_promotes(
            "inferred",
            "decision",
            "observation"
        ));
    }

    #[test]
    fn community_representatives_and_names_are_bounded_and_stable() {
        let members = vec![MemoryId([2; 32]), MemoryId([1; 32]), MemoryId([3; 32])];
        let vectors = BTreeMap::from([
            (MemoryId([1; 32]), vec![1.0, 0.0]),
            (MemoryId([2; 32]), vec![0.0, 1.0]),
            (MemoryId([3; 32]), vec![0.7, 0.7]),
        ]);
        assert_eq!(
            community_representative_ids(&members, &[vec![1.0, 0.0]], &vectors),
            vec![MemoryId([1; 32]), MemoryId([2; 32]), MemoryId([3; 32])]
        );
        assert_eq!(
            parse_community_name(&json!({"name":"  Project Atlas  "})).unwrap(),
            "Project Atlas"
        );
        assert!(parse_community_name(&json!({"name":"\n"})).is_err());
        let payload = build_community_naming_payload(&[CommunityNamingMemory {
            id: MemoryId([1; 32]),
            title: "T".into(),
            content: "x".repeat(3000),
        }])
        .unwrap();
        assert!(payload.len() < 2200);
        assert_eq!(
            community_name_schema()["properties"]["name"]["maxLength"],
            COMMUNITY_NAME_MAX_BYTES
        );
    }

    #[test]
    fn lifecycle_plan_promotes_only_authorized_candidates_and_advances_extracted_state() {
        let source = fact(1, "extracted", 1);
        let mut candidate = fact(2, "knowledge", 2);
        candidate.authority_kind = "direct".into();
        let plan = plan_lifecycle(&source, &[], false, std::slice::from_ref(&candidate), false);
        assert!(
            plan.iter()
                .any(|i| i.memory_id == candidate.id && i.lifecycle_state == "canonical")
        );
        assert!(
            plan.iter()
                .any(|i| i.memory_id == source.id && i.lifecycle_state == "knowledge")
        );
        let promoted = plan_lifecycle(&source, &[], false, std::slice::from_ref(&candidate), true);
        assert!(
            promoted
                .iter()
                .any(|i| i.memory_id == source.id && i.lifecycle_state == "canonical")
        );
        assert!(
            promoted
                .iter()
                .any(|i| i.memory_id == candidate.id && i.lifecycle_state == "canonical")
        );
    }
}

mod organization;
pub use organization::{DreamOrganizationError, plan_organization};
