use super::*;
use reliquary_perception_store::PRINCIPAL_ENTITY_KIND;
use std::{collections::BTreeSet, path::Path};

mod archive_memory;
mod freshness_verify;
mod perception_graph;

#[derive(Default)]
pub(super) struct ReplayCounts {
    pub archive: usize,
    pub memory: usize,
    pub memory_duplicates: usize,
    pub entities: usize,
    pub relationships: usize,
    pub graph: usize,
    pub freshness: usize,
    pub semantic_change: bool,
}

pub(super) fn require_compatible_fixed_state(
    left: &RelSession,
    right: &RelSession,
) -> Result<(), RelReconciliationError> {
    if left.imports.statuses() != right.imports.statuses() {
        return Err(RelReconciliationError::UnsupportedState(
            "divergent native import settlement authority",
        ));
    }
    if left.common.communities.current() != right.common.communities.current() {
        return Err(RelReconciliationError::UnsupportedState(
            "divergent Community projection",
        ));
    }
    if left.common.ego.version() != right.common.ego.version() {
        return Err(RelReconciliationError::UnsupportedState(
            "divergent Ego state",
        ));
    }
    Ok(())
}

pub(super) fn replay_right(
    left: &RelSession,
    right: &RelSession,
    target_path: &Path,
) -> Result<ReplayCounts, RelReconciliationError> {
    let mut target =
        Container::open_writer_as(target_path, FileKind::Reliquary).map_err(verification)?;
    let mut counts = ReplayCounts::default();
    archive_memory::replay_archive(&mut target, &right.archive, &mut counts)?;
    archive_memory::replay_echo(&mut target, &right.echo, &mut counts)?;
    archive_memory::replay_memories(&mut target, &right.common.memories, &mut counts)?;
    perception_graph::replay_perception(&mut target, &right.common.perception, &mut counts)?;
    perception_graph::replay_graph(
        &mut target,
        &left.common.graph,
        &right.common.graph,
        &merged_graph_admission(left, right),
        &mut counts,
    )?;
    freshness_verify::replay_freshness(&mut target, &right.freshness, &mut counts)?;
    target.sync().map_err(verification)?;
    counts.semantic_change = counts.archive
        + counts.memory
        + counts.entities
        + counts.relationships
        + counts.graph
        + counts.freshness
        > 0;
    Ok(counts)
}

pub(super) fn merged_graph_admission(left: &RelSession, right: &RelSession) -> GraphAdmission {
    let memories = left
        .common
        .memories
        .memory_ids()
        .chain(right.common.memories.memory_ids())
        .map(|id| id.0)
        .collect::<BTreeSet<_>>();
    let mut entities = BTreeSet::new();
    let mut principals = BTreeSet::new();
    for store in [&left.common.perception, &right.common.perception] {
        for entity in store.entities() {
            entities.insert((entity.id.0, entity.kind == PRINCIPAL_ENTITY_KIND));
        }
        for id in store.historical_entities() {
            let principal = store
                .entity_revision(id, 1)
                .is_some_and(|entity| entity.kind == PRINCIPAL_ENTITY_KIND);
            entities.insert((id.0, principal));
            if principal {
                principals.insert(id.0);
            }
        }
    }
    GraphAdmission::for_replay(
        left.owner_id,
        memories.clone(),
        memories,
        entities,
        principals.clone(),
        principals,
    )
}

pub(super) fn verify_merged(
    left: &RelSession,
    right: &RelSession,
    output: &RelSession,
) -> Result<(), RelReconciliationError> {
    freshness_verify::verify_merged(left, right, output)
}

pub(super) fn has_vectors(left: &RelSession, right: &RelSession) -> bool {
    [&left.common, &right.common].into_iter().any(|common| {
        common.packed_vectors.stats().objects > 0 || common.memory_vectors.stats().objects > 0
    }) || left.archive_vectors.stats().objects > 0
        || right.archive_vectors.stats().objects > 0
        || left.generations.stats().generations > 0
        || right.generations.stats().generations > 0
}

pub(super) fn conflict(error: impl fmt::Display) -> RelReconciliationError {
    RelReconciliationError::SemanticConflict(error.to_string())
}

fn verification(error: impl fmt::Display) -> RelReconciliationError {
    RelReconciliationError::Verification(error.to_string())
}
