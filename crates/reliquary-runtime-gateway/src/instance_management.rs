use super::*;
use reliquary_application::{
    ConversationInspection, ManagementError, ManagementPage, MemoryProvenance, MemorySummary,
    RelManagementHealth, SemanticOwnerStatistics, TranscriptTurn,
};
use reliquary_owner_session::MemoryId;

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
    GatewayError::OwnerSession(OwnerSessionError::Record(error.to_string()))
}
