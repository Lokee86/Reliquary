//! Filtered dashboard Memory listing over one immutable owner cut.

use super::*;
use crate::management_workflow::hex;
use reliquary_owner_session::model::{MemoryBody, MemoryRecord, NodeRef};

const MAX_QUERY_BYTES: usize = 4096;
const MAX_FILTER_BYTES: usize = 128;
const MAX_TITLE_BYTES: usize = 256;
const MAX_EXCERPT_BYTES: usize = 512;
const SCAN_BATCH: usize = 128;
const SCAN_LIMIT: usize = 512;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct MemoryListFilters {
    pub query: String,
    pub category: Option<String>,
    pub memory_type: Option<String>,
    pub lifecycle: Option<MemoryListLifecycle>,
    pub community_id: Option<[u8; 32]>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryListLifecycle {
    Active,
    Retired,
    Tombstoned,
    Superseded,
    Unavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryCanonicalStatus {
    Canonical,
    Duplicate,
    Superseded,
    Unknown,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct MemoryListEntry {
    pub id: String,
    pub revision: u64,
    pub body_id: Option<String>,
    pub title: Option<String>,
    pub excerpt: String,
    pub category: String,
    pub memory_type: String,
    pub lifecycle: MemoryListLifecycle,
    pub canonical_status: MemoryCanonicalStatus,
    pub community_id: Option<String>,
}

pub(super) fn inspect(
    read: &ManagementRead<'_>,
    filters: &MemoryListFilters,
    page: &mut PageBuilder,
) -> Result<(), ManagementError> {
    validate_filters(filters)?;
    let needle = filters.query.trim().to_lowercase();
    let cut = page.page.cut;
    let communities = read
        .community()
        .filter(|snapshot| snapshot.graph_version == cut.graph);
    if filters.community_id.is_some() && communities.is_none() {
        return Err(ManagementError::Unsupported);
    }

    // The cursor offset counts examined Memory IDs so sparse filters never
    // force a full owner scan in one request.
    let mut offset = page.offset;
    for _ in 0..SCAN_LIMIT / SCAN_BATCH {
        let ids = read.memory_ids_from_offset(offset, SCAN_BATCH);
        let exhausted = ids.len() < SCAN_BATCH;
        for id in ids {
            let community_id = communities.and_then(|snapshot| {
                snapshot
                    .communities
                    .iter()
                    .find(|community| community.members.binary_search(&id.0).is_ok())
                    .map(|community| community.id.0)
            });
            if let Some(item) = project(read, id, cut.memory, filters, &needle, community_id)? {
                if page.page.items.len() == page.limit
                    || !page.push(InspectionItem::MemoryListEntry(item))?
                {
                    page.continue_at(offset);
                    return Ok(());
                }
            }
            offset += 1;
        }
        if exhausted {
            return Ok(());
        }
    }
    if !read.memory_ids_from_offset(offset, 1).is_empty() {
        page.continue_at(offset);
    }
    Ok(())
}

pub(super) fn validate_filters(filters: &MemoryListFilters) -> Result<(), ManagementError> {
    if filters.query.len() > MAX_QUERY_BYTES
        || [filters.category.as_deref(), filters.memory_type.as_deref()]
            .into_iter()
            .flatten()
            .any(|value| value.trim().is_empty() || value.len() > MAX_FILTER_BYTES)
    {
        return Err(ManagementError::InvalidInput("Memory list filters"));
    }
    Ok(())
}

fn project(
    read: &ManagementRead<'_>,
    id: MemoryId,
    cut: u64,
    filters: &MemoryListFilters,
    needle: &str,
    community_id: Option<[u8; 32]>,
) -> Result<Option<MemoryListEntry>, ManagementError> {
    let Some(MatchingMemory {
        record,
        body,
        lifecycle,
        revision,
    }) = matching_memory(read, id, cut, filters, needle, community_id)?
    else {
        return Ok(None);
    };
    Ok(Some(MemoryListEntry {
        id: hex(&id.0),
        revision,
        body_id: (lifecycle != MemoryListLifecycle::Tombstoned).then(|| hex(&record.body_id.0)),
        title: body.map(|body| bounded(&body.title, MAX_TITLE_BYTES)),
        excerpt: body.map_or_else(String::new, |body| {
            bounded(&body.content, MAX_EXCERPT_BYTES)
        }),
        category: record.category.clone(),
        memory_type: record.memory_type.clone(),
        lifecycle,
        canonical_status: canonical_status(read, id, lifecycle),
        community_id: community_id.map(|id| hex(&id)),
    }))
}

pub(super) struct MatchingMemory<'a> {
    record: &'a MemoryRecord,
    pub(super) body: Option<&'a MemoryBody>,
    lifecycle: MemoryListLifecycle,
    revision: u64,
}

pub(super) fn matching_memory<'a>(
    read: &'a ManagementRead<'_>,
    id: MemoryId,
    cut: u64,
    filters: &MemoryListFilters,
    needle: &str,
    community_id: Option<[u8; 32]>,
) -> Result<Option<MatchingMemory<'a>>, ManagementError> {
    if filters
        .community_id
        .is_some_and(|expected| Some(expected) != community_id)
    {
        return Ok(None);
    }
    let tombstone = read.memory_terminal(id, cut);
    let (record, body) = if tombstone.is_some() {
        let record = read
            .memory_revision_records(id, cut)
            .and_then(|history| history.last())
            .ok_or(ManagementError::NotFound)?;
        (record, None)
    } else {
        read.memory_projection(id, None)
            .map_err(|error| ManagementError::Owner(error.to_string()))?
    };
    let lifecycle = if tombstone.is_some() {
        MemoryListLifecycle::Tombstoned
    } else if record.superseded_by.is_some() {
        MemoryListLifecycle::Superseded
    } else if record.archived {
        MemoryListLifecycle::Retired
    } else if body.is_none() {
        MemoryListLifecycle::Unavailable
    } else {
        MemoryListLifecycle::Active
    };
    if filters
        .category
        .as_ref()
        .is_some_and(|value| value != &record.category)
        || filters
            .memory_type
            .as_ref()
            .is_some_and(|value| value != &record.memory_type)
        || filters.lifecycle.is_some_and(|value| value != lifecycle)
        || (!needle.is_empty()
            && !body.is_some_and(|body| {
                body.title.to_lowercase().contains(needle)
                    || body.content.to_lowercase().contains(needle)
            }))
    {
        return Ok(None);
    }
    Ok(Some(MatchingMemory {
        record,
        body,
        revision: tombstone.map_or(record.revision, |entry| entry.revision),
        lifecycle,
    }))
}

pub(super) fn bounded(value: &str, bytes: usize) -> String {
    value[..value.floor_char_boundary(bytes)].to_owned()
}

fn canonical_status(
    read: &ManagementRead<'_>,
    id: MemoryId,
    lifecycle: MemoryListLifecycle,
) -> MemoryCanonicalStatus {
    match lifecycle {
        MemoryListLifecycle::Superseded => return MemoryCanonicalStatus::Superseded,
        MemoryListLifecycle::Tombstoned => return MemoryCanonicalStatus::Unknown,
        _ => {}
    }
    let node = NodeRef::Memory(id.0);
    let mut canonical = false;
    for relation in read
        .incident_relations(node)
        .filter(|relation| relation.kind == RelationKind::DuplicateOf)
    {
        if relation.source == node {
            return MemoryCanonicalStatus::Duplicate;
        }
        canonical = true;
    }
    if canonical {
        MemoryCanonicalStatus::Canonical
    } else {
        MemoryCanonicalStatus::Unknown
    }
}
