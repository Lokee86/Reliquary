use crate::dream_duplicate_index::{DuplicateIndex, DuplicateTemporalKey};
use crate::dream_owner_publisher::{dream_may_change_relation, validate_dream_memory_changes};
use crate::graph_store::GraphStore;
use crate::memory_store::MemoryReadView;
use crate::{
    DreamPublicationError, DreamPublicationOutcome, GraphRelation, GraphRelationKind, Memory,
    MemoryId,
};
use std::collections::{HashMap, HashSet};

pub(crate) enum DuplicatePlan {
    NoChange,
    Changes {
        changes: Vec<crate::GraphRelationChange>,
        publication: DuplicatePublication,
    },
}

pub(crate) struct DuplicatePublication {
    pub(crate) next_index: DuplicateIndex,
    pub(crate) authority_filtered: bool,
}

#[derive(Clone, Copy)]
enum RebuildPhase {
    Active,
    Pending(MemoryId),
    Finished,
}

/// Snapshot of precisely the IDs needed to rebuild the temporal duplicate index.
/// Each memory is loaded immediately before its source timestamp is requested.
pub(crate) struct DuplicateIndexSequence {
    graph_version: u64,
    relations: Vec<GraphRelation>,
    ids: Vec<MemoryId>,
    cursor: usize,
    keys: HashMap<MemoryId, DuplicateTemporalKey>,
    phase: RebuildPhase,
}

impl DuplicateIndexSequence {
    pub(crate) fn start(graph: &GraphStore, duplicate_index: &DuplicateIndex) -> Option<Self> {
        let graph_version = graph.memory_graph_version();
        if duplicate_index.memory_graph_version() == Some(graph_version) {
            return None;
        }
        let relations = graph.active_relations();
        // Preserve the original HashSet traversal order as a concrete snapshot.
        let ids: Vec<_> = relations
            .iter()
            .filter(|relation| relation.kind == GraphRelationKind::DuplicateOf && relation.active)
            .flat_map(|relation| [relation.source, relation.target])
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        Some(Self {
            graph_version,
            relations,
            keys: HashMap::with_capacity(ids.len()),
            ids,
            cursor: 0,
            phase: RebuildPhase::Active,
        })
    }

    pub(crate) fn advance(
        &mut self,
        memories: &MemoryReadView<'_>,
        duplicate_index: &mut DuplicateIndex,
    ) -> Result<Option<Memory>, DreamPublicationError> {
        if let RebuildPhase::Pending(id) = self.phase {
            return Err(DreamPublicationError::MissingSourceTimestamp(id));
        }
        if matches!(self.phase, RebuildPhase::Finished) {
            return Ok(None);
        }
        if let Some(id) = self.ids.get(self.cursor).copied() {
            let memory = memories.memory(id)?;
            self.phase = RebuildPhase::Pending(id);
            return Ok(Some(memory));
        }
        *duplicate_index = DuplicateIndex::rebuild(self.graph_version, &self.relations, &self.keys);
        self.phase = RebuildPhase::Finished;
        Ok(None)
    }

    pub(crate) fn resume(
        &mut self,
        memory_id: MemoryId,
        source_time: Option<i64>,
    ) -> Result<(), DreamPublicationError> {
        let RebuildPhase::Pending(id) = self.phase else {
            return Err(DreamPublicationError::MissingSourceTimestamp(memory_id));
        };
        if id != memory_id {
            return Err(DreamPublicationError::MissingSourceTimestamp(id));
        }
        let timestamp_ns = source_time.ok_or(DreamPublicationError::MissingSourceTimestamp(id))?;
        self.keys
            .insert(id, DuplicateTemporalKey::new(timestamp_ns, id));
        self.cursor += 1;
        self.phase = RebuildPhase::Active;
        Ok(())
    }
}

#[derive(Clone, Copy)]
enum PairPhase {
    Rebuild,
    A,
    B,
    Ready,
    PendingA,
    PendingB,
}

pub(crate) enum DuplicatePairProgress {
    SourceTime(Memory),
    Ready(DuplicatePlan),
}

pub(crate) struct DuplicateKeySequence {
    a: MemoryId,
    b: MemoryId,
    phase: PairPhase,
    rebuild: Option<DuplicateIndexSequence>,
    a_key: Option<DuplicateTemporalKey>,
    b_key: Option<DuplicateTemporalKey>,
}

impl DuplicateKeySequence {
    pub(crate) fn start(
        graph: &GraphStore,
        duplicate_index: &DuplicateIndex,
        a: MemoryId,
        b: MemoryId,
        expected_graph_version: u64,
    ) -> Result<Self, DreamPublicationError> {
        if expected_graph_version != graph.graph_version() {
            return Err(crate::GraphError::RevisionConflict {
                expected: expected_graph_version,
                actual: graph.graph_version(),
            }
            .into());
        }
        let rebuild = DuplicateIndexSequence::start(graph, duplicate_index);
        Ok(Self {
            a,
            b,
            phase: if rebuild.is_some() {
                PairPhase::Rebuild
            } else {
                PairPhase::A
            },
            rebuild,
            a_key: None,
            b_key: None,
        })
    }

    pub(crate) fn advance(
        &mut self,
        memories: &MemoryReadView<'_>,
        graph: &GraphStore,
        duplicate_index: &mut DuplicateIndex,
    ) -> Result<DuplicatePairProgress, DreamPublicationError> {
        loop {
            match self.phase {
                PairPhase::Rebuild => {
                    let rebuild = self.rebuild.as_mut().expect("rebuild phase");
                    if let Some(memory) = rebuild.advance(memories, duplicate_index)? {
                        return Ok(DuplicatePairProgress::SourceTime(memory));
                    }
                    self.rebuild = None;
                    self.phase = PairPhase::A;
                }
                PairPhase::A => {
                    let memory = memories.memory(self.a)?;
                    self.phase = PairPhase::PendingA;
                    return Ok(DuplicatePairProgress::SourceTime(memory));
                }
                PairPhase::B => {
                    let memory = memories.memory(self.b)?;
                    self.phase = PairPhase::PendingB;
                    return Ok(DuplicatePairProgress::SourceTime(memory));
                }
                PairPhase::PendingA => {
                    return Err(DreamPublicationError::MissingSourceTimestamp(self.a));
                }
                PairPhase::PendingB => {
                    return Err(DreamPublicationError::MissingSourceTimestamp(self.b));
                }
                PairPhase::Ready => {
                    let a_key = self.a_key.expect("A key resolved");
                    let b_key = self.b_key.expect("B key resolved");
                    let relations = graph.active_relations();
                    let mut next_index = duplicate_index.clone();
                    let mut changes =
                        next_index.plan_union(self.a, a_key, self.b, b_key, &relations);
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
                        return Ok(DuplicatePairProgress::Ready(DuplicatePlan::NoChange));
                    }
                    validate_dream_memory_changes(memories, &changes)?;
                    return Ok(DuplicatePairProgress::Ready(DuplicatePlan::Changes {
                        changes,
                        publication: DuplicatePublication {
                            next_index,
                            authority_filtered,
                        },
                    }));
                }
            }
        }
    }

    pub(crate) fn resume(
        &mut self,
        memory_id: MemoryId,
        source_time: Option<i64>,
    ) -> Result<(), DreamPublicationError> {
        match self.phase {
            PairPhase::Rebuild => {
                let rebuild = self.rebuild.as_mut().expect("rebuild phase");
                rebuild.resume(memory_id, source_time)
            }
            PairPhase::PendingA => {
                if memory_id != self.a {
                    return Err(DreamPublicationError::MissingSourceTimestamp(self.a));
                }
                let timestamp_ns =
                    source_time.ok_or(DreamPublicationError::MissingSourceTimestamp(self.a))?;
                self.a_key = Some(DuplicateTemporalKey::new(timestamp_ns, self.a));
                self.phase = PairPhase::B;
                Ok(())
            }
            PairPhase::PendingB => {
                if memory_id != self.b {
                    return Err(DreamPublicationError::MissingSourceTimestamp(self.b));
                }
                let timestamp_ns =
                    source_time.ok_or(DreamPublicationError::MissingSourceTimestamp(self.b))?;
                self.b_key = Some(DuplicateTemporalKey::new(timestamp_ns, self.b));
                self.phase = PairPhase::Ready;
                Ok(())
            }
            PairPhase::A => Err(DreamPublicationError::MissingSourceTimestamp(self.a)),
            PairPhase::B => Err(DreamPublicationError::MissingSourceTimestamp(self.b)),
            PairPhase::Ready => Err(DreamPublicationError::MissingSourceTimestamp(memory_id)),
        }
    }
}

pub(crate) fn finish_duplicate_publication(
    graph: &GraphStore,
    duplicate_index: &mut DuplicateIndex,
    mut publication: DuplicatePublication,
    published: Vec<crate::GraphRelation>,
) -> (Option<DuplicateIndexSequence>, DreamPublicationOutcome) {
    if publication.authority_filtered {
        let rebuild = DuplicateIndexSequence::start(graph, duplicate_index);
        (rebuild, DreamPublicationOutcome::Published(published))
    } else {
        publication
            .next_index
            .set_memory_graph_version(graph.memory_graph_version());
        *duplicate_index = publication.next_index;
        (None, DreamPublicationOutcome::Published(published))
    }
}
