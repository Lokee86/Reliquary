use crate::entity_audit_collisions::{
    alias_collisions, cross_kind_alias_shadows, exact_surface_collisions,
    normalized_surface_collisions,
};
use crate::{
    Cva, EntityAuditEntity, EntityAuditReport, EntityAuditResolutionViolation, EntityId,
    EntityResolutionReason, MemoryEntityResolutionStatus, Phylactery,
};
use std::collections::{BTreeMap, HashMap, HashSet};

macro_rules! impl_owner_audit {
    ($owner:ty) => {
        impl $owner {
            pub fn audit_entities(&self) -> EntityAuditReport {
                audit_owner(
                    self.entities(),
                    self.memory_ids(),
                    |memory_id| self.entity_associations_for_memory(memory_id),
                    self.entity_resolutions(),
                    |key| {
                        self.memory_routing_metadata(key.memory_id)
                            .and_then(|metadata| {
                                metadata.entity_mentions.iter().find(|mention| {
                                    mention.field == key.field
                                        && mention.start_byte == key.start_byte
                                        && mention.end_byte == key.end_byte
                                })
                            })
                            .map(|mention| mention.text.clone())
                            .unwrap_or_else(|| "<missing mention>".into())
                    },
                )
            }
        }
    };
}

impl_owner_audit!(Cva);
impl_owner_audit!(Phylactery);

fn audit_owner<FAssociations, FMention>(
    entities: Vec<crate::Entity>,
    memory_ids: Vec<crate::MemoryId>,
    mut associations_for_memory: FAssociations,
    resolutions: Vec<crate::MemoryEntityResolution>,
    mention_text: FMention,
) -> EntityAuditReport
where
    FAssociations: FnMut(crate::MemoryId) -> Vec<EntityId>,
    FMention: Fn(crate::MemoryEntityMentionKey) -> String,
{
    let mut degrees = HashMap::<EntityId, usize>::new();
    let mut association_count = 0usize;
    for memory_id in memory_ids {
        for entity_id in associations_for_memory(memory_id) {
            association_count += 1;
            *degrees.entry(entity_id).or_default() += 1;
        }
    }

    let ordinary_entities = entities
        .iter()
        .filter(|entity| entity.kind != crate::entity_principal_identity::PRINCIPAL_ENTITY_KIND)
        .cloned()
        .collect::<Vec<_>>();
    let audit_entities = ordinary_entities
        .iter()
        .map(|entity| EntityAuditEntity {
            id: entity.id,
            canonical_name: entity.canonical_name.clone(),
            kind: entity.kind.clone(),
            degree: degrees.get(&entity.id).copied().unwrap_or_default(),
        })
        .collect::<Vec<_>>();
    let active_ids = entities
        .iter()
        .map(|entity| entity.id)
        .collect::<HashSet<_>>();

    let mut report = EntityAuditReport {
        entity_count: ordinary_entities.len(),
        association_count,
        resolution_count: resolutions.len(),
        singleton_count: audit_entities
            .iter()
            .filter(|entity| entity.degree == 1)
            .count(),
        degree_ge3_count: audit_entities
            .iter()
            .filter(|entity| entity.degree >= 3)
            .count(),
        zero_degree_entities: audit_entities
            .iter()
            .filter(|entity| entity.degree == 0)
            .cloned()
            .collect(),
        exact_surface_collisions: exact_surface_collisions(&ordinary_entities, &audit_entities),
        normalized_surface_collisions: normalized_surface_collisions(
            &ordinary_entities,
            &audit_entities,
        ),
        alias_collisions: alias_collisions(&ordinary_entities, &audit_entities),
        cross_kind_alias_shadows: cross_kind_alias_shadows(&ordinary_entities, &audit_entities),
        ..EntityAuditReport::default()
    };

    for resolution in resolutions {
        let mention = mention_text(resolution.key);
        match &resolution.status {
            MemoryEntityResolutionStatus::Resolved { entity_id, reason } => {
                report.resolved_count += 1;
                increment_reason(&mut report.reason_counts, *reason);
                if reason.is_rejection_class() {
                    report
                        .rejection_invariant_violations
                        .push(EntityAuditResolutionViolation {
                            mention: mention.clone(),
                            status: "resolved".into(),
                            reason: *reason,
                        });
                }
                if !active_ids.contains(entity_id) {
                    report
                        .missing_resolved_targets
                        .push(EntityAuditResolutionViolation {
                            mention,
                            status: "resolved".into(),
                            reason: *reason,
                        });
                }
            }
            MemoryEntityResolutionStatus::Rejected { reason } => {
                report.rejected_count += 1;
                increment_reason(&mut report.reason_counts, *reason);
            }
            MemoryEntityResolutionStatus::Pending(value) => {
                report.pending_count += 1;
                increment_reason(&mut report.reason_counts, value.reason);
                if value.reason.is_rejection_class() {
                    report
                        .rejection_invariant_violations
                        .push(EntityAuditResolutionViolation {
                            mention: mention.clone(),
                            status: "pending".into(),
                            reason: value.reason,
                        });
                }
                for entity_id in &value.candidate_entity_ids {
                    if !active_ids.contains(entity_id) {
                        report
                            .missing_candidate_targets
                            .push(EntityAuditResolutionViolation {
                                mention: mention.clone(),
                                status: "pending".into(),
                                reason: value.reason,
                            });
                    }
                }
            }
            MemoryEntityResolutionStatus::Dormant(value) => {
                report.dormant_count += 1;
                increment_reason(&mut report.reason_counts, value.reason);
                if value.reason.is_rejection_class() {
                    report
                        .rejection_invariant_violations
                        .push(EntityAuditResolutionViolation {
                            mention: mention.clone(),
                            status: "dormant".into(),
                            reason: value.reason,
                        });
                }
                for entity_id in &value.candidate_entity_ids {
                    if !active_ids.contains(entity_id) {
                        report
                            .missing_candidate_targets
                            .push(EntityAuditResolutionViolation {
                                mention: mention.clone(),
                                status: "dormant".into(),
                                reason: value.reason,
                            });
                    }
                }
            }
        }
    }

    report
}

fn increment_reason(counts: &mut BTreeMap<String, usize>, reason: EntityResolutionReason) {
    *counts.entry(format!("{reason:?}")).or_default() += 1;
}
