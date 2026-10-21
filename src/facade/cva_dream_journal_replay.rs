use crate::{Archive, Container, CvaError};

/// Replay Dream journal records in physical order once Memory and source
/// admission state are finalized. Current compact Begins bind to the prefix
/// immediately before their own frame; later roots and settlement records
/// then use the same resident journal projection as legacy Begins.
pub(crate) fn replay_with_population(
    container: &mut Container,
    archive: &Archive,
    memories: &crate::memory_store::MemoryStore,
    journal: &mut crate::freshness_dream_journal::DreamFreshnessJournal,
) -> Result<(), CvaError> {
    if container.version().major == 1 {
        return Err(CvaError::Freshness(
            "CVDREAM2 Begin is not admitted in a V1 container".into(),
        ));
    }

    let mut prefix = container
        .owner_uuid()
        .map(crate::cva_dream_population::DreamPopulationPrefix::new);
    let owner = container.owner_uuid();
    let end = container.physical_length()?;
    container.visit_versioned_payloads_through(end, |chunk, payload, global_frontier| {
        if payload.starts_with(b"CVDREAM2") {
            let owner = owner.ok_or_else(|| {
                CvaError::Freshness("CVDREAM2 Begin requires a versioned owner identity".into())
            })?;
            let record = crate::dream_begin_codec::decode(payload).map_err(CvaError::Freshness)?;
            if record.owner_uuid != owner {
                return Err(CvaError::Freshness(
                    "Dream Freshness pass owner mismatch".into(),
                ));
            }
            let population = match record.population {
                crate::dream_begin_codec::BeginPopulation::CompactCut { memory_cut, digest } => {
                    let prefix = prefix.as_ref().ok_or_else(|| {
                        CvaError::Freshness(
                            "CVDREAM2 Begin requires a versioned owner identity".into(),
                        )
                    })?;
                    let ids =
                        prefix.materialize(memories, Some(owner), memory_cut, record.source_id)?;
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
    })
}
