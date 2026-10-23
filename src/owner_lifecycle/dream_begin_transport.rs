//! Exact Dream Begin evidence over accepted owner parts.
use super::OwnerError;
use crate::dream_begin_codec::{self, BeginPopulation, BeginRecord};
use crate::{InitialDreamPass, ObjectRef};

/// Resolve a source Begin at its own physical boundary, never at EOF.
pub(crate) fn materialized_dream_begin(
    container: &crate::Container,
    archive: &crate::Archive,
    memories: &crate::memory_store::MemoryStore,
    object: ObjectRef,
    payload: &[u8],
) -> Result<Option<InitialDreamPass>, OwnerError> {
    let owner = container
        .owner_uuid()
        .ok_or_else(|| OwnerError::Freshness("Dream Begin transport requires an owner".into()))?;
    if !payload.starts_with(b"CVDREAM2") {
        return Ok(None);
    }

    let record = dream_begin_codec::decode(payload).map_err(OwnerError::Freshness)?;
    if record.owner_uuid != owner {
        return Err(OwnerError::Freshness(
            "Dream Begin source owner/format mismatch".into(),
        ));
    }
    let ids = match record.population {
        BeginPopulation::ExplicitSet(ids) => ids,
        BeginPopulation::CompactCut { memory_cut, digest } => {
            let ids = population_at_prefix(
                container,
                archive,
                memories,
                object.offset(),
                memory_cut,
                record.source_id,
            )?;
            if crate::dream_population_digest::digest(&ids) != digest {
                return Err(OwnerError::Freshness(
                    "Dream Begin transport population digest differs".into(),
                ));
            }
            ids
        }
    };
    Ok(Some(InitialDreamPass {
        pass_id: record.pass_id,
        source_id: record.source_id,
        first_revision: 1,
        turn: record.turn,
        accepted_settlement_turn: None,
        preexisting_ids: ids,
        graph_version_before: record.graph_version_before,
        roots: Default::default(),
    }))
}

/// Compact authority is legal only after proving exact set equality at the
/// destination prefix. Manual subsets and changed birth order stay literal.
pub(crate) fn current_dream_begin_payload(
    container: &crate::Container,
    archive: &crate::Archive,
    memories: &crate::memory_store::MemoryStore,
    pass: &InitialDreamPass,
) -> Result<Vec<u8>, OwnerError> {
    if pass.first_revision != 1 || !pass.roots.is_empty() || pass.accepted_settlement_turn.is_some()
    {
        return Err(OwnerError::Freshness(
            "Dream Begin transport requires an initial pass".into(),
        ));
    }
    let owner = container
        .owner_uuid()
        .ok_or_else(|| OwnerError::Freshness("Dream Begin destination requires an owner".into()))?;
    let mut population = BeginPopulation::ExplicitSet(pass.preexisting_ids.clone());
    if !pass.preexisting_ids.is_empty() && memories.first_global_version(pass.source_id).is_some() {
        let end = container.physical_length()?;
        let cut = memories.memory_version();
        let ids = population_at_prefix(container, archive, memories, end, cut, pass.source_id)?;
        if ids == pass.preexisting_ids {
            population = BeginPopulation::CompactCut {
                memory_cut: cut,
                digest: crate::dream_population_digest::digest(&ids),
            };
        }
    }
    dream_begin_codec::encode(&BeginRecord {
        owner_uuid: owner,
        pass_id: pass.pass_id.clone(),
        source_id: pass.source_id,
        turn: pass.turn,
        graph_version_before: pass.graph_version_before,
        population,
    })
    .map_err(OwnerError::Freshness)
}

fn population_at_prefix(
    container: &crate::Container,
    archive: &crate::Archive,
    memories: &crate::memory_store::MemoryStore,
    prefix_end: u64,
    memory_cut: u64,
    source_id: crate::MemoryId,
) -> Result<std::collections::BTreeSet<crate::MemoryId>, OwnerError> {
    let owner = container.owner_uuid().ok_or_else(|| {
        OwnerError::Freshness("Dream population requires a versioned owner".into())
    })?;
    let state = super::DreamPopulationPrefix::scan_through(
        container, archive, memories, owner, prefix_end,
    )?;
    state.materialize(memories, Some(owner), memory_cut, source_id)
}
