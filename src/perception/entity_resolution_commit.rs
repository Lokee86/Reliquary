use crate::entity_resolution_processor_support::bootstrap_entity_draft;
use crate::{
    Cva, EntityDraft, EntityResolutionDecision, EntityResolutionEvaluation,
    EntityResolutionOutcome, EntityResolutionPrepared, EntityResolverError, MAX_ENTITY_ALIASES,
    MAX_ENTITY_RESOLUTION_CANDIDATES, Phylactery,
};

macro_rules! impl_owner {
    ($owner:ty) => {
        impl $owner {
            pub(crate) fn commit_entity_resolution(
                &mut self,
                prepared: EntityResolutionPrepared,
                evaluation: EntityResolutionEvaluation,
                now_ns: i64,
            ) -> Result<Option<EntityResolutionOutcome>, EntityResolverError> {
                if self.owner_id().as_deref() != Some(prepared.owner_id.as_str())
                    || self.memory_version() != prepared.memory_version
                    || self.entity_version() != prepared.entity_version
                    || self.graph_version() != prepared.graph_version
                    || self
                        .entity_resolution(prepared.key)
                        .map_or(0, |value| value.revision)
                        != prepared.expected_resolution_revision
                {
                    return Ok(None);
                }

                let key = prepared.key;
                let mut output = evaluation.output;
                if output.reason.is_rejection_class() {
                    output.decision = EntityResolutionDecision::Reject;
                }
                let expected_revision = prepared.expected_resolution_revision;
                let outcome = match output.decision {
                    EntityResolutionDecision::ResolveExisting(entity_id) => {
                        let entity = self.entity(entity_id)?;
                        if entity.kind == crate::entity_principal_identity::PRINCIPAL_ENTITY_KIND {
                            return Err(EntityResolverError::InvalidOutput(
                                "principal Entities are reserved for deterministic principal resolution"
                                    .into(),
                            ));
                        }
                        let surface = prepared.candidates.mention.text.trim();
                        let already_known = entity.canonical_name.eq_ignore_ascii_case(surface)
                            || entity
                                .aliases
                                .iter()
                                .any(|alias| alias.eq_ignore_ascii_case(surface));
                        let alias_equivalent = prepared
                            .candidates
                            .candidates
                            .iter()
                            .find(|candidate| candidate.entity.id == entity_id)
                            .is_some_and(|candidate| {
                                candidate.exact_surface
                                    || candidate.normalized_surface
                                    || candidate.alias_surface
                            });
                        if !surface.is_empty()
                            && !already_known
                            && alias_equivalent
                            && entity.aliases.len() < MAX_ENTITY_ALIASES
                        {
                            let mut aliases = entity.aliases.clone();
                            aliases.push(surface.to_owned());
                            let draft = EntityDraft {
                                canonical_name: entity.canonical_name.clone(),
                                aliases,
                                kind: entity.kind.clone(),
                                summary: entity.summary.clone(),
                                mutation_id: format!(
                                    "entity-resolution-alias:{}:{}",
                                    hex32(&entity.id.0),
                                    entity.revision + 1
                                ),
                                created_at_ns: entity.created_at_ns,
                                updated_at_ns: now_ns.max(entity.updated_at_ns),
                            };
                            self.publish_entity(Some(entity_id), entity.revision, draft)?;
                        }
                        let association_changed = self
                            .set_entity_association(
                                key.memory_id,
                                entity_id,
                                true,
                                self.graph_version(),
                            )?
                            .is_some();
                        let resolution_changed = self.put_entity_resolved(
                            key,
                            expected_revision,
                            entity_id,
                            output.reason,
                            now_ns,
                        )?;
                        EntityResolutionOutcome {
                            key,
                            decision: output.decision,
                            reason: output.reason,
                            entity_id: Some(entity_id),
                            entity_created: false,
                            association_changed,
                            resolution_changed,
                        }
                    }
                    EntityResolutionDecision::CreateNew => {
                        let materialization = evaluation.materialization.ok_or_else(|| {
                            EntityResolverError::InvalidOutput(
                                "create_new evaluation missing Entity metadata".into(),
                            )
                        })?;
                        if materialization.kind.trim()
                            == crate::entity_principal_identity::PRINCIPAL_ENTITY_KIND
                        {
                            return Err(EntityResolverError::InvalidOutput(
                                "principal Entities are reserved for deterministic principal resolution"
                                    .into(),
                            ));
                        }
                        let conflicts = self.entity_creation_conflicts(
                            prepared.candidates.mention.text.trim(),
                            &materialization.kind,
                            MAX_ENTITY_RESOLUTION_CANDIDATES,
                        );
                        if !conflicts.is_empty() {
                            let mut candidate_entity_ids =
                                conflicts.iter().map(|entity| entity.id).collect::<Vec<_>>();
                            for candidate in &prepared.candidates.candidates {
                                if candidate_entity_ids.len() >= MAX_ENTITY_RESOLUTION_CANDIDATES {
                                    break;
                                }
                                if !candidate_entity_ids.contains(&candidate.entity.id) {
                                    candidate_entity_ids.push(candidate.entity.id);
                                }
                            }
                            let resolution_changed = self.put_entity_unresolved(
                                key,
                                expected_revision,
                                candidate_entity_ids,
                                crate::EntityResolutionReason::Ambiguous,
                                prepared.candidate_fingerprint,
                                prepared.context_fingerprint,
                                now_ns,
                            )?;
                            return Ok(Some(EntityResolutionOutcome {
                                key,
                                decision: EntityResolutionDecision::Unresolved,
                                reason: crate::EntityResolutionReason::Ambiguous,
                                entity_id: None,
                                entity_created: false,
                                association_changed: false,
                                resolution_changed,
                            }));
                        }
                        let draft = bootstrap_entity_draft(
                            &prepared.memory,
                            &prepared.candidates,
                            materialization,
                        );
                        let (entity, entity_created) = self.publish_entity(None, 0, draft)?;
                        let association_changed = self
                            .set_entity_association(
                                key.memory_id,
                                entity.id,
                                true,
                                self.graph_version(),
                            )?
                            .is_some();
                        let resolution_changed = self.put_entity_resolved(
                            key,
                            expected_revision,
                            entity.id,
                            output.reason,
                            now_ns,
                        )?;
                        EntityResolutionOutcome {
                            key,
                            decision: output.decision,
                            reason: output.reason,
                            entity_id: Some(entity.id),
                            entity_created,
                            association_changed,
                            resolution_changed,
                        }
                    }
                    EntityResolutionDecision::Unresolved => {
                        let resolution_changed = self.put_entity_unresolved(
                            key,
                            expected_revision,
                            prepared
                                .candidates
                                .candidates
                                .iter()
                                .map(|value| value.entity.id)
                                .collect(),
                            output.reason,
                            prepared.candidate_fingerprint,
                            prepared.context_fingerprint,
                            now_ns,
                        )?;
                        EntityResolutionOutcome {
                            key,
                            decision: output.decision,
                            reason: output.reason,
                            entity_id: None,
                            entity_created: false,
                            association_changed: false,
                            resolution_changed,
                        }
                    }
                    EntityResolutionDecision::Reject => {
                        let resolution_changed = self.put_entity_rejected(
                            key,
                            expected_revision,
                            output.reason,
                            now_ns,
                        )?;
                        EntityResolutionOutcome {
                            key,
                            decision: output.decision,
                            reason: output.reason,
                            entity_id: None,
                            entity_created: false,
                            association_changed: false,
                            resolution_changed,
                        }
                    }
                };
                Ok(Some(outcome))
            }
        }
    };
}

impl_owner!(Cva);
impl_owner!(Phylactery);

fn hex32(bytes: &[u8; 32]) -> String {
    let mut out = String::with_capacity(64);
    for byte in bytes {
        use std::fmt::Write;
        let _ = write!(&mut out, "{byte:02x}");
    }
    out
}
