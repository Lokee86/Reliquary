//! Apply current native Dream publications to an unpublished checkpoint restore.
use super::super::*;
use crate::DreamMemoryCut;
use crate::records::{
    dream::{
        Population, StoredAdmission,
        dream_begin_codec::{self, BeginPopulation},
    },
    dream_batch::DreamBatchRecord,
    dream_empty::InitialDreamSettlement,
    dream_frontier::FrontierRecord,
};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Default)]
pub(super) struct NativeDreamSources {
    mapped: Option<native_import::ImportSourceMap>,
    checked: BTreeSet<reliquary_archive::EpisodeId>,
    birth_populations: Option<BTreeMap<u64, Population>>,
}

impl NativeDreamSources {
    fn birth_population(
        &mut self,
        session: &RelSession,
        import_id: &str,
        birth: u64,
    ) -> Result<Population, OwnerSessionError> {
        if self.birth_populations.is_none() {
            self.birth_populations = Some(session.native_import_birth_populations(import_id)?);
        }
        self.birth_populations
            .as_ref()
            .and_then(|populations| populations.get(&birth))
            .cloned()
            .ok_or(OwnerSessionError::StaleDreamCut)
    }

    fn verify_episode(
        &mut self,
        session: &RelSession,
        import_id: &str,
        source: MemoryId,
    ) -> Result<(), OwnerSessionError> {
        let record = session.common.memories.revision_record(source, Some(1))?;
        let id = reliquary_archive::EpisodeId(
            record
                .source_episode_id
                .ok_or(OwnerSessionError::StaleDreamCut)?,
        );
        if self.checked.contains(&id) {
            return Ok(());
        }
        let epoch = session
            .imports
            .open_epoch()
            .filter(|epoch| {
                epoch.import_id == import_id
                    && epoch.format_version == 2
                    && session.imports.settlement(import_id).is_none()
            })
            .ok_or(OwnerSessionError::StaleDreamCut)?;
        let key = native_import::decode_import_key(&epoch.import_key)
            .ok_or(OwnerSessionError::StaleDreamCut)?;
        if self.mapped.is_none() {
            let receipt = session
                .archive
                .import_receipt(&key)?
                .ok_or(OwnerSessionError::StaleDreamCut)?;
            self.mapped = Some(native_import::ImportSourceMap::from_receipt(
                receipt.as_ref(),
            ));
        }
        let mapped = self.mapped.as_ref().expect("native receipt loaded");
        if !native_import::native_dream_episode_is_mapped(
            &session.archive,
            id,
            |conversation, node| mapped.contains(conversation, node),
        )? {
            return Err(OwnerSessionError::StaleDreamCut);
        }
        self.checked.insert(id);
        Ok(())
    }
}

pub(super) fn replay(
    session: &mut RelSession,
    payload: &[u8],
    import_id: &str,
    pre_import_memory: u64,
    native_sources: &mut NativeDreamSources,
) -> Result<(), OwnerSessionError> {
    match payload.get(..8) {
        Some(b"RELDRC01") => replay_frontier(
            session,
            import_id,
            pre_import_memory,
            FrontierRecord::decode(payload)
                .map_err(OwnerSessionError::Record)?
                .ok_or(OwnerSessionError::StaleDreamCut)?,
            native_sources,
        ),
        Some(b"RELDRB01") => replay_batch(
            session,
            import_id,
            DreamBatchRecord::decode(payload)
                .map_err(OwnerSessionError::Record)?
                .ok_or(OwnerSessionError::StaleDreamCut)?,
        ),
        Some(b"RELDRF02") => replay_settlement(
            session,
            import_id,
            InitialDreamSettlement::decode(payload)
                .map_err(OwnerSessionError::Record)?
                .ok_or(OwnerSessionError::StaleDreamCut)?,
        ),
        _ => Err(OwnerSessionError::StaleDreamCut),
    }
}

fn replay_frontier(
    session: &mut RelSession,
    import_id: &str,
    pre_import_memory: u64,
    frontier: FrontierRecord,
    native_sources: &mut NativeDreamSources,
) -> Result<(), OwnerSessionError> {
    let common = &session.common;
    let source = frontier.source.cut().id;
    let begin = frontier
        .begin
        .as_deref()
        .ok_or(OwnerSessionError::StaleDreamCut)
        .and_then(|payload| {
            dream_begin_codec::decode(payload).map_err(OwnerSessionError::Record)
        })?;
    let birth = import_birth(session, import_id, source)?;
    let population = match &begin.population {
        BeginPopulation::NativeBirth {
            first_memory_version,
            ..
        } => {
            if *first_memory_version != birth || birth <= pre_import_memory {
                return Err(OwnerSessionError::StaleDreamCut);
            }
            native_sources.verify_episode(session, import_id, source)?;
            // The Dream store checks the digest before installing the frontier.
            native_sources.birth_population(session, import_id, birth)?
        }
        BeginPopulation::ExplicitSet(ids) if ids.len() <= 4096 => {
            let exact = dream_population::birth_population_at(&common.memories, birth)?;
            if *ids != exact {
                return Err(OwnerSessionError::StaleDreamCut);
            }
            Population::from_set(exact)
        }
        _ => return Err(OwnerSessionError::StaleDreamCut),
    };
    if begin.turn != session.rel_turn_count()
        || frontier.pass_id != RelWriter::native_import_dream_identity(import_id, source).0
        || common.dreams.has_other_pending_frontier(
            &native_import::native_import_dream_prefix(import_id),
            &frontier.pass_id,
        )
        || frontier.candidates.iter().any(|candidate| {
            let id = MemoryId(candidate.id);
            common.dream_pairs.contains(source, id)
                || (!common
                    .memories
                    .first_memory_version(id)
                    .is_some_and(|version| version <= pre_import_memory)
                    && common
                        .dreams
                        .settlement_record(
                            &RelWriter::native_import_dream_identity(import_id, id).0,
                        )
                        .is_none())
        })
    {
        return Err(OwnerSessionError::StaleDreamCut);
    }
    replay_prefix::validate_frontier_cut(
        session.owner_id,
        &common.memories,
        common.memories.memory_version(),
        common.graph.graph_version(),
        &frontier,
    )?;
    let admission =
        StoredAdmission::with_population(begin.clone(), frontier.memory_version, population);
    session
        .common
        .dreams
        .replay_initial_frontier(frontier, begin, admission)
        .map_err(OwnerSessionError::Record)
}

fn replay_batch(
    session: &mut RelSession,
    import_id: &str,
    record: DreamBatchRecord,
) -> Result<(), OwnerSessionError> {
    let observed = record.observed[0].cut();
    import_birth(session, import_id, observed.id)?;
    let (pass, publication) = RelWriter::native_import_dream_identity(import_id, observed.id);
    let common = &mut session.common;
    let frontier = common
        .dreams
        .initial_frontier(&pass)
        .ok_or(OwnerSessionError::StaleDreamCut)?;
    let candidates: BTreeSet<_> = frontier.candidates.iter().map(|cut| cut.id).collect();
    if !common.dreams.frontier_has_bound_begin(&pass)
        || record.publication_id != publication
        || frontier.source != observed
        || frontier.graph_version != record.graph_version
        || candidates.len() != record.pairs.len()
        || record
            .pairs
            .iter()
            .any(|pair| !candidates.contains(&MemoryId(pair.candidate)))
        || frontier
            .candidates
            .iter()
            .any(|cut| !record.observed.iter().any(|o| o.cut() == *cut))
    {
        return Err(OwnerSessionError::StaleDreamCut);
    }
    replay_prefix::validate_batch_cut(
        session.owner_id,
        &common.memories,
        common.memories.memory_version(),
        common.graph.graph_version(),
        &record,
    )?;
    let admission =
        GraphAdmission::for_changes(session.owner_id, record.observed.iter().map(|o| o.id), []);
    record.replay_graph(&mut common.graph, &admission)?;
    let mut touched = Vec::with_capacity(record.lifecycles.len());
    for lifecycle in &record.lifecycles {
        common.memories.apply_dream_lifecycle(&lifecycle.event())?;
        touched.push(MemoryId(lifecycle.id));
    }
    common.dream_pairs.apply_batch(record)?;
    common.refresh_dream_index(&touched)
}

fn replay_settlement(
    session: &mut RelSession,
    import_id: &str,
    record: InitialDreamSettlement,
) -> Result<(), OwnerSessionError> {
    let birth = import_birth(session, import_id, record.source)?;
    let (pass, _) = RelWriter::native_import_dream_identity(import_id, record.source);
    let common = &mut session.common;
    let admission = common
        .dreams
        .admission(&pass)
        .ok_or(OwnerSessionError::StaleDreamCut)?;
    let begin = common
        .dreams
        .begun(&pass)
        .ok_or(OwnerSessionError::StaleDreamCut)?;
    let population_bound = if let Some(first_memory_version) = begin.native_birth() {
        // Admission already authenticated the precise birth cut on this
        // suffix or in the verified checkpoint. The grouped birth below
        // remains the same immutable revision-one record.
        first_memory_version == birth
    } else if let Some(first_memory_version) = begin.inferred_explicit_birth() {
        first_memory_version == birth && admission.preexisting_len() <= 4096
    } else {
        let population = admission.preexisting_ids();
        population.len() <= 4096
            && dream_population::BirthPopulationReplay::new(&common.memories)
                .contains(birth, population)?
    };
    let source = common.memories.revision_record(record.source, None)?;
    if record.pass_id != pass
        || !common.dreams.frontier_has_bound_begin(&pass)
        || common.dreams.settlement_record(&pass).is_some()
        || record.admit == source.archived
        || record.body
            != common
                .memories
                .revision_record(record.source, Some(1))?
                .body_id
        || record.community_generation
            != common
                .communities
                .current()
                .filter(|snapshot| snapshot.graph_version == record.graph_version)
                .map(|snapshot| snapshot.generation)
        || !common
            .memories
            .record_at_cut(record.source, admission.memory_cut)
            .is_some_and(|memory| !memory.archived)
        || !population_bound
        || record.roots != dream_initial::roots(common, &pass)?
    {
        return Err(OwnerSessionError::StaleDreamCut);
    }
    dream_empty::preflight_source(
        common,
        &pass,
        DreamMemoryCut {
            id: record.source,
            revision: record.revision,
            body_id: record.body,
        },
        record.graph_version,
        record.accepted_turn,
    )?;
    dream_empty::validate(common, session.owner_id, &record)?;
    let effects =
        dream_empty::validate_rel_freshness(common, session.owner_id, &session.freshness, &record)
            .map_err(OwnerSessionError::Record)?;
    dream_initial::validate_effects(
        common,
        &record,
        &effects
            .into_iter()
            .map(|(id, value)| (MemoryId(id.0), value))
            .collect(),
        Some(session.freshness.policy().linkage_principal),
    )?;
    if let Some(promotion) = record.memory_event() {
        common.memories.apply_initial_promotion(&promotion)?;
    }
    for effect in &record.freshness_effects {
        session
            .freshness
            .ingest_payload(effect)
            .map_err(OwnerSessionError::Record)?;
    }
    let source = record.source;
    common
        .dreams
        .apply_initial_settlement(record)
        .map_err(OwnerSessionError::Record)?;
    common.refresh_dream_index(&[source])
}

// The accepted grouped completion binds the original revision to the import
// and the physical start of its creation cohort. A reference to an older
// Memory cannot supply this proof and takes canonical replay.
fn import_birth(
    session: &RelSession,
    import_id: &str,
    source: MemoryId,
) -> Result<u64, OwnerSessionError> {
    let record = session.common.memories.revision_record(source, Some(1))?;
    let episode = record
        .source_episode_id
        .ok_or(OwnerSessionError::StaleDreamCut)?;
    session
        .grouped_import_birth(import_id, record, episode)?
        .ok_or(OwnerSessionError::StaleDreamCut)
}
