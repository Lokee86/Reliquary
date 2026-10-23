//! Read-only accepted Dream graph-root evidence at the producer's retained cut.
use crate::owner_lifecycle::{HistorySource, OwnerError};
use crate::{InitialDreamPass, MemoryId};
use std::collections::BTreeSet;
pub(crate) struct CommittedDreamEvidence {
    pub(crate) roots: BTreeSet<MemoryId>,
    pub(crate) duplicate_origin: bool,
    #[cfg(feature = "freshness-performance-trace")]
    pub(crate) trace: DreamEvidenceTrace,
}
#[cfg(feature = "freshness-performance-trace")]
pub(crate) struct DreamEvidenceTrace {
    pub(crate) elapsed_ns: u128,
    pub(crate) payload_bytes: u64,
    pub(crate) object_bytes: u64,
    pub(crate) chunks: u64,
    pub(crate) mutations: u64,
    pub(crate) filtered: u64,
}
pub(crate) fn read_dream_evidence(
    source: &HistorySource<'_>,
    graph: &crate::graph_store::GraphStore,
    dream: &crate::freshness_dream_journal::DreamFreshnessJournal,
    pass: &InitialDreamPass,
) -> Result<CommittedDreamEvidence, OwnerError> {
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
    {
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
        let mut cursor = source
            .cursor()
            .map_err(|error| OwnerError::Container(error.0))?;
        (|| {
            while let Some(frame) = cursor
                .next_frame()
                .map_err(|error| OwnerError::Container(error.0))?
            {
                ingest_frame(frame)?;
            }
            Ok::<(), OwnerError>(())
        })()
    }?;
    Ok(CommittedDreamEvidence {
        roots,
        duplicate_origin,
        #[cfg(feature = "freshness-performance-trace")]
        trace: DreamEvidenceTrace {
            elapsed_ns: trace_started.elapsed().as_nanos(),
            payload_bytes: trace_payload_bytes,
            object_bytes: trace_object_bytes,
            chunks: trace_chunks,
            mutations: trace_mutations,
            filtered: trace_filtered,
        },
    })
}
