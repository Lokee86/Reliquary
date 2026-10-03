use crate::{Cva, DreamPublicationOutcome, GraphRelationOrigin, MemoryId, PropagationEvent};
use std::collections::{BTreeMap, BTreeSet};

/// Capture the durable first-pass identity before any Dream relation is published.
/// The Freshness owner returns the original frozen cut on retry.
pub(crate) fn begin_initial_pass(
    cva: &mut Cva,
    source_id: MemoryId,
    candidate_ids: impl Iterator<Item = MemoryId>,
) -> Result<Option<crate::InitialDreamPass>, crate::CvaError> {
    if let Some(pending) = cva.pending_initial_dream_pass(source_id) {
        return Ok(Some(pending));
    }
    let source = cva.memory(source_id).map_err(crate::CvaError::from)?;
    let first = cva
        .memory_revision(source_id, 1)
        .map_err(crate::CvaError::from)?;
    let Some(owner_uuid) = cva.owner_uuid() else {
        return Ok(None);
    };
    let pass_id = initial_pass_id(owner_uuid, source_id);
    if cva.initial_dream_pass_settled(&pass_id) {
        return Ok(None);
    }
    let has_publication_birth = cva.freshness_is_publication_birth(source_id);
    let first_extracted_settlement = has_publication_birth
        && first.lifecycle_state == "extracted"
        && source.lifecycle_state == "extracted"
        && !source.archived;
    let direct_active_birth = has_publication_birth
        && !source.archived
        && matches!(first.lifecycle_state.as_str(), "knowledge" | "canonical");
    if !first_extracted_settlement && !direct_active_birth {
        return Ok(None);
    }

    let mut preexisting_ids = BTreeSet::new();
    let candidates = candidate_ids
        .chain(cva.freshness_publication_birth_ids())
        .collect::<BTreeSet<_>>();
    for id in candidates {
        if id == source_id {
            continue;
        }
        let first_memory = cva.memory_revision(id, 1).map_err(crate::CvaError::from)?;
        let same_grouped_birth = cva.freshness_same_publication_cohort(source_id, id)?;
        let has_durable_birth = cva.freshness_is_publication_birth(id);
        if !same_grouped_birth
            && has_durable_birth
            && first_memory.global_version < first.global_version
        {
            preexisting_ids.insert(id);
        }
    }
    cva.begin_initial_dream_pass(
        pass_id,
        source_id,
        1,
        cva.rel_turn_count(),
        preexisting_ids,
        cva.graph_version(),
    )
    .map(Some)
}

pub(crate) fn record_publication_roots(
    cva: &mut Cva,
    source_id: MemoryId,
    publication: &DreamPublicationOutcome,
) -> Result<(), crate::CvaError> {
    let Some(pass) = cva.pending_initial_dream_pass(source_id) else {
        return Ok(());
    };
    let mut roots = roots_from_current_graph(cva, source_id, &pass);
    if let DreamPublicationOutcome::Published(relations) = publication {
        for relation in relations {
            if relation.origin != GraphRelationOrigin::Dream {
                continue;
            }
            let neighbor = if relation.source == source_id {
                Some(relation.target)
            } else if relation.target == source_id {
                Some(relation.source)
            } else {
                None
            };
            if let Some(neighbor) = neighbor
                && pass.preexisting_ids.contains(&neighbor)
            {
                roots.insert(neighbor);
            }
        }
    }
    cva.record_initial_dream_roots(&pass.pass_id, roots)
}

/// Frozen inputs for propagation. Runtime hosts release their owner mutex before
/// evaluating this value; only preparation/commit touch the Container.
pub(crate) struct InitialSettlementWork {
    pass_id: String,
    active_admission: bool,
    proof: crate::FreshnessEventProof,
    event: Option<PropagationEvent>,
    adjacency: crate::freshness::MemoryAdjacency,
    memberships: crate::freshness::CommunityMembership,
    policy: crate::freshness::FreshnessPolicy,
}
pub(crate) struct InitialSettlementResult {
    pass_id: String,
    active_admission: bool,
    proof: crate::FreshnessEventProof,
    effects: BTreeMap<MemoryId, i16>,
}
impl InitialSettlementWork {
    pub(crate) fn evaluate(
        self,
        workers: usize,
    ) -> Result<InitialSettlementResult, crate::CvaError> {
        let effects = match self.event {
            Some(event) => crate::freshness::propagate_with_policy(
                &event,
                &self.adjacency,
                &self.memberships,
                workers.max(1),
                &self.policy,
            )
            .map_err(|error| crate::CvaError::Freshness(error.to_string()))?,
            None => BTreeMap::new(),
        };
        Ok(InitialSettlementResult {
            pass_id: self.pass_id,
            active_admission: self.active_admission,
            proof: self.proof,
            effects,
        })
    }
}
impl InitialSettlementResult {
    pub(crate) fn commit(self, cva: &mut Cva) -> Result<(), crate::CvaError> {
        // The owner checks durable acceptance, the original pinned proof and
        // idempotent receipt again after the runtime lock is reacquired.
        cva.settle_initial_dream_pass(
            &self.pass_id,
            self.active_admission,
            self.proof,
            self.effects,
        )
        .map(|_| ())
    }
}
pub(crate) fn prepare_initial_settlement(
    cva: &mut Cva,
    source_id: MemoryId,
    active_admission: bool,
) -> Result<Option<InitialSettlementWork>, crate::CvaError> {
    let Some(pass) = cva.pending_initial_dream_pass(source_id) else {
        return Ok(None);
    };
    if cva.recover_committed_initial_dream_event(&pass.pass_id, active_admission)? {
        return Ok(None);
    }
    let raw_roots = pass
        .roots
        .iter()
        .copied()
        .chain(cva.initial_dream_committed_roots(&pass)?)
        .collect::<BTreeSet<_>>();
    cva.record_initial_dream_roots(&pass.pass_id, raw_roots.clone())?;
    let roots = fold_duplicate_roots(cva, raw_roots, source_id)?;
    let (proof, adjacency, memberships) = cva.initial_dream_graph_snapshot(&pass.pass_id)?;
    let policy = cva.freshness_policy();
    let event = if roots.is_empty() {
        None
    } else {
        Some(PropagationEvent {
            roots: roots.into_iter().collect(),
            principal: policy.linkage_principal,
            excluded: cva.freshness_publication_cohort(source_id)?,
        })
    };
    Ok(Some(InitialSettlementWork {
        pass_id: pass.pass_id,
        active_admission,
        proof,
        event,
        adjacency,
        memberships,
        policy,
    }))
}
pub(crate) fn settle_initial_pass(
    cva: &mut Cva,
    source_id: MemoryId,
    active_admission: bool,
    workers: usize,
) -> Result<(), crate::CvaError> {
    if let Some(work) = prepare_initial_settlement(cva, source_id, active_admission)? {
        work.evaluate(workers)?.commit(cva)?;
    }
    Ok(())
}

fn roots_from_current_graph(
    cva: &Cva,
    source_id: MemoryId,
    pass: &crate::InitialDreamPass,
) -> BTreeSet<MemoryId> {
    cva.graph_relations()
        .into_iter()
        .filter(|relation| {
            relation.origin == GraphRelationOrigin::Dream
                && relation.active
                && relation.graph_version > pass.graph_version_before
                && (relation.source == source_id || relation.target == source_id)
        })
        .filter_map(|relation| {
            let neighbor = if relation.source == source_id {
                relation.target
            } else {
                relation.source
            };
            pass.preexisting_ids.contains(&neighbor).then_some(neighbor)
        })
        .collect()
}

fn fold_duplicate_roots(
    cva: &mut Cva,
    roots: BTreeSet<MemoryId>,
    source_id: MemoryId,
) -> Result<BTreeSet<MemoryId>, crate::CvaError> {
    let relations = cva.graph_relations();
    let first = cva
        .memory_revision(source_id, 1)
        .map_err(crate::CvaError::from)?;
    let mut result = BTreeSet::new();
    for root in roots {
        let is_duplicate = relations.iter().any(|relation| {
            relation.kind == crate::GraphRelationKind::DuplicateOf
                && (relation.source == root || relation.target == root)
        });
        if !is_duplicate {
            result.insert(root);
            continue;
        }
        let representative = crate::dream_canonical::existing_duplicate_representative(
            cva,
            root,
            first.global_version,
            source_id,
            &relations,
        )
        .map_err(|error| crate::CvaError::Freshness(error.to_string()))?;
        if let Some(representative) = representative {
            result.insert(representative);
        } else if !cva.memory(root).map_err(crate::CvaError::from)?.archived {
            result.insert(root);
        }
    }
    Ok(result)
}

fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write;
        let _ = write!(&mut out, "{byte:02x}");
    }
    out
}

pub(crate) fn initial_pass_id(owner: [u8; 16], source: MemoryId) -> String {
    format!("dream-initial:{}:{}", hex(&owner), hex(&source.0))
}
