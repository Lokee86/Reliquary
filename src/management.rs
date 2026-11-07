use crate::{
    ConversationInspection, Instance, ManagementPage, MemoryId, MemoryProvenance, MemorySummary,
    OwnerGrant, OwnerId, OwnerKind, ProductError, Rel, RelManagementHealth,
    SemanticOwnerStatistics, TranscriptTurn,
};
use std::sync::{Arc, Mutex};

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
