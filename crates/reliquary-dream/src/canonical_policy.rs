use reliquary_graph::{GraphRelation, GraphRelationKind, MemoryId};
use std::collections::HashSet;

/// Owner-hydrated facts needed for deterministic canonical representative decisions.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DreamCanonicalMemory {
    pub id: MemoryId,
    pub lifecycle_state: String,
    pub archived: bool,
    pub superseded_by: Option<MemoryId>,
    pub source_timestamp_ns: Option<i64>,
    pub authority_anchor: Option<(String, String)>,
    /// Owner-proved durable birth evidence used only by historical policy.
    pub publication_birth: bool,
}

/// Proof computed by the owning Memory/Freshness adapter for a possible superseder.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DreamCanonicalSupersederFact {
    pub target: MemoryId,
    pub superseder: MemoryId,
    pub target_had_canonical_authority: bool,
}

/// Selects source-related superseders whose target has owner-proved canonical authority.
/// `authority` contains the hydrated result of Root's current/historical revision checks.
pub fn canonical_superseders(
    source: MemoryId,
    relations: &[GraphRelation],
    authority: &[DreamCanonicalSupersederFact],
) -> Vec<MemoryId> {
    let authority: HashSet<_> = authority
        .iter()
        .filter(|fact| fact.target_had_canonical_authority)
        .map(|fact| (fact.target, fact.superseder))
        .collect();
    let mut candidates: Vec<_> = relations
        .iter()
        .filter(|relation| {
            relation.kind == GraphRelationKind::Supersedes
                && (relation.source == source || relation.target == source)
                && unique_superseder(relations, relation.target) == Some(relation.source)
                && authority.contains(&(relation.target, relation.source))
        })
        .map(|relation| relation.source)
        .collect();
    candidates.sort();
    candidates.dedup();
    candidates
}

/// Returns the only distinct active superseder of a target, if it is unique.
pub fn unique_superseder(relations: &[GraphRelation], target: MemoryId) -> Option<MemoryId> {
    let mut sources: Vec<_> = relations
        .iter()
        .filter(|relation| {
            relation.kind == GraphRelationKind::Supersedes && relation.target == target
        })
        .map(|relation| relation.source)
        .collect();
    sources.sort();
    sources.dedup();
    match sources.as_slice() {
        [source] => Some(*source),
        _ => None,
    }
}

/// Resolves a supersession chain only when archived pointers, unique active edges,
/// durable birth proofs, and acyclicity agree on the supplied owner snapshot.
pub fn historical_supersession_terminal(
    root: MemoryId,
    relations: &[GraphRelation],
    memories: &[DreamCanonicalMemory],
) -> Option<MemoryId> {
    let facts: std::collections::HashMap<_, _> =
        memories.iter().map(|memory| (memory.id, memory)).collect();
    let mut current = root;
    let mut visited = HashSet::new();
    loop {
        if !visited.insert(current) {
            return None;
        }
        let memory = facts.get(&current)?;
        let Some(superseder) = memory.superseded_by else {
            break;
        };
        if !memory.archived
            || unique_superseder(relations, current) != Some(superseder)
            || !facts
                .get(&superseder)
                .is_some_and(|fact| fact.publication_birth)
        {
            return None;
        }
        current = superseder;
    }
    Some(current)
}

/// Applies duplicate corroboration rules to the authorized component snapshot.
pub fn corroborated_representative(
    source: MemoryId,
    relations: &[GraphRelation],
    memories: &[DreamCanonicalMemory],
    will_archive: &[MemoryId],
) -> Option<MemoryId> {
    let component = super::duplicate_policy::duplicate_component(source, relations);
    if component.len() < 2 {
        return None;
    }
    let component: HashSet<_> = component.into_iter().collect();
    let mut anchors = HashSet::new();
    let mut active: Vec<_> = memories
        .iter()
        .filter(|memory| component.contains(&memory.id))
        .collect();
    for memory in &active {
        if let Some(anchor) = &memory.authority_anchor {
            anchors.insert(anchor);
        }
    }
    active.retain(|memory| !memory.archived && !will_archive.contains(&memory.id));
    if anchors.len() < 2 || active.is_empty() {
        return None;
    }
    active.sort_by_key(|memory| {
        super::duplicate_policy::representative_order_key(
            &memory.lifecycle_state,
            memory.source_timestamp_ns,
            memory.id,
        )
    });
    active.first().map(|memory| memory.id)
}

/// Returns whether the authorized duplicate component contains another active Memory.
pub fn has_active_duplicate_peer(
    source: MemoryId,
    relations: &[GraphRelation],
    memories: &[DreamCanonicalMemory],
    will_archive: &[MemoryId],
) -> bool {
    let component: HashSet<_> = super::duplicate_policy::duplicate_component(source, relations)
        .into_iter()
        .collect();
    memories.iter().any(|memory| {
        memory.id != source
            && component.contains(&memory.id)
            && !memory.archived
            && !will_archive.contains(&memory.id)
    })
}

/// Chooses the deterministic representative from owner-authorized eligible facts.
pub fn select_duplicate_representative(memories: &[DreamCanonicalMemory]) -> Option<MemoryId> {
    memories
        .iter()
        .filter(|memory| !memory.archived)
        .min_by_key(|memory| {
            super::duplicate_policy::representative_order_key(
                &memory.lifecycle_state,
                memory.source_timestamp_ns,
                memory.id,
            )
        })
        .map(|memory| memory.id)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn relation(source: MemoryId, target: MemoryId) -> GraphRelation {
        GraphRelation {
            source,
            target,
            kind: GraphRelationKind::Supersedes,
            active: true,
            origin: reliquary_graph::GraphRelationOrigin::Dream,
            global_version: 1,
            graph_version: 1,
        }
    }

    fn memory(
        id: u8,
        archived: bool,
        timestamp: i64,
        anchor: Option<(&str, &str)>,
    ) -> DreamCanonicalMemory {
        DreamCanonicalMemory {
            id: [id; 32],
            lifecycle_state: "knowledge".into(),
            archived,
            superseded_by: None,
            source_timestamp_ns: Some(timestamp),
            authority_anchor: anchor.map(|(a, b)| (a.into(), b.into())),
            publication_birth: false,
        }
    }

    #[test]
    fn superseder_must_be_unique_and_owner_authorized() {
        let relations = [relation([2; 32], [1; 32]), relation([3; 32], [1; 32])];
        assert_eq!(unique_superseder(&relations, [1; 32]), None);
        let relations = [relation([2; 32], [1; 32])];
        assert_eq!(
            canonical_superseders(
                [2; 32],
                &relations,
                &[DreamCanonicalSupersederFact {
                    target: [1; 32],
                    superseder: [2; 32],
                    target_had_canonical_authority: true,
                }]
            ),
            vec![[2; 32]],
        );
        assert!(canonical_superseders([2; 32], &relations, &[]).is_empty());
    }

    #[test]
    fn corroboration_requires_distinct_anchors_and_ranks_authorized_component() {
        let mut relations = [relation([2; 32], [1; 32])];
        relations[0].kind = GraphRelationKind::DuplicateOf;
        let memories = [
            memory(1, false, 10, Some(("chat-a", "node-a"))),
            memory(2, false, 20, Some(("chat-b", "node-b"))),
        ];
        assert_eq!(
            corroborated_representative([2; 32], &relations, &memories, &[]),
            Some([1; 32])
        );
        let same_anchor = [
            memory(1, false, 10, Some(("chat-a", "node-a"))),
            memory(2, false, 20, Some(("chat-a", "node-a"))),
        ];
        assert_eq!(
            corroborated_representative([2; 32], &relations, &same_anchor, &[]),
            None
        );
    }

    #[test]
    fn historical_terminal_rejects_cycle_conflict_and_unproved_birth() {
        let cycle = [relation([2; 32], [1; 32]), relation([1; 32], [2; 32])];
        let facts = [
            DreamCanonicalMemory {
                id: [1; 32],
                lifecycle_state: "knowledge".into(),
                archived: true,
                source_timestamp_ns: Some(1),
                authority_anchor: None,
                superseded_by: Some([2; 32]),
                publication_birth: true,
            },
            DreamCanonicalMemory {
                id: [2; 32],
                lifecycle_state: "knowledge".into(),
                archived: true,
                source_timestamp_ns: Some(2),
                authority_anchor: None,
                superseded_by: Some([1; 32]),
                publication_birth: true,
            },
        ];
        assert_eq!(
            historical_supersession_terminal([1; 32], &cycle, &facts),
            None
        );

        let conflict = [relation([2; 32], [1; 32]), relation([3; 32], [1; 32])];
        assert_eq!(
            historical_supersession_terminal([1; 32], &conflict, &facts),
            None
        );

        let mut unproved = facts;
        unproved[1].publication_birth = false;
        assert_eq!(
            historical_supersession_terminal([1; 32], &cycle, &unproved),
            None
        );
    }
}
