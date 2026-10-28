use crate::{EntityId, EntityResolutionReason, MemoryEntityMentionKey};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EntityResolutionDecision {
    ResolveExisting(EntityId),
    CreateNew,
    Unresolved,
    Reject,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EntityAdmissionDecision {
    CreateNew,
    Unresolved,
    Reject,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EntityMaterialization {
    pub kind: String,
    pub summary: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EntityResolverOutput {
    pub decision: EntityResolutionDecision,
    pub reason: EntityResolutionReason,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EntityAdmissionOutput {
    pub decision: EntityAdmissionDecision,
    pub reason: EntityResolutionReason,
    pub materialization: Option<EntityMaterialization>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EntityResolutionOutcome {
    pub key: MemoryEntityMentionKey,
    pub decision: EntityResolutionDecision,
    pub reason: EntityResolutionReason,
    pub entity_id: Option<EntityId>,
    pub entity_created: bool,
    pub association_changed: bool,
    pub resolution_changed: bool,
}
