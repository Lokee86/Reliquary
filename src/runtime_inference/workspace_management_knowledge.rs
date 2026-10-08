use super::*;
use crate::{CommunityId, Entity, GraphRelationKind, Memory, MemoryId, SemanticGraphRelation};

impl WorkspaceManagement {
    /// Output pagination over the existing owner read. This does not promise bounded
    /// source allocation: the A knowledge primitive currently hydrates the full owner.
    pub fn memories(
        &self,
        owner: &str,
        offset: usize,
        limit: usize,
    ) -> Result<ManagementPage<MemorySummary>, ManagementError> {
        validate_page(offset, limit)?;
        let token = self.token(owner)?;
        let (memories, _, _, _, _) = self
            .gateway
            .read_knowledge_for_owner(&self.instance, owner)
            .map_err(unavailable)?;
        let mut values = memories
            .into_iter()
            .map(|m| MemorySummary {
                id: m.id,
                revision: m.revision,
                title: m.title,
                category: m.category,
                lifecycle_state: m.lifecycle_state,
                archived: m.archived,
                superseded_by: m.superseded_by,
            })
            .collect::<Vec<_>>();
        values.sort_by_key(|m| m.id);
        let result = page(values, offset, limit);
        text_bound(
            result
                .items
                .iter()
                .map(|m| m.title.len() + m.category.len() + m.lifecycle_state.len())
                .sum(),
        )?;
        self.deliver(&token, result)
    }
    pub fn memory(&self, owner: &str, id: MemoryId) -> Result<Memory, ManagementError> {
        let token = self.token(owner)?;
        let value = self
            .gateway
            .knowledge_memory_for_owner(&self.instance, owner, id)
            .map_err(unavailable)?;
        text_bound(value.title.len() + value.content.len())?;
        self.deliver(&token, value)
    }
    pub fn provenance(
        &self,
        owner: &str,
        id: MemoryId,
    ) -> Result<crate::RuntimeKnowledgeProvenanceResolution, ManagementError> {
        let token = self.token(owner)?;
        let value = self
            .gateway
            .resolve_knowledge_provenance_for_owner(&self.instance, owner, id)
            .map_err(unavailable)?;
        let evidence = match &value {
            crate::RuntimeKnowledgeProvenanceResolution::None { provenance, .. } => {
                provenance.as_ref()
            }
            crate::RuntimeKnowledgeProvenanceResolution::Unresolved { .. } => None,
            crate::RuntimeKnowledgeProvenanceResolution::Resolved { provenance, .. } => {
                Some(provenance)
            }
        };
        if let Some(p) = evidence {
            let bytes = p.memory.title.len()
                + p.memory.content.len()
                + p.source_turn
                    .iter()
                    .chain(p.source_episode_turns.iter())
                    .chain(p.content_source_turn.iter())
                    .chain(p.grounding_source_turn.iter())
                    .map(|turn| turn.content.len())
                    .sum::<usize>();
            text_bound(bytes)?;
        }
        self.deliver(&token, value)
    }
    pub fn entities(
        &self,
        owner: &str,
        offset: usize,
        limit: usize,
    ) -> Result<ManagementPage<Entity>, ManagementError> {
        validate_page(offset, limit)?;
        let token = self.token(owner)?;
        let mut values = self
            .gateway
            .entities_for_owner(&self.instance, owner)
            .map_err(unavailable)?;
        // Reserved human principals are not ordinary knowledge inventory.
        values.retain(|e| e.kind != crate::entity_principal::PRINCIPAL_ENTITY_KIND);
        values.sort_by_key(|e| e.id);
        let result = page(values, offset, limit);
        text_bound(
            result
                .items
                .iter()
                .map(|e| {
                    e.canonical_name.len()
                        + e.summary.len()
                        + e.aliases.iter().map(String::len).sum::<usize>()
                })
                .sum(),
        )?;
        self.deliver(&token, result)
    }
    pub fn relations(
        &self,
        owner: &str,
        offset: usize,
        limit: usize,
    ) -> Result<ManagementPage<SemanticGraphRelation>, ManagementError> {
        validate_page(offset, limit)?;
        let token = self.token(owner)?;
        let mut values = self
            .gateway
            .semantic_graph_relations_for_owner(&self.instance, owner)
            .map_err(unavailable)?;
        // Stable exact semantic endpoints and kind define page order.
        values.sort_by_key(|r| (r.source, r.target, r.kind, r.graph_version));
        let result = page(std::mem::take(&mut values), offset, limit);
        self.deliver(&token, result)
    }
    pub fn communities(
        &self,
        owner: &str,
        offset: usize,
        limit: usize,
    ) -> Result<ManagementPage<CommunitySummary>, ManagementError> {
        validate_page(offset, limit)?;
        let token = self.token(owner)?;
        let (_, _, _, snapshot, names) = self
            .gateway
            .read_knowledge_for_owner(&self.instance, owner)
            .map_err(unavailable)?;
        let mut values = Vec::new();
        if let Some(snapshot) = snapshot {
            for community in snapshot.communities {
                let name = names
                    .iter()
                    .find(|n| n.community_id == community.id)
                    .map(|n| n.name.clone());
                values.push(CommunitySummary {
                    id: community.id,
                    members: community.members.len(),
                    name,
                    generation: snapshot.generation,
                    derived_graph_version: snapshot.derived_graph_version,
                });
            }
        }
        values.sort_by_key(|c| c.id);
        let result = page(values, offset, limit);
        text_bound(
            result
                .items
                .iter()
                .map(|c| c.name.as_ref().map_or(0, String::len))
                .sum(),
        )?;
        self.deliver(&token, result)
    }
    pub fn create_memory(
        &self,
        owner: &str,
        title: String,
        content: String,
        category: String,
        memory_type: String,
        now_ns: i64,
    ) -> Result<Memory, ManagementError> {
        self.gateway
            .create_knowledge_memory_for_owner(
                &self.instance,
                owner,
                title,
                content,
                category,
                memory_type,
                now_ns,
            )
            .map_err(unavailable)
    }
    /// Existing successor/retirement semantics, never an in-place or generic patch.
    pub fn replace_memory(
        &self,
        owner: &str,
        id: MemoryId,
        title: String,
        content: String,
        now_ns: i64,
    ) -> Result<Memory, ManagementError> {
        self.gateway
            .replace_knowledge_memory_for_owner(&self.instance, owner, id, title, content, now_ns)
            .map_err(unavailable)
    }
    pub fn rename_community(
        &self,
        owner: &str,
        id: CommunityId,
        name: String,
    ) -> Result<(), ManagementError> {
        self.gateway
            .rename_knowledge_community_for_owner(&self.instance, owner, id, name)
            .map_err(unavailable)
    }
    pub fn change_relation(
        &self,
        owner: &str,
        source: MemoryId,
        target: MemoryId,
        old_kind: Option<GraphRelationKind>,
        new_kind: Option<GraphRelationKind>,
    ) -> Result<(), ManagementError> {
        self.gateway
            .mutate_knowledge_relation_for_owner(
                &self.instance,
                owner,
                source,
                target,
                old_kind,
                new_kind,
            )
            .map_err(unavailable)
    }
}
