use crate::{
    ConversationInspection, Instance, ManagementPage, MemoryId, MemoryProvenance, MemorySummary,
    OwnerGrant, OwnerId, OwnerKind, ProductError, Rel, RelManagementHealth,
    SemanticOwnerStatistics, TranscriptTurn,
};
use std::sync::{Arc, Mutex};

pub use reliquary_application::inspection::{
    CommunityMetadata, CommunityNameOrigin, CreateRelationshipRequest, EntityRetirementRequest,
    ExactMemoryProvenance, GraphDirection, GraphEdge, GraphNode, ImportedSourceReference,
    InspectedEntity, InspectionCursor, InspectionCut, InspectionItem, InspectionPage,
    InspectionQuery, MemoryBodyId, MemoryCanonicalStatus, MemoryGraphEdge, MemoryGraphNode,
    MemoryGraphPage, MemoryGraphQuery, MemoryListEntry, MemoryListFilters, MemoryListLifecycle,
    MemoryRetirementRequest, MemoryTextField, RelationKind, RelationOrigin, Relationship,
    RelationshipEntityRef, RelationshipMemoryRef, RelationshipParticipant,
    RelationshipParticipantEvidence, RelationshipRetirementRequest, RelationshipTombstone,
    SemanticGraphProjectionItem, SemanticGraphProjectionStatus, SourceAvailability,
    SourceCoordinate, SourceTurn,
};
pub use reliquary_application::inspection::{
    MemoryAssociationAction, MemoryAssociationReceipt, MemoryAssociationRequest,
};
pub use reliquary_application::{
    CommunityRenameRequest, CommunityRenameResult, CorrectMemoryReceipt, CorrectMemoryRequest,
    CreateMemoryReceipt, CreateMemoryRequest, RetireMemoryReceipt,
};
pub use reliquary_owner_session::WithdrawEntityMentionRequest;
pub use reliquary_owner_session::model::{
    EntityResolutionReason, EntityTombstone, MemoryEntityMentionKey, MemoryEntityResolution,
    MemoryEntityResolutionStatus, TerminalTombstone,
};

pub use reliquary_graph::{CommunityId, CommunitySnapshot, NodeRef, Relation};
pub use reliquary_owner_session::{
    SemanticGraphCommunity, SemanticGraphCommunityMemberPage, SemanticGraphCommunityPage,
    SemanticGraphCut, SemanticGraphMemory, SemanticGraphMemoryPage, SemanticGraphRelationPage,
};
impl Instance {
    /// Read the current graph and inspection cut without visiting a Memory.
    pub fn semantic_graph_inspection_cut(
        &self,
        owner: OwnerId,
        kind: OwnerKind,
    ) -> Result<InspectionCut, ProductError> {
        match kind {
            OwnerKind::Rel => self.require_rel_read(owner)?,
            OwnerKind::Phylactery => self.require_phy_read(owner)?,
        }
        self.inner
            .semantic_graph_inspection_cut(owner.into(), kind == OwnerKind::Phylactery)
            .map_err(Into::into)
    }

    /// Test one Memory against the same filters used by the list at this cut.
    pub fn memory_matches_filter(
        &self,
        owner: OwnerId,
        kind: OwnerKind,
        expected_cut: InspectionCut,
        filters: &MemoryListFilters,
        id: MemoryId,
    ) -> Result<bool, ProductError> {
        match kind {
            OwnerKind::Rel => self.require_rel_read(owner)?,
            OwnerKind::Phylactery => self.require_phy_read(owner)?,
        }
        self.inner
            .memory_matches_filter(
                owner.into(),
                kind == OwnerKind::Phylactery,
                expected_cut,
                filters,
                id,
            )
            .map_err(Into::into)
    }

    /// Visit filtered renderer metadata and induced relationships at one exact
    /// owner cut. Returning `ControlFlow::Break` stops the read at the caller's
    /// budget. The visitor must not reenter this owner while the read is active.
    /// A scan limit means the caller must discard its incomplete projection.
    pub fn visit_semantic_graph_projection(
        &self,
        owner: OwnerId,
        kind: OwnerKind,
        expected_cut: InspectionCut,
        filters: &MemoryListFilters,
        visit: impl FnMut(SemanticGraphProjectionItem) -> std::ops::ControlFlow<()>,
    ) -> Result<SemanticGraphProjectionStatus, ProductError> {
        match kind {
            OwnerKind::Rel => self.require_rel_read(owner)?,
            OwnerKind::Phylactery => self.require_phy_read(owner)?,
        }
        self.inner
            .visit_semantic_graph_projection(
                owner.into(),
                kind == OwnerKind::Phylactery,
                expected_cut,
                filters,
                visit,
            )
            .map_err(Into::into)
    }

    /// Read one bounded page from an exact-cut semantic graph source.
    pub fn semantic_graph_memory_page(
        &self,
        owner: OwnerId,
        kind: OwnerKind,
        expected_cut: Option<SemanticGraphCut>,
        after: Option<MemoryId>,
        limit: usize,
    ) -> Result<(SemanticGraphCut, SemanticGraphMemoryPage), ProductError> {
        match kind {
            OwnerKind::Rel => self.require_rel_read(owner)?,
            OwnerKind::Phylactery => self.require_phy_read(owner)?,
        }
        self.inner
            .semantic_graph_memory_page(
                owner.into(),
                kind == OwnerKind::Phylactery,
                expected_cut,
                after,
                limit,
            )
            .map_err(Into::into)
    }

    /// Read active relations at the caller-pinned semantic graph cut.
    pub fn semantic_graph_relation_page(
        &self,
        owner: OwnerId,
        kind: OwnerKind,
        expected_cut: SemanticGraphCut,
        offset: usize,
        limit: usize,
    ) -> Result<(SemanticGraphCut, SemanticGraphRelationPage), ProductError> {
        match kind {
            OwnerKind::Rel => self.require_rel_read(owner)?,
            OwnerKind::Phylactery => self.require_phy_read(owner)?,
        }
        self.inner
            .semantic_graph_relation_page(
                owner.into(),
                kind == OwnerKind::Phylactery,
                expected_cut,
                offset,
                limit,
            )
            .map_err(Into::into)
    }

    /// Read a bounded page of accepted community metadata at the pinned cut.
    pub fn semantic_graph_community_page(
        &self,
        owner: OwnerId,
        kind: OwnerKind,
        expected_cut: SemanticGraphCut,
        after: Option<reliquary_graph::CommunityId>,
        limit: usize,
    ) -> Result<(SemanticGraphCut, SemanticGraphCommunityPage), ProductError> {
        match kind {
            OwnerKind::Rel => self.require_rel_read(owner)?,
            OwnerKind::Phylactery => self.require_phy_read(owner)?,
        }
        self.inner
            .semantic_graph_community_page(
                owner.into(),
                kind == OwnerKind::Phylactery,
                expected_cut,
                after,
                limit,
            )
            .map_err(Into::into)
    }

    /// Read a bounded member page for one accepted community at the pinned cut.
    pub fn semantic_graph_community_member_page(
        &self,
        owner: OwnerId,
        kind: OwnerKind,
        expected_cut: SemanticGraphCut,
        community_id: reliquary_graph::CommunityId,
        after: Option<MemoryId>,
        limit: usize,
    ) -> Result<(SemanticGraphCut, SemanticGraphCommunityMemberPage), ProductError> {
        match kind {
            OwnerKind::Rel => self.require_rel_read(owner)?,
            OwnerKind::Phylactery => self.require_phy_read(owner)?,
        }
        self.inner
            .semantic_graph_community_member_page(
                owner.into(),
                kind == OwnerKind::Phylactery,
                expected_cut,
                community_id,
                after,
                limit,
            )
            .map_err(Into::into)
    }
    /// Inspect one explicitly scoped mounted owner. A cursor only continues a read.
    pub fn inspect_owner(
        &self,
        owner: OwnerId,
        kind: OwnerKind,
        query: &InspectionQuery,
        cursor: Option<&InspectionCursor>,
        limit: usize,
    ) -> Result<InspectionPage, ProductError> {
        match kind {
            OwnerKind::Rel => self.require_rel_read(owner)?,
            OwnerKind::Phylactery => self.require_phy_read(owner)?,
        }
        self.inner
            .inspect_management(
                owner.into(),
                kind == OwnerKind::Phylactery,
                query,
                cursor,
                limit,
            )
            .map_err(Into::into)
    }

    pub(crate) fn mounted_owner_info(
        &self,
        owner: OwnerId,
        kind: OwnerKind,
    ) -> Result<crate::OwnerInfo, ProductError> {
        let info = self
            .inner
            .mounted_management_info(owner.into(), kind == OwnerKind::Phylactery)?;
        Ok(crate::OwnerInfo {
            format: "2.1",
            kind,
            owner_id: owner,
            scope: info.scope.into(),
            memory_count: info.memory_count,
            memory_version: info.memory_version,
            graph_version: info.graph_version,
            archive_version: info.archive_version,
            rel_turn_count: info.rel_turn_count,
        })
    }
}

#[derive(Clone)]
pub struct WorkspaceManagement {
    instance: Instance,
    owner: OwnerId,
    selected: Arc<Mutex<Option<(String, String)>>>,
}

impl WorkspaceManagement {
    pub(crate) fn new(instance: Instance, owner: OwnerId) -> Self {
        Self {
            instance,
            owner,
            selected: Arc::new(Mutex::new(None)),
        }
    }
    pub fn owner_id(&self) -> OwnerId {
        self.owner
    }
    pub fn inspect(
        &self,
        query: &InspectionQuery,
        cursor: Option<&InspectionCursor>,
        limit: usize,
    ) -> Result<InspectionPage, ProductError> {
        self.instance
            .inspect_owner(self.owner, OwnerKind::Rel, query, cursor, limit)
    }
    pub fn select_conversation(
        &self,
        conversation_id: impl Into<String>,
        leaf_node_id: impl Into<String>,
    ) -> Result<(), ProductError> {
        self.instance.require_rel_read(self.owner)?;
        let conversation_id = conversation_id.into();
        let leaf_node_id = leaf_node_id.into();
        if conversation_id.trim().is_empty() || leaf_node_id.trim().is_empty() {
            return Err(ProductError::invalid(
                "conversation and leaf node IDs are required",
            ));
        }
        *self
            .selected
            .lock()
            .map_err(|_| ProductError::invalid("conversation selection is unavailable"))? =
            Some((conversation_id, leaf_node_id));
        Ok(())
    }
    pub fn conversations(
        &self,
        offset: usize,
        limit: usize,
    ) -> Result<ManagementPage<ConversationInspection>, ProductError> {
        self.instance.require_rel_read(self.owner)?;
        self.instance
            .inner
            .management_conversations(self.owner.into(), offset, limit)
            .map_err(Into::into)
    }
    pub fn transcript(
        &self,
        offset: usize,
        limit: usize,
    ) -> Result<ManagementPage<TranscriptTurn>, ProductError> {
        self.instance.require_rel_read(self.owner)?;
        let (conversation, leaf) = self
            .selected
            .lock()
            .map_err(|_| ProductError::invalid("conversation selection is unavailable"))?
            .clone()
            .ok_or_else(|| {
                ProductError::invalid("select a conversation before reading its transcript")
            })?;
        self.instance
            .inner
            .management_transcript(self.owner.into(), &conversation, &leaf, offset, limit)
            .map_err(Into::into)
    }
    pub fn set_title(&self, conversation_id: &str, title: &str) -> Result<bool, ProductError> {
        self.instance.require(OwnerGrant::ManageRel(self.owner))?;
        self.instance
            .inner
            .management_set_title(self.owner.into(), conversation_id, title)
            .map_err(Into::into)
    }
    #[allow(clippy::too_many_arguments)]
    pub fn create_memory(
        &self,
        title: String,
        content: String,
        category: String,
        memory_type: String,
        created_at_ns: i64,
    ) -> Result<MemorySummary, ProductError> {
        self.instance.require(OwnerGrant::ManageRel(self.owner))?;
        self.instance
            .inner
            .management_create_memory(
                self.owner.into(),
                title,
                content,
                category,
                memory_type,
                created_at_ns,
            )
            .map_err(Into::into)
    }
    pub fn memories(
        &self,
        offset: usize,
        limit: usize,
    ) -> Result<ManagementPage<MemorySummary>, ProductError> {
        self.instance.require_rel_read(self.owner)?;
        self.instance
            .inner
            .management_memories(self.owner.into(), offset, limit)
            .map_err(Into::into)
    }

    /// Publish a direct Memory using a caller-supplied, principal-bound request ID.
    pub fn create_memory_with_request(
        &self,
        request: CreateMemoryRequest,
    ) -> Result<MemorySummary, ProductError> {
        self.instance.require(OwnerGrant::ManageRel(self.owner))?;
        self.instance
            .inner
            .management_create_requested_memory(self.owner.into(), request)
            .map_err(Into::into)
    }

    /// Create a manual Memory at the expected owner cut. An exact accepted
    /// request replays its receipt within the same owner history generation.
    pub fn create_memory_at_cut(
        &self,
        expected: InspectionCut,
        request: CreateMemoryRequest,
    ) -> Result<CreateMemoryReceipt, ProductError> {
        self.instance.require(OwnerGrant::ManageRel(self.owner))?;
        self.instance
            .inner
            .management_create_requested_memory_at_cut(self.owner.into(), expected, request)
            .map_err(Into::into)
    }

    /// Publish an immutable successor and supersede its exact original at one cut.
    pub fn correct_memory_at_cut(
        &self,
        expected: InspectionCut,
        request: CorrectMemoryRequest,
    ) -> Result<CorrectMemoryReceipt, ProductError> {
        self.instance.require(OwnerGrant::ManageRel(self.owner))?;
        self.instance
            .inner
            .management_correct_memory_at_cut(self.owner.into(), expected, request)
            .map_err(Into::into)
    }

    /// Retire an exact archived supersession after verifying reference closure.
    pub fn retire_memory_at_cut(
        &self,
        expected: InspectionCut,
        request_id: &str,
        request: MemoryRetirementRequest,
    ) -> Result<RetireMemoryReceipt, ProductError> {
        self.instance.require(OwnerGrant::ManageRel(self.owner))?;
        self.instance
            .inner
            .management_retire_memory_at_cut(self.owner.into(), expected, request_id, request)
            .map_err(Into::into)
    }

    /// Rename one current Community with a durable, principal-bound request receipt.
    pub fn rename_community_at_cut(
        &self,
        expected: InspectionCut,
        request: CommunityRenameRequest,
    ) -> Result<CommunityRenameResult, ProductError> {
        self.instance.require(OwnerGrant::ManageRel(self.owner))?;
        self.instance
            .inner
            .management_rename_community_at_cut(self.owner.into(), expected, request)
            .map_err(Into::into)
    }

    pub fn retire_memory(
        &self,
        request: MemoryRetirementRequest,
    ) -> Result<TerminalTombstone, ProductError> {
        self.instance.require(OwnerGrant::ManageRel(self.owner))?;
        self.instance
            .inner
            .retire_semantic_memory(self.owner.into(), request)
            .map_err(Into::into)
    }

    pub fn retire_entity(
        &self,
        request: EntityRetirementRequest,
    ) -> Result<EntityTombstone, ProductError> {
        self.instance.require(OwnerGrant::ManageRel(self.owner))?;
        self.instance
            .inner
            .retire_semantic_entity(self.owner.into(), request)
            .map_err(Into::into)
    }

    pub fn withdraw_entity_mention(
        &self,
        request: WithdrawEntityMentionRequest,
    ) -> Result<MemoryEntityResolution, ProductError> {
        self.instance.require(OwnerGrant::ManageRel(self.owner))?;
        self.instance
            .inner
            .withdraw_semantic_entity_mention(self.owner.into(), request)
            .map_err(Into::into)
    }

    /// The supplied mutation ID is a request ID scoped to the authenticated principal.
    pub fn change_memory_association(
        &self,
        expected: InspectionCut,
        request: MemoryAssociationRequest,
    ) -> Result<MemoryAssociationReceipt, ProductError> {
        self.instance.require(OwnerGrant::ManageRel(self.owner))?;
        self.instance
            .inner
            .management_change_memory_association(self.owner.into(), expected, request)
            .map_err(Into::into)
    }

    pub fn create_relationship(
        &self,
        request: CreateRelationshipRequest,
    ) -> Result<Relationship, ProductError> {
        self.instance.require(OwnerGrant::ManageRel(self.owner))?;
        self.instance
            .inner
            .management_create_relationship(self.owner.into(), request)
            .map_err(Into::into)
    }

    pub fn retire_relationship(
        &self,
        request: RelationshipRetirementRequest,
    ) -> Result<RelationshipTombstone, ProductError> {
        self.instance.require(OwnerGrant::ManageRel(self.owner))?;
        self.instance
            .inner
            .retire_semantic_relationship(self.owner.into(), request)
            .map_err(Into::into)
    }
    pub fn memory(&self, memory_id: MemoryId) -> Result<MemorySummary, ProductError> {
        self.instance.require_rel_read(self.owner)?;
        self.instance
            .inner
            .management_memory(self.owner.into(), memory_id)
            .map_err(Into::into)
    }
    pub fn provenance(&self, memory_id: MemoryId) -> Result<MemoryProvenance, ProductError> {
        self.instance.require_rel_read(self.owner)?;
        self.instance
            .inner
            .management_provenance(self.owner.into(), memory_id)
            .map_err(Into::into)
    }
    pub fn statistics(&self) -> Result<SemanticOwnerStatistics, ProductError> {
        self.instance.require_rel_read(self.owner)?;
        self.instance
            .inner
            .management_statistics(self.owner.into())
            .map_err(Into::into)
    }
    pub fn health(&self) -> Result<RelManagementHealth, ProductError> {
        self.instance.require_rel_read(self.owner)?;
        self.instance
            .inner
            .management_health(self.owner.into())
            .map_err(Into::into)
    }
    pub fn quiesce(&self) -> Result<(), ProductError> {
        self.instance.require(OwnerGrant::ManageRel(self.owner))?;
        self.instance.gateway.quiesce(self.owner)
    }
    pub fn verify_quiesced(&self) -> Result<crate::OwnerInfo, ProductError> {
        self.instance.require(OwnerGrant::ManageRel(self.owner))?;
        let (path, kind, mounted) = self.instance.gateway.record(self.owner)?;
        if mounted {
            return Err(ProductError::new(
                crate::ProductErrorKind::Conflict,
                "owner must be quiesced before independent verification",
            ));
        }
        if kind != OwnerKind::Rel {
            return Err(ProductError::new(
                crate::ProductErrorKind::WrongFormat,
                "management owner is not REL",
            ));
        }
        Rel::verify(path)
    }
    /// Reconcile the closed-and-drained owner by independently admitting its
    /// durable state before it is mounted again.
    pub fn reconcile_quiesced(&self) -> Result<crate::OwnerInfo, ProductError> {
        self.verify_quiesced()
    }
    pub fn reopen(&self) -> Result<(), ProductError> {
        self.instance.require(OwnerGrant::ManageRel(self.owner))?;
        self.instance.gateway.reopen(self.owner)
    }
}
