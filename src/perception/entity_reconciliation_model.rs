use crate::{
    Entity, EntityAuditReport, EntityId, EntityReconciliationCandidate,
    EntityReconciliationRelation, Memory, MemoryEntityMentionKey,
};

pub const DEFAULT_ENTITY_RECONCILIATION_PAIR_LIMIT: usize = 128;
pub const DEFAULT_ENTITY_RECONCILIATION_ROUNDS: usize = 3;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EntityReconciliationReport {
    pub rounds: usize,
    pub candidate_pairs: usize,
    pub deterministic_merges: usize,
    pub model_merges: usize,
    pub rejected_mentions_reconsidered: usize,
    pub unresolved_pairs: usize,
    pub final_audit: EntityAuditReport,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct EntityReconciliationPreparedPair {
    pub candidate: EntityReconciliationCandidate,
    pub left: Entity,
    pub right: Entity,
    pub left_support: Vec<Memory>,
    pub right_support: Vec<Memory>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct EntityReconciliationPrepared {
    pub entity_version: u64,
    pub graph_version: u64,
    pub memory_version: u64,
    pub pairs: Vec<EntityReconciliationPreparedPair>,
    pub late_rejected: Vec<MemoryEntityMentionKey>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct EntityReconciliationPairDecision {
    pub left: EntityId,
    pub right: EntityId,
    pub relation: EntityReconciliationRelation,
    pub deterministic: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct EntityReconciliationEvaluation {
    pub decisions: Vec<EntityReconciliationPairDecision>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct EntityReconciliationCommit {
    pub report: EntityReconciliationReport,
    pub wake_keys: Vec<MemoryEntityMentionKey>,
}

impl EntityReconciliationReport {
    pub fn changed(&self) -> bool {
        self.deterministic_merges > 0
            || self.model_merges > 0
            || self.rejected_mentions_reconsidered > 0
    }
}
