use crate::dream_duplicate_index::{DuplicateIndex, DuplicateTemporalKey};
use crate::dream_owner_publisher::{dream_may_change_relation, validate_dream_memory_changes};
use crate::graph_store::GraphStore;
use crate::memory_store::MemoryReadView;
use crate::{DreamPublicationError, DreamPublicationOutcome, GraphRelationKind, Memory, MemoryId};
use std::collections::{HashMap, HashSet};

pub(crate) fn prepare_duplicate_pair<R>(
    memories: &MemoryReadView<'_>,
    graph: &GraphStore,
    duplicate_index: &mut DuplicateIndex,
    a: MemoryId,
    b: MemoryId,
    expected_graph_version: u64,
    source_time: &R,
) -> Result<crate::dream_owner_publisher::DreamPublicationPlan, DreamPublicationError>
where
    R: Fn(&Memory) -> Option<i64>,
{
    if expected_graph_version != graph.graph_version() {
        return Err(crate::GraphError::RevisionConflict {
            expected: expected_graph_version,
            actual: graph.graph_version(),
        }
        .into());
    }
    ensure_duplicate_index(memories, graph, duplicate_index, source_time)?;
    let a_key = duplicate_key(memories, a, source_time)?;
    let b_key = duplicate_key(memories, b, source_time)?;
    let relations = graph.active_relations();
    let mut next_index = duplicate_index.clone();
    let mut changes = next_index.plan_union(a, a_key, b, b_key, &relations);
    let planned_changes = changes.len();
    changes.retain(|change| dream_may_change_relation(graph, *change));
    let authority_filtered = changes.len() != planned_changes;
    changes.sort_by_key(|change| {
        (
            change.source.0,
            change.target.0,
            change.kind.code(),
            change.active,
        )
    });
    if changes.is_empty() {
        return Ok(
            crate::dream_owner_publisher::DreamPublicationPlan::Complete(
                DreamPublicationOutcome::NoChange,
            ),
        );
    }
    validate_dream_memory_changes(memories, &changes)?;
    Ok(
        crate::dream_owner_publisher::DreamPublicationPlan::Changes {
            changes,
            duplicate: Some(crate::dream_owner_publisher::DuplicatePublication {
                next_index,
                authority_filtered,
            }),
        },
    )
}

pub(crate) fn finish_duplicate_publication<R>(
    memories: &MemoryReadView<'_>,
    graph: &GraphStore,
    duplicate_index: &mut DuplicateIndex,
    mut publication: crate::dream_owner_publisher::DuplicatePublication,
    source_time: &R,
) -> Result<(), DreamPublicationError>
where
    R: Fn(&Memory) -> Option<i64>,
{
    if publication.authority_filtered {
        ensure_duplicate_index(memories, graph, duplicate_index, source_time)?;
    } else {
        publication
            .next_index
            .set_memory_graph_version(graph.memory_graph_version());
        *duplicate_index = publication.next_index;
    }
    Ok(())
}

fn ensure_duplicate_index<R>(
    memories: &MemoryReadView<'_>,
    graph: &GraphStore,
    duplicate_index: &mut DuplicateIndex,
    source_time: &R,
) -> Result<(), DreamPublicationError>
where
    R: Fn(&Memory) -> Option<i64>,
{
    let graph_version = graph.memory_graph_version();
    if duplicate_index.memory_graph_version() == Some(graph_version) {
        return Ok(());
    }
    let relations = graph.active_relations();
    let ids: HashSet<_> = relations
        .iter()
        .filter(|relation| relation.kind == GraphRelationKind::DuplicateOf && relation.active)
        .flat_map(|relation| [relation.source, relation.target])
        .collect();
    let mut keys = HashMap::with_capacity(ids.len());
    for memory_id in ids {
        keys.insert(memory_id, duplicate_key(memories, memory_id, source_time)?);
    }
    *duplicate_index = DuplicateIndex::rebuild(graph_version, &relations, &keys);
    Ok(())
}

fn duplicate_key<R>(
    memories: &MemoryReadView<'_>,
    memory_id: MemoryId,
    source_time: &R,
) -> Result<DuplicateTemporalKey, DreamPublicationError>
where
    R: Fn(&Memory) -> Option<i64>,
{
    let memory = memories.memory(memory_id)?;
    let timestamp_ns =
        source_time(&memory).ok_or(DreamPublicationError::MissingSourceTimestamp(memory_id))?;
    Ok(DuplicateTemporalKey::new(timestamp_ns, memory_id))
}
