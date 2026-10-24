use crate::dream_canonical::{canonical_superseders, duplicate_component_has_active_peer};
use crate::graph_store::GraphStore;
use crate::memory_store::MemoryOwnerWriter;
use crate::memory_store::lifecycle_update::{
    MemoryLifecycleUpdate, apply_memory_lifecycle_updates,
};
use crate::{DreamLifecycleError, DreamLifecycleResult, GraphRelationKind, Memory, MemoryId};

/// Dream computes lifecycle policy from owner facts, then sends ordered passive
/// updates through a concrete Memory writer view. It never receives a Container
/// handle or a callback into runtime composition.
pub(crate) fn reconcile_phy_lifecycle(
    owner: &mut MemoryOwnerWriter<'_>,
    graph: &GraphStore,
    source_id: MemoryId,
) -> Result<DreamLifecycleResult, DreamLifecycleError> {
    let relations = graph.active_relations();
    let mut revised = Vec::new();
    let mut archived = Vec::new();
    let mut canonicalized = canonical_superseders(owner, source_id, &relations)?;

    let mut superseded_targets: Vec<_> = relations
        .iter()
        .filter(|relation| {
            relation.kind == GraphRelationKind::Supersedes
                && (relation.source == source_id || relation.target == source_id)
        })
        .map(|relation| relation.target)
        .collect();
    superseded_targets.sort_by_key(|id| id.0);
    superseded_targets.dedup();

    let changes = superseded_targets
        .iter()
        .map(|target| {
            update(
                graph,
                *target,
                "archived",
                true,
                crate::graph_relation_sets::unique_superseder(&relations, *target),
            )
        })
        .collect::<Vec<_>>();
    for (target, memory) in superseded_targets
        .iter()
        .copied()
        .zip(apply(owner, &changes)?)
    {
        if let Some(memory) = memory {
            archived.push(target);
            revised.push(memory);
        }
    }

    let source = owner.memory(source_id)?;
    if !source.archived
        && source.lifecycle_state == "extracted"
        && duplicate_component_has_active_peer(owner, source_id, &relations)?
    {
        let change = update(graph, source_id, "archived", true, source.superseded_by);
        if let Some(memory) = apply_one(owner, change)? {
            archived.push(source_id);
            revised.push(memory);
        }
    }

    let source = owner.memory(source_id)?;
    if !source.archived && crate::dream_canonical::explicit_authority_promotes(&source) {
        canonicalized.push(source_id);
    }
    canonicalized.sort_by_key(|id| id.0);
    canonicalized.dedup();

    let mut promoted = Vec::new();
    for id in canonicalized {
        let memory = owner.memory(id)?;
        if memory.archived || memory.lifecycle_state == "canonical" {
            continue;
        }
        let change = update(graph, id, "canonical", false, memory.superseded_by);
        if let Some(memory) = apply_one(owner, change)? {
            revised.push(memory);
            promoted.push(id);
        }
    }

    let source = owner.memory(source_id)?;
    let promoted_to_knowledge = if !source.archived && source.lifecycle_state == "extracted" {
        let change = update(graph, source_id, "knowledge", false, source.superseded_by);
        if let Some(memory) = apply_one(owner, change)? {
            revised.push(memory);
            true
        } else {
            false
        }
    } else {
        false
    };

    archived.sort_by_key(|id| id.0);
    archived.dedup();
    promoted.sort_by_key(|id| id.0);
    Ok(DreamLifecycleResult {
        source: owner.memory(source_id)?,
        revised,
        archived,
        canonicalized: promoted,
        promoted_to_knowledge,
    })
}

fn apply(
    owner: &mut MemoryOwnerWriter<'_>,
    updates: &[MemoryLifecycleUpdate],
) -> Result<Vec<Option<Memory>>, DreamLifecycleError> {
    Ok(apply_memory_lifecycle_updates(owner, updates)?)
}

fn apply_one(
    owner: &mut MemoryOwnerWriter<'_>,
    update: MemoryLifecycleUpdate,
) -> Result<Option<Memory>, DreamLifecycleError> {
    Ok(apply(owner, &[update])?
        .into_iter()
        .next()
        .expect("one update"))
}

fn update(
    graph: &GraphStore,
    id: MemoryId,
    lifecycle_state: &str,
    archived: bool,
    superseded_by: Option<MemoryId>,
) -> MemoryLifecycleUpdate {
    MemoryLifecycleUpdate {
        id,
        lifecycle_state: lifecycle_state.into(),
        archived,
        superseded_by,
        mutation_id: lifecycle_mutation_id(
            id,
            lifecycle_state,
            superseded_by,
            graph.graph_version(),
        ),
    }
}

fn lifecycle_mutation_id(
    id: MemoryId,
    lifecycle_state: &str,
    superseded_by: Option<MemoryId>,
    graph_version: u64,
) -> String {
    let superseder = superseded_by
        .map(|value| bytes_hex(&value.0))
        .unwrap_or_else(|| "none".into());
    format!(
        "dream-lifecycle:{}:{graph_version}:{lifecycle_state}:{superseder}",
        bytes_hex(&id.0)
    )
}

fn bytes_hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write;
        let _ = write!(&mut out, "{byte:02x}");
    }
    out
}
