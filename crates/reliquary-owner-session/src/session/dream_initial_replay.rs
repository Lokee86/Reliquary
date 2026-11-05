//! Authenticate first-cycle receipts at their actual physical prefix.
use super::*;
use crate::records::{
    dream_empty::InitialDreamSettlement, dream_group::GroupedDreamRelation,
    dream_supersession::GroupedDreamSupersession,
};
use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;

pub(super) fn validate(
    container: &Container,
    dreams: &DreamEvidenceStore,
    memories: &MemoryStore,
) -> Result<(), OwnerSessionError> {
    let mut bodies = BTreeMap::new();
    let mut active = BTreeSet::new();
    let mut changed = BTreeSet::new();
    let mut begun = BTreeMap::new();
    let mut memory_head = 0;
    let mut receipts = Vec::new();
    let mut graph = 0;
    let mut community = None;
    let mut generation = 0;
    for frame in container.read_frames()? {
        let envelope = reliquary_container::decode_publication(&frame.payload)?;
        let payload = envelope
            .as_ref()
            .map_or(frame.payload.as_slice(), |e| e.body);
        if payload.starts_with(b"RELMCRT1") {
            let record = reliquary_memory_core::decode_grouped_creation(payload)?;
            if !record.archived
                && matches!(record.lifecycle_state.as_str(), "knowledge" | "canonical")
            {
                active.insert(record.id);
            }
            memory_head = record.memory_version;
            bodies.insert(record.id, record.body_id);
        } else if payload.starts_with(b"RELINSC7") {
            for record in GroupedInsomniaCompletion::decode(payload)
                .map_err(OwnerSessionError::Record)?
                .memory_records
            {
                let record = reliquary_memory_core::decode_grouped_creation(&record)?;
                if !record.archived
                    && matches!(record.lifecycle_state.as_str(), "knowledge" | "canonical")
                {
                    active.insert(record.id);
                }
                memory_head = record.memory_version;
                bodies.insert(record.id, record.body_id);
            }
        }
        if let Some((id, version)) = reliquary_memory_core::history_change_in_frame(payload)? {
            memory_head = version;
            changed.insert(id);
        }
        if let Some(relation) = reliquary_graph::relation_in_frame(payload)? {
            graph = relation.graph_version;
        }
        if let Some(receipt) =
            GroupedDreamRelation::decode(payload).map_err(OwnerSessionError::Record)?
        {
            graph = receipt.relation().graph_version;
            receipts.push(receipt);
        }
        if let Some(receipt) =
            GroupedDreamSupersession::decode(payload).map_err(OwnerSessionError::Record)?
        {
            graph = receipt.relation().graph_version;
            let (older, version) = reliquary_memory_core::history_change_in_frame(&receipt.delta)?
                .ok_or(OwnerSessionError::StaleDreamCut)?;
            memory_head = version;
            changed.insert(older);
        }
        if payload.starts_with(b"CVDREAM2") {
            let begin = crate::records::dream::dream_begin_codec::decode(payload)
                .map_err(OwnerSessionError::Record)?;
            begun.insert(begin.pass_id, (graph, memory_head));
        }
        if let Some(version) = reliquary_graph::community_graph_version_in_frame(payload)? {
            generation += 1;
            community = Some((generation, version));
        }
        if let Some(record) =
            InitialDreamSettlement::decode(payload).map_err(OwnerSessionError::Record)?
        {
            let begin = dreams
                .begin(&record.pass_id)
                .ok_or(OwnerSessionError::StaleDreamCut)?;
            let admission = dreams
                .admission(&record.pass_id)
                .ok_or(OwnerSessionError::StaleDreamCut)?;
            let population =
                super::dream_initial::birth_population(container, memories, record.source)?;
            let roots: BTreeSet<_> = receipts
                .iter()
                .filter_map(|receipt| {
                    (receipt.0.source.id == record.source
                        && population.contains(&receipt.0.candidate.id)
                        && receipt.relation().graph_version > begin.graph_version_before)
                        .then_some(receipt.0.candidate.id)
                })
                .collect();
            if receipts.iter().any(|receipt| {
                receipt.0.source.id == record.source
                    && population.contains(&receipt.0.candidate.id)
                    && receipt.relation().graph_version > begin.graph_version_before
                    && receipt.relation().kind == reliquary_graph::RelationKind::DuplicateOf
                    && receipt.relation().source != NodeRef::Memory(record.source.0)
            }) {
                return Err(OwnerSessionError::StaleDreamCut);
            }
            let effects = FreshnessStore::initial_dream_effects(&record.freshness_effects)
                .map_err(OwnerSessionError::Record)?;
            if population.len() > 4096
                || effects.len() > 64
                || effects.keys().any(|id| {
                    !active.contains(&MemoryId(id.0)) || changed.contains(&MemoryId(id.0))
                })
                || (!active.contains(&record.source) && record.memory_delta.is_none())
                || begun.get(&record.pass_id)
                    != Some(&(begin.graph_version_before, admission.memory_cut))
                || record.owner != container.owner_id()
                || record.community_generation
                    != community
                        .filter(|(_, version)| *version == graph)
                        .map(|(generation, _)| generation)
                || record.graph_version != graph
                || bodies.get(&record.source) != Some(&record.body)
                || changed.contains(&record.source)
                || admission.preexisting_ids != population
                || roots != record.roots
                || roots.iter().any(|id| changed.contains(id))
            {
                return Err(OwnerSessionError::Record(
                    "Dream settlement lacks exact owner physical-prefix evidence".into(),
                ));
            }
            if let Some(delta) = &record.memory_delta {
                memory_head = reliquary_memory_core::history_change_in_frame(delta)?
                    .ok_or(OwnerSessionError::StaleDreamCut)?
                    .1;
                active.insert(record.source);
                changed.insert(record.source);
            }
        }
    }
    Ok(())
}

/// Read the bounded owner carriers relevant to compact Begin prefix checking.
pub(super) fn prefix_effect(
    reader: &mut impl std::io::Read,
    magic: [u8; 8],
) -> Result<(Option<u64>, bool), OwnerSessionError> {
    let mut payload = magic.to_vec();
    reader
        .take(128 * 1024 * 1024 + 1)
        .read_to_end(&mut payload)
        .map_err(|e| OwnerSessionError::Record(e.to_string()))?;
    if payload.len() > 128 * 1024 * 1024 {
        return Err(OwnerSessionError::StaleDreamCut);
    }
    if let Some(record) =
        InitialDreamSettlement::decode(&payload).map_err(OwnerSessionError::Record)?
    {
        return Ok((None, record.memory_delta.is_some()));
    }
    if let Some(record) =
        GroupedDreamRelation::decode(&payload).map_err(OwnerSessionError::Record)?
    {
        return Ok((Some(record.relation().graph_version), false));
    }
    if let Some(record) =
        GroupedDreamSupersession::decode(&payload).map_err(OwnerSessionError::Record)?
    {
        return Ok((Some(record.relation().graph_version), true));
    }
    Ok((None, false))
}
