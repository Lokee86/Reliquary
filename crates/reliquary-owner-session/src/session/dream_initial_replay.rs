//! Authenticate first-cycle receipts at their actual physical prefix.
use super::dream_population::BirthPopulationReplay;
use super::*;
use crate::records::{
    dream::{dream_begin_codec::BeginPopulation, freshness_journal::DreamEntry},
    dream_empty::InitialDreamSettlement,
    dream_group::GroupedDreamRelation,
    dream_supersession::GroupedDreamSupersession,
};
use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;

fn dream_initial_frame(prefix: &[u8]) -> bool {
    // Import epochs and the legacy Dream journal are inspected before
    // unwrapping publication envelopes.
    if crate::records::import::is_import_epoch_marker(prefix) || prefix.starts_with(b"CVDREAM1") {
        return true;
    }
    let payload = match reliquary_container::decode_publication(prefix) {
        Ok(Some(envelope)) => envelope.body,
        Ok(None) => prefix,
        Err(_) => return true,
    };
    if crate::records::insomnia_completion::is_grouped_completion(payload) {
        return true;
    }
    // Keep owner families that reject unknown versions, and exact markers
    // for the remaining records that carry the Memory, Graph or Dream cuts.
    payload.starts_with(b"RELMCOR")
        || payload.starts_with(b"RELMRET")
        || [
            b"RELDRB", b"RELDRC", b"RELDRF", b"RELDRG", b"RELDRS", b"RELGPB", b"RELASC",
        ]
        .iter()
        .any(|marker| payload.starts_with(*marker))
        || [
            b"CVDREAM2",
            b"RELDRM01",
            b"RELDRP02",
            b"RELMCRT1",
            b"RELMDLT1",
            b"RELMTMB1",
            b"RELGRF01",
            b"RELCOM01",
            b"RELCOM02",
            b"RELCMP01",
            b"RELCMN01",
        ]
        .iter()
        .any(|marker| payload.starts_with(*marker))
}

#[test]
fn initial_replay_selects_clocked_proofs_but_skips_unrelated_bodies() {
    let timed_envelope = |marker: &[u8; 8]| {
        let mut bytes = b"RELPUB01".to_vec();
        bytes.extend_from_slice(&1u64.to_le_bytes());
        bytes.push(1);
        bytes.extend_from_slice(&0i64.to_le_bytes());
        bytes.extend_from_slice(marker);
        bytes
    };

    assert!(dream_initial_frame(b"RELIEPS2"));
    assert!(!dream_initial_frame(&timed_envelope(b"RELIEPS2")));
    assert!(dream_initial_frame(&timed_envelope(b"RELGPB99")));
    assert!(dream_initial_frame(b"RELDRF99"));
    assert!(dream_initial_frame(b"RELPUB99"));
    assert!(!dream_initial_frame(b"RELMBOD1"));
    assert!(!dream_initial_frame(&timed_envelope(b"RELMBOD1")));
}

pub(super) fn validate(
    container: &Container,
    dreams: &DreamEvidenceStore,
    memories: &MemoryStore,
    graph_store: &GraphStore,
    birth_versions: &BTreeMap<MemoryId, u64>,
    check_native_policy: bool,
    frontier_positions: &[(reliquary_container::ObjectRef, String)],
) -> Result<(), OwnerSessionError> {
    let mut bodies = BTreeMap::new();
    let mut birth_population = BirthPopulationReplay::new(memories);
    let mut begun = BTreeMap::new();
    let mut frontiers = BTreeMap::new();
    let mut pending_frontiers = BTreeSet::<String>::new();
    let mut settled_passes = BTreeSet::new();
    let mut imports = check_native_policy.then(|| ImportEpochStore::prefix(container.owner_id()));
    let mut evaluated = BTreeSet::new();
    let mut pair_refs = Vec::new();
    let mut memory_head = 0;
    let mut direct_links: BTreeMap<MemoryId, BTreeMap<MemoryId, u64>> = BTreeMap::new();
    let mut archivals = BTreeMap::new();
    let mut duplicates = BTreeMap::new();
    let mut graph = 0;
    let mut duplicate_history = graph_store.duplicate_history();
    let mut community = None;
    let mut generation = 0;
    let mut accepted_frontiers = frontier_positions.iter().peekable();
    let mut frames = container.matching_physical_cursor(dream_initial_frame)?;
    let mut buffer = Vec::new();
    while let Some(mut frame) = frames.next_frame()? {
        let object = frame.object();
        if accepted_frontiers
            .peek()
            .is_some_and(|(position, _)| position.offset < object.offset)
        {
            return Err(OwnerSessionError::StaleDreamCut);
        }
        let accepted = accepted_frontiers.next_if(|(position, _)| position.offset == object.offset);
        buffer.clear();
        // Reuse only the raw Frontier admitted at this position on this lease.
        // Any other carrier still follows the ordinary decode path.
        if let Some((position, _)) = accepted {
            let mut magic = [0; 8];
            frame
                .read_exact(&mut magic)
                .map_err(reliquary_container::ContainerError::from)?;
            if *position != object || &magic != b"RELDRC01" {
                return Err(OwnerSessionError::StaleDreamCut);
            }
        } else {
            frame
                .read_to_end(&mut buffer)
                .map_err(reliquary_container::ContainerError::from)?;
        }
        let frame_payload = buffer.as_slice();
        if let Some(imports) = &mut imports {
            if crate::records::import::is_import_epoch_marker(frame_payload) {
                imports
                    .ingest_prefix(frame_payload)
                    .map_err(OwnerSessionError::Record)?;
                continue;
            }
            if let Some(DreamEntry::Settled(pass) | DreamEntry::ReplaySettled(pass)) = dreams
                .journal()
                .replay_entry(frame_payload)
                .map_err(OwnerSessionError::Record)?
            {
                pending_frontiers.remove(&pass);
            }
        }
        let envelope = reliquary_container::decode_publication(frame_payload)?;
        let payload = envelope.as_ref().map_or(frame_payload, |e| e.body);
        let frontier = if let Some((_, pass_id)) = accepted {
            let (candidates, has_begin) = dreams
                .frontier_replay_facts(pass_id)
                .ok_or(OwnerSessionError::StaleDreamCut)?;
            let begin = if has_begin {
                let begin = dreams
                    .begun(pass_id)
                    .ok_or(OwnerSessionError::StaleDreamCut)?;
                Some((begin.source_id, begin.native_birth().is_some()))
            } else {
                None
            };
            Some((pass_id.clone(), Cow::Borrowed(candidates), begin))
        } else {
            crate::records::dream_frontier::FrontierRecord::decode_with_begin(payload)
                .map_err(OwnerSessionError::Record)?
                .map(|(frontier, begin)| {
                    let begin = begin.map(|begin| {
                        (
                            begin.source_id,
                            matches!(begin.population, BeginPopulation::NativeBirth { .. }),
                        )
                    });
                    (frontier.pass_id, Cow::Owned(frontier.candidates), begin)
                })
        };
        if let Some((pass_id, candidates, begin)) = frontier {
            if let Some((source, native_birth)) = begin {
                if native_birth {
                    let epoch = imports
                        .as_ref()
                        .and_then(ImportEpochStore::open_epoch)
                        .ok_or(OwnerSessionError::StaleDreamCut)?;
                    let prefix = super::native_import::native_import_dream_prefix(&epoch.import_id);
                    let has_other_pending = pending_frontiers
                        .range(prefix.clone()..)
                        .take_while(|pass| pass.starts_with(&prefix))
                        .any(|pass| pass != &pass_id);
                    if has_other_pending
                        || candidates.iter().any(|candidate| {
                            let id = MemoryId(candidate.id);
                            evaluated.contains(&ordered(source, id))
                                || (!memories
                                    .first_memory_version(id)
                                    .is_some_and(|version| version <= epoch.pre.memory_version)
                                    && !settled_passes.contains(
                                        &RelWriter::native_import_dream_identity(
                                            &epoch.import_id,
                                            id,
                                        )
                                        .0,
                                    ))
                        })
                    {
                        return Err(OwnerSessionError::StaleDreamCut);
                    }
                }
                begun.insert(pass_id.clone(), (graph, memory_head));
            }
            if check_native_policy {
                pending_frontiers.insert(pass_id.clone());
            }
            frontiers.insert(pass_id, candidates);
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
                record_version(&mut archivals, MemoryId(event.id), graph);
            }
            for pair in record.pairs.iter().filter(|p| p.direct) {
                let source = MemoryId(pair.source);
                if pair.kind == 5 {
                    record_version(&mut duplicates, source, graph);
                }
                record_version(
                    direct_links.entry(source).or_default(),
                    MemoryId(pair.candidate),
                    graph,
                );
            }
            continue;
        }
        if let Some(record) = crate::records::user_memory::UserMemoryRecord::decode(payload)
            .map_err(OwnerSessionError::Record)?
        {
            use crate::records::user_memory::UserMemoryEffect;
            let previous = MemoryId(record.previous.id);
            if let UserMemoryEffect::Correction { creation, .. } = &record.effect {
                let replacement = reliquary_memory_core::decode_grouped_creation(creation)?;
                bodies.insert(replacement.id, replacement.body_id);
                graph = record
                    .relation()
                    .expect("correction relation")
                    .graph_version;
                record_version(&mut archivals, previous, graph);
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
            memory_head = record.memory_version;
            bodies.insert(record.id, record.body_id);
        } else if crate::records::insomnia_completion::is_grouped_completion(payload) {
            for record in GroupedInsomniaCompletion::decode(payload)
                .map_err(OwnerSessionError::Record)?
                .memory_records
            {
                let record = reliquary_memory_core::decode_grouped_creation(&record)?;
                memory_head = record.memory_version;
                bodies.insert(record.id, record.body_id);
            }
        }
        if let Some((_, version)) = reliquary_memory_core::history_change_in_frame(payload)? {
            memory_head = version;
        }
        if let Some(publication) = reliquary_graph::clocked_graph_in_frame(frame_payload)? {
            graph = publication.graph_version;
        } else if let Some(relation) = reliquary_graph::relation_in_frame(payload)? {
            graph = relation.graph_version;
        }
        if let Some(receipt) =
            GroupedDreamRelation::decode(payload).map_err(OwnerSessionError::Record)?
        {
            graph = receipt.relation().graph_version;
            let source = receipt.0.source.id;
            let candidate = receipt.0.candidate.id;
            if receipt.relation().kind == reliquary_graph::RelationKind::DuplicateOf {
                record_version(&mut duplicates, source, graph);
            }
            evaluated.insert(ordered(source, candidate));
            record_version(direct_links.entry(source).or_default(), candidate, graph);
        }
        if let Some(receipt) =
            GroupedDreamSupersession::decode(payload).map_err(OwnerSessionError::Record)?
        {
            graph = receipt.relation().graph_version;
            evaluated.insert(ordered(receipt.cut.source.id, receipt.cut.candidate.id));
            let (older, version) = reliquary_memory_core::history_change_in_frame(&receipt.delta)?
                .ok_or(OwnerSessionError::StaleDreamCut)?;
            memory_head = version;
            record_version(&mut archivals, older, graph);
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
                .begun(&record.pass_id)
                .ok_or(OwnerSessionError::StaleDreamCut)?;
            let admission = dreams
                .admission(&record.pass_id)
                .ok_or(OwnerSessionError::StaleDreamCut)?;
            let birth_version = *birth_versions
                .get(&record.source)
                .ok_or(OwnerSessionError::StaleDreamCut)?;
            // A destination replay can preserve a verified source Dream's
            // explicit population even after introducing independent left-side
            // births. Compact populations remain exact physical-prefix proofs.
            let native_birth = begin.native_birth();
            let population_valid = if let Some(first) = native_birth {
                // Native frontier admission already authenticated the exact set.
                first == birth_version
            } else if let Some(first) = begin.inferred_explicit_birth() {
                first == birth_version
            } else if begin.is_compact() {
                admission.preexisting_ids() == birth_population.at(birth_version)?
            } else {
                birth_population.contains(birth_version, admission.preexisting_ids())?
            };
            let roots: BTreeSet<_> = direct_links
                .get(&record.source)
                .into_iter()
                .flatten()
                .filter_map(|(candidate, version)| {
                    (*version > begin.graph_version_before
                        && admission.preexisting_contains(candidate))
                    .then_some(*candidate)
                })
                .collect();
            if frontiers.get(&record.pass_id).is_some_and(|candidates| {
                candidates
                    .iter()
                    .any(|c| !evaluated.contains(&ordered(record.source, MemoryId(c.id))))
            }) {
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
                .get(&record.source)
                .is_some_and(|version| *version > begin.graph_version_before);
            let archival_origin = archivals
                .get(&record.source)
                .is_some_and(|version| *version >= begin.graph_version_before);
            let (reachable, _) = duplicate_history.context_at(
                record.graph_version,
                &record.roots.iter().map(|id| id.0).collect::<Vec<_>>(),
                128,
            )?;
            if record.admit == source.archived
                || (source.archived && !archival_origin)
                || (source.archived && !duplicate_origin && !record.credit_roots.is_empty())
                || record
                    .credit_roots
                    .iter()
                    .any(|id| !admission.preexisting_contains(id))
                || record
                    .credit_roots
                    .iter()
                    .any(|id| !reachable.contains(&id.0))
            {
                return Err(OwnerSessionError::StaleDreamCut);
            }
            let effects = FreshnessStore::initial_dream_effects(&record.freshness_effects)
                .map_err(OwnerSessionError::Record)?;
            if (native_birth.is_none() && admission.preexisting_len() > 4096)
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
            }
            if check_native_policy {
                pending_frontiers.remove(&record.pass_id);
                settled_passes.insert(record.pass_id);
            }
        }
    }
    if accepted_frontiers.next().is_some() {
        return Err(OwnerSessionError::StaleDreamCut);
    }
    Ok(())
}

// Every check asks if any event observed so far meets a lower Graph bound.
fn record_version(index: &mut BTreeMap<MemoryId, u64>, id: MemoryId, version: u64) {
    let latest = index.entry(id).or_insert(version);
    *latest = (*latest).max(version);
}

fn ordered(a: MemoryId, b: MemoryId) -> (MemoryId, MemoryId) {
    if a < b { (a, b) } else { (b, a) }
}
