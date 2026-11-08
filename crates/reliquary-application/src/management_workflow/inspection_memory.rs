use super::*;
use crate::management_workflow::{MemorySummary, hex};
use reliquary_owner_session::model::{MemoryBody, MemoryRecord, TerminalTombstone};

pub(super) fn inspect(
    read: &ManagementRead<'_>,
    query: &InspectionQuery,
    page: &mut PageBuilder,
) -> Result<(), ManagementError> {
    let head = page.page.cut.memory;
    match *query {
        InspectionQuery::Memories => {
            let mut offset = page.offset;
            let mut after = None;
            while offset > 0 {
                let batch = read.memory_ids(after, offset);
                if batch.is_empty() {
                    return Ok(());
                }
                after = batch.last().copied();
                offset -= batch.len();
            }
            let ids = read.memory_ids(after, page.limit);
            let extra = if ids.len() == page.limit {
                read.memory_ids(ids.last().copied(), 1)
            } else {
                Vec::new()
            };
            page.extend(ids.into_iter().chain(extra), |id| {
                match read.memory_terminal(id, head) {
                    Some(tombstone) => Ok(terminal_item(id, tombstone)),
                    None => project_memory(exact(read, id, None)?),
                }
            })
        }
        InspectionQuery::Memory { id, revision } => {
            if revision.is_none() {
                if let Some(tombstone) = read.memory_terminal(id, head) {
                    return page.single(terminal_item(id, tombstone));
                }
            }
            page.single(project_memory(exact(read, id, revision)?)?)
        }
        InspectionQuery::MemoryHistory { id, memory_cut } => {
            let cut = memory_cut.unwrap_or(head);
            if cut > head {
                return Err(ManagementError::Stale);
            }
            let records = read
                .memory_revision_records(id, cut)
                .ok_or(ManagementError::NotFound)?;
            let revisions = records.iter().map(|record| Some(record.revision));
            let tombstone = read.memory_terminal(id, cut);
            let items = revisions.chain(tombstone.map(|_| None)).skip(page.offset);
            page.extend(items, |revision| match revision {
                Some(revision) => project_memory(exact(read, id, Some(revision))?),
                None => Ok(terminal_item(id, tombstone.expect("selected tombstone"))),
            })
        }
        InspectionQuery::Provenance { id, revision } => {
            let (record, _) = exact(read, id, revision)?;
            let provenance = provenance::prepare_provenance(read, record)?;
            page.single(InspectionItem::Provenance(Box::new(provenance)))
        }
        _ => unreachable!("Memory inspection query"),
    }
}

fn exact<'a>(
    read: &'a ManagementRead<'_>,
    id: MemoryId,
    revision: Option<u64>,
) -> Result<(&'a MemoryRecord, Option<&'a MemoryBody>), ManagementError> {
    read.memory_projection(id, revision).map_err(memory_error)
}

fn memory_error(error: MemoryError) -> ManagementError {
    match error {
        MemoryError::MissingMemory => ManagementError::NotFound,
        MemoryError::TerminalTombstone => ManagementError::Terminal,
        other => ManagementError::Owner(other.to_string()),
    }
}

fn terminal_item(id: MemoryId, tombstone: TerminalTombstone) -> InspectionItem {
    InspectionItem::MemoryTerminal {
        id: hex(&id.0),
        revision: tombstone.revision,
        memory_version: tombstone.memory_version,
    }
}

fn project_memory(
    (record, body): (&MemoryRecord, Option<&MemoryBody>),
) -> Result<InspectionItem, ManagementError> {
    let (title, content) = body.map_or(("", ""), |body| {
        (body.title.as_str(), body.content.as_str())
    });
    let bytes = title.len()
        + content.len()
        + record.category.len()
        + record.memory_type.len()
        + record.lifecycle_state.len();
    if bytes > MAX_MANAGEMENT_PAGE_BYTES - 4096 {
        return Err(ManagementError::PageBytesExceeded);
    }
    Ok(InspectionItem::Memory(MemorySummary {
        id: hex(&record.id.0),
        revision: record.revision,
        body_id: hex(&record.body_id.0),
        memory_version: record.memory_version,
        title: title.to_owned(),
        content: content.to_owned(),
        category: record.category.clone(),
        memory_type: record.memory_type.clone(),
        lifecycle_state: record.lifecycle_state.clone(),
        archived: record.archived,
        payload_available: body.is_some(),
    }))
}
