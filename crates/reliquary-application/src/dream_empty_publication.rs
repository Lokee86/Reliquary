//! Application settlement of an owner-backed first Dream pass.
use crate::{DreamOrganizationDraft, DreamWorkflowError, dream_publication::DreamOwnerWriter};
use reliquary_owner_session::{DreamMemoryCut, OwnerSessionError};

/// A checked Dream proposal with one source and zero classified candidates
/// can settle its already-begun first cycle without fabricating a zero-link
/// classifier result. Previously committed direct first-cycle receipts remain
/// eligible roots after interruption. The owner authenticates Begin and cut.
pub fn settle_initial_dream_cycle(
    writer: &mut DreamOwnerWriter<'_>,
    authorized_owner: [u8; 16],
    pass_id: &str,
    draft: &DreamOrganizationDraft,
    accepted_turn: u64,
) -> Result<bool, DreamWorkflowError> {
    if writer.owner_id() != authorized_owner || draft.owner_id != authorized_owner {
        return Err(DreamWorkflowError::OwnerMismatch);
    }
    let Some(source) = draft.observed_memories.first() else {
        return Err(DreamWorkflowError::UnsupportedPublication);
    };
    if !draft.classifications.is_empty()
        || !draft.verifications.is_empty()
        || !draft.plan.relation_changes.is_empty()
        || !draft.plan.lifecycle_intents.is_empty()
    {
        return Err(DreamWorkflowError::UnsupportedPublication);
    }
    let cut = DreamMemoryCut {
        id: source.memory_id,
        revision: source.revision,
        body_id: source.body_id,
    };
    let snapshot = match writer {
        DreamOwnerWriter::Rel(w) => w.initial_dream_snapshot(pass_id),
        DreamOwnerWriter::Phy(w) => w.initial_dream_snapshot(pass_id),
    }
    .map_err(|e| DreamWorkflowError::OwnerWrite(e.to_string()))?;
    let (effects, promote) =
        prepare_initial_dream_settlement(snapshot, matches!(writer, DreamOwnerWriter::Rel(_)))?;
    let result = match writer {
        DreamOwnerWriter::Rel(writer) => writer.commit_initial_dream_pass(
            pass_id,
            cut,
            draft.graph_version,
            accepted_turn,
            effects,
            promote,
        ),
        DreamOwnerWriter::Phy(writer) => writer.commit_initial_dream_pass(
            pass_id,
            cut,
            draft.graph_version,
            accepted_turn,
            effects,
            promote,
        ),
    };
    result.map_err(|error| match error {
        OwnerSessionError::StaleDreamCut => DreamWorkflowError::StaleSnapshot,
        _ => DreamWorkflowError::OwnerWrite(error.to_string()),
    })
}

pub(crate) fn prepare_initial_dream_settlement(
    snapshot: reliquary_owner_session::InitialDreamSnapshot,
    rel: bool,
) -> Result<
    (
        std::collections::BTreeMap<reliquary_owner_session::MemoryId, i16>,
        bool,
    ),
    DreamWorkflowError,
> {
    let fact = reliquary_dream::MemoryFact {
        id: snapshot.source.id,
        lifecycle_state: snapshot.source.lifecycle_state.clone(),
        archived: snapshot.source.archived,
        superseded_by: snapshot.source.superseded_by,
        source_time_ns: snapshot.source.source_time_ns,
        authority_kind: snapshot.source.authority_kind.clone(),
        category: snapshot.source.category.clone(),
        memory_type: snapshot.source.memory_type.clone(),
    };
    let current: std::collections::BTreeMap<_, _> = snapshot
        .relations
        .iter()
        .map(|r| ((r.source, r.target, r.kind), *r))
        .collect();
    let duplicate = current.values().any(|r| {
        r.active
            && r.kind == reliquary_owner_session::model::RelationKind::DuplicateOf
            && [r.source, r.target]
                .contains(&reliquary_owner_session::model::NodeRef::Memory(fact.id.0))
    });
    let lifecycle = if snapshot.settled {
        Vec::new()
    } else {
        reliquary_dream::plan_lifecycle(&fact, &[], duplicate, &[], false)
    };
    if lifecycle.iter().any(|intent| {
        intent.memory_id != fact.id
            || intent.lifecycle_state != "knowledge"
            || intent.archived
            || intent.superseded_by.is_some()
    }) {
        return Err(DreamWorkflowError::UnsupportedPublication);
    }
    let promote =
        !lifecycle.is_empty() || (snapshot.settled && fact.lifecycle_state == "extracted");
    let facts: Vec<_> = snapshot
        .duplicate_facts
        .iter()
        .map(|r| reliquary_dream::MemoryFact {
            id: r.id,
            lifecycle_state: r.lifecycle_state.clone(),
            archived: r.archived,
            superseded_by: r.superseded_by,
            source_time_ns: r.source_time_ns,
            authority_kind: r.authority_kind.clone(),
            category: r.category.clone(),
            memory_type: r.memory_type.clone(),
        })
        .collect();
    let graph: Vec<_> = current.values().copied().collect();
    let mut credit_roots = std::collections::BTreeSet::new();
    if !snapshot.source.archived || snapshot.duplicate_origin {
        for root in &snapshot.roots {
            let component = reliquary_dream::duplicate_component(*root, &graph);
            let members: Vec<_> = facts
                .iter()
                .filter(|f| component.contains(&f.id))
                .cloned()
                .collect();
            if let Some(id) = reliquary_dream::duplicate_representative(&members) {
                if id != fact.id
                    && snapshot.preexisting.contains(&id)
                    && snapshot.eligible.contains(&id)
                {
                    credit_roots.insert(id);
                }
            }
        }
    }
    let mut effects = std::collections::BTreeMap::new();
    if rel && !snapshot.settled && !credit_roots.is_empty() {
        use reliquary_freshness::{
            CommunityMembership, MemoryAdjacency, PropagationEvent, propagate_with_policy,
        };
        use reliquary_owner_session::model::NodeRef;
        let mut adjacency = MemoryAdjacency::new();
        for relation in current.values().filter(|r| r.active) {
            if let (NodeRef::Memory(a), NodeRef::Memory(b)) = (relation.source, relation.target) {
                adjacency
                    .entry(reliquary_owner_session::MemoryId(a))
                    .or_default()
                    .insert(reliquary_owner_session::MemoryId(b));
                adjacency
                    .entry(reliquary_owner_session::MemoryId(b))
                    .or_default()
                    .insert(reliquary_owner_session::MemoryId(a));
            }
        }
        let mut membership = CommunityMembership::new();
        if let Some(community) = snapshot.community {
            for group in community.communities {
                for id in group.members {
                    membership.insert(reliquary_owner_session::MemoryId(id), group.id);
                }
            }
        }
        effects = propagate_with_policy(
            &PropagationEvent {
                roots: credit_roots.into_iter().collect(),
                principal: snapshot.policy.linkage_principal,
                excluded: [fact.id].into(),
            },
            &adjacency,
            &membership,
            1,
            &snapshot.policy,
        )
        .map_err(|e| DreamWorkflowError::OwnerWrite(e.to_string()))?;
        effects.retain(|id, _| snapshot.eligible.contains(id));
    }
    Ok((effects, promote))
}

/// Settle a checked no-candidate pass, including an isolated first Memory.
pub fn commit_empty_dream_cycle(
    writer: &mut DreamOwnerWriter<'_>,
    authorized_owner: [u8; 16],
    pass_id: &str,
    draft: &DreamOrganizationDraft,
    accepted_turn: u64,
) -> Result<bool, DreamWorkflowError> {
    settle_initial_dream_cycle(writer, authorized_owner, pass_id, draft, accepted_turn)
}

#[cfg(test)]
#[path = "dream_empty_publication/tests.rs"]
mod tests;
