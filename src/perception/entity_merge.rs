use crate::{
    Cva, CvaError, EntityDraft, EntityError, EntityId, EntityMergeOutcome, EntityRef,
    GraphRelationOrigin, MAX_ENTITY_ALIASES, MemoryEntityMentionKey, MemoryEntityResolutionStatus,
    MemoryError, Phylactery, PhylacteryError, RelationshipDraft, SemanticGraphRelationChange,
};

macro_rules! impl_entity_merge {
    ($owner:ty, $error:ty) => {
        impl $owner {
            pub fn merge_entities(
                &mut self,
                survivor_id: EntityId,
                retired_id: EntityId,
                now_ns: i64,
            ) -> Result<EntityMergeOutcome, $error> {
                if survivor_id == retired_id {
                    return Err(EntityError::InvalidField("Entity merge identity").into());
                }
                if self.retired_entity_replacement(retired_id) == Some(survivor_id) {
                    return Ok(EntityMergeOutcome {
                        survivor_id,
                        retired_id,
                        aliases_added: 0,
                        associations_retargeted: 0,
                        resolutions_retargeted: 0,
                        relationships_retargeted: 0,
                        changed: false,
                    });
                }

                let survivor = self.entity(survivor_id)?;
                let retired = self.entity(retired_id)?;
                if survivor.kind == "principal" || retired.kind == "principal" {
                    return Err(EntityError::InvalidField("principal Entity merge").into());
                }
                if survivor.kind != retired.kind {
                    return Err(EntityError::InvalidField("Entity merge kind").into());
                }

                let active_entities = self.entities();
                let aliases = merged_aliases(&survivor, &retired, &active_entities)?;
                let aliases_added = aliases.len().saturating_sub(survivor.aliases.len());
                if aliases_added > 0 {
                    self.publish_entity(
                        Some(survivor_id),
                        survivor.revision,
                        EntityDraft {
                            canonical_name: survivor.canonical_name.clone(),
                            aliases,
                            kind: survivor.kind.clone(),
                            summary: survivor.summary.clone(),
                            mutation_id: format!(
                                "entity-merge:{}:{}:{}",
                                short_id(survivor_id),
                                short_id(retired_id),
                                survivor.revision + 1
                            ),
                            created_at_ns: survivor.created_at_ns,
                            updated_at_ns: now_ns.max(survivor.updated_at_ns),
                        },
                    )?;
                }

                let resolutions_retargeted = if self.entities.contains(survivor_id) {
                    self.entity_resolutions.retarget_entity_references(
                        &mut self.container,
                        retired_id,
                        survivor_id,
                        now_ns,
                    )?
                } else {
                    return Err(MemoryError::InvalidField(
                        "Entity resolution Entity reference",
                    )
                    .into());
                };

                let memories = self.memories_for_entity(retired_id);
                let mut changes = Vec::with_capacity(memories.len().saturating_mul(2));
                for memory_id in &memories {
                    if !self
                        .entity_associations_for_memory(*memory_id)
                        .contains(&survivor_id)
                    {
                        changes.push(SemanticGraphRelationChange::entity_association(
                            *memory_id,
                            survivor_id,
                            true,
                        ));
                    }
                    changes.push(SemanticGraphRelationChange::entity_association(
                        *memory_id,
                        retired_id,
                        false,
                    ));
                }
                if !changes.is_empty() {
                    self.set_semantic_relations_with_origin(
                        &changes,
                        GraphRelationOrigin::Perception,
                        self.graph_version(),
                    )?;
                }

                let owner_id = self
                    .owner_id()
                    .ok_or(crate::RelationshipError::MissingOwnerIdentity)?;
                let retired_ref = EntityRef {
                    owner_id,
                    entity_id: retired_id,
                };
                let relationships = self.relationships_for_entity(&retired_ref);
                let mut relationships_retargeted = 0;
                for relationship in relationships {
                    let mut participants = relationship.participants.clone();
                    for participant in &mut participants {
                        if participant.entity == retired_ref {
                            participant.entity.entity_id = survivor_id;
                        }
                    }
                    self.publish_relationship(
                        Some(relationship.id),
                        relationship.revision,
                        RelationshipDraft {
                            kind: relationship.kind,
                            participants,
                            evidence: relationship.evidence,
                            summary: relationship.summary,
                            mutation_id: format!(
                                "entity-merge-relationship:{}:{}:{}:{}",
                                short_id(survivor_id),
                                short_id(retired_id),
                                short_bytes(&relationship.id.0),
                                relationship.revision + 1
                            ),
                            created_at_ns: relationship.created_at_ns,
                            updated_at_ns: now_ns.max(relationship.updated_at_ns),
                        },
                    )?;
                    relationships_retargeted += 1;
                }

                let retired = self.entity(retired_id)?;
                self.entities.tombstone(
                    &mut self.container,
                    retired_id,
                    retired.revision,
                    survivor_id,
                )?;

                Ok(EntityMergeOutcome {
                    survivor_id,
                    retired_id,
                    aliases_added,
                    associations_retargeted: memories.len(),
                    resolutions_retargeted,
                    relationships_retargeted,
                    changed: true,
                })
            }

            pub fn retarget_entity_resolution(
                &mut self,
                key: MemoryEntityMentionKey,
                from: EntityId,
                to: EntityId,
                now_ns: i64,
            ) -> Result<bool, $error> {
                if from == to {
                    return Ok(false);
                }
                let from_entity = self.entity(from)?;
                let to_entity = self.entity(to)?;
                if from_entity.kind == crate::entity_principal_identity::PRINCIPAL_ENTITY_KIND
                    || to_entity.kind == crate::entity_principal_identity::PRINCIPAL_ENTITY_KIND
                {
                    return Err(EntityError::InvalidField("principal Entity retarget").into());
                }
                self.memories.validate_entity_mention_key(key)?;
                if !self.entities.contains(to) {
                    return Err(MemoryError::InvalidField(
                        "Entity resolution Entity reference",
                    )
                    .into());
                }
                let resolution_changed = self.entity_resolutions.retarget_resolved(
                    &mut self.container,
                    key,
                    from,
                    to,
                    now_ns,
                )?;

                let memory_id = key.memory_id;
                let mut changed = resolution_changed;
                if !self.entity_associations_for_memory(memory_id).contains(&to) {
                    changed |= self
                        .set_entity_association(memory_id, to, true, self.graph_version())?
                        .is_some();
                }
                let still_targets_from = self
                    .entity_resolutions_for_memory(memory_id)
                    .iter()
                    .any(|value| {
                        matches!(
                            value.status,
                            MemoryEntityResolutionStatus::Resolved { entity_id, .. }
                                if entity_id == from
                        )
                    });
                if !still_targets_from
                    && self
                        .entity_associations_for_memory(memory_id)
                        .contains(&from)
                {
                    changed |= self
                        .set_entity_association(memory_id, from, false, self.graph_version())?
                        .is_some();
                }
                Ok(changed)
            }
        }
    };
}

impl_entity_merge!(Cva, CvaError);
impl_entity_merge!(Phylactery, PhylacteryError);

fn merged_aliases(
    survivor: &crate::Entity,
    retired: &crate::Entity,
    active_entities: &[crate::Entity],
) -> Result<Vec<String>, EntityError> {
    let mut aliases = survivor.aliases.clone();
    for value in std::iter::once(retired.canonical_name.as_str())
        .chain(retired.aliases.iter().map(String::as_str))
    {
        if survivor.canonical_name.eq_ignore_ascii_case(value)
            || aliases
                .iter()
                .any(|alias| alias.eq_ignore_ascii_case(value))
            || surface_claimed_by_other_entity(value, survivor.id, retired.id, active_entities)
        {
            continue;
        }
        aliases.push(value.to_owned());
    }
    aliases.sort();
    aliases.dedup();
    if aliases.len() > MAX_ENTITY_ALIASES {
        return Err(EntityError::FieldTooLarge);
    }
    Ok(aliases)
}

fn surface_claimed_by_other_entity(
    surface: &str,
    survivor_id: EntityId,
    retired_id: EntityId,
    active_entities: &[crate::Entity],
) -> bool {
    active_entities.iter().any(|entity| {
        entity.id != survivor_id
            && entity.id != retired_id
            && (entity.canonical_name.eq_ignore_ascii_case(surface)
                || entity
                    .aliases
                    .iter()
                    .any(|alias| alias.eq_ignore_ascii_case(surface)))
    })
}

fn short_id(id: EntityId) -> String {
    short_bytes(&id.0)
}

fn short_bytes(bytes: &[u8; 32]) -> String {
    bytes
        .iter()
        .take(8)
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
