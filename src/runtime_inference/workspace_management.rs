//! Typed workspace management over existing Gateway owners. No semantic state is stored here.
use super::{
    GatewayRuntime, InstanceRuntime, OwnerAccessToken, OwnerGrant, OwnerKind,
    ReliquaryRuntimeHostError,
};
use crate::{
    CommunityId, ConversationPolicy, ManagedConversationState, MemoryId, ProjectFileRef, StoredFile,
};
use std::sync::Arc;

#[path = "workspace_management_conversations.rs"]
mod conversations;
#[path = "workspace_management_files.rs"]
mod files;
#[path = "workspace_management_health.rs"]
mod health;
#[path = "workspace_management_knowledge.rs"]
mod knowledge;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManagedBranch {
    pub branch_id: String,
    pub head_node_id: Option<String>,
    pub fork_parent_node_id: Option<String>,
    pub display_label: Option<String>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConversationInspection {
    pub conversation_id: String,
    pub mount_epoch: u64,
    pub journal_sequence: u64,
    pub policy: ConversationPolicy,
    pub policy_revision: u64,
    pub selection: ManagedConversationState,
    pub branches: ManagementPage<ManagedBranch>,
    pub submissions: usize,
    pub native_generations: usize,
    pub host_operations: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceFile {
    pub file: StoredFile,
    pub project_reference: Option<ProjectFileRef>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemorySummary {
    pub id: MemoryId,
    pub revision: u64,
    pub title: String,
    pub category: String,
    pub lifecycle_state: String,
    pub archived: bool,
    pub superseded_by: Option<MemoryId>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommunitySummary {
    pub id: CommunityId,
    pub members: usize,
    pub name: Option<String>,
    pub generation: u64,
    pub derived_graph_version: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManagementVersions {
    pub global: Option<u64>,
    pub archive: Option<u64>,
    pub memory: Option<u64>,
    pub entity: Option<u64>,
    pub graph: Option<u64>,
    pub vector: Option<u64>,
    pub journal: Option<u64>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RelManagementHealth {
    pub mount_epoch: u64,
    pub versions: ManagementVersions,
    pub conversations: usize,
    pub branches: usize,
    pub pending_submissions: usize,
    pub unknown_host_operations: usize,
    pub cancellation_requests: usize,
    pub held_native_generations: usize,
    pub insomnia: crate::InsomniaStats,
    pub insomnia_enabled: bool,
    pub workers: usize,
    pub background_enabled: bool,
    pub external_execution_ready: bool,
    pub external_work_held: bool,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticOwnerStatistics {
    pub memories: crate::MemoryStats,
    pub vectors: crate::MemoryVectorStats,
    pub entities: crate::EntityStats,
    pub vector_pending: usize,
    pub dream_pending: usize,
    pub perception_pending: usize,
    pub workers: usize,
    pub background_enabled: bool,
}

pub const MAX_MANAGEMENT_PAGE_ITEMS: usize = 128;
pub const MAX_MANAGEMENT_PAGE_BYTES: usize = 1024 * 1024;
pub const MAX_MANAGEMENT_FILE_BYTES: usize = 8 * 1024 * 1024;

/// Pages are fresh reads, not retained snapshots. Repeat pagination after a mutation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManagementPage<T> {
    pub items: Vec<T>,
    pub total_items: usize,
    pub next_offset: Option<usize>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnsupportedManagementOperation {
    FileRename,
    FileMove,
    FileUnlink,
    StandaloneFileInventory,
    StandaloneFileImport,
    SelectiveImportExport,
    MemoryDeletion,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ManagementError {
    InvalidPage,
    ResponseTooLarge,
    Unavailable,
    Unsupported(UnsupportedManagementOperation),
}
impl std::fmt::Display for ManagementError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::InvalidPage => "management page size or offset is invalid",
            Self::ResponseTooLarge => "management response exceeds its bound",
            Self::Unavailable => "management operation is unavailable",
            Self::Unsupported(_) => "management operation requires an owner contract",
        })
    }
}
impl std::error::Error for ManagementError {}
fn unavailable(_: ReliquaryRuntimeHostError) -> ManagementError {
    ManagementError::Unavailable
}

fn validate_page(offset: usize, limit: usize) -> Result<(), ManagementError> {
    if limit == 0 || limit > MAX_MANAGEMENT_PAGE_ITEMS || offset.checked_add(limit).is_none() {
        return Err(ManagementError::InvalidPage);
    }
    Ok(())
}
fn page<T>(values: Vec<T>, offset: usize, limit: usize) -> ManagementPage<T> {
    let total_items = values.len();
    let end = offset.saturating_add(limit).min(total_items);
    ManagementPage {
        items: values.into_iter().skip(offset).take(limit).collect(),
        total_items,
        next_offset: (end < total_items).then_some(end),
    }
}
fn text_bound(bytes: usize) -> Result<(), ManagementError> {
    if bytes > MAX_MANAGEMENT_PAGE_BYTES {
        Err(ManagementError::ResponseTooLarge)
    } else {
        Ok(())
    }
}

/// Immutable results only; grants are revalidated by the original concrete owners.
pub struct WorkspaceManagement {
    gateway: Arc<GatewayRuntime>,
    instance: Arc<InstanceRuntime>,
}
impl GatewayRuntime {
    pub fn workspace_management(
        self: &Arc<Self>,
        instance: Arc<InstanceRuntime>,
    ) -> WorkspaceManagement {
        WorkspaceManagement {
            gateway: Arc::clone(self),
            instance,
        }
    }
}
impl WorkspaceManagement {
    fn token(&self, owner: &str) -> Result<OwnerAccessToken, ManagementError> {
        self.gateway
            .authorize(
                &self.instance,
                owner,
                if owner.starts_with("phy-") {
                    OwnerGrant::UseOwnPhy
                } else {
                    OwnerGrant::ViewRel
                },
            )
            .map_err(unavailable)
    }
    /// Delivery-only validation. The closures cannot inspect or mutate an owner.
    fn deliver<T>(&self, token: &OwnerAccessToken, value: T) -> Result<T, ManagementError> {
        match token.kind {
            OwnerKind::Rel => self.gateway.with_rel(token, |_| Ok(value)),
            OwnerKind::Phy => self.gateway.with_phy(token, |_| Ok(value)),
        }
        .map_err(unavailable)
    }
    pub fn owners(
        &self,
        offset: usize,
        limit: usize,
    ) -> Result<ManagementPage<String>, ManagementError> {
        validate_page(offset, limit)?;
        let authorization = self
            .instance
            .authorization()
            .map_err(|_| ManagementError::Unavailable)?;
        let mut owners = self
            .gateway
            .viewable_rel_owners(&self.instance)
            .map_err(unavailable)?;
        if let Some(phy) = self.instance.attached_phy_owner_id() {
            if self.gateway.semantic_owner(&self.instance, phy).is_ok() {
                owners.push(phy.to_owned());
            }
        }
        owners.sort();
        let attachment = self
            .gateway
            .attachment_entry(&self.instance)
            .map_err(unavailable)?;
        let _delivery = attachment.leases.acquire_current().map_err(unavailable)?;
        if self
            .instance
            .authorization()
            .map_err(|_| ManagementError::Unavailable)?
            != authorization
        {
            return Err(ManagementError::Unavailable);
        }
        Ok(page(owners, offset, limit))
    }
    /// Unsupported capabilities are explicit and still require exact-owner access.
    pub fn unsupported_operation(
        &self,
        owner: &str,
        operation: UnsupportedManagementOperation,
    ) -> Result<(), ManagementError> {
        let token = self.token(owner)?;
        self.deliver(&token, ())?;
        Err(ManagementError::Unsupported(operation))
    }
}
#[cfg(test)]
#[path = "workspace_management_tests.rs"]
mod tests;
