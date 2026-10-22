use super::OwnerError;
use crate::{Archive, Container, InitialDreamPass, MemoryId};
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
) -> Result<(), OwnerError> {
    replay_population(container, archive, memories, journal, None)
}

/// Replay physical Dream records, optionally collecting the concrete retained
/// completed-pass evidence immediately before each journal transition.
fn replay_population(
    container: &Container,
    archive: &Archive,
    memories: &crate::memory_store::MemoryStore,
    journal: &mut crate::freshness_dream_journal::DreamFreshnessJournal,
    mut completed: Option<&mut BTreeMap<MemoryId, InitialDreamPass>>,
) -> Result<(), OwnerError> {
    let mut prefix = container
        .owner_uuid()
        .map(crate::owner_lifecycle::DreamPopulationPrefix::new);
    let owner = container.owner_uuid();
    let end = container.physical_length()?;
    {
        let mut ingest_frame =
            |frame: crate::container::PhysicalFrame| -> Result<(), super::OwnerError> {
                let chunk = frame.object;
                let payload = frame.payload.as_slice();
                let global_frontier = frame.latest_global;
                {
                    if let Some(completed) = completed.as_deref_mut() {
                        observe_completed_frame(payload, journal, completed)?;
                    }
                    if payload.starts_with(b"CVDREAM2") {
                        let owner = owner.ok_or_else(|| {
                            OwnerError::Freshness(
                                "CVDREAM2 Begin requires a versioned owner identity".into(),
                            )
                        })?;
                        let record = crate::dream_begin_codec::decode(payload)
                            .map_err(OwnerError::Freshness)?;
                        if record.owner_uuid != owner {
                            return Err(OwnerError::Freshness(
                                "Dream Freshness pass owner mismatch".into(),
                            ));
                        }
                        let population = match record.population {
                            crate::dream_begin_codec::BeginPopulation::CompactCut {
                                memory_cut,
                                digest,
                            } => {
                                let prefix = prefix.as_ref().ok_or_else(|| {
                                    OwnerError::Freshness(
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
                                    return Err(OwnerError::Freshness(
                                        "Dream population digest differs from physical prefix"
                                            .into(),
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
                            .map_err(OwnerError::Freshness)?;
                    } else if payload.starts_with(b"CVDREAM") && !payload.starts_with(b"CVDREAM1") {
                        return Err(OwnerError::Freshness(
                            "unsupported Dream journal version".into(),
                        ));
                    } else {
                        journal.ingest(payload).map_err(OwnerError::Freshness)?;
                    }
                    if let Some(prefix) = prefix.as_mut() {
                        prefix.ingest(archive, memories, chunk, payload, global_frontier)?;
                    }
                    Ok(())
                }
            };
        let mut cursor = container.physical_cursor_through(end)?;
        (|| {
            while let Some(frame) = cursor.next_frame()? {
                ingest_frame(frame)?;
            }
            Ok::<(), super::OwnerError>(())
        })()
    }
}

/// Reconstruct completed first-cycle proofs from the complete REL history.
/// This deliberately replays the actual legacy/current Begin authority before
/// collecting settled proofs, so CompactCut uses the same physical prefix
/// reconstruction as ordinary owner replay.
pub(crate) fn retained_completed_passes(
    container: &Container,
    archive: &Archive,
    memories: &crate::memory_store::MemoryStore,
) -> Result<BTreeMap<MemoryId, InitialDreamPass>, OwnerError> {
    let owner = container.owner_uuid();
    if owner.is_none() {
        return Err(OwnerError::Freshness(
            "historical Dream proof requires a durable REL owner".into(),
        ));
    }
    let scan = container;
    if scan.owner_uuid() != owner {
        return Err(OwnerError::Freshness(
            "historical Dream proof owner changed during read".into(),
        ));
    }
    let mut journal = crate::freshness_dream_journal::DreamFreshnessJournal::new(owner);
    let mut completed = BTreeMap::new();
    replay_population(scan, archive, memories, &mut journal, Some(&mut completed))?;
    Ok(completed)
}

fn observe_completed_frame(
    payload: &[u8],
    state: &crate::freshness_dream_journal::DreamFreshnessJournal,
    completed: &mut BTreeMap<MemoryId, InitialDreamPass>,
) -> Result<(), OwnerError> {
    let Some(pass_id) =
        crate::freshness_dream_journal::DreamFreshnessJournal::settled_pass_id(payload)
            .map_err(OwnerError::Freshness)?
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
        return Err(OwnerError::Freshness(
            "conflicting retained first-cycle proof".into(),
        ));
    }
    Ok(())
}
