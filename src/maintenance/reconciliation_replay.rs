//! Validated replay operations over the concrete REL constituents.
use crate::CvaReconcileError;
use crate::archive_ordered_replay::{ArchiveReplaySource, ArchiveTail, FileMemoryLinkReplay};
use crate::graph_ordered_replay::{GraphKey, GraphReplayResult, GraphTail, GraphTailReadError};
use crate::owner_lifecycle::OwnerError;
use crate::owner_lifecycle::ReliquaryOwner;
use crate::owner_lifecycle::archive_ordered_replay::ArchiveReplayDestination;
use crate::owner_lifecycle::graph_ordered_replay::GraphReplayDestination;
use std::collections::HashMap;

pub(crate) fn read_archive_tail(
    owner: &ReliquaryOwner,
    start: usize,
) -> Result<ArchiveTail, CvaReconcileError> {
    crate::archive_ordered_replay::read_archive_range(
        ArchiveReplaySource {
            container: &owner.container,
            archive: &owner.archive,
            project_files: &owner.project_files,
        },
        start,
        usize::MAX,
    )
    .map_err(Into::into)
}

pub(crate) fn replay_archive_tail(
    owner: &mut ReliquaryOwner,
    tail: &ArchiveTail,
) -> Result<usize, CvaReconcileError> {
    crate::owner_lifecycle::archive_ordered_replay::replay_archive_tail(
        ArchiveReplayDestination {
            container: &mut owner.container,
            archive: &mut owner.archive,
            project_files: &mut owner.project_files,
        },
        tail,
    )
}

pub(crate) fn replay_file_memory_links(
    owner: &mut ReliquaryOwner,
    links: &[FileMemoryLinkReplay],
) -> Result<usize, CvaReconcileError> {
    crate::owner_lifecycle::archive_ordered_replay::replay_file_memory_links(
        &mut owner.container,
        &mut owner.archive,
        &owner.memories,
        links,
    )
}

pub(crate) fn read_entity_tail(
    owner: &ReliquaryOwner,
    start: usize,
) -> Result<crate::entity_ordered_replay::EntityTail, CvaReconcileError> {
    crate::entity_ordered_replay::read_entity_range(
        &owner.container,
        &owner.entities,
        start,
        usize::MAX,
    )
    .map_err(Into::into)
}

pub(crate) fn replay_entity_tail(
    owner: &mut ReliquaryOwner,
    tail: crate::entity_ordered_replay::EntityTail,
) -> Result<crate::entity_ordered_replay::EntityReplayResult, CvaReconcileError> {
    crate::entity_ordered_replay::replay_entity_tail(
        &mut owner.container,
        &mut owner.entities,
        tail,
    )
    .map_err(Into::into)
}

pub(crate) fn read_relationship_tail(
    owner: &ReliquaryOwner,
    start: usize,
) -> Result<crate::relationship_ordered_replay::RelationshipTail, CvaReconcileError> {
    crate::relationship_ordered_replay::read_relationship_range(
        &owner.container,
        &owner.relationships,
        start,
        usize::MAX,
    )
    .map_err(Into::into)
}

pub(crate) fn replay_relationship_tail(
    owner: &mut ReliquaryOwner,
    tail: crate::relationship_ordered_replay::RelationshipTail,
) -> Result<crate::relationship_ordered_replay::RelationshipReplayResult, CvaReconcileError> {
    crate::relationship_ordered_replay::replay_relationship_tail(
        &mut owner.container,
        &mut owner.relationships,
        tail,
    )
    .map_err(Into::into)
}

pub(crate) fn read_memory_tail(
    owner: &mut ReliquaryOwner,
    start: usize,
) -> Result<crate::memory_ordered_replay::MemoryTail, CvaReconcileError> {
    crate::memory_ordered_replay::read_memory_range(
        crate::memory_ordered_replay::MemoryReplaySource {
            container: &mut owner.container,
            memories: &owner.memories,
        },
        start,
        usize::MAX,
    )
}

pub(crate) fn replay_memory_tail(
    owner: &mut ReliquaryOwner,
    tail: crate::memory_ordered_replay::MemoryTail,
) -> Result<crate::memory_ordered_replay::MemoryReplayResult, CvaReconcileError> {
    crate::memory_ordered_replay::replay_memory_tail(
        crate::memory_ordered_replay::MemoryReplayDestination {
            archive: &owner.archive,
            memories: &mut owner.memories,
            insomnia: &mut owner.insomnia,
            container: &mut owner.container,
        },
        tail,
    )
}

pub(crate) fn read_graph_tail(
    owner: &ReliquaryOwner,
    start: usize,
) -> Result<GraphTail, CvaReconcileError> {
    crate::graph_ordered_replay::read_graph_range(&owner.container, &owner.graph, start, usize::MAX)
        .map_err(|error| match error {
            GraphTailReadError::Graph(error) => CvaReconcileError::from(error),
            GraphTailReadError::InvalidGraphVersionRecord => {
                CvaReconcileError::InvalidGraphVersionRecord
            }
        })
}

pub(crate) fn validate_graph_tails(
    left: &GraphTail,
    right: &GraphTail,
) -> Result<HashMap<GraphKey, usize>, CvaReconcileError> {
    crate::graph_ordered_replay::replay_skips(left, right)
        .map_err(crate::owner_lifecycle::graph_ordered_replay::conflict_error)
}

pub(crate) fn replay_graph_tail(
    owner: &mut ReliquaryOwner,
    tail: &GraphTail,
) -> Result<GraphReplayResult, CvaReconcileError> {
    replay_graph_with_skips(owner, tail, &HashMap::new(), false)
}

pub(crate) fn replay_graph_with_skips(
    owner: &mut ReliquaryOwner,
    tail: &GraphTail,
    skips: &HashMap<GraphKey, usize>,
    preserve_import_postimages: bool,
) -> Result<GraphReplayResult, CvaReconcileError> {
    crate::owner_lifecycle::graph_ordered_replay::replay_with_skips(
        GraphReplayDestination {
            container: &mut owner.container,
            memories: &owner.memories,
            entities: &owner.entities,
            graph: &mut owner.graph,
        },
        tail,
        skips,
        preserve_import_postimages,
    )
}

pub(crate) fn merge_interaction_streams(
    left: Vec<crate::InteractionStreamRecord>,
    right: Vec<crate::InteractionStreamRecord>,
) -> Result<(Vec<crate::InteractionStreamRecord>, bool), CvaReconcileError> {
    crate::interaction_stream_reconciliation::merge_interaction_streams(left, right)
        .map_err(OwnerError::from)
        .map_err(Into::into)
}

pub(crate) fn replay_interaction_streams(
    owner: &mut ReliquaryOwner,
    records: Vec<crate::InteractionStreamRecord>,
) -> Result<(), CvaReconcileError> {
    crate::interaction_stream_reconciliation::replay_interaction_streams(
        &mut owner.interaction_streams,
        &mut owner.container,
        records,
    )
    .map_err(OwnerError::from)
    .map_err(Into::into)
}
