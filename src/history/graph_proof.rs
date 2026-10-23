//! Canonical graph evidence assembled from accepted immutable owner facts.
use crate::{
    GraphRelationKind, InitialDreamPass, MemoryId, SemanticGraphRelation,
    history::manifest_encoding::Value, owner_lifecycle::OwnerError,
};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) struct GraphSelection {
    pub(crate) relationships: Value,
    pub(crate) neighbors: BTreeMap<MemoryId, BTreeSet<MemoryId>>,
    pub(crate) duplicates: BTreeSet<MemoryId>,
    excluded: BTreeMap<u8, u64>,
}

pub(crate) fn select(
    mut relations: Vec<SemanticGraphRelation>,
    ids: &BTreeSet<MemoryId>,
    scoped: bool,
    memories: &crate::memory_store::MemoryStore,
) -> Result<GraphSelection, OwnerError> {
    relations.sort_by_key(|r| (r.source, r.target, r.kind.code()));
    let mut typed = Vec::new();
    let mut neighbors = BTreeMap::<MemoryId, BTreeSet<MemoryId>>::new();
    let mut duplicates = BTreeSet::new();
    let mut excluded = BTreeMap::<u8, u64>::new();
    for relation in relations {
        let incident = [relation.source, relation.target]
            .iter()
            .filter_map(|node| node.as_memory())
            .any(|id| ids.contains(&id));
        if scoped && !incident {
            continue;
        }
        for node in [relation.source, relation.target] {
            if let Some(id) = node.as_memory() {
                if !memories.contains_memory(id) {
                    return Err(OwnerError::Freshness(
                        "dangling/foreign historical graph endpoint".into(),
                    ));
                }
            } else {
                *excluded.entry(node.kind.code()).or_default() += 1;
            }
        }
        if scoped
            && !matches!((relation.source.as_memory(), relation.target.as_memory()),
            (Some(a), Some(b)) if ids.contains(&a) && ids.contains(&b))
        {
            continue;
        }
        if let (Some(a), Some(b)) = (relation.source.as_memory(), relation.target.as_memory()) {
            if !scoped || (ids.contains(&a) && ids.contains(&b)) {
                neighbors.entry(a).or_default().insert(b);
                neighbors.entry(b).or_default().insert(a);
                if relation.kind.code() == GraphRelationKind::DuplicateOf.code() {
                    duplicates.insert(a);
                    duplicates.insert(b);
                }
            }
        } else if scoped {
            continue;
        }
        typed.push(relationship_value(relation));
    }
    Ok(GraphSelection {
        relationships: Value::Array(typed),
        neighbors,
        duplicates,
        excluded,
    })
}

fn relationship_value(relation: SemanticGraphRelation) -> Value {
    Value::map([
        (
            "source_kind",
            Value::Unsigned(u64::from(relation.source.kind.code())),
        ),
        ("source_id", Value::bytes(relation.source.id)),
        (
            "target_kind",
            Value::Unsigned(u64::from(relation.target.kind.code())),
        ),
        ("target_id", Value::bytes(relation.target.id)),
        ("kind", Value::Unsigned(u64::from(relation.kind.code()))),
        ("origin", Value::Unsigned(u64::from(relation.origin.code()))),
        ("global_version", Value::Unsigned(relation.global_version)),
        ("graph_version", Value::Unsigned(relation.graph_version)),
    ])
}

pub(crate) fn observe_supersession(
    selection: &mut GraphSelection,
    id: MemoryId,
    target: Option<MemoryId>,
    target_exists: bool,
    ids: &BTreeSet<MemoryId>,
    scoped: bool,
) -> Result<(), OwnerError> {
    if let Some(target) = target {
        if !target_exists {
            return Err(OwnerError::Freshness(
                "dangling/foreign supersession authority".into(),
            ));
        }
        if !scoped || ids.contains(&target) {
            selection.duplicates.insert(id);
            selection.duplicates.insert(target);
        }
    }
    Ok(())
}

pub(crate) fn authority_value(
    id: MemoryId,
    representative: MemoryId,
    body_id: crate::MemoryBodyId,
) -> Value {
    Value::map([
        ("memory_id", Value::bytes(id.0)),
        ("representative", Value::bytes(representative.0)),
        ("body_id", Value::bytes(body_id.0)),
    ])
}

pub(crate) fn validate_pass(
    id: MemoryId,
    pass: &InitialDreamPass,
    cohort: &BTreeSet<MemoryId>,
    ids: &BTreeSet<MemoryId>,
    turn_count: u64,
) -> Result<(), OwnerError> {
    if id != pass.source_id
        || !cohort.contains(&id)
        || pass.first_revision != 1
        || pass.turn > turn_count
        || pass.accepted_settlement_turn.is_none_or(|p| p > turn_count)
        || pass
            .preexisting_ids
            .iter()
            .chain(&pass.roots)
            .any(|id| !ids.contains(id))
    {
        return Err(OwnerError::Freshness(
            "invalid owner-local historical Dream proof".into(),
        ));
    }
    Ok(())
}

pub(crate) fn dream_value(
    id: MemoryId,
    pass: InitialDreamPass,
    duplicate_origin: bool,
    representatives: BTreeMap<MemoryId, MemoryId>,
) -> Value {
    Value::map([
        ("source_id", Value::bytes(id.0)),
        ("pass_id", Value::text(pass.pass_id)),
        ("first_revision", Value::Unsigned(pass.first_revision)),
        ("duplicate_origin", Value::Bool(duplicate_origin)),
        ("turn", Value::Unsigned(pass.turn)),
        (
            "settlement_turn",
            pass.accepted_settlement_turn
                .map_or(Value::Null, Value::Unsigned),
        ),
        (
            "graph_version_before",
            Value::Unsigned(pass.graph_version_before),
        ),
        (
            "preexisting_ids",
            Value::Array(
                pass.preexisting_ids
                    .into_iter()
                    .map(|id| Value::bytes(id.0))
                    .collect(),
            ),
        ),
        (
            "roots",
            Value::Array(pass.roots.iter().map(|id| Value::bytes(id.0)).collect()),
        ),
        (
            "representatives",
            Value::Array(
                representatives
                    .into_iter()
                    .map(|(id, rep)| {
                        Value::map([
                            ("root", Value::bytes(id.0)),
                            ("representative", Value::bytes(rep.0)),
                        ])
                    })
                    .collect(),
            ),
        ),
        (
            "mode",
            Value::text(if pass.roots.is_empty() {
                "verified-no-link"
            } else {
                "actual-first-cycle"
            }),
        ),
    ])
}

pub(crate) fn finalize(
    selection: GraphSelection,
    authorities: Vec<Value>,
    dream: Vec<Value>,
    graph_version: u64,
    memory_graph_version: u64,
) -> Result<(Value, BTreeMap<MemoryId, BTreeSet<MemoryId>>), OwnerError> {
    let adjacency = Value::Array(
        selection
            .neighbors
            .clone()
            .into_iter()
            .map(|(id, ns)| {
                Value::map([
                    ("memory_id", Value::bytes(id.0)),
                    (
                        "neighbors",
                        Value::Array(ns.into_iter().map(|n| Value::bytes(n.0)).collect()),
                    ),
                ])
            })
            .collect(),
    );
    let authority = Value::Array(authorities);
    let typed = selection.relationships;
    let value = Value::map([
        ("graph_version", Value::Unsigned(graph_version)),
        (
            "memory_graph_version",
            Value::Unsigned(memory_graph_version),
        ),
        ("relationships", typed.clone()),
        (
            "relationship_digest",
            Value::bytes(
                typed
                    .digest("reliquary:historical-relationships:v1")
                    .map_err(|e| OwnerError::Freshness(e.to_string()))?,
            ),
        ),
        ("adjacency", adjacency.clone()),
        (
            "adjacency_digest",
            Value::bytes(
                adjacency
                    .digest("reliquary:historical-adjacency:v1")
                    .map_err(|e| OwnerError::Freshness(e.to_string()))?,
            ),
        ),
        ("duplicate_authority", authority.clone()),
        (
            "duplicate_authority_digest",
            Value::bytes(
                authority
                    .digest("reliquary:historical-duplicate-authority:v1")
                    .map_err(|e| OwnerError::Freshness(e.to_string()))?,
            ),
        ),
        ("dream_proof", Value::Array(dream)),
        (
            "excluded_endpoint_kinds",
            Value::Array(
                selection
                    .excluded
                    .into_iter()
                    .map(|(kind, count)| {
                        Value::map([
                            ("kind", Value::Unsigned(u64::from(kind))),
                            ("count", Value::Unsigned(count)),
                        ])
                    })
                    .collect(),
            ),
        ),
    ]);
    Ok((value, selection.neighbors))
}

fn graph_admission_failure(message: &str) -> OwnerError {
    OwnerError::Freshness(message.into())
}
/// Assemble the retained graph admission proof without writer or facade access.
pub(crate) struct GraphAdmissionProof {
    pub(crate) result: Result<(Value, crate::freshness::MemoryAdjacency), OwnerError>,
    #[cfg(feature = "freshness-performance-trace")]
    pub(crate) traces: Vec<crate::history::dream_evidence::DreamEvidenceTrace>,
}
pub(crate) fn read_graph_admission(
    access: &crate::owner_lifecycle::HistoryReadAccess<'_>,
    archive: &crate::Archive,
    memories: &crate::memory_store::MemoryStore,
    births: &crate::freshness_dream_journal::DreamFreshnessJournal,
    graph: &crate::graph_store::GraphStore,
    ids: &BTreeSet<MemoryId>,
    cohort: &BTreeSet<MemoryId>,
    scoped: bool,
) -> GraphAdmissionProof {
    #[cfg(feature = "freshness-performance-trace")]
    let mut traces = Vec::new();
    let result = (|| -> Result<(Value, crate::freshness::MemoryAdjacency), OwnerError> {
        use crate::history::graph_proof as proof;
        let mut selection =
            proof::select(graph.active_semantic_relations(), ids, scoped, memories)?;
        for &id in ids {
            let target = access.current_memory(memories, id)?.superseded_by;
            let exists = target.is_none_or(|target| memories.contains_memory(target));
            proof::observe_supersession(&mut selection, id, target, exists, ids, scoped)?;
        }
        let mut authorities = Vec::new();
        for id in selection.duplicates.iter().copied() {
            let representative =
                crate::history::dream_representative::historical_duplicate_representative(
                    access,
                    archive,
                    memories,
                    births,
                    id,
                    &graph.active_relations(),
                )
                .map_err(|e| graph_admission_failure(&e.to_string()))?
                .ok_or_else(|| {
                    graph_admission_failure("unresolved historical duplicate authority")
                })?;
            let memory = access.current_memory(memories, representative)?;
            if memory.archived
                || memory.superseded_by.is_some()
                || !matches!(memory.lifecycle_state.as_str(), "knowledge" | "canonical")
            {
                return Err(graph_admission_failure(
                    "unresolved historical duplicate authority",
                ));
            }
            let body_id = memories.current_body_id(representative)?;
            authorities.push(proof::authority_value(id, representative, body_id));
        }
        let passes = access.retained_completed_dream_passes(archive, memories)?;
        let turn_count = archive.rel_turn_count();
        let mut dream = Vec::new();
        for (id, pass) in passes {
            if !cohort.contains(&id) {
                continue;
            }
            proof::validate_pass(id, &pass, cohort, ids, turn_count)?;
            let (roots, duplicate_origin) = {
                let source = access
                    .snapshot()
                    .map_err(|error| OwnerError::Container(error.0))?;
                let proof = crate::history::dream_evidence::read_dream_evidence(
                    &source, graph, births, &pass,
                )?;
                {
                    #[cfg(feature = "freshness-performance-trace")]
                    traces.push(proof.trace);
                    (proof.roots, proof.duplicate_origin)
                }
            };
            if roots != pass.roots {
                return Err(graph_admission_failure(
                    "retained first-cycle roots conflict with accepted graph proof",
                ));
            }
            let representatives =
                crate::history::dream_representative::historical_dream_representatives(
                    access,
                    archive,
                    memories,
                    births,
                    &graph.active_relations(),
                    id,
                    &pass.roots,
                )?;
            dream.push(proof::dream_value(
                id,
                pass,
                duplicate_origin,
                representatives,
            ));
        }
        let (value, adjacency) = proof::finalize(
            selection,
            authorities,
            dream,
            graph.graph_version(),
            graph.memory_graph_version(),
        )
        .map_err(OwnerError::from)?;
        Ok((value, adjacency))
    })();
    GraphAdmissionProof {
        result,
        #[cfg(feature = "freshness-performance-trace")]
        traces,
    }
}
