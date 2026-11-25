//! Owner-lease admission of a no-candidate initial Dream completion.
//! The accepted Graph cut and final journal settlement share one frame.
use super::*;
use crate::records::dream::{Population, StoredAdmission};
use crate::records::{DreamMemoryCut, dream_empty::InitialDreamSettlement};

pub(super) enum InitialDreamBirth {
    Physical,
    Grouped(u64),
    NativeFrontier(Population),
}

pub(super) fn initial_admission(
    container: &Container,
    common: &CommonState,
    source: DreamMemoryCut,
    pass_id: String,
    turn: u64,
    birth: InitialDreamBirth,
) -> Result<DreamBeginAdmission, OwnerSessionError> {
    initial_stored_admission(container, common, source, pass_id, turn, birth)
        .map(|admission| admission.to_admission())
}

pub(super) fn initial_stored_admission(
    container: &Container,
    common: &CommonState,
    source: DreamMemoryCut,
    pass_id: String,
    turn: u64,
    birth: InitialDreamBirth,
) -> Result<StoredAdmission, OwnerSessionError> {
    if pass_id.is_empty() || pass_id.trim() != pass_id || pass_id.len() > 256 || turn == 0 {
        return Err(OwnerSessionError::StaleDreamCut);
    }
    if common.dreams.has_other_initial_pass(source.id, &pass_id) {
        return Err(OwnerSessionError::StaleDreamCut);
    }
    if let Some(begin) = common.dreams.begun(&pass_id) {
        if begin.source_id == source.id && begin.turn == turn {
            let admission = common
                .dreams
                .admission(&pass_id)
                .ok_or(OwnerSessionError::StaleDreamCut)?;
            if !common
                .memories
                .record_at_cut(source.id, admission.memory_cut)
                .is_some_and(|r| r.revision == source.revision && r.body_id == source.body_id)
            {
                return Err(OwnerSessionError::StaleDreamCut);
            }
            return Ok(admission.clone());
        }
        return Err(OwnerSessionError::StaleDreamCut);
    }
    super::dream_no_link::validate_memory(common, source)?;
    if source.revision == 0 || turn == 0 {
        return Err(OwnerSessionError::StaleDreamCut);
    }
    let population = match birth {
        InitialDreamBirth::NativeFrontier(population) => population,
        legacy => {
            let population = match legacy {
                InitialDreamBirth::Grouped(version) => {
                    super::dream_population::birth_population_at(&common.memories, version)?
                }
                _ => super::dream_population::birth_population(
                    container,
                    &common.memories,
                    source.id,
                )?,
            };
            if population.len() > 4096 {
                return Err(OwnerSessionError::StaleDreamCut);
            }
            Population::from_set(population)
        }
    };
    Ok(StoredAdmission::new(
        container.owner_id(),
        pass_id,
        source.id,
        turn,
        common.graph.graph_version(),
        common.memories.memory_version(),
        population,
    ))
}

pub(super) fn validate(
    common: &CommonState,
    owner: [u8; 16],
    record: &InitialDreamSettlement,
) -> Result<(), OwnerSessionError> {
    if record.owner != owner {
        return Err(OwnerSessionError::StaleDreamCut);
    }
    let begun = common
        .dreams
        .begun(&record.pass_id)
        .ok_or(OwnerSessionError::StaleDreamCut)?;
    if record.accepted_turn < begun.turn
        || record.validate().is_err()
        || begun.source_id != record.source
        || begun.graph_version_before > record.graph_version
    {
        return Err(OwnerSessionError::StaleDreamCut);
    }
    // A completed retry is accepted only by exact durable receipt in Dream,
    // even if other Graph work has advanced after its original publication.
    if let Some(existing) = common.dreams.settlement_record(&record.pass_id) {
        return if existing == record {
            Ok(())
        } else {
            Err(OwnerSessionError::StaleDreamCut)
        };
    }
    if common.graph.graph_version() != record.graph_version {
        return Err(OwnerSessionError::StaleDreamCut);
    }
    validate_source(
        common,
        &record.pass_id,
        DreamMemoryCut {
            id: record.source,
            revision: record.revision,
            body_id: record.body,
        },
    )?;
    let history = common.memories.history(record.source)?;
    if !history.revisions.last().is_some_and(|r| {
        matches!(r.record.lifecycle_state.as_str(), "knowledge" | "canonical")
            || (r.record.lifecycle_state == "extracted" && record.memory_delta.is_some())
            || (r.record.archived && !record.admit)
    }) {
        return Err(OwnerSessionError::StaleDreamCut);
    }
    if record.revision == 0 {
        return Err(OwnerSessionError::StaleDreamCut);
    }
    Ok(())
}

#[path = "dream_empty/phy.rs"]
mod phy;
#[path = "dream_empty/rel.rs"]
mod rel;
#[cfg(test)]
#[path = "dream_empty/tests.rs"]
mod tests;

#[cfg(test)]
#[path = "dream_empty/initial_tests.rs"]
mod initial_tests;

#[cfg(test)]
#[path = "dream_empty/empty_phy_tests.rs"]
mod empty_phy_tests;
#[cfg(test)]
#[path = "dream_empty/empty_rel_tests.rs"]
mod empty_rel_tests;

#[cfg(test)]
#[path = "dream_empty/initial_negative.rs"]
mod initial_negative;

fn promotion(
    common: &CommonState,
    pass: &str,
    source: DreamMemoryCut,
    promote: bool,
) -> Result<Option<Vec<u8>>, OwnerSessionError> {
    if let Some(record) = common.dreams.settlement_record(pass) {
        return Ok(record.memory_delta.clone());
    }
    if promote {
        Ok(Some(common.memories.prepare_initial_promotion(
            source.id,
            source.revision,
            InitialDreamSettlement::promotion_mutation(source.id),
        )?))
    } else {
        Ok(None)
    }
}

#[cfg(test)]
#[path = "dream_empty/promotion_tests.rs"]
mod promotion_tests;

pub(super) fn preflight_source(
    common: &CommonState,
    pass: &str,
    source: DreamMemoryCut,
    graph: u64,
    turn: u64,
) -> Result<(), OwnerSessionError> {
    if let Some(record) = common.dreams.settlement_record(pass) {
        return if record.source == source.id
            && record.revision == source.revision
            && record.body == source.body_id
            && record.graph_version == graph
            && record.accepted_turn == turn
        {
            Ok(())
        } else {
            Err(OwnerSessionError::StaleDreamCut)
        };
    }
    let begin = common
        .dreams
        .begun(pass)
        .ok_or(OwnerSessionError::StaleDreamCut)?;
    if begin.source_id != source.id
        || source.revision == 0
        || graph != common.graph.graph_version()
        || graph < begin.graph_version_before
        || turn < begin.turn
    {
        return Err(OwnerSessionError::StaleDreamCut);
    }
    if common.dreams.initial_frontier(pass).is_some_and(|f| {
        f.candidates
            .iter()
            .any(|c| !common.dream_pairs.contains(source.id, c.id))
    }) {
        return Err(OwnerSessionError::StaleDreamCut);
    }
    validate_source(common, pass, source)
}

#[cfg(test)]
#[path = "dream_empty/interruption_tests.rs"]
mod interruption_tests;

#[cfg(test)]
#[path = "dream_empty/sequential_tests.rs"]
mod sequential_tests;

fn validate_source(
    common: &CommonState,
    pass: &str,
    source: DreamMemoryCut,
) -> Result<(), OwnerSessionError> {
    let current = common
        .memories
        .current(source.id)
        .map_err(|_| OwnerSessionError::StaleDreamCut)?;
    let begin = common
        .dreams
        .begun(pass)
        .ok_or(OwnerSessionError::StaleDreamCut)?;
    if current.record.revision != source.revision
        || current.record.body_id != source.body_id
        || !matches!(
            current.payload,
            reliquary_memory_core::PayloadState::Available(_)
        )
        || (current.record.archived
            && !common
                .dream_pairs
                .archival_origin(source.id, begin.graph_version_before))
    {
        return Err(OwnerSessionError::StaleDreamCut);
    }
    Ok(())
}

pub(super) fn validate_rel_freshness(
    common: &CommonState,
    owner: [u8; 16],
    freshness: &FreshnessStore,
    record: &InitialDreamSettlement,
) -> Result<BTreeMap<FreshnessMemoryId, i16>, String> {
    if record.owner != owner {
        return Err("Dream Freshness owner mismatch".into());
    }
    let begin = common
        .dreams
        .begun(&record.pass_id)
        .ok_or("Dream Freshness lacks Begin")?;
    let effects = freshness.validate_initial_dream_lifecycle(
        FreshnessMemoryId(record.source.0),
        begin.turn,
        record.accepted_turn,
        record.graph_version,
        record.community_generation,
        &record.freshness_effects,
        record.admit,
    )?;
    if record.credit_roots.is_empty() != effects.is_empty()
        || effects.contains_key(&FreshnessMemoryId(record.source.0))
        || record.credit_roots.iter().any(|id| {
            effects.get(&FreshnessMemoryId(id.0)) != Some(&freshness.policy().linkage_principal)
        })
    {
        return Err("Dream initial roots differ from Freshness evidence".into());
    }
    Ok(effects)
}
