//! Read-only streaming projection for prepared graph renderers.

use super::{
    InspectionCut, ManagementError, ManagementRead, MemoryId, MemoryListFilters, inspection_cut,
    memory_list::{matching_memory, validate_filters},
};
use reliquary_owner_session::{
    SemanticGraphCommunity,
    model::{NodeRef, Relation},
};
use std::collections::{HashMap, HashSet};
use std::ops::ControlFlow;

const MEMORY_BATCH: usize = 128;
const GRAPH_TITLE_BYTES: usize = 256;
const MAX_SCANNED_MEMORIES: usize = 2_000_000;
const MAX_SCANNED_RELATION_STATES: usize = 20_000_000;

/// Read-only renderer metadata at one accepted inspection cut. Memory bodies
/// stay in Root even when a query matches their content.
pub enum SemanticGraphProjectionItem {
    Community(SemanticGraphCommunity),
    Memory {
        id: MemoryId,
        title: Option<String>,
        community_id: Option<[u8; 32]>,
    },
    Relation(Relation),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SemanticGraphProjectionStatus {
    Complete,
    VisitorStopped,
    ScanLimit,
}

pub fn memory_matches_filter(
    read: ManagementRead<'_>,
    expected_cut: InspectionCut,
    filters: &MemoryListFilters,
    id: MemoryId,
) -> Result<bool, ManagementError> {
    let cut = inspection_cut(&read);
    if cut != expected_cut {
        return Err(ManagementError::Stale);
    }
    validate_filters(filters)?;
    let communities = read
        .community()
        .filter(|snapshot| snapshot.graph_version == cut.graph);
    if filters.community_id.is_some() && communities.is_none() {
        return Err(ManagementError::Unsupported);
    }
    if read.memory_revision_records(id, cut.memory).is_none() {
        return Ok(false);
    }
    let community_id = filters.community_id.and_then(|expected| {
        let groups = &communities?.communities;
        let index = groups
            .binary_search_by_key(&expected, |community| community.id.0)
            .ok()?;
        groups[index]
            .members
            .binary_search(&id.0)
            .is_ok()
            .then_some(expected)
    });
    Ok(matching_memory(
        &read,
        id,
        cut.memory,
        filters,
        &filters.query.trim().to_lowercase(),
        community_id,
    )?
    .is_some())
}

pub fn visit_semantic_graph_projection(
    read: ManagementRead<'_>,
    expected_cut: InspectionCut,
    filters: &MemoryListFilters,
    mut visit: impl FnMut(SemanticGraphProjectionItem) -> ControlFlow<()>,
) -> Result<SemanticGraphProjectionStatus, ManagementError> {
    use SemanticGraphProjectionStatus::{Complete, ScanLimit, VisitorStopped};
    let cut = inspection_cut(&read);
    if cut != expected_cut {
        return Err(ManagementError::Stale);
    }
    validate_filters(filters)?;
    if read.memory_count() > MAX_SCANNED_MEMORIES {
        return Ok(ScanLimit);
    }
    let needle = filters.query.trim().to_lowercase();
    let communities = read
        .community()
        .filter(|snapshot| snapshot.graph_version == cut.graph);
    if filters.community_id.is_some() && communities.is_none() {
        return Err(ManagementError::Unsupported);
    }
    if communities.is_some_and(|snapshot| snapshot.communities.len() > MAX_SCANNED_MEMORIES) {
        return Ok(ScanLimit);
    }

    let mut membership = HashMap::new();
    let mut examined_members = 0_usize;
    for community in communities
        .into_iter()
        .flat_map(|snapshot| &snapshot.communities)
    {
        if filters
            .community_id
            .is_some_and(|expected| expected != community.id.0)
        {
            continue;
        }
        examined_members = examined_members.saturating_add(community.members.len());
        if examined_members > MAX_SCANNED_MEMORIES {
            return Ok(ScanLimit);
        }
        if visit(SemanticGraphProjectionItem::Community(
            SemanticGraphCommunity {
                id: community.id,
                name: read.community_name(community.id).map(str::to_owned),
                member_count: community.members.len(),
            },
        ))
        .is_break()
        {
            return Ok(VisitorStopped);
        }
        for id in &community.members {
            membership.entry(*id).or_insert(community.id.0);
        }
    }

    let mut after = None;
    let mut eligible = HashSet::new();
    loop {
        let ids = read.memory_ids(after, MEMORY_BATCH);
        if ids.is_empty() {
            break;
        }
        after = ids.last().copied();
        for id in ids {
            let community_id = membership.get(&id.0).copied();
            if let Some(memory) =
                matching_memory(&read, id, cut.memory, filters, &needle, community_id)?
            {
                if visit(SemanticGraphProjectionItem::Memory {
                    id,
                    title: memory
                        .body
                        .map(|body| super::memory_list::bounded(&body.title, GRAPH_TITLE_BYTES)),
                    community_id,
                })
                .is_break()
                {
                    return Ok(VisitorStopped);
                }
                eligible.insert(id.0);
            }
        }
    }
    if eligible.is_empty() {
        return Ok(Complete);
    }
    if read.relation_state_count() > MAX_SCANNED_RELATION_STATES {
        return Ok(ScanLimit);
    }
    for relation in read.active_relations() {
        if let (NodeRef::Memory(source), NodeRef::Memory(target)) =
            (relation.source, relation.target)
            && eligible.contains(&source)
            && eligible.contains(&target)
            && visit(SemanticGraphProjectionItem::Relation(*relation)).is_break()
        {
            return Ok(VisitorStopped);
        }
    }
    Ok(Complete)
}
