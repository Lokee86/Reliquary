use crate::memory_store::MemoryStore;
use crate::{
    Container, Memory, MemoryDraft, MemoryError, MemoryId, MemoryRoutingMetadata, MemorySourceRef,
};

pub(crate) struct PersonalMemoryPublication<'a> {
    container: &'a mut Container,
    memories: &'a mut MemoryStore,
}
impl<'a> PersonalMemoryPublication<'a> {
    pub(crate) fn new(container: &'a mut Container, memories: &'a mut MemoryStore) -> Self {
        Self {
            container,
            memories,
        }
    }
    pub(crate) fn owner_id(&self) -> Option<String> {
        self.container.owner_id()
    }
    pub(crate) fn publish(
        &mut self,
        draft: MemoryDraft,
        source: MemorySourceRef,
        temporal: Option<crate::MemoryTemporalInference>,
    ) -> Result<(Memory, bool), MemoryError> {
        self.memories
            .publish_with_source_ref_and_temporal_inference(
                self.container,
                None,
                0,
                draft,
                Some(source),
                temporal,
            )
    }
    pub(crate) fn memory(&self, id: MemoryId) -> Result<Memory, MemoryError> {
        self.memories.memory(self.container, id)
    }
    pub(crate) fn put_routing_metadata(
        &mut self,
        metadata: MemoryRoutingMetadata,
    ) -> Result<(), MemoryError> {
        self.memories
            .put_routing_metadata(self.container, metadata)
            .map(|_| ())
    }
    pub(crate) fn sync(&mut self) -> Result<(), String> {
        self.container.sync().map_err(|e| e.to_string())
    }
}

use crate::MemoryRef;
use crate::insomnia::{InsomniaProcessError, PreparedUserMemory, UserPublication};
use crate::memory_model::memory_id;
pub(crate) fn publish_user_application(
    phylactery: &mut crate::owner_lifecycle::personal_memory_publication::PersonalMemoryPublication<
        '_,
    >,
    drafts: &[PreparedUserMemory],
) -> Result<UserPublication, InsomniaProcessError> {
    let owner_id = phylactery.owner_id().ok_or_else(|| {
        InsomniaProcessError::Phylactery(
            "Phylactery has no durable owner ID; migrate it before routed Insomnia processing"
                .into(),
        )
    })?;
    let mut created = Vec::new();
    let mut existing = Vec::new();
    let mut refs = Vec::with_capacity(drafts.len());
    for prepared in drafts {
        let temporal_inference = prepared.memory.bound_temporal_inference();
        let draft = &prepared.memory.draft;
        debug_assert_eq!(
            prepared.memory.temporal.analysis.source_timestamp_ns,
            draft.source_time_ns
        );
        let (memory, was_created) = match phylactery.publish(
            prepared.memory.draft.clone(),
            prepared.source_ref.clone(),
            temporal_inference,
        ) {
            Ok(value) => value,
            Err(crate::MemoryError::MutationConflict) => {
                let id = memory_id(&draft.mutation_id);
                let memory = phylactery.memory(id)?;
                if !same_routed_user_semantics(&memory, draft, &prepared.source_ref) {
                    return Err(crate::MemoryError::MutationConflict.into());
                }
                (memory, false)
            }
            Err(error) => return Err(error.into()),
        };
        if let Some(metadata) = prepared.memory.routing_metadata.clone() {
            phylactery.put_routing_metadata(metadata)?;
        }
        let memory = phylactery.memory(memory.id)?;
        if was_created {
            created.push(memory.clone());
        } else {
            existing.push(memory.clone());
        }
        refs.push(MemoryRef {
            owner_id: owner_id.clone(),
            memory_id: memory.id,
        });
    }
    phylactery
        .sync()
        .map_err(|error| InsomniaProcessError::Phylactery(error.to_string()))?;
    Ok(UserPublication {
        created,
        existing,
        refs,
    })
}

fn same_routed_user_semantics(
    memory: &Memory,
    draft: &MemoryDraft,
    source_ref: &MemorySourceRef,
) -> bool {
    memory.category == draft.category
        && memory.memory_type == draft.memory_type
        && memory.authority_kind == draft.authority_kind
        && memory.temporal_status == draft.temporal_status
        && memory.scope == draft.scope
        && memory.lifecycle_state == draft.lifecycle_state
        && memory.archived == draft.archived
        && memory.superseded_by == draft.superseded_by
        && memory.parent_id == draft.parent_id
        && memory.source_node_id.is_none()
        && memory.content_source_conversation_id.is_none()
        && memory.content_source_node_id.is_none()
        && memory.grounding_source_conversation_id.is_none()
        && memory.grounding_source_node_id.is_none()
        && memory.source_episode_id.is_none()
        && memory.source_time_ns == draft.source_time_ns
        && memory.source_ref.as_ref() == Some(source_ref)
        && memory.mutation_id == draft.mutation_id
        && memory.created_at_ns == draft.created_at_ns
}
