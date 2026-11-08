//! Bounded, non-reinforcing inspection over the accepted owner projection.

use super::{MAX_MANAGEMENT_PAGE_BYTES, ManagementError, validate_page};
use reliquary_owner_session::{ManagementRead, OwnerScope, OwnerSessionError};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[path = "inspection_archive.rs"]
mod archive;
#[path = "inspection_memory.rs"]
mod memory;
#[path = "inspection_provenance.rs"]
mod provenance;
#[path = "inspection_semantic.rs"]
mod semantic;
pub use archive::{ImportedSourceReference, SourceAvailability, SourceTurn};
pub use provenance::{
    ExactMemoryProvenance, SourceCoordinate, finish_provenance, resolve_provenance,
};
pub use semantic::{CommunityMetadata, GraphDirection, GraphEdge, GraphNode, InspectedEntity};

pub use reliquary_owner_session::model::{
    EntityId, MemoryBodyId, MemoryTextField, RelationKind, RelationOrigin, Relationship,
    RelationshipEntityRef, RelationshipId, RelationshipMemoryRef, RelationshipParticipant,
    RelationshipTombstone,
};
use reliquary_owner_session::model::{MemoryError, MemoryId};
pub use reliquary_owner_session::{
    CreateRelationshipRequest, EntityRetirementRequest, MemoryRetirementRequest,
    RelationshipParticipantEvidence, RelationshipRetirementRequest,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InspectionQuery {
    Memories,
    Memory {
        id: MemoryId,
        revision: Option<u64>,
    },
    MemoryHistory {
        id: MemoryId,
        memory_cut: Option<u64>,
    },
    Provenance {
        id: MemoryId,
        revision: Option<u64>,
    },
    Conversations,
    Branches {
        conversation: String,
    },
    Transcript {
        conversation: String,
        leaf: String,
    },
    SourceTurn {
        conversation: String,
        node: String,
    },
    Neighbors {
        anchor: GraphNode,
    },
    Edge {
        source: GraphNode,
        target: GraphNode,
        kind: RelationKind,
    },
    Entities,
    Entity(EntityId),
    Relationships,
    Relationship(RelationshipId),
    Communities,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct InspectionCut {
    pub owner_id: [u8; 16],
    pub generation: [u8; 32],
    pub archive: Option<u64>,
    pub memory: u64,
    pub perception: u64,
    pub relationship: u64,
    pub graph: u64,
    pub community: Option<u64>,
}

/// An opaque continuation for a single query and accepted owner cut.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct InspectionCursor {
    cut: InspectionCut,
    file_binding: [u8; 32],
    query_digest: [u8; 32],
    offset: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct InspectionPage {
    pub cut: InspectionCut,
    pub items: Vec<InspectionItem>,
    pub next: Option<InspectionCursor>,
    /// More items exist beyond the supported inspection window.
    pub truncated: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum InspectionItem {
    Memory(super::MemorySummary),
    MemoryTerminal {
        id: String,
        revision: u64,
        memory_version: u64,
    },
    Provenance(Box<ExactMemoryProvenance>),
    Conversation(super::ConversationInspection),
    Branch(super::ManagedBranch),
    Turn(SourceTurn),
    Edge(GraphEdge),
    Entity(InspectedEntity),
    Relationship(Relationship),
    Community(CommunityMetadata),
}

pub fn inspect(
    read: ManagementRead<'_>,
    query: &InspectionQuery,
    cursor: Option<&InspectionCursor>,
    limit: usize,
) -> Result<InspectionPage, ManagementError> {
    let cut = inspection_cut(&read);
    let file_binding = Sha256::digest(
        [
            read.file_identity.0.to_le_bytes(),
            read.file_identity.1.to_le_bytes(),
        ]
        .concat(),
    )
    .into();
    archive::validate_keys(query)?;
    let query_digest = Sha256::digest(format!("management-inspection-v1:{query:?}")).into();
    let offset = cursor.map_or(0, |cursor| cursor.offset);
    validate_page(offset, limit)?;
    if cursor.is_some_and(|cursor| {
        cursor.cut != cut
            || cursor.query_digest != query_digest
            || cursor.file_binding != file_binding
    }) {
        return Err(ManagementError::Stale);
    }
    let mut page = PageBuilder {
        page: InspectionPage {
            cut,
            items: Vec::new(),
            next: None,
            truncated: false,
        },
        offset,
        limit,
        query_digest,
        file_binding,
        remaining_bytes: MAX_MANAGEMENT_PAGE_BYTES - 4096,
    };
    match query {
        InspectionQuery::Memories
        | InspectionQuery::Memory { .. }
        | InspectionQuery::MemoryHistory { .. }
        | InspectionQuery::Provenance { .. } => memory::inspect(&read, query, &mut page)?,
        InspectionQuery::Conversations
        | InspectionQuery::Branches { .. }
        | InspectionQuery::Transcript { .. }
        | InspectionQuery::SourceTurn { .. } => archive::inspect(
            read.rel.ok_or(ManagementError::Unsupported)?,
            query,
            &mut page,
        )?,
        _ => semantic::inspect(&read, query, &mut page)?,
    }
    Ok(page.page)
}

fn inspection_cut(read: &ManagementRead<'_>) -> InspectionCut {
    let (memory, perception, relationship, graph) = read.versions();
    InspectionCut {
        owner_id: read.owner_id,
        generation: read.history_generation,
        archive: read.rel.map(|rel| rel.archive_version()),
        memory,
        perception,
        relationship,
        graph,
        community: read.community().map(|snapshot| snapshot.generation),
    }
}

#[derive(Clone, Copy, Debug)]
pub struct MountedOwnerInfo {
    pub scope: OwnerScope,
    pub memory_count: usize,
    pub memory_version: u64,
    pub graph_version: u64,
    pub archive_version: Option<u64>,
    pub rel_turn_count: Option<u64>,
}

pub fn mounted_owner_info(read: ManagementRead<'_>) -> MountedOwnerInfo {
    let cut = inspection_cut(&read);
    MountedOwnerInfo {
        scope: read.rel.map_or(OwnerScope::Unspecified, |rel| rel.scope()),
        memory_count: read.memory_count(),
        memory_version: cut.memory,
        graph_version: cut.graph,
        archive_version: cut.archive,
        rel_turn_count: read.rel.map(|rel| rel.rel_turn_count()),
    }
}

struct PageBuilder {
    page: InspectionPage,
    offset: usize,
    limit: usize,
    query_digest: [u8; 32],
    file_binding: [u8; 32],
    remaining_bytes: usize,
}

impl PageBuilder {
    fn extend<T>(
        &mut self,
        mut items: impl Iterator<Item = T>,
        mut project: impl FnMut(T) -> Result<InspectionItem, ManagementError>,
    ) -> Result<(), ManagementError> {
        for value in items.by_ref().take(self.limit) {
            let item = match project(value) {
                Err(ManagementError::PageBytesExceeded) if !self.page.items.is_empty() => {
                    self.continue_page();
                    return Ok(());
                }
                result => result?,
            };
            let bytes = serde_json::to_vec(&item)
                .map_err(|error| ManagementError::Owner(error.to_string()))?
                .len()
                + 1;
            if bytes > self.remaining_bytes {
                if self.page.items.is_empty() {
                    return Err(ManagementError::PageBytesExceeded);
                }
                self.continue_page();
                return Ok(());
            }
            self.remaining_bytes -= bytes;
            self.page.items.push(item);
        }
        if items.next().is_some() {
            self.continue_page();
        }
        Ok(())
    }

    fn single(&mut self, item: InspectionItem) -> Result<(), ManagementError> {
        self.extend(std::iter::once(item).skip(self.offset), Ok)
    }

    fn continue_page(&mut self) {
        let offset = self.offset + self.page.items.len();
        if validate_page(offset, 1).is_ok() {
            self.page.next = Some(InspectionCursor {
                cut: self.page.cut,
                file_binding: self.file_binding,
                query_digest: self.query_digest,
                offset,
            });
        } else {
            self.page.truncated = true;
        }
    }
}

pub(super) fn write_error(error: OwnerSessionError) -> ManagementError {
    match error {
        OwnerSessionError::Memory(MemoryError::MutationConflict) => ManagementError::Conflict,
        OwnerSessionError::Memory(MemoryError::RevisionConflict) => ManagementError::Stale,
        OwnerSessionError::Memory(MemoryError::FieldTooLarge | MemoryError::InvalidField(_)) => {
            ManagementError::InvalidInput("Memory field")
        }
        other => ManagementError::Owner(other.to_string()),
    }
}
