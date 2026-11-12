use super::super::*;
use reliquary_memory_core::{MemoryBody, MemoryRecord};

/// Immutable owner-local projection. The stores and writer remain private.
pub struct ManagementRead<'a> {
    common: &'a CommonState,
    pub owner_id: [u8; 16],
    pub history_generation: [u8; 32],
    pub file_identity: (u64, u64),
    pub rel: Option<&'a RelSession>,
}

impl RelSession {
    pub fn management_read(&self) -> ManagementRead<'_> {
        ManagementRead {
            common: &self.common,
            owner_id: self.owner_id,
            history_generation: self.history_generation,
            file_identity: self.container_file_identity,
            rel: Some(self),
        }
    }
}

impl PhySession {
    pub fn management_read(&self) -> ManagementRead<'_> {
        ManagementRead {
            common: &self.common,
            owner_id: self.owner_id,
            history_generation: self.history_generation,
            file_identity: self.container_file_identity,
            rel: None,
        }
    }
}

impl PhyWriter {
    pub fn management_read(&self) -> Result<ManagementRead<'_>, OwnerSessionError> {
        if self.poisoned {
            return Err(OwnerSessionError::Record(
                "PHY writer requires reopen".into(),
            ));
        }
        Ok(ManagementRead {
            common: &self.common,
            owner_id: self.owner_id,
            history_generation: self.history_generation,
            file_identity: self.container_file_identity,
            rel: None,
        })
    }
}

impl ManagementRead<'_> {
    /// Open the renderer's bounded semantic graph view at this read's exact owner cut.
    pub fn semantic_graph_snapshot(&self) -> super::SemanticGraphSnapshot<'_> {
        let (memory, perception, relationship, graph) = self.versions();
        let community = self.community().map(|snapshot| snapshot.generation);
        super::SemanticGraphSnapshot {
            common: self.common,
            cut: super::SemanticGraphCut {
                owner_id: self.owner_id,
                generation: self.history_generation,
                archive: self.rel.map(|rel| rel.archive_version()),
                memory,
                perception,
                relationship,
                graph,
                community,
            },
        }
    }

    /// Exact first-cycle identity and settlement, including manually started passes.
    pub fn first_dream_pass(&self, source: MemoryId) -> Option<(&str, bool)> {
        self.common.dreams.first_pass(source)
    }

    pub fn mention(&self, key: MemoryEntityMentionKey) -> Option<&MemoryEntityResolution> {
        self.common.perception.mention(key)
    }

    pub fn versions(&self) -> (u64, u64, u64, u64) {
        (
            self.common.memories.memory_version(),
            self.common.perception.version(),
            self.common.perception.relationship_version(),
            self.common.graph.graph_version(),
        )
    }

    pub fn memory_count(&self) -> usize {
        self.common.memories.retained_count()
    }

    pub fn memory_ids(&self, after: Option<MemoryId>, limit: usize) -> Vec<MemoryId> {
        self.common.memories.memory_ids_page(after, limit.min(128))
    }

    pub fn memory_ids_from_offset(&self, offset: usize, limit: usize) -> Vec<MemoryId> {
        self.common
            .memories
            .memory_ids()
            .skip(offset)
            .take(limit.min(128))
            .collect()
    }

    pub fn memory_revision_records(&self, id: MemoryId, cut: u64) -> Option<&[MemoryRecord]> {
        self.common.memories.revision_records_at_cut(id, cut)
    }

    pub fn memory_projection(
        &self,
        id: MemoryId,
        revision: Option<u64>,
    ) -> Result<(&MemoryRecord, Option<&MemoryBody>), MemoryError> {
        self.common.memories.revision_projection(id, revision)
    }

    pub fn memory_terminal(&self, id: MemoryId, cut: u64) -> Option<TerminalTombstone> {
        self.common.memories.terminal_at_cut(id, cut)
    }

    pub fn memory_is_readable(&self, id: MemoryId) -> bool {
        self.common.memories.current_body_available(id)
    }

    pub fn entities(&self) -> impl Iterator<Item = &Entity> {
        self.common.perception.entities()
    }

    pub fn entity(&self, id: EntityId) -> Option<&Entity> {
        self.common.perception.entity(id)
    }

    pub fn entity_terminal(&self, id: EntityId) -> bool {
        self.common.perception.entity_tombstone(id).is_some()
    }

    pub fn relationships(&self) -> impl Iterator<Item = &Relationship> {
        self.common.perception.relationships()
    }

    pub fn relationship(&self, id: RelationshipId) -> Option<&Relationship> {
        self.common.perception.relationship(id)
    }

    pub fn relationship_terminal(&self, id: RelationshipId) -> bool {
        self.common.perception.relationship_tombstone(id).is_some()
    }

    pub fn incident_relations(&self, node: NodeRef) -> impl Iterator<Item = &Relation> {
        self.common.graph.incident_relations(node)
    }

    pub fn active_relations(&self) -> impl Iterator<Item = &Relation> {
        self.common.graph.active_relations()
    }

    pub fn relation_state_count(&self) -> usize {
        self.common.graph.relation_state_count()
    }

    pub fn relation(
        &self,
        source: NodeRef,
        target: NodeRef,
        kind: reliquary_graph::RelationKind,
    ) -> Option<&Relation> {
        self.common.graph.relation(source, target, kind)
    }

    pub fn community(&self) -> Option<&CommunitySnapshot> {
        self.common.communities.current()
    }

    pub fn community_name(&self, id: reliquary_graph::CommunityId) -> Option<&str> {
        self.common.communities.user_name(id)
    }

    pub fn community_rename_receipt(
        &self,
        key: &[u8; 32],
    ) -> Option<&reliquary_graph::CommunityRenameReceipt> {
        self.common.communities.rename_receipt(key)
    }
}
