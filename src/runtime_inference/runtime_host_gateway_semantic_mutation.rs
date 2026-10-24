use super::super::events::RelInvalidation;
use super::super::mutation_events::RelMutationFamily;
use super::*;
use crate::{GraphRelationKind, GraphRelationOrigin, Memory, MemoryId};

// The verified attachment chooses a single owner and grant. No caller obtains
// a mutable semantic store or an independently mutable PHY handle.
impl GatewayRuntime {
    fn semantic_write_token(
        &self,
        instance: &InstanceRuntime,
        owner_id: &str,
    ) -> Result<OwnerAccessToken, ReliquaryRuntimeHostError> {
        let grant = if owner_id.starts_with("phy-") {
            OwnerGrant::UseOwnPhy
        } else {
            OwnerGrant::ManageRel
        };
        self.authorize(instance, owner_id, grant)
    }

    pub fn create_knowledge_memory_for_owner(
        &self,
        instance: &InstanceRuntime,
        owner_id: &str,
        title: String,
        content: String,
        category: String,
        memory_type: String,
        now_ns: i64,
    ) -> Result<Memory, ReliquaryRuntimeHostError> {
        let token = self.semantic_write_token(instance, owner_id)?;
        let draft = crate::knowledge_mutation::user_creation_draft(
            title,
            content,
            category,
            memory_type,
            now_ns,
        );
        match token.kind {
            OwnerKind::Rel => {
                let memory =
                    self.with_rel_mutation(&token, RelMutationFamily::Semantic, |runtime| {
                        let (memory, _) = runtime
                            .cva
                            .runtime_publish_knowledge_memory(None, 0, draft)
                            .map_err(operation)?;
                        Ok((
                            memory.clone(),
                            false,
                            vec![RelInvalidation::Memory(memory.id)],
                        ))
                    })?;
                self.wake_rel(owner_id)?;
                Ok(memory)
            }
            OwnerKind::Phy => {
                let memory = self.with_phy(&token, |phy| {
                    let (memory, _) = phy
                        .runtime_publish_knowledge_memory(None, 0, draft, None)
                        .map_err(operation)?;
                    phy.sync().map_err(operation)?;
                    Ok(memory)
                })?;
                self.wake_phy(owner_id)?;
                Ok(memory)
            }
        }
    }

    /// Historical knowledge edits publish a successor and retire the old memory.
    /// They are not the revision-checked in-place editor used by other clients.
    pub fn replace_knowledge_memory_for_owner(
        &self,
        instance: &InstanceRuntime,
        owner_id: &str,
        memory_id: MemoryId,
        title: String,
        content: String,
        now_ns: i64,
    ) -> Result<Memory, ReliquaryRuntimeHostError> {
        let token = self.semantic_write_token(instance, owner_id)?;
        match token.kind {
            OwnerKind::Rel => {
                let value =
                    self.with_rel_mutation(&token, RelMutationFamily::Semantic, |runtime| {
                        let replacement = runtime
                            .cva
                            .runtime_replace_knowledge_memory(memory_id, title, content, now_ns)
                            .map_err(operation)?;
                        Ok((
                            replacement.clone(),
                            false,
                            vec![
                                RelInvalidation::Memory(memory_id),
                                RelInvalidation::Memory(replacement.id),
                            ],
                        ))
                    })?;
                self.wake_rel(owner_id)?;
                Ok(value)
            }
            OwnerKind::Phy => {
                let value = self.with_phy(&token, |phy| {
                    let replacement = phy
                        .runtime_replace_knowledge_memory(memory_id, title, content, now_ns)
                        .map_err(operation)?;
                    phy.sync().map_err(operation)?;
                    Ok(replacement)
                })?;
                self.wake_phy(owner_id)?;
                Ok(value)
            }
        }
    }

    pub fn mutate_knowledge_relation_for_owner(
        &self,
        instance: &InstanceRuntime,
        owner_id: &str,
        source: MemoryId,
        target: MemoryId,
        old_kind: Option<GraphRelationKind>,
        new_kind: Option<GraphRelationKind>,
    ) -> Result<(), ReliquaryRuntimeHostError> {
        let token = self.semantic_write_token(instance, owner_id)?;
        let changes = relation_changes(source, target, old_kind, new_kind)?;
        match token.kind {
            OwnerKind::Rel => {
                self.with_rel_mutation(&token, RelMutationFamily::Semantic, |runtime| {
                    let expected_graph_version = runtime.cva.runtime_graph_version();
                    runtime
                        .cva
                        .runtime_set_knowledge_relations(
                            &changes,
                            GraphRelationOrigin::User,
                            expected_graph_version,
                        )
                        .map_err(operation)?;
                    Ok((
                        (),
                        false,
                        vec![
                            RelInvalidation::Memory(source),
                            RelInvalidation::Memory(target),
                        ],
                    ))
                })?;
                self.wake_rel(owner_id)
            }
            OwnerKind::Phy => {
                self.with_phy(&token, |phy| {
                    let expected_graph_version = phy.graph_version();
                    phy.runtime_set_knowledge_relations(
                        &changes,
                        GraphRelationOrigin::User,
                        expected_graph_version,
                    )
                    .map_err(operation)?;
                    phy.sync().map_err(operation)
                })?;
                self.wake_phy(owner_id)
            }
        }
    }

    pub fn rename_knowledge_community_for_owner(
        &self,
        instance: &InstanceRuntime,
        owner_id: &str,
        community_id: crate::CommunityId,
        name: String,
    ) -> Result<(), ReliquaryRuntimeHostError> {
        let token = self.semantic_write_token(instance, owner_id)?;
        match token.kind {
            OwnerKind::Rel => {
                self.with_rel_mutation(&token, RelMutationFamily::Semantic, |runtime| {
                    let changed = runtime
                        .cva
                        .runtime_set_knowledge_community_name(community_id, name)
                        .map_err(operation)?;
                    Ok(((), changed, vec![RelInvalidation::Community(community_id)]))
                })
            }
            OwnerKind::Phy => self.with_phy(&token, |phy| {
                phy.runtime_set_knowledge_community_name(community_id, name)
                    .map_err(operation)?;
                phy.sync().map_err(operation)
            }),
        }
    }
}

fn relation_changes(
    source: MemoryId,
    target: MemoryId,
    old_kind: Option<GraphRelationKind>,
    new_kind: Option<GraphRelationKind>,
) -> Result<Vec<crate::GraphRelationChange>, ReliquaryRuntimeHostError> {
    if old_kind.is_none() && new_kind.is_none() {
        return Err(operation(
            "Knowledge relation mutation requires an old or new relation kind",
        ));
    }
    let mut changes = Vec::with_capacity(2);
    if let Some(kind) = old_kind {
        if Some(kind) != new_kind {
            changes.push(crate::GraphRelationChange {
                source,
                target,
                kind,
                active: false,
            });
        }
    }
    if let Some(kind) = new_kind {
        changes.push(crate::GraphRelationChange {
            source,
            target,
            kind,
            active: true,
        });
    }
    Ok(changes)
}
