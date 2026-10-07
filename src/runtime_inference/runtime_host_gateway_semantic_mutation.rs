use super::super::events::RelInvalidation;
use super::super::mutation_events::RelMutationFamily;
use super::*;
use crate::{GraphRelation, GraphRelationKind, GraphRelationOrigin, Memory, MemoryDraft, MemoryId};
use uuid::Uuid;

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
        let draft = user_creation_draft(title, content, category, memory_type, now_ns);
        match token.kind {
            OwnerKind::Rel => {
                let memory =
                    self.with_rel_mutation(&token, RelMutationFamily::Semantic, |runtime| {
                        let (memory, _) = runtime
                            .cva
                            .publish_memory(None, 0, draft)
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
                    let (memory, _) = phy.publish_memory(None, 0, draft).map_err(operation)?;
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
                        let replacement = replace_memory_with_successor(
                            &mut runtime.cva,
                            memory_id,
                            title,
                            content,
                            now_ns,
                        )?;
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
                    let replacement =
                        replace_memory_with_successor(phy, memory_id, title, content, now_ns)?;
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
                    let version = runtime.cva.graph_version();
                    runtime
                        .cva
                        .set_memory_relations_with_origin(
                            &changes,
                            GraphRelationOrigin::User,
                            version,
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
                    let version = phy.graph_version();
                    phy.set_memory_relations_with_origin(
                        &changes,
                        GraphRelationOrigin::User,
                        version,
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
                self.rename_knowledge_community_for(instance, owner_id, community_id, name)
            }
            OwnerKind::Phy => self.with_phy(&token, |phy| {
                phy.set_community_name(community_id, name)
                    .map_err(operation)?;
                phy.sync().map_err(operation)
            }),
        }
    }
}

fn user_creation_draft(
    title: String,
    content: String,
    category: String,
    memory_type: String,
    now_ns: i64,
) -> MemoryDraft {
    MemoryDraft {
        category,
        memory_type,
        authority_kind: "direct".into(),
        title,
        content,
        scope: "private".into(),
        lifecycle_state: "extracted".into(),
        archived: false,
        superseded_by: None,
        parent_id: None,
        source_node_id: None,
        content_source_conversation_id: None,
        content_source_node_id: None,
        grounding_source_conversation_id: None,
        grounding_source_node_id: None,
        source_episode_id: None,
        source_time_ns: Some(now_ns),
        temporal_status: "unknown".into(),
        mutation_id: format!("user_creation:{}", Uuid::new_v4()),
        created_at_ns: now_ns,
        updated_at_ns: now_ns,
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

trait KnowledgeMutationStore {
    fn read_memory(&mut self, id: MemoryId) -> Result<Memory, ReliquaryRuntimeHostError>;
    fn relations(&self) -> Vec<GraphRelation>;
    fn publish_replacement(
        &mut self,
        current: &Memory,
        draft: MemoryDraft,
    ) -> Result<Memory, ReliquaryRuntimeHostError>;
    fn publish_retirement(
        &mut self,
        current: &Memory,
        draft: MemoryDraft,
    ) -> Result<(), ReliquaryRuntimeHostError>;
    fn set_relation(
        &mut self,
        source: MemoryId,
        target: MemoryId,
        kind: GraphRelationKind,
        origin: GraphRelationOrigin,
    ) -> Result<(), ReliquaryRuntimeHostError>;
}

impl KnowledgeMutationStore for crate::Cva {
    fn read_memory(&mut self, id: MemoryId) -> Result<Memory, ReliquaryRuntimeHostError> {
        self.memory(id).map_err(operation)
    }
    fn relations(&self) -> Vec<GraphRelation> {
        self.graph_relations()
    }
    fn publish_replacement(
        &mut self,
        _current: &Memory,
        draft: MemoryDraft,
    ) -> Result<Memory, ReliquaryRuntimeHostError> {
        self.publish_memory(None, 0, draft)
            .map(|x| x.0)
            .map_err(operation)
    }
    fn publish_retirement(
        &mut self,
        current: &Memory,
        draft: MemoryDraft,
    ) -> Result<(), ReliquaryRuntimeHostError> {
        self.publish_memory(Some(current.id), current.revision, draft)
            .map_err(operation)?;
        Ok(())
    }
    fn set_relation(
        &mut self,
        source: MemoryId,
        target: MemoryId,
        kind: GraphRelationKind,
        origin: GraphRelationOrigin,
    ) -> Result<(), ReliquaryRuntimeHostError> {
        self.set_memory_relation_with_origin(
            source,
            target,
            kind,
            true,
            origin,
            self.graph_version(),
        )
        .map(|_| ())
        .map_err(operation)
    }
}

impl KnowledgeMutationStore for crate::Phylactery {
    fn read_memory(&mut self, id: MemoryId) -> Result<Memory, ReliquaryRuntimeHostError> {
        self.memory(id).map_err(operation)
    }
    fn relations(&self) -> Vec<GraphRelation> {
        self.graph_relations()
    }
    fn publish_replacement(
        &mut self,
        current: &Memory,
        draft: MemoryDraft,
    ) -> Result<Memory, ReliquaryRuntimeHostError> {
        let result = match current.source_ref.clone() {
            Some(source_ref) => self.publish_memory_with_source_ref(None, 0, draft, source_ref),
            None => self.publish_memory(None, 0, draft),
        };
        result.map(|x| x.0).map_err(operation)
    }
    fn publish_retirement(
        &mut self,
        current: &Memory,
        draft: MemoryDraft,
    ) -> Result<(), ReliquaryRuntimeHostError> {
        self.publish_memory(Some(current.id), current.revision, draft)
            .map_err(operation)?;
        Ok(())
    }
    fn set_relation(
        &mut self,
        source: MemoryId,
        target: MemoryId,
        kind: GraphRelationKind,
        origin: GraphRelationOrigin,
    ) -> Result<(), ReliquaryRuntimeHostError> {
        self.set_memory_relation_with_origin(
            source,
            target,
            kind,
            true,
            origin,
            self.graph_version(),
        )
        .map(|_| ())
        .map_err(operation)
    }
}

fn replace_memory_with_successor<T: KnowledgeMutationStore>(
    store: &mut T,
    memory_id: MemoryId,
    title: String,
    content: String,
    now_ns: i64,
) -> Result<Memory, ReliquaryRuntimeHostError> {
    let current = store.read_memory(memory_id)?;
    let relations = store.relations();
    let replacement = store.publish_replacement(
        &current,
        replacement_draft(&current, title, content, now_ns),
    )?;
    for relation in relations
        .iter()
        .filter(|r| r.source == memory_id || r.target == memory_id)
    {
        let source = if relation.source == memory_id {
            replacement.id
        } else {
            relation.source
        };
        let target = if relation.target == memory_id {
            replacement.id
        } else {
            relation.target
        };
        store.set_relation(source, target, relation.kind, relation.origin)?;
    }
    store.set_relation(
        replacement.id,
        current.id,
        GraphRelationKind::Supersedes,
        GraphRelationOrigin::User,
    )?;
    let mut retired = replacement_draft(
        &current,
        current.title.clone(),
        current.content.clone(),
        now_ns,
    );
    retired.mutation_id = format!(
        "knowledge-retire:{}:{}",
        hex(&current.id.0),
        hex(&replacement.id.0),
    );
    retired.lifecycle_state = "archived".into();
    retired.archived = true;
    retired.superseded_by = Some(replacement.id);
    store.publish_retirement(&current, retired)?;
    Ok(replacement)
}

fn replacement_draft(current: &Memory, title: String, content: String, now_ns: i64) -> MemoryDraft {
    let provenance = if current.mutation_id.starts_with("user_creation:") {
        "user_creation"
    } else {
        "knowledge-replacement"
    };
    MemoryDraft {
        category: current.category.clone(),
        memory_type: current.memory_type.clone(),
        authority_kind: current.authority_kind.clone(),
        title,
        content,
        scope: current.scope.clone(),
        lifecycle_state: current.lifecycle_state.clone(),
        archived: current.archived,
        superseded_by: None,
        parent_id: current.parent_id,
        source_node_id: current.source_node_id.clone(),
        content_source_conversation_id: current.content_source_conversation_id.clone(),
        content_source_node_id: current.content_source_node_id.clone(),
        grounding_source_conversation_id: current.grounding_source_conversation_id.clone(),
        grounding_source_node_id: current.grounding_source_node_id.clone(),
        source_episode_id: current.source_episode_id,
        source_time_ns: current.source_time_ns,
        temporal_status: current.temporal_status.clone(),
        mutation_id: format!("{provenance}:{}", Uuid::new_v4()),
        created_at_ns: current.created_at_ns,
        updated_at_ns: now_ns.max(current.updated_at_ns.saturating_add(1)),
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
