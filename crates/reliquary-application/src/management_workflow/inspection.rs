//! Bounded, non-reinforcing inspection over the accepted owner projection.

use super::{MAX_MANAGEMENT_PAGE_BYTES, ManagementError, validate_page};
use reliquary_owner_session::{ManagementRead, OwnerScope, OwnerSessionError};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[path = "inspection_archive.rs"]
mod archive;
#[path = "inspection_association.rs"]
mod association;
#[path = "inspection_graph.rs"]
mod graph;
#[path = "inspection_graph_projection.rs"]
mod graph_projection;
pub use association::{
    MemoryAssociationAction, MemoryAssociationReceipt, MemoryAssociationRequest,
    change_memory_association,
};
#[path = "inspection_memory.rs"]
mod memory;
#[path = "inspection_memory_list.rs"]
mod memory_list;
#[path = "inspection_provenance.rs"]
mod provenance;
#[path = "inspection_semantic.rs"]
mod semantic;
pub use archive::{ImportedSourceReference, SourceAvailability, SourceTurn};
pub use graph::{MemoryGraphEdge, MemoryGraphNode, MemoryGraphPage, MemoryGraphQuery};
pub use graph_projection::{
    SemanticGraphProjectionItem, SemanticGraphProjectionStatus, memory_matches_filter,
    visit_semantic_graph_projection,
};
pub use memory_list::{
    MemoryCanonicalStatus, MemoryListEntry, MemoryListFilters, MemoryListLifecycle,
};
pub use provenance::{
    ExactMemoryProvenance, SourceCoordinate, finish_provenance, resolve_provenance,
};
pub use semantic::{
    CommunityMetadata, CommunityNameOrigin, GraphDirection, GraphEdge, GraphNode, InspectedEntity,
};

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
    MemorySummaries(MemoryListFilters),
    MemoryGraph(MemoryGraphQuery),
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
    CommunityMembers {
        id: [u8; 32],
        expected_generation: u64,
    },
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
    MemoryListEntry(MemoryListEntry),
    MemoryGraphPage(MemoryGraphPage),
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
    CommunityMember {
        id: [u8; 32],
    },
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
        InspectionQuery::MemoryGraph(query) => graph::inspect(&read, query, &mut page)?,
        InspectionQuery::MemorySummaries(filters) => {
            memory_list::inspect(&read, filters, &mut page)?
        }
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

pub fn inspection_cut(read: &ManagementRead<'_>) -> InspectionCut {
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
            if !self.push(item)? {
                self.continue_page();
                return Ok(());
            }
        }
        if items.next().is_some() {
            self.continue_page();
        }
        Ok(())
    }

    fn push(&mut self, item: InspectionItem) -> Result<bool, ManagementError> {
        let bytes = serde_json::to_vec(&item)
            .map_err(|error| ManagementError::Owner(error.to_string()))?
            .len()
            + 1;
        if bytes > self.remaining_bytes {
            if self.page.items.is_empty() {
                return Err(ManagementError::PageBytesExceeded);
            }
            return Ok(false);
        }
        self.remaining_bytes -= bytes;
        self.page.items.push(item);
        Ok(true)
    }

    fn single(&mut self, item: InspectionItem) -> Result<(), ManagementError> {
        self.extend(std::iter::once(item).skip(self.offset), Ok)
    }

    fn continue_page(&mut self) {
        self.continue_at(self.offset + self.page.items.len());
    }

    fn continue_at(&mut self, offset: usize) {
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
