use super::*;
use std::collections::BTreeSet;

pub(super) fn common_history(
    common: &CommonState,
    generations: Option<&VectorGenerationStore>,
    cut: HistoricalOwnerCut,
) -> Result<CommonHistory, HistoricalProofError> {
    if cut.memory_version > common.memories.memory_version() {
        return Err(HistoricalProofError::FutureCut("Memory"));
    }
    let mut memories = Vec::new();
    let mut unavailable_payloads = 0;
    for id in common.memories.memory_ids() {
        let mut history = common.memories.history(id).map_err(invalid)?;
        history
            .revisions
            .retain(|revision| revision.record.memory_version <= cut.memory_version);
        if history.revisions.is_empty() {
            continue;
        }
        if history
            .tombstone
            .is_some_and(|tombstone| tombstone.memory_version > cut.memory_version)
        {
            history.tombstone = None;
        }
        unavailable_payloads += history
            .revisions
            .iter()
            .filter(|revision| {
                matches!(
                    revision.payload,
                    reliquary_memory_core::PayloadState::Absent
                )
            })
            .count();
        memories.push((id, history));
    }
    let perception = common
        .perception
        .history_at(cut.perception_version)
        .map_err(|_| HistoricalProofError::FutureCut("Perception"))?;
    let graph = common
        .graph
        .relations_at(cut.graph_version)
        .map_err(|_| HistoricalProofError::FutureCut("Graph"))?;
    let graph_history = common
        .graph
        .history_at(cut.graph_version)
        .map_err(|_| HistoricalProofError::FutureCut("Graph"))?;
    let community_history = common
        .communities
        .history_at_graph_version(cut.graph_version);
    let community_renames = common
        .communities
        .rename_history()
        .iter()
        .take_while(|receipt| receipt.request.graph_version <= cut.graph_version)
        .cloned()
        .collect();
    let ego = common
        .ego
        .history_at(cut.ego_version)
        .map_err(|_| HistoricalProofError::FutureCut("Ego"))?;
    let generation_history = match generations {
        Some(store) => {
            if cut.vector_version > store.vector_version() {
                return Err(HistoricalProofError::FutureCut("vector"));
            }
            store
                .generations()
                .iter()
                .filter(|generation| generation.vector_version <= cut.vector_version)
                .copied()
                .collect()
        }
        None if cut.vector_version == 0 => Vec::new(),
        None => return Err(HistoricalProofError::FutureCut("vector")),
    };
    unavailable_payloads += perception.entity_description_prunes.len()
        + perception.relationship_description_prunes.len();
    let history = CommonHistory {
        memories,
        perception,
        graph_history,
        graph,
        community_history,
        community_renames,
        ego,
        generations: generation_history,
        unavailable_payloads,
    };
    validate_common_cut(&history)?;
    Ok(history)
}

fn validate_common_cut(history: &CommonHistory) -> Result<(), HistoricalProofError> {
    let memories = history
        .memories
        .iter()
        .filter(|(_, history)| history.tombstone.is_none())
        .map(|(id, _)| id.0)
        .collect::<BTreeSet<_>>();
    let entities = history
        .perception
        .entities
        .iter()
        .map(|entity| entity.id.0)
        .collect::<BTreeSet<_>>();
    if history
        .graph
        .iter()
        .flat_map(|relation| [relation.source, relation.target])
        .any(|node| match node {
            NodeRef::Memory(id) => !memories.contains(&id),
            NodeRef::Entity(id) => !entities.contains(&id),
        })
    {
        return Err(HistoricalProofError::InconsistentCut(
            "Graph endpoint is absent at the selected Memory/Perception cut",
        ));
    }
    let memory_version = history
        .memories
        .iter()
        .flat_map(|(_, value)| value.revisions.iter())
        .map(|revision| revision.record.memory_version)
        .max()
        .unwrap_or(0);
    if history
        .ego
        .personality
        .as_ref()
        .is_some_and(|value| value.source_memory_version > memory_version)
    {
        return Err(HistoricalProofError::InconsistentCut(
            "Ego projection exceeds the selected Memory cut",
        ));
    }
    Ok(())
}

pub(super) fn validate_rel_cut(
    common: &CommonHistory,
    archive: &reliquary_archive::ArchiveHistorySnapshot,
    freshness: &BTreeMap<FreshnessMemoryId, FreshnessRecord>,
) -> Result<(), HistoricalProofError> {
    let memories = common
        .memories
        .iter()
        .map(|(id, _)| id.0)
        .collect::<BTreeSet<_>>();
    if freshness.keys().any(|id| !memories.contains(&id.0)) {
        return Err(HistoricalProofError::InconsistentCut(
            "Freshness references an unavailable Memory",
        ));
    }
    if common
        .generations
        .iter()
        .any(|generation| generation.source_archive_version > archive.archive_version)
    {
        return Err(HistoricalProofError::InconsistentCut(
            "vector generation exceeds the selected Archive cut",
        ));
    }
    Ok(())
}

pub(super) fn common_count(history: &CommonHistory) -> usize {
    history
        .memories
        .iter()
        .map(|(_, memory)| memory.revisions.len() + usize::from(memory.tombstone.is_some()))
        .sum::<usize>()
        + history.perception.entity_revisions.len()
        + history.perception.entity_tombstones.len()
        + history.perception.entity_description_prunes.len()
        + history.perception.mention_revisions.len()
        + history.perception.relationship_revisions.len()
        + history.perception.relationship_tombstones.len()
        + history.perception.relationship_description_prunes.len()
        + history.graph_history.len()
        + history.community_history.len()
        + history.community_renames.len()
        + history.ego.records.len()
        + history.generations.len()
}

fn invalid(error: impl fmt::Display) -> HistoricalProofError {
    HistoricalProofError::InvalidState(error.to_string())
}
