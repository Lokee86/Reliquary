use crate::{EntityId, EntityResolutionReason};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EntityAuditEntity {
    pub id: EntityId,
    pub canonical_name: String,
    pub kind: String,
    pub degree: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EntityAuditCollision {
    pub key: String,
    pub entities: Vec<EntityAuditEntity>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EntityAuditResolutionViolation {
    pub mention: String,
    pub status: String,
    pub reason: EntityResolutionReason,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct EntityAuditReport {
    pub entity_count: usize,
    pub association_count: usize,
    pub resolution_count: usize,
    pub resolved_count: usize,
    pub rejected_count: usize,
    pub pending_count: usize,
    pub dormant_count: usize,
    pub singleton_count: usize,
    pub degree_ge3_count: usize,
    pub reason_counts: BTreeMap<String, usize>,
    pub zero_degree_entities: Vec<EntityAuditEntity>,
    pub exact_surface_collisions: Vec<EntityAuditCollision>,
    pub normalized_surface_collisions: Vec<EntityAuditCollision>,
    pub alias_collisions: Vec<EntityAuditCollision>,
    pub cross_kind_alias_shadows: Vec<EntityAuditCollision>,
    pub rejection_invariant_violations: Vec<EntityAuditResolutionViolation>,
    pub missing_resolved_targets: Vec<EntityAuditResolutionViolation>,
    pub missing_candidate_targets: Vec<EntityAuditResolutionViolation>,
}

impl EntityAuditReport {
    pub fn finding_count(&self) -> usize {
        self.zero_degree_entities.len()
            + self.exact_surface_collisions.len()
            + self.normalized_surface_collisions.len()
            + self.alias_collisions.len()
            + self.cross_kind_alias_shadows.len()
            + self.rejection_invariant_violations.len()
            + self.missing_resolved_targets.len()
            + self.missing_candidate_targets.len()
    }

    pub fn is_clean(&self) -> bool {
        self.finding_count() == 0
    }
}
