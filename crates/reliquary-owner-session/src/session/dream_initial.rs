//! Immutable first-cycle inputs and owner-only evidence checks.
use super::*;
use crate::records::dream_empty::InitialDreamSettlement;
use std::collections::{BTreeMap, BTreeSet};

/// Selected immutable facts for Application's pure Freshness evaluator.
#[derive(Clone, Debug)]
pub struct InitialDreamSnapshot {
    pub roots: BTreeSet<MemoryId>,
    pub preexisting: BTreeSet<MemoryId>,
    pub duplicate_facts: Vec<reliquary_memory_core::MemoryRecord>,
    pub duplicate_origin: bool,
    pub source: reliquary_memory_core::MemoryRecord,
    pub relations: Vec<Relation>,
    pub community: Option<CommunitySnapshot>,
    pub eligible: BTreeSet<MemoryId>,
    pub policy: reliquary_freshness_store::FreshnessPolicy,
    pub settled: bool,
}

pub(super) use super::dream_population::birth_population;

pub(super) fn roots(
    common: &CommonState,
    pass: &str,
) -> Result<BTreeSet<MemoryId>, OwnerSessionError> {
    if let Some(record) = common.dreams.settlement_record(pass) {
        return Ok(record.roots.clone());
    }
    let begin = common
        .dreams
        .begin(pass)
        .ok_or(OwnerSessionError::StaleDreamCut)?;
    let admitted = common
        .dreams
        .admission(pass)
        .ok_or(OwnerSessionError::StaleDreamCut)?;
    let mut roots = BTreeSet::new();
    for (candidate, _) in common
        .dream_pairs
        .initial_links(begin.source_id, begin.graph_version_before)
    {
        if admitted.preexisting_ids.contains(&candidate) {
            roots.insert(candidate);
        }
    }
    if roots.len() > 16 {
        return Err(OwnerSessionError::StaleDreamCut);
    }
    Ok(roots)
}

pub(super) fn validate_effects(
    common: &CommonState,
    record: &InitialDreamSettlement,
    effects: &BTreeMap<MemoryId, i16>,
    principal: Option<i16>,
) -> Result<(), OwnerSessionError> {
    if common.dreams.settlement_record(&record.pass_id).is_some() {
        return Ok(());
    }
    let Some(principal) = principal else {
        return if effects.is_empty() {
            Ok(())
        } else {
            Err(OwnerSessionError::StaleDreamCut)
        };
    };
    if effects.len() > 64
        || record.credit_roots.is_empty() != effects.is_empty()
        || effects.contains_key(&record.source)
        || record
            .credit_roots
            .iter()
            .any(|id| effects.get(id) != Some(&principal))
    {
        return Err(OwnerSessionError::StaleDreamCut);
    }
    validate_credit(
        common,
        &record.pass_id,
        &record.roots,
        &record.credit_roots,
        common.graph.graph_version(),
    )?;
    for id in effects.keys() {
        let history = common.memories.history(*id)?;
        let current = history
            .revisions
            .last()
            .ok_or(OwnerSessionError::StaleDreamCut)?;
        if history.tombstone.is_some()
            || current.record.archived
            || !matches!(
                current.record.lifecycle_state.as_str(),
                "knowledge" | "canonical"
            )
        {
            return Err(OwnerSessionError::StaleDreamCut);
        }
    }
    Ok(())
}

fn snapshot(
    common: &CommonState,
    pass: &str,
    freshness: Option<&FreshnessStore>,
) -> Result<InitialDreamSnapshot, OwnerSessionError> {
    let roots = roots(common, pass)?;
    let mut eligible = BTreeSet::new();
    for id in common.memories.memory_ids() {
        let history = common.memories.history(id)?;
        if history.tombstone.is_none()
            && history.revisions.last().is_some_and(|r| {
                !r.record.archived
                    && matches!(r.record.lifecycle_state.as_str(), "knowledge" | "canonical")
            })
            && freshness.is_some_and(|f| f.has_durable_birth(FreshnessMemoryId(id.0)))
        {
            eligible.insert(id);
        }
    }
    let source_id = common
        .dreams
        .begin(pass)
        .ok_or(OwnerSessionError::StaleDreamCut)?
        .source_id;
    let source = match common.dreams.settlement_record(pass) {
        Some(record) => common.memories.revision(source_id, record.revision)?.record,
        None => common.memories.current(source_id)?.record,
    };
    let preexisting = common
        .dreams
        .admission(pass)
        .ok_or(OwnerSessionError::StaleDreamCut)?
        .preexisting_ids
        .clone();
    let begin = common
        .dreams
        .begin(pass)
        .ok_or(OwnerSessionError::StaleDreamCut)?;
    let seeds: Vec<_> = std::iter::once(source_id.0)
        .chain(roots.iter().map(|id| id.0))
        .collect();
    let (ids, _) = common.graph.duplicate_context(&seeds, 128)?;
    let duplicate_facts = ids
        .into_iter()
        .filter_map(|id| {
            common
                .memories
                .record_at_cut(MemoryId(id), common.memories.memory_version())
                .cloned()
        })
        .collect();
    let duplicate_origin = common
        .dream_pairs
        .duplicate_origin(source_id, begin.graph_version_before);
    Ok(InitialDreamSnapshot {
        roots,
        preexisting,
        duplicate_facts,
        duplicate_origin,
        source,
        relations: common.graph.history().copied().collect(),
        community: common
            .communities
            .current()
            .filter(|c| c.graph_version == common.graph.graph_version())
            .cloned(),
        eligible,
        policy: freshness.map_or_else(Default::default, FreshnessStore::policy),
        settled: common.dreams.settlement_record(pass).is_some(),
    })
}
impl RelWriter {
    pub fn initial_dream_snapshot(
        &self,
        pass: &str,
    ) -> Result<InitialDreamSnapshot, OwnerSessionError> {
        snapshot(&self.session.common, pass, Some(&self.session.freshness))
    }
}
impl PhyWriter {
    pub fn initial_dream_snapshot(
        &self,
        pass: &str,
    ) -> Result<InitialDreamSnapshot, OwnerSessionError> {
        snapshot(&self.common, pass, None)
    }
}

pub(super) fn validate_credit(
    common: &CommonState,
    pass: &str,
    roots: &BTreeSet<MemoryId>,
    credits: &BTreeSet<MemoryId>,
    version: u64,
) -> Result<(), OwnerSessionError> {
    let begin = common
        .dreams
        .begin(pass)
        .ok_or(OwnerSessionError::StaleDreamCut)?;
    let admitted = common
        .dreams
        .admission(pass)
        .ok_or(OwnerSessionError::StaleDreamCut)?;
    if credits.contains(&begin.source_id) || !credits.is_subset(&admitted.preexisting_ids) {
        return Err(OwnerSessionError::StaleDreamCut);
    }
    let graph = common.graph.snapshot_at(version)?;
    let (reachable, _) =
        graph.duplicate_context(&roots.iter().map(|id| id.0).collect::<Vec<_>>(), 128)?;
    if credits.iter().any(|id| !reachable.contains(&id.0)) {
        return Err(OwnerSessionError::StaleDreamCut);
    }
    if common.memories.current(begin.source_id)?.record.archived
        && !common
            .dream_pairs
            .duplicate_origin(begin.source_id, begin.graph_version_before)
        && !credits.is_empty()
    {
        return Err(OwnerSessionError::StaleDreamCut);
    }
    Ok(())
}
