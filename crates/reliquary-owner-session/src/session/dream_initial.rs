//! Immutable first-cycle inputs and owner-only evidence checks.
use super::*;
use crate::records::dream_empty::InitialDreamSettlement;
use std::collections::{BTreeMap, BTreeSet};

/// Selected immutable facts for Application's pure Freshness evaluator.
#[derive(Clone, Debug)]
pub struct InitialDreamSnapshot {
    pub roots: BTreeSet<MemoryId>,
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
    for candidate in &admitted.preexisting_ids {
        if let Some(receipt) = common
            .dream_pairs
            .grouped_receipt(begin.source_id, *candidate)
        {
            let relation = receipt.relation();
            if receipt.0.source.id == begin.source_id
                && relation.graph_version > begin.graph_version_before
            {
                // Only actual direct accepted publications, never adjacency,
                // user relations, duplicate-chain rewires or later reprocessing.
                super::dream_no_link::validate_memory(common, receipt.0.candidate)?;
                if relation.kind == reliquary_graph::RelationKind::DuplicateOf
                    && relation.source != NodeRef::Memory(begin.source_id.0)
                {
                    return Err(OwnerSessionError::StaleDreamCut);
                }
                roots.insert(*candidate);
            }
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
        || record.roots.is_empty() != effects.is_empty()
        || effects.contains_key(&record.source)
        || record
            .roots
            .iter()
            .any(|id| effects.get(id) != Some(&principal))
    {
        return Err(OwnerSessionError::StaleDreamCut);
    }
    for id in effects.keys() {
        let history = common.memories.history(*id)?;
        let current = history
            .revisions
            .last()
            .ok_or(OwnerSessionError::StaleDreamCut)?;
        if history.tombstone.is_some()
            || history.revisions.len() != 1
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
            && history.revisions.len() == 1
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
    let source = common
        .memories
        .history(source_id)?
        .revisions
        .first()
        .ok_or(OwnerSessionError::StaleDreamCut)?
        .record
        .clone();
    Ok(InitialDreamSnapshot {
        roots,
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
