//! Import-owned historical event preparation over accepted History/owner facts.
//! Freshness retains semantic/Dream planning; publication is a separate operation.

use crate::freshness::{
    historical_dream::{
        HistoricalDreamMemory, HistoricalDreamPlan, HistoricalFirstCycleProof,
        plan_historical_dream_events,
    },
    historical_planner::{HistoricalTimeline, MemoryAvailability, SourceKey},
    historical_propagation::HistoricalPropagationEvent,
    historical_semantic::{
        HistoricalSemanticPlan, HistoricalSemanticSource, HistoricalSemanticSourceVector,
        HistoricalSemanticTarget,
    },
    semantic_policy::{SemanticPolicy, SemanticVectorProfile},
};
use crate::history::manifest_encoding::{HistoryProofError, Value};
use crate::{MemoryId, owner_lifecycle::OwnerError};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct NativeHistoricalEvents {
    pub(crate) semantic: HistoricalSemanticPlan,
    pub(crate) dream: HistoricalDreamPlan,
    pub(crate) ordered: Vec<([u8; 32], HistoricalPropagationEvent)>,
}

#[derive(Clone, Debug)]
pub(crate) struct EventMemoryFacts {
    pub(crate) id: MemoryId,
    pub(crate) cohort_key: [u8; 32],
    pub(crate) source_episode: Option<[u8; 32]>,
    pub(crate) availability: Option<u64>,
    pub(crate) credit_eligible: bool,
    pub(crate) necessary_sources: BTreeSet<SourceKey>,
}

#[derive(Clone, Debug)]
pub(crate) struct EventFragmentFacts {
    pub(crate) id: [u8; 32],
    pub(crate) source_keys: BTreeSet<SourceKey>,
    pub(crate) evidence_available: bool,
    pub(crate) evidence_digest: [u8; 32],
    pub(crate) vectors: Vec<HistoricalSemanticSourceVector>,
}

#[derive(Clone, Debug)]
pub(crate) struct EventTargetFacts {
    pub(crate) id: MemoryId,
    pub(crate) body_id: [u8; 32],
    pub(crate) availability: Option<u64>,
    pub(crate) credit_eligible: bool,
    pub(crate) binding_id: [u8; 32],
    pub(crate) row: u64,
    pub(crate) row_digest: [u8; 32],
    pub(crate) vector: Vec<f32>,
}

pub(crate) fn merge_policy_cohorts(
    memories: &[EventMemoryFacts],
    publication_cohorts: &BTreeMap<MemoryId, BTreeSet<MemoryId>>,
    merge_episode_peers: bool,
) -> BTreeMap<MemoryId, [u8; 32]> {
    let mut keys: BTreeMap<_, _> = memories
        .iter()
        .map(|memory| (memory.id, memory.cohort_key))
        .collect();
    if !merge_episode_peers {
        return keys;
    }
    let mut parents: BTreeMap<_, _> = keys.keys().map(|id| (*id, *id)).collect();
    let mut episodes = BTreeMap::new();
    fn root(parents: &BTreeMap<MemoryId, MemoryId>, mut id: MemoryId) -> MemoryId {
        while parents[&id] != id {
            id = parents[&id];
        }
        id
    }
    for memory in memories {
        let mut peers = publication_cohorts
            .get(&memory.id)
            .cloned()
            .unwrap_or_default();
        if let Some(episode) = memory.source_episode {
            if let Some(peer) = episodes.insert(episode, memory.id) {
                peers.insert(peer);
            }
        }
        for peer in peers.into_iter().filter(|peer| keys.contains_key(peer)) {
            let a = root(&parents, memory.id);
            let b = root(&parents, peer);
            parents.insert(a.max(b), a.min(b));
        }
    }
    for (id, key) in &mut keys {
        *key = root(&parents, *id).0;
    }
    keys
}

pub(crate) fn scoped_fragment_sources(
    raw_keys: BTreeSet<(String, String)>,
    source_nodes: Option<&BTreeSet<(String, String)>>,
    position: u64,
    base_cut: Option<u64>,
) -> Result<Option<BTreeSet<SourceKey>>, OwnerError> {
    if base_cut.is_some_and(|cut| position <= cut) {
        return Ok(None);
    }
    if let Some(source_nodes) = source_nodes {
        let overlaps = raw_keys.iter().any(|key| source_nodes.contains(key));
        let fully_scoped =
            !raw_keys.is_empty() && raw_keys.iter().all(|key| source_nodes.contains(key));
        if overlaps && !fully_scoped {
            return Err(OwnerError::Freshness(
                "semantic fragment mixes import and external source nodes".into(),
            ));
        }
        if !fully_scoped {
            return Ok(None);
        }
    }
    Ok(Some(
        raw_keys
            .into_iter()
            .map(|(conversation_id, node_id)| SourceKey {
                conversation_id,
                node_id,
            })
            .collect(),
    ))
}

pub(crate) fn fragment_is_after_base(position: u64, base_cut: Option<u64>) -> bool {
    !base_cut.is_some_and(|cut| position <= cut)
}

pub(crate) fn credit_excluded_targets(
    source_keys: &BTreeSet<SourceKey>,
    memories: &[EventMemoryFacts],
    policy_cohorts: &BTreeMap<MemoryId, [u8; 32]>,
) -> BTreeSet<MemoryId> {
    let source_cohorts: BTreeSet<_> = memories
        .iter()
        .filter(|memory| {
            memory
                .necessary_sources
                .iter()
                .any(|key| source_keys.contains(key))
        })
        .map(|memory| policy_cohorts[&memory.id])
        .collect();
    memories
        .iter()
        .filter(|memory| source_cohorts.contains(&policy_cohorts[&memory.id]))
        .map(|memory| memory.id)
        .collect()
}

pub(crate) fn semantic_source(
    fragment: EventFragmentFacts,
    owner_id: &str,
    profile: SemanticVectorProfile,
    credit_excluded: BTreeSet<MemoryId>,
) -> HistoricalSemanticSource {
    HistoricalSemanticSource {
        fragment_id: fragment.id,
        source_keys: fragment.source_keys,
        owner_id: owner_id.to_owned(),
        profile,
        evidence_available: fragment.evidence_available,
        evidence_digest: fragment.evidence_digest,
        vectors: fragment.vectors,
        credit_excluded,
    }
}

pub(crate) fn semantic_target(
    facts: EventTargetFacts,
    owner_id: &str,
    profile: SemanticVectorProfile,
) -> Result<HistoricalSemanticTarget, OwnerError> {
    let digest = Value::map([
        ("memory_id", Value::bytes(facts.id.0)),
        ("body_id", Value::bytes(facts.body_id)),
        ("binding_id", Value::bytes(facts.binding_id)),
        ("profile_id", Value::bytes(profile.profile_id)),
        ("row", Value::Unsigned(facts.row)),
        ("row_digest", Value::bytes(facts.row_digest)),
    ])
    .digest("reliquary:historical-target-binding:v1")
    .map_err(proof_error)?;
    Ok(HistoricalSemanticTarget {
        memory_id: facts.id,
        owner_id: owner_id.to_owned(),
        availability: availability(facts.availability, facts.credit_eligible),
        lifecycle_eligible: facts.credit_eligible,
        profile,
        vector: facts.vector,
        vector_binding_id: facts.binding_id,
        evidence_digest: digest,
    })
}

pub(crate) fn first_cycle_proof(
    owner_uuid: [u8; 16],
    source_id: MemoryId,
    pass_id: &str,
    first_revision: u64,
    graph_version_before: u64,
    accepted_settlement_turn: Option<u64>,
    roots: &BTreeSet<MemoryId>,
    duplicate_origin: bool,
) -> Result<HistoricalFirstCycleProof, OwnerError> {
    let evidence = Value::map([
        ("owner_uuid", Value::bytes(owner_uuid)),
        ("source_id", Value::bytes(source_id.0)),
        ("pass_id", Value::text(pass_id)),
        ("first_revision", Value::Unsigned(first_revision)),
        (
            "graph_version_before",
            Value::Unsigned(graph_version_before),
        ),
        (
            "accepted_settlement_turn",
            accepted_settlement_turn.map_or(Value::Null, Value::Unsigned),
        ),
        (
            "roots",
            Value::Array(roots.iter().map(|id| Value::bytes(id.0)).collect()),
        ),
        ("duplicate_origin", Value::Bool(duplicate_origin)),
    ])
    .digest("reliquary:historical-first-cycle-evidence:v1")
    .map_err(proof_error)?;
    Ok(HistoricalFirstCycleProof::Completed {
        roots: roots.clone(),
        duplicate_origin,
        evidence_digest: evidence,
    })
}

pub(crate) fn validate_first_cycle_roots(
    pass_roots: &BTreeSet<MemoryId>,
    roots: &BTreeSet<MemoryId>,
) -> Result<(), OwnerError> {
    if roots != pass_roots {
        return Err(OwnerError::Freshness(
            "retained historical first-cycle roots conflict with accepted graph evidence".into(),
        ));
    }
    Ok(())
}

pub(crate) fn record_representatives(
    into: &mut BTreeMap<MemoryId, MemoryId>,
    additions: impl IntoIterator<Item = (MemoryId, MemoryId)>,
) -> Result<(), OwnerError> {
    for (root, representative) in additions {
        if into
            .insert(root, representative)
            .is_some_and(|old| old != representative)
        {
            return Err(OwnerError::Freshness(
                "conflicting historical representative authority".into(),
            ));
        }
    }
    Ok(())
}

pub(crate) fn unsupported_first_cycle() -> HistoricalFirstCycleProof {
    HistoricalFirstCycleProof::Unsupported
}

pub(crate) fn dream_memories(
    memories: &[EventMemoryFacts],
    policy_cohorts: &BTreeMap<MemoryId, [u8; 32]>,
) -> Vec<HistoricalDreamMemory> {
    memories
        .iter()
        .map(|memory| HistoricalDreamMemory {
            id: memory.id,
            cohort: policy_cohorts[&memory.id]
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect(),
            birth_position: memory.availability,
            credit_eligible: memory.credit_eligible,
        })
        .collect()
}

pub(crate) fn validate_dream_sources(
    dream_sources: &BTreeSet<MemoryId>,
    recipients: &BTreeSet<MemoryId>,
) -> Result<(), OwnerError> {
    if dream_sources.iter().any(|id| !recipients.contains(id)) {
        return Err(OwnerError::Freshness(
            "epoch-born Dream source is outside the recipient population".into(),
        ));
    }
    Ok(())
}

pub(crate) fn semantic_plan(
    owner: [u8; 16],
    owner_id: &str,
    timeline: &HistoricalTimeline,
    sources: Vec<HistoricalSemanticSource>,
    targets: Vec<HistoricalSemanticTarget>,
) -> Result<HistoricalSemanticPlan, OwnerError> {
    HistoricalSemanticPlan::plan(
        owner,
        owner_id,
        SemanticPolicy::default(),
        timeline,
        sources,
        targets,
    )
    .map_err(|error| OwnerError::Freshness(error.to_string()))
}

pub(crate) fn dream_plan(
    owner: [u8; 16],
    import_id: [u8; 32],
    memories: &[HistoricalDreamMemory],
    proofs: &BTreeMap<MemoryId, HistoricalFirstCycleProof>,
    representatives: &BTreeMap<MemoryId, MemoryId>,
) -> Result<HistoricalDreamPlan, OwnerError> {
    plan_historical_dream_events(owner, import_id, memories, proofs, &[], representatives)
        .map_err(|error| OwnerError::Freshness(error.to_string()))
}

pub(crate) fn ordered_events(
    semantic: &HistoricalSemanticPlan,
    dream: &HistoricalDreamPlan,
) -> Result<Vec<([u8; 32], HistoricalPropagationEvent)>, OwnerError> {
    let mut ordered = Vec::new();
    for event in &semantic.events {
        ordered.push((
            event.event_id,
            HistoricalPropagationEvent::from_semantic(event)
                .map_err(|error| OwnerError::Freshness(error.to_string()))?,
        ));
    }
    ordered.extend(
        dream
            .events
            .iter()
            .map(|event| (event.event_id, event.propagation.clone())),
    );
    ordered.sort_by_key(|(id, event)| (event.position, *id));
    let mut seen = BTreeSet::new();
    if ordered.iter().any(|(id, _)| !seen.insert(*id)) {
        return Err(OwnerError::Freshness(
            "conflicting canonical historical event identities".into(),
        ));
    }
    Ok(ordered)
}

pub(crate) fn native_events(
    semantic: HistoricalSemanticPlan,
    dream: HistoricalDreamPlan,
) -> Result<NativeHistoricalEvents, OwnerError> {
    let ordered = ordered_events(&semantic, &dream)?;
    Ok(NativeHistoricalEvents {
        semantic,
        dream,
        ordered,
    })
}

fn availability(position: Option<u64>, credit_eligible: bool) -> MemoryAvailability {
    match position {
        Some(position) if credit_eligible => MemoryAvailability::Supported { position },
        Some(_) => MemoryAvailability::CreditExcluded,
        None => MemoryAvailability::Unsupported,
    }
}

fn proof_error(error: HistoryProofError) -> OwnerError {
    OwnerError::Freshness(error.to_string())
}
