//! Authorized composition of host-native conversation context with owner-bound
//! Archive project evidence. Archive turns are never appended as a second
//! transcript; they are consulted only for retained attachment provenance.
use super::*;
use reliquary_application::{
    CompactionBudget, CompactionCheckpoint, CompactionPlan, ContextMemoryEvidence,
    ContextMemoryScope, ContextRepositoryEvidence, ContextRequest, ContextTokenCounter,
    ContextView, PreparedContextWorkflow, finalize_context_workflow, prepare_context_workflow,
};
use sha2::{Digest, Sha256};
use std::sync::atomic::Ordering;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectContextAttachmentRef {
    pub conversation_id: String,
    pub source_node_id: String,
    pub attachment_id: [u8; 32],
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ContextMemoryOwnerKind {
    Rel,
    Phy,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ContextMemoryRef {
    pub owner: OwnerId,
    pub kind: ContextMemoryOwnerKind,
    pub memory_id: [u8; 32],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ContextMemoryBinding {
    source: ContextMemoryRef,
    revision: u64,
    body_id: [u8; 32],
}

#[derive(Clone)]
struct AuthorizedProjectContextSource {
    selector: ProjectContextAttachmentRef,
    descriptor: reliquary_owner_session::model::SourceAttachment,
    active: Arc<AtomicBool>,
}

/// Private source scope kept across model inference. Requests become visible
/// only through an Instance method that rechecks the captured authorization.
pub struct AuthorizedContextWorkflow {
    instance_id: Uuid,
    principal: String,
    owner: OwnerId,
    conversation_id: String,
    native_view: ContextView,
    native_digest: [u8; 32],
    source_view: ContextView,
    source_digest: [u8; 32],
    prepared: PreparedContextWorkflow,
    sources: Vec<AuthorizedProjectContextSource>,
    memories: Vec<ContextMemoryBinding>,
}

/// Opaque checkpoint authority bound to the exact host/owner context scope.
pub struct AuthorizedContextCheckpoint {
    instance_id: Uuid,
    principal: String,
    owner: OwnerId,
    conversation_id: String,
    checkpoint: CompactionCheckpoint,
    native_view: ContextView,
    native_digest: [u8; 32],
    source_view: ContextView,
    source_digest: [u8; 32],
    sources: Vec<AuthorizedProjectContextSource>,
    memories: Vec<ContextMemoryBinding>,
}
impl AuthorizedContextWorkflow {
    fn request(&self) -> &ContextRequest {
        self.prepared.request()
    }
    fn compaction_plan(&self) -> Option<&CompactionPlan> {
        self.prepared.compaction_plan()
    }
}

pub(super) fn context_view_digest(view: &ContextView) -> [u8; 32] {
    let mut digest = Sha256::new();
    for turn in &view.turns {
        digest.update([match turn.role {
            reliquary_application::ContextRole::User => 1,
            reliquary_application::ContextRole::Assistant => 2,
        }]);
        for value in [&turn.message_id, &turn.content] {
            digest.update((value.len() as u64).to_le_bytes());
            digest.update(value.as_bytes());
        }
    }
    digest.finalize().into()
}

mod finalize;
mod prepare;
mod sources;

#[cfg(test)]
#[path = "instance_context/tests.rs"]
mod tests;
