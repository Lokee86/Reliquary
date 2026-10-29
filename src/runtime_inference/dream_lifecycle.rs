use crate::dream_canonical::{
    canonical_superseders, corroborated_representative, duplicate_component_has_active_peer,
    unique_superseder,
};
use crate::{
    Cva, DreamLifecycleError, DreamLifecycleResult, GraphRelationKind, Memory, MemoryDraft,
    MemoryId,
};

impl Cva {
    pub fn reconcile_dream_lifecycle(
        &mut self,
        source_id: MemoryId,
    ) -> Result<DreamLifecycleResult, DreamLifecycleError> {
        let relations = self.graph_relations();
        let mut canonicalized = canonical_superseders(self, source_id, &relations)?;
        let mut superseded_targets: Vec<_> = relations
            .iter()
            .filter(|relation| {
                relation.kind == GraphRelationKind::Supersedes
                    && (relation.source == source_id || relation.target == source_id)
            })
            .map(|relation| relation.target)
            .collect();
        superseded_targets.sort_by_key(|id| id.0);
        superseded_targets.dedup();

        let source = self.memory(source_id)?;
        let has_active_duplicate_peer = !superseded_targets.contains(&source_id)
            && !source.archived
            && source.lifecycle_state == "extracted"
            && duplicate_component_has_active_peer(
                self,
                source_id,
                &relations,
                &superseded_targets,
            )?;
        let mut will_archive = superseded_targets.clone();
        if has_active_duplicate_peer {
            will_archive.push(source_id);
        }
        if let Some(representative) =
            corroborated_representative(self, source_id, &relations, &will_archive)?
        {
            canonicalized.push(representative);
        }
        let mut targets = Vec::with_capacity(superseded_targets.len());
        for id in superseded_targets {
            targets.push((
                lifecycle_memory(self.memory(id)?),
                unique_superseder(&relations, id),
            ));
        }
        let canonical_facts = canonicalized
            .into_iter()
            .map(|id| self.memory(id).map(lifecycle_memory))
            .collect::<Result<Vec<_>, _>>()?;
        let plan = reliquary_dream::plan_lifecycle(
            lifecycle_memory(source.clone()),
            &targets,
            has_active_duplicate_peer,
            canonical_facts,
            reliquary_dream::explicit_authority_promotes(
                &source.authority_kind,
                &source.category,
                &source.memory_type,
            ),
        );
        let mut revised = Vec::new();
        let mut archived = Vec::new();
        let mut promoted = Vec::new();
        let mut promoted_to_knowledge = false;
        for intent in plan {
            if let Some(memory) = self.revise_lifecycle(
                intent.memory_id,
                intent.lifecycle_state,
                intent.archived,
                intent.superseded_by,
            )? {
                revised.push(memory);
                match intent.lifecycle_state {
                    "archived" => archived.push(intent.memory_id),
                    "canonical" => promoted.push(intent.memory_id),
                    "knowledge" => promoted_to_knowledge = true,
                    _ => unreachable!("Dream lifecycle policy returned an unknown state"),
                }
            }
        }

        archived.sort_by_key(|id| id.0);
        archived.dedup();
        promoted.sort_by_key(|id| id.0);
        Ok(DreamLifecycleResult {
            source: self.memory(source_id)?,
            revised,
            archived,
            canonicalized: promoted,
            promoted_to_knowledge,
        })
    }

    fn revise_lifecycle(
        &mut self,
        id: MemoryId,
        lifecycle_state: &str,
        archived: bool,
        superseded_by: Option<MemoryId>,
    ) -> Result<Option<Memory>, DreamLifecycleError> {
        let current = self.memory(id)?;
        if current.lifecycle_state == lifecycle_state
            && current.archived == archived
            && current.superseded_by == superseded_by
        {
            return Ok(None);
        }
        let graph_version = self.graph_version();
        let draft = lifecycle_draft(
            &current,
            lifecycle_state,
            archived,
            superseded_by,
            graph_version,
        );
        let (memory, changed) = self.publish_memory(Some(id), current.revision, draft)?;
        Ok(changed.then_some(memory))
    }
}

fn lifecycle_memory(memory: Memory) -> reliquary_dream::DreamLifecycleMemory {
    reliquary_dream::DreamLifecycleMemory {
        id: memory.id,
        lifecycle_state: match memory.lifecycle_state.as_str() {
            "extracted" => "extracted",
            "knowledge" => "knowledge",
            "canonical" => "canonical",
            _ => "other",
        },
        archived: memory.archived,
        superseded_by: memory.superseded_by,
    }
}

fn lifecycle_draft(
    memory: &Memory,
    lifecycle_state: &str,
    archived: bool,
    superseded_by: Option<MemoryId>,
    graph_version: u64,
) -> MemoryDraft {
    MemoryDraft {
        category: memory.category.clone(),
        memory_type: memory.memory_type.clone(),
        authority_kind: memory.authority_kind.clone(),
        temporal_status: memory.temporal_status.clone(),
        title: memory.title.clone(),
        content: memory.content.clone(),
        scope: memory.scope.clone(),
        lifecycle_state: lifecycle_state.into(),
        archived,
        superseded_by,
        parent_id: memory.parent_id,
        source_node_id: memory.source_node_id.clone(),
        content_source_conversation_id: memory.content_source_conversation_id.clone(),
        content_source_node_id: memory.content_source_node_id.clone(),
        grounding_source_conversation_id: memory.grounding_source_conversation_id.clone(),
        grounding_source_node_id: memory.grounding_source_node_id.clone(),
        source_episode_id: memory.source_episode_id,
        source_time_ns: memory.source_time_ns,
        mutation_id: lifecycle_mutation_id(
            memory.id,
            lifecycle_state,
            superseded_by,
            graph_version,
        ),
        created_at_ns: memory.created_at_ns,
        updated_at_ns: memory.updated_at_ns.saturating_add(1),
    }
}

fn lifecycle_mutation_id(
    id: MemoryId,
    lifecycle_state: &str,
    superseded_by: Option<MemoryId>,
    graph_version: u64,
) -> String {
    let id = bytes_hex(&id.0);
    let superseder = superseded_by
        .map(|value| bytes_hex(&value.0))
        .unwrap_or_else(|| "none".into());
    format!("dream-lifecycle:{id}:{graph_version}:{lifecycle_state}:{superseder}")
}

fn bytes_hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write;
        let _ = write!(&mut out, "{byte:02x}");
    }
    out
}
