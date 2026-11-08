use super::*;
use reliquary_application::{
    ConversationInspection, CreateMemoryReceipt, CreateMemoryRequest, ManagementError,
    ManagementPage, MemoryProvenance, MemorySummary, RelManagementHealth, SemanticOwnerStatistics,
    TranscriptTurn,
};
use reliquary_application::{CorrectMemoryReceipt, CorrectMemoryRequest, RetireMemoryReceipt};
use reliquary_owner_session::MemoryId;

#[path = "instance_inspection.rs"]
mod inspection;

impl Instance {
    pub fn management_conversations(
        &self,
        owner: OwnerId,
        offset: usize,
        limit: usize,
    ) -> Result<ManagementPage<ConversationInspection>, GatewayError> {
        self.with_reliquary(owner, |session| {
            reliquary_application::management_conversations(session, offset, limit)
        })?
        .map_err(management_error)
    }

    pub fn management_transcript(
        &self,
        owner: OwnerId,
        conversation_id: &str,
        leaf_node_id: &str,
        offset: usize,
        limit: usize,
    ) -> Result<ManagementPage<TranscriptTurn>, GatewayError> {
        self.with_reliquary(owner, |session| {
            reliquary_application::management_transcript(
                session,
                conversation_id,
                leaf_node_id,
                offset,
                limit,
            )
        })?
        .map_err(management_error)
    }

    pub fn management_rename_community_at_cut(
        &self,
        owner: OwnerId,
        expected: reliquary_application::inspection::InspectionCut,
        request: reliquary_application::CommunityRenameRequest,
    ) -> Result<reliquary_application::CommunityRenameResult, GatewayError> {
        self.with_rel_writer(owner, |writer| {
            reliquary_application::rename_community_at_cut(
                writer,
                self.principal(),
                expected,
                request,
            )
        })?
        .map_err(management_error)
    }

    pub fn management_set_title(
        &self,
        owner: OwnerId,
        conversation_id: &str,
        title: &str,
    ) -> Result<bool, GatewayError> {
        self.with_rel_writer(owner, |writer| {
            reliquary_application::set_management_title(writer, conversation_id, title)
        })?
        .map_err(management_error)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn management_create_memory(
        &self,
        owner: OwnerId,
        title: String,
        content: String,
        category: String,
        memory_type: String,
        created_at_ns: i64,
    ) -> Result<MemorySummary, GatewayError> {
        let principal = self.principal.to_string();
        self.with_rel_writer(owner, |writer| {
            reliquary_application::create_direct_memory(
                writer,
                &principal,
                title,
                content,
                category,
                memory_type,
                created_at_ns,
            )
        })?
        .map_err(management_error)
    }

    pub fn management_memories(
        &self,
        owner: OwnerId,
        offset: usize,
        limit: usize,
    ) -> Result<ManagementPage<MemorySummary>, GatewayError> {
        self.with_reliquary(owner, |session| {
            reliquary_application::management_memories(session, offset, limit)
        })?
        .map_err(management_error)
    }

    pub fn management_create_requested_memory(
        &self,
        owner: OwnerId,
        request: CreateMemoryRequest,
    ) -> Result<MemorySummary, GatewayError> {
        self.with_rel_writer(owner, |writer| {
            reliquary_application::create_requested_memory(writer, self.principal(), request)
        })?
        .map_err(management_error)
    }

    pub fn management_create_requested_memory_at_cut(
        &self,
        owner: OwnerId,
        expected: reliquary_application::inspection::InspectionCut,
        request: CreateMemoryRequest,
    ) -> Result<CreateMemoryReceipt, GatewayError> {
        self.with_rel_writer(owner, |writer| {
            reliquary_application::create_requested_memory_at_cut(
                writer,
                self.principal(),
                expected,
                request,
            )
        })?
        .map_err(management_error)
    }

    pub fn management_change_memory_association(
        &self,
        owner: OwnerId,
        expected: reliquary_application::inspection::InspectionCut,
        request: reliquary_application::inspection::MemoryAssociationRequest,
    ) -> Result<reliquary_application::inspection::MemoryAssociationReceipt, GatewayError> {
        self.with_rel_writer(owner, |writer| {
            reliquary_application::inspection::change_memory_association(
                writer,
                self.principal(),
                expected,
                request,
            )
        })?
        .map_err(management_error)
    }

    pub fn management_correct_memory_at_cut(
        &self,
        owner: OwnerId,
        expected: reliquary_application::inspection::InspectionCut,
        request: CorrectMemoryRequest,
    ) -> Result<CorrectMemoryReceipt, GatewayError> {
        self.with_rel_writer(owner, |writer| {
            reliquary_application::correct_memory_at_cut(
                writer,
                self.principal(),
                expected,
                request,
            )
        })?
        .map_err(management_error)
    }

    pub fn management_retire_memory_at_cut(
        &self,
        owner: OwnerId,
        expected: reliquary_application::inspection::InspectionCut,
        request_id: &str,
        request: reliquary_owner_session::MemoryRetirementRequest,
    ) -> Result<RetireMemoryReceipt, GatewayError> {
        self.with_rel_writer(owner, |writer| {
            reliquary_application::retire_memory_at_cut(
                writer,
                self.principal(),
                expected,
                request_id,
                request,
            )
        })?
        .map_err(management_error)
    }

    pub fn management_create_relationship(
        &self,
        owner: OwnerId,
        mut request: reliquary_owner_session::CreateRelationshipRequest,
    ) -> Result<reliquary_owner_session::model::Relationship, GatewayError> {
        request.mutation_id = reliquary_application::user_mutation_id(
            self.principal(),
            "relationship",
            &request.mutation_id,
        )
        .map_err(management_error)?;
        self.publish_semantic_relationship(owner, request)
    }

    pub fn management_memory(
        &self,
        owner: OwnerId,
        memory_id: MemoryId,
    ) -> Result<MemorySummary, GatewayError> {
        self.with_reliquary(owner, |session| {
            reliquary_application::management_memory(session, memory_id)
        })?
        .map_err(management_error)
    }

    pub fn management_provenance(
        &self,
        owner: OwnerId,
        memory_id: MemoryId,
    ) -> Result<MemoryProvenance, GatewayError> {
        self.with_reliquary(owner, |session| {
            reliquary_application::management_provenance(session, memory_id)
        })?
        .map_err(management_error)
    }

    pub fn management_statistics(
        &self,
        owner: OwnerId,
    ) -> Result<SemanticOwnerStatistics, GatewayError> {
        self.with_reliquary(owner, reliquary_application::management_statistics)?
            .map_err(management_error)
    }

    pub fn management_health(&self, owner: OwnerId) -> Result<RelManagementHealth, GatewayError> {
        self.with_reliquary(owner, reliquary_application::management_health)?
            .map_err(management_error)
    }
}

fn management_error(error: ManagementError) -> GatewayError {
    GatewayError::Management(error)
}
