use crate::{Archive, CvaError, InitialDreamPass, MemoryId};
use reliquary_container::Container;
use std::collections::BTreeMap;

/// Replay Dream journal records in physical order once Memory and source
/// admission state are finalized. Current compact Begins bind to the prefix
/// immediately before their own frame; later roots and settlement records
/// then use the same resident journal projection.
pub(crate) fn replay_with_population(
    container: &Container,
    archive: &Archive,
    memories: &crate::memory_store::MemoryStore,
    journal: &mut crate::freshness_dream_journal::DreamFreshnessJournal,
) -> Result<(), CvaError> {
    replay_with_population_and_visit(container, archive, memories, journal, |_, _| Ok(()))
}

/// Replay the physical Dream stream, allowing a concrete owner coordinator to
/// observe each frame immediately before it changes the resident journal.
pub(crate) fn replay_with_population_and_visit(
    container: &Container,
    archive: &Archive,
    memories: &crate::memory_store::MemoryStore,
    journal: &mut crate::freshness_dream_journal::DreamFreshnessJournal,
    mut before_frame: impl FnMut(
        &[u8],
        &crate::freshness_dream_journal::DreamFreshnessJournal,
    ) -> Result<(), CvaError>,
) -> Result<(), CvaError> {
    let mut prefix = container
        .owner_uuid()
        .map(crate::cva_dream_population::DreamPopulationPrefix::new);
    let owner = container.owner_uuid();
    let end = container.physical_length()?;
    reliquary_owner_lifecycle::replay_prefix(container, end, |physical_frame| {
        let chunk = physical_frame.object;
        let payload = physical_frame.payload.as_slice();
        let global_frontier = physical_frame.latest_global;
        (|| -> Result<(), CvaError> {
            before_frame(payload, journal)?;
            if payload.starts_with(b"CVDREAM2") {
                let owner = owner.ok_or_else(|| {
                    CvaError::Freshness("CVDREAM2 Begin requires a versioned owner identity".into())
                })?;
                let record =
                    crate::dream_begin_codec::decode(payload).map_err(CvaError::Freshness)?;
                if record.owner_uuid != owner {
                    return Err(CvaError::Freshness(
                        "Dream Freshness pass owner mismatch".into(),
                    ));
                }
                let population = match record.population {
                    crate::dream_begin_codec::BeginPopulation::CompactCut {
                        memory_cut,
                        digest,
                    } => {
                        let prefix = prefix.as_ref().ok_or_else(|| {
                            CvaError::Freshness(
                                "CVDREAM2 Begin requires a versioned owner identity".into(),
                            )
                        })?;
                        let ids = prefix.materialize(
                            memories,
                            Some(owner),
                            memory_cut,
                            record.source_id,
                        )?;
                        if crate::dream_population_digest::digest(&ids) != digest {
                            return Err(CvaError::Freshness(
                                "Dream population digest differs from physical prefix".into(),
                            ));
                        }
                        ids
                    }
                    crate::dream_begin_codec::BeginPopulation::ExplicitSet(ids) => ids,
                };
                journal
                    .ingest_materialized_begin(
                        owner,
                        crate::InitialDreamPass {
                            pass_id: record.pass_id,
                            source_id: record.source_id,
                            first_revision: 1,
                            turn: record.turn,
                            accepted_settlement_turn: None,
                            preexisting_ids: population,
                            graph_version_before: record.graph_version_before,
                            roots: Default::default(),
                        },
                    )
                    .map_err(CvaError::Freshness)?;
            } else if payload.starts_with(b"CVDREAM") && !payload.starts_with(b"CVDREAM1") {
                return Err(CvaError::Freshness(
                    "unsupported Dream journal version".into(),
                ));
            } else {
                journal.ingest(payload).map_err(CvaError::Freshness)?;
            }
            if let Some(prefix) = prefix.as_mut() {
                prefix.ingest(archive, memories, chunk, payload, global_frontier)?;
            }
            Ok(())
        })()
    })
}

/// Reconstruct completed first-cycle proofs from the complete REL history.
/// This deliberately replays the actual legacy/current Begin authority before
/// collecting settled proofs, so CompactCut uses the same physical prefix
/// reconstruction as ordinary owner replay.
pub(crate) fn retained_completed_passes(
    container: &Container,
    archive: &Archive,
    memories: &crate::memory_store::MemoryStore,
) -> Result<BTreeMap<MemoryId, InitialDreamPass>, CvaError> {
    let owner = container.owner_uuid();
    if owner.is_none() {
        return Err(CvaError::Freshness(
            "historical Dream proof requires a durable REL owner".into(),
        ));
    }
    let scan = container;
    if scan.owner_uuid() != owner {
        return Err(CvaError::Freshness(
            "historical Dream proof owner changed during read".into(),
        ));
    }
    let mut journal = crate::freshness_dream_journal::DreamFreshnessJournal::new(owner);
    let mut completed = BTreeMap::new();
    replay_with_population_and_visit(scan, archive, memories, &mut journal, |payload, state| {
        let Some(pass_id) =
            crate::freshness_dream_journal::DreamFreshnessJournal::settled_pass_id(payload)
                .map_err(CvaError::Freshness)?
        else {
            return Ok(());
        };
        let Some(pass) = state.pending(&pass_id) else {
            return Ok(());
        };
        // A bare/replayed settled marker is not proof of an empty result.
        if state.accepted_graph_cut(&pass.pass_id).is_none() {
            return Ok(());
        }
        if let Some(old) = completed.insert(pass.source_id, pass.clone())
            && old != pass
        {
            return Err(CvaError::Freshness(
                "conflicting retained first-cycle proof".into(),
            ));
        }
        Ok(())
    })?;
    Ok(completed)
}
