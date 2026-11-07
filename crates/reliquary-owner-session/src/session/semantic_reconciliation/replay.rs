use super::*;
use reliquary_perception_store::PRINCIPAL_ENTITY_KIND;
use std::{collections::BTreeSet, path::Path};

mod archive_memory;
mod freshness_verify;
mod imports;
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

pub(super) struct ReplayOutcome {
    pub counts: ReplayCounts,
    pub destination_dream_admissions: Vec<DreamBeginAdmission>,
}

pub(super) fn require_compatible_fixed_state(
    left: &RelSession,
    right: &RelSession,
) -> Result<(), RelReconciliationError> {
    imports::validate(left, right)?;
    // Only completed native-import Episode journals are replayed through
    // destination owner admission. Live Insomnia work must not disappear when
    // a right-only operational tail is merged into the staged left owner.
    for episode in right.archive.episodes() {
        if episode.origin == reliquary_archive::EpisodeOrigin::Live {
            let id = WorkEpisodeId(episode.id.0);
            if let Some(right_work) = right.insomnia_work(id)
                && left.insomnia_work(id) != Some(right_work)
            {
                return Err(RelReconciliationError::UnsupportedState(
                    "divergent live Insomnia journal",
                ));
            }
        }
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
    right_container: &Container,
    target_path: &Path,
    left_dream_admissions: &[DreamBeginAdmission],
) -> Result<ReplayOutcome, RelReconciliationError> {
    let imported = imports::replay(
        left,
        right,
        right_container,
        target_path,
        left_dream_admissions,
    )?;
    let mut writer = RelWriter::open_path(
        target_path,
        merged_graph_admission(left, right),
        [],
        imported.destination_dream_admissions.iter().cloned(),
    )
    .map_err(verification)?;
    let mut counts = imported.counts;
    archive_memory::replay_archive(&mut writer.container, &right.archive, &mut counts)?;
    archive_memory::replay_echo(&mut writer.container, &right.echo, &mut counts)?;
    archive_memory::replay_memories(
        &mut writer.container,
        &right.common.memories,
        &mut writer.session.common.memories,
        &mut counts,
    )?;
    perception_graph::replay_perception(
        &mut writer.container,
        &right.common.perception,
        &mut counts,
    )?;
    perception_graph::replay_graph(
        &mut writer.container,
        &mut writer.session.common.graph,
        &left.common.graph,
        &right.common.graph,
        &merged_graph_admission(left, right),
        &mut counts,
    )?;
    freshness_verify::replay_freshness(
        &mut writer.container,
        &right.freshness,
        &mut writer.session.freshness,
        &imported.rebased_freshness,
        &mut counts,
    )?;
    writer.container.sync().map_err(verification)?;
    counts.semantic_change = counts.archive
        + counts.memory
        + counts.entities
        + counts.relationships
        + counts.graph
        + counts.freshness
        > 0;
    Ok(ReplayOutcome {
        counts,
        destination_dream_admissions: imported.destination_dream_admissions,
    })
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
    let rebased_freshness = imports::rebased_freshness(left, right)?;
    freshness_verify::verify_merged(left, right, output, &rebased_freshness)?;
    for source in [left, right] {
        for work in source.insomnia.work_items() {
            let id = work.episode_id;
            if output.insomnia_work(id) != Some(work)
                || output.insomnia.work_history_digest(id)
                    != source.insomnia.work_history_digest(id)
            {
                return Err(conflict(
                    "reconciliation lost retained Insomnia work-journal revisions",
                ));
            }
        }
        for expected in source.common.dreams.initial_settlements() {
            let actual = output
                .common
                .dreams
                .settlement_record(&expected.pass_id)
                .ok_or_else(|| conflict("reconciliation lost accepted initial Dream settlement"))?;
            if output
                .common
                .dreams
                .admission(&expected.pass_id)
                .map(|a| &a.preexisting_ids)
                != source
                    .common
                    .dreams
                    .admission(&expected.pass_id)
                    .map(|a| &a.preexisting_ids)
                || actual.source != expected.source
                || actual.revision != expected.revision
                || actual.body != expected.body
                || actual.roots != expected.roots
                || actual.credit_roots != expected.credit_roots
                || actual.admit != expected.admit
                || actual.memory_delta.is_some() != expected.memory_delta.is_some()
            {
                return Err(conflict(
                    "reconciliation changed initial Dream settlement semantics",
                ));
            }
        }
    }
    imports::verify(left, right, output)
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
