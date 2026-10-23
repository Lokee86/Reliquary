//! Read-only accepted Dream graph-root evidence at the producer's retained cut.
use crate::owner_lifecycle::{HistorySource, OwnerError};
use crate::{InitialDreamPass, MemoryId};
use std::collections::BTreeSet;

/// Narrow immutable access to retained Dream proof inputs.  Callers cannot
/// obtain the underlying stores or select a different read checkpoint.
pub(crate) struct DreamEvidenceReadAccess<'a> {
    history: &'a crate::owner_lifecycle::HistoryReadAccess<'a>,
    graph: &'a crate::graph_store::GraphStore,
    dream: &'a crate::freshness_dream_journal::DreamFreshnessJournal,
}

impl<'a> DreamEvidenceReadAccess<'a> {
    pub(crate) fn new(
        history: &'a crate::owner_lifecycle::HistoryReadAccess<'a>,
        graph: &'a crate::graph_store::GraphStore,
        dream: &'a crate::freshness_dream_journal::DreamFreshnessJournal,
    ) -> Self {
        Self {
            history,
            graph,
            dream,
        }
    }

    pub(crate) fn retained_completed(
        &self,
        archive: &crate::Archive,
        memories: &crate::memory_store::MemoryStore,
    ) -> Result<std::collections::BTreeMap<crate::MemoryId, crate::InitialDreamPass>, OwnerError>
    {
        self.history
            .retained_completed_dream_passes(archive, memories)
    }

    pub(crate) fn publication_cohort(
        &self,
        member: crate::MemoryId,
    ) -> Result<BTreeSet<crate::MemoryId>, OwnerError> {
        let source = self
            .history
            .snapshot()
            .map_err(|error| OwnerError::Container(error.0))?;
        crate::history::publication_birth_cohort(&source, member)
    }

    pub(crate) fn read_evidence(&self, pass: &crate::InitialDreamPass) -> DreamEvidenceReadOutcome {
        #[cfg(feature = "freshness-performance-trace")]
        let started = std::time::Instant::now();
        match self.history.snapshot() {
            Ok(source) => read_dream_evidence_diagnostic(&source, self.graph, self.dream, pass),
            Err(error) => DreamEvidenceReadOutcome {
                result: Err(OwnerError::Container(error.0)),
                #[cfg(feature = "freshness-performance-trace")]
                trace: DreamEvidenceTrace {
                    elapsed_ns: started.elapsed().as_nanos(),
                    payload_bytes: 0,
                    object_bytes: 0,
                    chunks: 0,
                    mutations: 0,
                    filtered: 0,
                },
            },
        }
    }

    pub(crate) fn representatives(
        &self,
        archive: &crate::Archive,
        memories: &crate::memory_store::MemoryStore,
        source_id: crate::MemoryId,
        roots: &BTreeSet<crate::MemoryId>,
    ) -> Result<std::collections::BTreeMap<crate::MemoryId, crate::MemoryId>, OwnerError> {
        crate::history::dream_representative::historical_dream_representatives(
            self.history,
            archive,
            memories,
            self.dream,
            &self.graph.active_relations(),
            source_id,
            roots,
        )
    }
}
pub(crate) struct CommittedDreamEvidence {
    pub(crate) roots: BTreeSet<MemoryId>,
    pub(crate) duplicate_origin: bool,
    #[cfg(feature = "freshness-performance-trace")]
    pub(crate) trace: DreamEvidenceTrace,
}
#[cfg(feature = "freshness-performance-trace")]
#[derive(Clone, Copy)]
pub(crate) struct DreamEvidenceTrace {
    pub(crate) elapsed_ns: u128,
    pub(crate) payload_bytes: u64,
    pub(crate) object_bytes: u64,
    pub(crate) chunks: u64,
    pub(crate) mutations: u64,
    pub(crate) filtered: u64,
}
pub(crate) struct DreamEvidenceReadOutcome {
    pub(crate) result: Result<CommittedDreamEvidence, OwnerError>,
    #[cfg(feature = "freshness-performance-trace")]
    pub(crate) trace: DreamEvidenceTrace,
}
pub(crate) fn read_dream_evidence(
    source: &HistorySource<'_>,
    graph: &crate::graph_store::GraphStore,
    dream: &crate::freshness_dream_journal::DreamFreshnessJournal,
    pass: &InitialDreamPass,
) -> Result<CommittedDreamEvidence, OwnerError> {
    read_dream_evidence_diagnostic(source, graph, dream, pass).result
}

fn read_dream_evidence_diagnostic(
    source: &HistorySource<'_>,
    graph: &crate::graph_store::GraphStore,
    dream: &crate::freshness_dream_journal::DreamFreshnessJournal,
    pass: &InitialDreamPass,
) -> DreamEvidenceReadOutcome {
    let mut roots = std::collections::BTreeSet::new();
    let mut duplicate_origin = false;
    #[cfg(feature = "freshness-performance-trace")]
    let trace_started = std::time::Instant::now();
    #[cfg(feature = "freshness-performance-trace")]
    let (
        mut trace_payload_bytes,
        mut trace_object_bytes,
        mut trace_chunks,
        mut trace_mutations,
        mut trace_filtered,
    ) = (0_u64, 0_u64, 0_u64, 0_u64, 0_u64);
    let scan_result = {
        let mut ingest_frame =
            |frame: crate::owner_lifecycle::HistoryFrame| -> Result<(), OwnerError> {
                let object = frame.object;
                let payload = frame.payload.as_slice();
                {
                    #[cfg(feature = "freshness-performance-trace")]
                    {
                        trace_payload_bytes += payload.len() as u64;
                        trace_chunks += 1;
                    }
                    let Some(version) =
                        crate::graph_compact_codec::publication::version_at(object, payload)?
                    else {
                        return Ok(());
                    };
                    let end = dream
                        .accepted_graph_cut(&pass.pass_id)
                        .map(|(graph, _)| graph)
                        .unwrap_or(graph.graph_version());
                    if version.graph_version <= pass.graph_version_before
                        || version.graph_version > end
                    {
                        #[cfg(feature = "freshness-performance-trace")]
                        {
                            trace_filtered += 1;
                        }
                        return Ok(());
                    }
                    let payload = source
                        .read_object(version.mutation)
                        .map_err(|error| OwnerError::Container(error.0))?;
                    #[cfg(feature = "freshness-performance-trace")]
                    {
                        trace_object_bytes += payload.len() as u64;
                    }
                    let mutations = graph.decode_stored_mutations(&payload)?.ok_or_else(|| {
                        OwnerError::Freshness(
                            "committed Dream graph version lacks its mutation".into(),
                        )
                    })?;
                    #[cfg(feature = "freshness-performance-trace")]
                    {
                        trace_mutations += mutations.len() as u64;
                    }
                    for mutation in mutations {
                        if mutation.origin != crate::GraphRelationOrigin::Dream || !mutation.active
                        {
                            continue;
                        }
                        let (Some(source), Some(target)) =
                            (mutation.source.as_memory(), mutation.target.as_memory())
                        else {
                            continue;
                        };
                        let neighbor = if source == pass.source_id {
                            Some(target)
                        } else if target == pass.source_id {
                            Some(source)
                        } else {
                            None
                        };
                        if let Some(neighbor) =
                            neighbor.filter(|id| pass.preexisting_ids.contains(id))
                        {
                            duplicate_origin |= mutation.kind.code()
                                == crate::GraphRelationKind::DuplicateOf.code();
                            roots.insert(neighbor);
                        }
                    }
                    Ok(())
                }
            };
        (|| {
            let mut cursor = source
                .cursor()
                .map_err(|error| OwnerError::Container(error.0))?;
            while let Some(frame) = cursor
                .next_frame()
                .map_err(|error| OwnerError::Container(error.0))?
            {
                ingest_frame(frame)?;
            }
            Ok::<(), OwnerError>(())
        })()
    };
    #[cfg(feature = "freshness-performance-trace")]
    let trace = DreamEvidenceTrace {
        elapsed_ns: trace_started.elapsed().as_nanos(),
        payload_bytes: trace_payload_bytes,
        object_bytes: trace_object_bytes,
        chunks: trace_chunks,
        mutations: trace_mutations,
        filtered: trace_filtered,
    };
    let result = scan_result.map(|()| CommittedDreamEvidence {
        roots,
        duplicate_origin,
        #[cfg(feature = "freshness-performance-trace")]
        trace,
    });
    DreamEvidenceReadOutcome {
        result,
        #[cfg(feature = "freshness-performance-trace")]
        trace,
    }
}
