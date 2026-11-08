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
    graph_store: &GraphStore,
) -> Result<(), OwnerSessionError> {
    let mut bodies = BTreeMap::new();
    let mut active = BTreeSet::new();
    let mut changed = BTreeSet::new();
    let mut begun = BTreeMap::new();
    let mut frontiers = BTreeMap::new();
    let mut evaluated = BTreeSet::new();
    let mut pair_refs = Vec::new();
    let mut memory_head = 0;
    let mut receipts = Vec::new();
    let mut batch_links = Vec::new();
    let mut archivals = Vec::new();
    let mut duplicates = Vec::new();
    let mut graph = 0;
    let mut community = None;
    let mut generation = 0;
    for frame in container.read_frames()? {
        let envelope = reliquary_container::decode_publication(&frame.payload)?;
        let payload = envelope
            .as_ref()
            .map_or(frame.payload.as_slice(), |e| e.body);
        if let Some(frontier) = crate::records::dream_frontier::FrontierRecord::decode(payload)
            .map_err(OwnerSessionError::Record)?
        {
            if let Some(payload) = &frontier.begin {
                let begin = crate::records::dream::dream_begin_codec::decode(payload)
                    .map_err(OwnerSessionError::Record)?;
                begun.insert(begin.pass_id, (graph, memory_head));
            }
            frontiers.insert(frontier.pass_id.clone(), frontier);
            continue;
        }
        if payload.starts_with(b"RELDRM01") {
            if payload.len() != 44 {
                return Err(OwnerSessionError::StaleDreamCut);
            }
            pair_refs.push(MemoryId(payload[12..44].try_into().unwrap()));
        }
        if payload.starts_with(b"RELDRP02") {
            if payload.len() != 16 {
                return Err(OwnerSessionError::StaleDreamCut);
            }
            let a = u32::from_le_bytes(payload[8..12].try_into().unwrap());
            let b = u32::from_le_bytes(payload[12..16].try_into().unwrap());
            let a = *pair_refs
                .get(a.checked_sub(1).ok_or(OwnerSessionError::StaleDreamCut)? as usize)
                .ok_or(OwnerSessionError::StaleDreamCut)?;
            let b = *pair_refs
                .get(b.checked_sub(1).ok_or(OwnerSessionError::StaleDreamCut)? as usize)
                .ok_or(OwnerSessionError::StaleDreamCut)?;
            evaluated.insert(ordered(a, b));
        }
        if let Some(record) = crate::records::dream_batch::DreamBatchRecord::decode(payload)
            .map_err(OwnerSessionError::Record)?
        {
            for p in &record.pairs {
                evaluated.insert(ordered(MemoryId(p.source), MemoryId(p.candidate)));
            }
            for event in &record.lifecycles {
                memory_head = reliquary_memory_core::history_change_in_frame(&event.delta)?
                    .ok_or(OwnerSessionError::StaleDreamCut)?
                    .1;
            }
            graph = record.graph_version + record.graph_records.len() as u64;
            for event in record.lifecycles.iter().filter(|l| l.archived) {
                archivals.push((MemoryId(event.id), graph));
            }
            for pair in record.pairs.iter().filter(|p| p.direct && p.kind == 5) {
                duplicates.push((MemoryId(pair.source), graph));
            }
            for pair in record.pairs.iter().filter(|p| p.direct) {
                batch_links.push((MemoryId(pair.source), MemoryId(pair.candidate), graph));
            }
            continue;
        }
        if let Some(record) = crate::records::user_memory::UserMemoryRecord::decode(payload)
            .map_err(OwnerSessionError::Record)?
        {
            use crate::records::user_memory::UserMemoryEffect;
            let previous = MemoryId(record.previous.id);
            active.remove(&previous);
            changed.insert(previous);
            if let UserMemoryEffect::Correction { creation, .. } = &record.effect {
                let replacement = reliquary_memory_core::decode_grouped_creation(creation)?;
                active.insert(replacement.id);
                bodies.insert(replacement.id, replacement.body_id);
                graph = record
                    .relation()
                    .expect("correction relation")
                    .graph_version;
                archivals.push((previous, graph));
            }
            if let Some(change) = record.history_frames().last() {
                memory_head = reliquary_memory_core::history_change_in_frame(change)?
                    .ok_or(OwnerSessionError::StaleDreamCut)?
                    .1;
            }
            continue;
        }
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
            if receipt.relation().kind == reliquary_graph::RelationKind::DuplicateOf {
                duplicates.push((receipt.0.source.id, graph));
            }
            evaluated.insert(ordered(receipt.0.source.id, receipt.0.candidate.id));
            receipts.push(receipt);
        }
        if let Some(receipt) =
            GroupedDreamSupersession::decode(payload).map_err(OwnerSessionError::Record)?
        {
            graph = receipt.relation().graph_version;
            evaluated.insert(ordered(receipt.cut.source.id, receipt.cut.candidate.id));
            let (older, version) = reliquary_memory_core::history_change_in_frame(&receipt.delta)?
                .ok_or(OwnerSessionError::StaleDreamCut)?;
            memory_head = version;
            changed.insert(older);
            archivals.push((older, graph));
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
            let physical_population =
                super::dream_initial::birth_population(container, memories, record.source)?;
            // A destination replay can preserve a verified source Dream's
            // explicit population even after introducing independent left-side
            // births. Compact populations remain exact physical-prefix proofs.
            let population = &admission.preexisting_ids;
            let population_valid = match &begin.population {
                crate::records::dream::dream_begin_codec::BeginPopulation::CompactCut {
                    ..
                } => population == &physical_population,
                crate::records::dream::dream_begin_codec::BeginPopulation::ExplicitSet(_) => {
                    population.is_subset(&physical_population)
                }
            };
            let roots: BTreeSet<_> = receipts
                .iter()
                .filter_map(|receipt| {
                    (receipt.0.source.id == record.source
                        && population.contains(&receipt.0.candidate.id)
                        && receipt.relation().graph_version > begin.graph_version_before)
                        .then_some(receipt.0.candidate.id)
                })
                .chain(
                    batch_links
                        .iter()
                        .filter(|(source, candidate, version)| {
                            *source == record.source
                                && population.contains(candidate)
                                && *version > begin.graph_version_before
                        })
                        .map(|(_, candidate, _)| *candidate),
                )
                .collect();
            if frontiers.get(&record.pass_id).is_some_and(
                |f: &crate::records::dream_frontier::FrontierRecord| {
                    f.candidates
                        .iter()
                        .any(|c| !evaluated.contains(&ordered(record.source, MemoryId(c.id))))
                },
            ) {
                return Err(OwnerSessionError::StaleDreamCut);
            }
            if !memories
                .record_at_cut(record.source, admission.memory_cut)
                .is_some_and(|r| !r.archived)
            {
                return Err(OwnerSessionError::StaleDreamCut);
            }
            let source = memories
                .record_at_cut(record.source, memory_head)
                .ok_or(OwnerSessionError::StaleDreamCut)?;
            let duplicate_origin = duplicates
                .iter()
                .any(|(id, version)| *id == record.source && *version > begin.graph_version_before);
            let archival_origin = archivals.iter().any(|(id, version)| {
                *id == record.source && *version >= begin.graph_version_before
            });
            let graph_view = graph_store.snapshot_at(record.graph_version)?;
            let (reachable, _) = graph_view
                .duplicate_context(&record.roots.iter().map(|id| id.0).collect::<Vec<_>>(), 128)?;
            if record.admit == source.archived
                || (source.archived && !archival_origin)
                || (source.archived && !duplicate_origin && !record.credit_roots.is_empty())
                || !record.credit_roots.is_subset(&population)
                || record
                    .credit_roots
                    .iter()
                    .any(|id| !reachable.contains(&id.0))
            {
                return Err(OwnerSessionError::StaleDreamCut);
            }
            let effects = FreshnessStore::initial_dream_effects(&record.freshness_effects)
                .map_err(OwnerSessionError::Record)?;
            if population.len() > 4096
                || effects.len() > 64
                || effects.keys().any(|id| {
                    !memories
                        .record_at_cut(MemoryId(id.0), memory_head)
                        .is_some_and(|r| {
                            !r.archived
                                && matches!(r.lifecycle_state.as_str(), "knowledge" | "canonical")
                        })
                })
                || !memories
                    .record_at_cut(record.source, memory_head)
                    .is_some_and(|r| {
                        r.body_id == record.body
                            && r.revision == record.revision
                            && (matches!(r.lifecycle_state.as_str(), "knowledge" | "canonical")
                                || (r.lifecycle_state == "extracted"
                                    && record.memory_delta.is_some())
                                || (r.archived && !record.admit))
                    })
                || begun.get(&record.pass_id)
                    != Some(&(begin.graph_version_before, admission.memory_cut))
                || record.owner != container.owner_id()
                || record.community_generation
                    != community
                        .filter(|(_, version)| *version == graph)
                        .map(|(generation, _)| generation)
                || record.graph_version != graph
                || bodies.get(&record.source) != Some(&record.body)
                || !population_valid
                || roots != record.roots
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

fn ordered(a: MemoryId, b: MemoryId) -> (MemoryId, MemoryId) {
    if a < b { (a, b) } else { (b, a) }
}
