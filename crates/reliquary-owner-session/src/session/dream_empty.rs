//! Owner-lease admission of a no-candidate initial Dream completion.
//! The accepted Graph cut and final journal settlement share one frame.
use super::*;
use crate::records::{DreamMemoryCut, dream_empty::InitialDreamSettlement};

pub(super) fn initial_admission(
    container: &Container,
    common: &CommonState,
    source: DreamMemoryCut,
    pass_id: String,
    turn: u64,
) -> Result<DreamBeginAdmission, OwnerSessionError> {
    if pass_id.is_empty() || pass_id.trim() != pass_id || pass_id.len() > 256 || turn == 0 {
        return Err(OwnerSessionError::StaleDreamCut);
    }
    if common.dreams.has_other_initial_pass(source.id, &pass_id) {
        return Err(OwnerSessionError::StaleDreamCut);
    }
    if let Some(begin) = common.dreams.begin(&pass_id) {
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
            return Ok(common.dreams.admission(&pass_id).unwrap().clone());
        }
        return Err(OwnerSessionError::StaleDreamCut);
    }
    super::dream_no_link::validate_memory(common, source)?;
    if source.revision == 0 || turn == 0 {
        return Err(OwnerSessionError::StaleDreamCut);
    }
    let population =
        super::dream_initial::birth_population(container, &common.memories, source.id)?;
    if population.len() > 4096 {
        return Err(OwnerSessionError::StaleDreamCut);
    }
    Ok(DreamBeginAdmission {
        owner_uuid: container.owner_id(),
        pass_id,
        source_id: source.id,
        turn,
        graph_version_before: common.graph.graph_version(),
        preexisting_ids: population,
        memory_cut: common.memories.memory_version(),
    })
}

fn validate(
    common: &CommonState,
    owner: [u8; 16],
    record: &InitialDreamSettlement,
) -> Result<(), OwnerSessionError> {
    if record.owner != owner {
        return Err(OwnerSessionError::StaleDreamCut);
    }
    let begun = common
        .dreams
        .begin(&record.pass_id)
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

fn preflight_source(
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
        .begin(pass)
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
        .begin(pass)
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
