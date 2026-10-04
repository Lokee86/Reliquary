use super::{
    GatewayRuntime, InstanceRuntime, OwnerAccessToken, OwnerGrant, ReliquaryRuntimeHostError,
    operation,
};
use crate::{
    ArchiveSearchHit, CommunityId, ConversationSearchHit, ConversationSummary, EchoEvent, Entity,
    GraphRelationChange, GraphRelationKind, GraphRelationOrigin, Memory, MemoryDraft, MemoryId,
    MemoryProvenance, ResolvedInteractionTurn, StoredFile,
};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::Arc;
use uuid::Uuid;

#[path = "runtime_host_gateway_ambient_extended.rs"]
mod extended;
#[path = "runtime_host_gateway_ambient_project.rs"]
mod project;

impl GatewayRuntime {
    /// Hold only the attachment's short delivery lease while the result is
    /// handed back. Callers must finish provider/filesystem work before this
    /// boundary and must reuse the authorization captured before that work.
    fn with_instance_delivery<T>(
        &self,
        instance: &InstanceRuntime,
        authorization: super::InstanceAuthorization,
        deliver: impl FnOnce() -> Result<T, ReliquaryRuntimeHostError>,
    ) -> Result<T, ReliquaryRuntimeHostError> {
        let attachment = self.attachment_entry(instance)?;
        let _lease = attachment.leases.acquire_current()?;
        if instance.authorization().map_err(operation)? != authorization {
            return Err(operation("instance authorization changed before delivery"));
        }
        let value = deliver()?;
        if instance.authorization().map_err(operation)? != authorization {
            return Err(operation("instance authorization changed before delivery"));
        }
        Ok(value)
    }

    /// Revalidate a captured owner token under short attachment and owner
    /// admission leases at the final delivery boundary. No provider or
    /// filesystem operation may run inside this closure.
    fn deliver_rel<T>(
        &self,
        instance: &InstanceRuntime,
        token: &OwnerAccessToken,
        value: T,
    ) -> Result<T, ReliquaryRuntimeHostError> {
        if instance.instance_id() != token.instance_id {
            return Err(operation("owner token belongs to a different instance"));
        }
        self.with_rel(token, |_| Ok(value))
    }

    fn ambient_rel_token(
        &self,
        instance: &InstanceRuntime,
    ) -> Result<OwnerAccessToken, ReliquaryRuntimeHostError> {
        let owner_id = self
            .active_rel_id(instance)?
            .ok_or_else(|| operation("instance has no selected REL"))?;
        self.authorize(instance, &owner_id, OwnerGrant::ViewRel)
    }

    fn explicit_rel_token(
        &self,
        instance: &InstanceRuntime,
        owner_id: &str,
    ) -> Result<OwnerAccessToken, ReliquaryRuntimeHostError> {
        self.authorize(instance, owner_id, OwnerGrant::ViewRel)
    }

    /// Read summaries from the REL selected by this exact instance attachment.
    pub fn conversation_summaries(
        &self,
        instance: &InstanceRuntime,
    ) -> Result<Vec<ConversationSummary>, ReliquaryRuntimeHostError> {
        let token = self.ambient_rel_token(instance)?;
        self.with_rel(&token, |runtime| Ok(runtime.conversation_summaries()))
    }

    pub fn conversation_summaries_for(
        &self,
        instance: &InstanceRuntime,
        owner_id: &str,
    ) -> Result<Vec<ConversationSummary>, ReliquaryRuntimeHostError> {
        let token = self.explicit_rel_token(instance, owner_id)?;
        self.with_rel(&token, |runtime| Ok(runtime.conversation_summaries()))
    }

    /// Read a transcript from the REL selected by this exact instance attachment.
    pub fn conversation_transcript(
        &self,
        instance: &InstanceRuntime,
        conversation_id: &str,
        leaf_node_id: &str,
    ) -> Result<Vec<ResolvedInteractionTurn>, ReliquaryRuntimeHostError> {
        let token = self.ambient_rel_token(instance)?;
        self.with_rel(&token, |runtime| {
            runtime
                .conversation_transcript(conversation_id, leaf_node_id)
                .map_err(operation)
        })
    }

    /// Search a branch in the REL selected by this exact instance attachment.
    pub fn search_conversation_branch(
        &self,
        instance: &InstanceRuntime,
        conversation_id: &str,
        leaf_node_id: &str,
        query: &str,
        limit: usize,
    ) -> Result<Vec<ConversationSearchHit>, ReliquaryRuntimeHostError> {
        let token = self.ambient_rel_token(instance)?;
        self.with_rel(&token, |runtime| {
            runtime
                .search_conversation_branch(conversation_id, leaf_node_id, query, limit)
                .map_err(operation)
        })
    }

    pub fn echo_events(
        &self,
        instance: &InstanceRuntime,
        conversation_id: &str,
        message_id: &str,
    ) -> Result<Vec<EchoEvent>, ReliquaryRuntimeHostError> {
        let token = self.ambient_rel_token(instance)?;
        self.with_rel(&token, |runtime| {
            Ok(runtime.cva().echo_events(conversation_id, message_id))
        })
    }

    /// Explicit-owner transcript read still requires this instance's ViewRel grant.
    pub fn conversation_transcript_for(
        &self,
        instance: &InstanceRuntime,
        owner_id: &str,
        conversation_id: &str,
        leaf_node_id: &str,
    ) -> Result<Vec<ResolvedInteractionTurn>, ReliquaryRuntimeHostError> {
        let token = self.explicit_rel_token(instance, owner_id)?;
        self.with_rel(&token, |runtime| {
            runtime
                .conversation_transcript(conversation_id, leaf_node_id)
                .map_err(operation)
        })
    }

    /// Explicit-owner branch search still requires this instance's ViewRel grant.
    pub fn search_conversation_branch_for(
        &self,
        instance: &InstanceRuntime,
        owner_id: &str,
        conversation_id: &str,
        leaf_node_id: &str,
        query: &str,
        limit: usize,
    ) -> Result<Vec<ConversationSearchHit>, ReliquaryRuntimeHostError> {
        let token = self.explicit_rel_token(instance, owner_id)?;
        self.with_rel(&token, |runtime| {
            runtime
                .search_conversation_branch(conversation_id, leaf_node_id, query, limit)
                .map_err(operation)
        })
    }

    pub fn echo_events_for(
        &self,
        instance: &InstanceRuntime,
        owner_id: &str,
        conversation_id: &str,
        message_id: &str,
    ) -> Result<Vec<EchoEvent>, ReliquaryRuntimeHostError> {
        let token = self.explicit_rel_token(instance, owner_id)?;
        self.with_rel(&token, |runtime| {
            Ok(runtime.cva().echo_events(conversation_id, message_id))
        })
    }

    pub fn read_echo_source_for(
        &self,
        instance: &InstanceRuntime,
        owner_id: &str,
        conversation_id: &str,
        start_message_id: &str,
        end_message_id: &str,
    ) -> Result<crate::RuntimeEchoSource, ReliquaryRuntimeHostError> {
        // The token is bound to the exact REL containing both the transcript and Echo.
        let token = self.explicit_rel_token(instance, owner_id)?;
        let transcript = self.with_rel(&token, |runtime| {
            runtime
                .conversation_transcript(conversation_id, end_message_id)
                .map_err(operation)
        })?;
        let start = transcript
            .iter()
            .position(|turn| turn.message_id == start_message_id)
            .ok_or_else(|| operation("Echo source start is not on the returned transcript path"))?;
        if transcript.last().map(|turn| turn.message_id.as_str()) != Some(end_message_id) {
            return Err(operation(
                "Echo source end is not the requested transcript leaf",
            ));
        }
        let mut turns = Vec::new();
        for turn in &transcript[start..] {
            let events = self.with_rel(&token, |runtime| {
                Ok(runtime.cva().echo_events(conversation_id, &turn.message_id))
            })?;
            if !events.is_empty() {
                turns.push(crate::RuntimeEchoTurn {
                    message_id: turn.message_id.clone(),
                    events,
                });
            }
        }
        self.deliver_rel(
            instance,
            &token,
            crate::RuntimeEchoSource {
                owner_id: owner_id.to_owned(),
                conversation_id: conversation_id.to_owned(),
                start_message_id: start_message_id.to_owned(),
                end_message_id: end_message_id.to_owned(),
                turns,
            },
        )
    }

    pub fn files_for_source(
        &self,
        instance: &InstanceRuntime,
        conversation_id: &str,
        node_id: &str,
    ) -> Result<Vec<StoredFile>, ReliquaryRuntimeHostError> {
        let token = self.ambient_rel_token(instance)?;
        self.with_rel(&token, |runtime| {
            Ok(runtime.cva().files_for_source(conversation_id, node_id))
        })
    }

    pub fn files_for_source_for(
        &self,
        instance: &InstanceRuntime,
        owner_id: &str,
        conversation_id: &str,
        node_id: &str,
    ) -> Result<Vec<StoredFile>, ReliquaryRuntimeHostError> {
        let token = self.explicit_rel_token(instance, owner_id)?;
        self.with_rel(&token, |runtime| {
            Ok(runtime.cva().files_for_source(conversation_id, node_id))
        })
    }

    pub fn project_file_ref(
        &self,
        instance: &InstanceRuntime,
        file_id: crate::FileId,
    ) -> Result<Option<crate::ProjectFileRef>, ReliquaryRuntimeHostError> {
        let token = self.ambient_rel_token(instance)?;
        self.with_rel(&token, |runtime| {
            Ok(runtime.cva().project_file_ref(file_id))
        })
    }

    pub fn project_file_ref_for(
        &self,
        instance: &InstanceRuntime,
        owner_id: &str,
        file_id: crate::FileId,
    ) -> Result<Option<crate::ProjectFileRef>, ReliquaryRuntimeHostError> {
        let token = self.explicit_rel_token(instance, owner_id)?;
        self.with_rel(&token, |runtime| {
            Ok(runtime.cva().project_file_ref(file_id))
        })
    }

    pub fn file_bytes(
        &self,
        instance: &InstanceRuntime,
        file_id: crate::FileId,
    ) -> Result<Vec<u8>, ReliquaryRuntimeHostError> {
        let token = self.ambient_rel_token(instance)?;
        self.with_rel(&token, |runtime| {
            runtime.cva.file_bytes(file_id).map_err(operation)
        })
    }

    pub fn file_bytes_for(
        &self,
        instance: &InstanceRuntime,
        owner_id: &str,
        file_id: crate::FileId,
    ) -> Result<Vec<u8>, ReliquaryRuntimeHostError> {
        let token = self.explicit_rel_token(instance, owner_id)?;
        self.with_rel(&token, |runtime| {
            runtime.cva.file_bytes(file_id).map_err(operation)
        })
    }

    pub fn search_archive(
        &self,
        instance: &InstanceRuntime,
        query: &str,
        limit: usize,
    ) -> Result<Vec<ArchiveSearchHit>, ReliquaryRuntimeHostError> {
        let token = self.ambient_rel_token(instance)?;
        self.with_rel(&token, |runtime| {
            runtime.cva.search_archive(query, limit).map_err(operation)
        })
    }

    pub fn search_archive_for(
        &self,
        instance: &InstanceRuntime,
        owner_id: &str,
        query: &str,
        limit: usize,
    ) -> Result<Vec<ArchiveSearchHit>, ReliquaryRuntimeHostError> {
        let token = self.explicit_rel_token(instance, owner_id)?;
        self.with_rel(&token, |runtime| {
            runtime.cva.search_archive(query, limit).map_err(operation)
        })
    }

    pub fn read_reliquary_knowledge(
        &self,
        instance: &InstanceRuntime,
    ) -> Result<crate::RuntimeKnowledgeState, ReliquaryRuntimeHostError> {
        let token = self.ambient_rel_token(instance)?;
        self.read_knowledge_with_token(&token)
    }

    pub fn read_reliquary_knowledge_for(
        &self,
        instance: &InstanceRuntime,
        owner_id: &str,
    ) -> Result<crate::RuntimeKnowledgeState, ReliquaryRuntimeHostError> {
        let token = self.explicit_rel_token(instance, owner_id)?;
        self.read_knowledge_with_token(&token)
    }

    fn read_knowledge_with_token(
        &self,
        token: &OwnerAccessToken,
    ) -> Result<crate::RuntimeKnowledgeState, ReliquaryRuntimeHostError> {
        self.with_rel(token, |runtime| {
            let cva = &mut runtime.cva;
            let memories = cva
                .memory_ids()
                .into_iter()
                .map(|id| cva.memory(id).map_err(operation))
                .collect::<Result<Vec<_>, _>>()?;
            Ok((
                memories,
                cva.entities()
                    .into_iter()
                    .filter(|entity| entity.kind != crate::entity_principal::PRINCIPAL_ENTITY_KIND)
                    .collect::<Vec<Entity>>(),
                cva.semantic_graph_relations(),
                cva.community_snapshot(),
                cva.community_semantic_names(),
            ))
        })
    }

    pub fn knowledge_memory(
        &self,
        instance: &InstanceRuntime,
        memory_id: MemoryId,
    ) -> Result<Memory, ReliquaryRuntimeHostError> {
        let token = self.ambient_rel_token(instance)?;
        self.with_rel(&token, |runtime| {
            runtime.cva.memory(memory_id).map_err(operation)
        })
    }

    pub fn knowledge_memory_for(
        &self,
        instance: &InstanceRuntime,
        owner_id: &str,
        memory_id: MemoryId,
    ) -> Result<Memory, ReliquaryRuntimeHostError> {
        let token = self.explicit_rel_token(instance, owner_id)?;
        self.with_rel(&token, |runtime| {
            runtime.cva.memory(memory_id).map_err(operation)
        })
    }

    pub fn read_memory_provenance(
        &self,
        instance: &InstanceRuntime,
        memory_id: MemoryId,
    ) -> Result<MemoryProvenance, ReliquaryRuntimeHostError> {
        let token = self.ambient_rel_token(instance)?;
        self.read_memory_provenance_with_token(instance, &token, memory_id)
    }

    pub fn read_memory_provenance_for(
        &self,
        instance: &InstanceRuntime,
        owner_id: &str,
        memory_id: MemoryId,
    ) -> Result<MemoryProvenance, ReliquaryRuntimeHostError> {
        let token = self.explicit_rel_token(instance, owner_id)?;
        self.read_memory_provenance_with_token(instance, &token, memory_id)
    }

    fn read_memory_provenance_with_token(
        &self,
        instance: &InstanceRuntime,
        token: &OwnerAccessToken,
        memory_id: MemoryId,
    ) -> Result<MemoryProvenance, ReliquaryRuntimeHostError> {
        let memory = self.with_rel(token, |runtime| {
            runtime.cva.memory(memory_id).map_err(operation)
        })?;
        if let Some(source_ref) = memory.source_ref.clone() {
            // A foreign source owner is a separate authorization boundary even if
            // this memory is already readable from its containing REL.
            let provenance = self.resolve_external_memory_provenance_for(
                instance,
                &source_ref.owner_id,
                memory,
            )?;
            return self.deliver_rel(instance, token, provenance);
        }
        let provenance = self.with_rel(token, |runtime| {
            resolve_memory_provenance(&mut runtime.cva, memory)
        })?;
        self.deliver_rel(instance, token, provenance)
    }

    /// Resolve an external source reference only after authorizing that exact source REL.
    pub fn resolve_external_memory_provenance_for(
        &self,
        instance: &InstanceRuntime,
        source_owner_id: &str,
        memory: Memory,
    ) -> Result<MemoryProvenance, ReliquaryRuntimeHostError> {
        let source_ref = memory
            .source_ref
            .clone()
            .ok_or_else(|| operation("Memory does not have an external source reference"))?;
        if source_ref.owner_id != source_owner_id {
            return Err(operation(
                "provenance source owner does not match the requested REL",
            ));
        }
        let token = self.explicit_rel_token(instance, source_owner_id)?;
        let mut memory = memory;
        let provenance = self.with_rel(&token, |runtime| {
            if runtime.cva().owner_id().as_deref() != Some(source_owner_id) {
                return Err(operation(
                    "external Memory provenance belongs to a different REL",
                ));
            }
            memory.source_episode_id = Some(source_ref.source_episode_id);
            memory.source_node_id = Some(source_ref.source_node_id);
            memory.content_source_conversation_id = source_ref.content_source_conversation_id;
            memory.content_source_node_id = source_ref.content_source_node_id;
            memory.grounding_source_conversation_id = source_ref.grounding_source_conversation_id;
            memory.grounding_source_node_id = source_ref.grounding_source_node_id;
            resolve_memory_provenance(&mut runtime.cva, memory)
        })?;
        self.deliver_rel(instance, &token, provenance)
    }

    pub fn rel_metadata(
        &self,
        instance: &InstanceRuntime,
    ) -> Result<crate::RelMetadata, ReliquaryRuntimeHostError> {
        let token = self.ambient_rel_token(instance)?;
        self.with_rel(&token, |runtime| Ok(runtime.cva().rel_metadata()))
    }

    pub fn rel_metadata_for(
        &self,
        instance: &InstanceRuntime,
        owner_id: &str,
    ) -> Result<crate::RelMetadata, ReliquaryRuntimeHostError> {
        let token = self.explicit_rel_token(instance, owner_id)?;
        self.with_rel(&token, |runtime| Ok(runtime.cva().rel_metadata()))
    }

    pub fn dependency_closure(
        &self,
        instance: &InstanceRuntime,
    ) -> Result<Vec<String>, ReliquaryRuntimeHostError> {
        let root = self
            .active_rel_id(instance)?
            .ok_or_else(|| operation("instance has no selected REL"))?;
        self.dependency_closure_for(instance, &root)
    }

    /// Return only the mounted dependency closure that this attachment may view.
    /// Unauthorized dependency metadata is never used to expand the returned graph.
    pub fn dependency_closure_for(
        &self,
        instance: &InstanceRuntime,
        owner_id: &str,
    ) -> Result<Vec<String>, ReliquaryRuntimeHostError> {
        let authorization = instance.authorization().map_err(operation)?;
        let root_token = self.explicit_rel_token(instance, owner_id)?;
        let grants = instance.grants_snapshot().map_err(operation)?;
        let mut queue = vec![owner_id.to_owned()];
        let mut visited = BTreeSet::new();
        let mut ordered = Vec::new();
        while let Some(current) = queue.pop() {
            if !visited.insert(current.clone()) {
                continue;
            }
            let token = if current == owner_id {
                root_token.clone()
            } else if grants
                .get(&current)
                .is_some_and(|set| set.contains(&OwnerGrant::ViewRel))
            {
                match self.authorize(instance, &current, OwnerGrant::ViewRel) {
                    Ok(token) => token,
                    Err(_) => continue,
                }
            } else {
                continue;
            };
            let dependencies = self.with_rel(&token, |runtime| {
                Ok(runtime.cva().rel_metadata().dependencies)
            })?;
            ordered.push(current);
            // Reverse push preserves the REL's canonical dependency ordering.
            for dependency in dependencies.into_iter().rev() {
                if grants
                    .get(&dependency)
                    .is_some_and(|set| set.contains(&OwnerGrant::ViewRel))
                {
                    queue.push(dependency);
                }
            }
        }
        self.with_instance_delivery(instance, authorization, || Ok(ordered))
    }

    pub fn set_rel_metadata(
        &self,
        instance: &InstanceRuntime,
        type_label: Option<String>,
        dependencies: Vec<String>,
    ) -> Result<bool, ReliquaryRuntimeHostError> {
        let owner_id = self
            .active_rel_id(instance)?
            .ok_or_else(|| operation("instance has no selected REL"))?;
        self.set_rel_metadata_for(instance, &owner_id, type_label, dependencies)
    }

    pub fn set_rel_metadata_for(
        &self,
        instance: &InstanceRuntime,
        owner_id: &str,
        type_label: Option<String>,
        mut dependencies: Vec<String>,
    ) -> Result<bool, ReliquaryRuntimeHostError> {
        let token = self.authorize(instance, owner_id, OwnerGrant::ManageRel)?;
        let _topology = self
            .topology_transition
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
        dependencies.sort();
        dependencies.dedup();
        if dependencies.iter().any(|dependency| dependency == owner_id) {
            return Err(operation("REL cannot depend on itself"));
        }
        for dependency in &dependencies {
            self.authorize(instance, dependency, OwnerGrant::ViewRel)?;
        }
        self.validate_gateway_dependency_update(owner_id, &dependencies)?;
        let changed = self.with_rel(&token, |runtime| {
            let changed = runtime
                .cva
                .set_rel_metadata(type_label, dependencies)
                .map_err(operation)?;
            if changed {
                runtime.cva.sync().map_err(operation)?;
            }
            Ok(changed)
        })?;
        Ok(changed)
    }

    fn validate_gateway_dependency_update(
        &self,
        owner_id: &str,
        dependencies: &[String],
    ) -> Result<(), ReliquaryRuntimeHostError> {
        // Clone handles while holding the registry lock, then drop it before any
        // owner or runtime mutex is acquired.
        let owners = {
            let rels = self
                .rel_executions
                .read()
                .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
            rels.iter()
                .map(|(id, rel)| (id.clone(), Arc::clone(&rel.owner)))
                .collect::<Vec<_>>()
        };
        let mut graph = BTreeMap::<String, Vec<String>>::new();
        for (id, owner) in owners {
            let execution = owner
                .lock()
                .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
            let runtime = execution
                .runtime
                .as_ref()
                .ok_or_else(|| operation("REL runtime is unavailable"))?;
            let runtime = runtime
                .lock()
                .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
            graph.insert(id, runtime.cva().rel_metadata().dependencies);
        }
        if !graph.contains_key(owner_id) {
            return Err(operation(format!("REL owner {owner_id} is not mounted")));
        }
        for dependency in dependencies {
            if !graph.contains_key(dependency) {
                return Err(operation(format!(
                    "REL dependency {dependency} is not mounted"
                )));
            }
        }
        graph.insert(owner_id.to_owned(), dependencies.to_vec());
        let mut visiting = BTreeSet::new();
        let mut visited = BTreeSet::new();
        fn visit(
            id: &str,
            graph: &BTreeMap<String, Vec<String>>,
            visiting: &mut BTreeSet<String>,
            visited: &mut BTreeSet<String>,
        ) -> Result<(), ReliquaryRuntimeHostError> {
            if visited.contains(id) {
                return Ok(());
            }
            if !visiting.insert(id.to_owned()) {
                return Err(operation("REL dependency graph contains a cycle"));
            }
            if let Some(next) = graph.get(id) {
                for dependency in next {
                    visit(dependency, graph, visiting, visited)?;
                }
            }
            visiting.remove(id);
            visited.insert(id.to_owned());
            Ok(())
        }
        for id in graph.keys() {
            visit(id, &graph, &mut visiting, &mut visited)?;
        }
        Ok(())
    }

    pub fn create_knowledge_memory(
        &self,
        instance: &InstanceRuntime,
        title: String,
        content: String,
        category: String,
        memory_type: String,
        now_ns: i64,
    ) -> Result<Memory, ReliquaryRuntimeHostError> {
        let owner_id = self
            .active_rel_id(instance)?
            .ok_or_else(|| operation("instance has no selected REL"))?;
        self.create_knowledge_memory_for(
            instance,
            &owner_id,
            title,
            content,
            category,
            memory_type,
            now_ns,
        )
    }

    pub fn create_knowledge_memory_for(
        &self,
        instance: &InstanceRuntime,
        owner_id: &str,
        title: String,
        content: String,
        category: String,
        memory_type: String,
        now_ns: i64,
    ) -> Result<Memory, ReliquaryRuntimeHostError> {
        let token = self.authorize(instance, owner_id, OwnerGrant::ManageRel)?;
        let draft = user_creation_draft(title, content, category, memory_type, now_ns);
        let memory = self.with_rel(&token, |runtime| {
            let (memory, _) = runtime
                .cva
                .publish_memory(None, 0, draft)
                .map_err(operation)?;
            runtime.cva.sync().map_err(operation)?;
            Ok(memory)
        })?;
        self.wake_rel(owner_id)?;
        Ok(memory)
    }

    pub fn rename_knowledge_community_for(
        &self,
        instance: &InstanceRuntime,
        owner_id: &str,
        community_id: CommunityId,
        name: String,
    ) -> Result<(), ReliquaryRuntimeHostError> {
        let token = self.authorize(instance, owner_id, OwnerGrant::ManageRel)?;
        self.with_rel(&token, |runtime| {
            runtime
                .cva
                .set_community_name(community_id, name)
                .map_err(operation)?;
            runtime.cva.sync().map_err(operation)
        })
    }

    pub fn mutate_knowledge_relation_for(
        &self,
        instance: &InstanceRuntime,
        owner_id: &str,
        source: MemoryId,
        target: MemoryId,
        old_kind: Option<GraphRelationKind>,
        new_kind: Option<GraphRelationKind>,
    ) -> Result<(), ReliquaryRuntimeHostError> {
        let token = self.authorize(instance, owner_id, OwnerGrant::ManageRel)?;
        let changes = relation_changes(source, target, old_kind, new_kind)?;
        self.with_rel(&token, |runtime| {
            let version = runtime.cva().graph_version();
            runtime
                .cva
                .set_memory_relations_with_origin(&changes, GraphRelationOrigin::User, version)
                .map_err(operation)?;
            runtime.cva.sync().map_err(operation)
        })?;
        self.wake_rel(owner_id)
    }

    pub fn retrieve_reliquary_memories_for(
        &self,
        instance: &InstanceRuntime,
        owner_id: &str,
        profile: crate::CompatibilityProfileId,
        query_vector: &[f32],
        config: crate::MemoryRetrievalConfig,
    ) -> Result<crate::MemorySearchLane, ReliquaryRuntimeHostError> {
        let token = self.explicit_rel_token(instance, owner_id)?;
        self.retrieve_reliquary_memories_with_token(&token, profile, query_vector, config)
    }

    pub(super) fn retrieve_reliquary_memories_with_token(
        &self,
        token: &OwnerAccessToken,
        profile: crate::CompatibilityProfileId,
        query_vector: &[f32],
        config: crate::MemoryRetrievalConfig,
    ) -> Result<crate::MemorySearchLane, ReliquaryRuntimeHostError> {
        self.with_rel(token, |runtime| {
            let owner_id = runtime
                .cva()
                .owner_id()
                .ok_or_else(|| operation("REL does not have durable owner identity"))?;
            let result = runtime
                .cva
                .retrieve_memories(profile, query_vector, config)
                .map_err(operation)?;
            let scores = result
                .seeds
                .iter()
                .map(|hit| (hit.memory_id, hit.score))
                .collect::<HashMap<_, _>>();
            let items = result
                .memories
                .iter()
                .map(|id| {
                    runtime
                        .cva
                        .memory(*id)
                        .map(|memory| crate::MemorySearchItem {
                            memory,
                            seed_score: scores.get(id).copied(),
                        })
                        .map_err(operation)
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(crate::MemorySearchLane {
                owner_id,
                retrieval: result,
                items,
            })
        })
    }

    fn wake_rel(&self, owner_id: &str) -> Result<(), ReliquaryRuntimeHostError> {
        let owner = {
            let rels = self
                .rel_executions
                .read()
                .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
            Arc::clone(
                &rels
                    .get(owner_id)
                    .ok_or_else(|| operation(format!("REL owner {owner_id} is not mounted")))?
                    .owner,
            )
        };
        let signal = Arc::clone(
            &owner
                .lock()
                .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
                .signal,
        );
        crate::runtime_host::notify_work(&signal)
    }

    pub fn background_status(
        &self,
        instance: &InstanceRuntime,
    ) -> Result<crate::RuntimeBackgroundStatus, ReliquaryRuntimeHostError> {
        let token = self.ambient_rel_token(instance)?;
        // Snapshot worker-owned configuration without retaining the registry lock.
        let owner = {
            let rels = self
                .rel_executions
                .read()
                .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
            Arc::clone(
                &rels
                    .get(token.owner_id())
                    .ok_or_else(|| operation("selected REL is not mounted"))?
                    .owner,
            )
        };
        let (profile, perception_pending) = {
            let execution = owner
                .lock()
                .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
            let profile = execution
                .memory_profiles
                .lock()
                .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
                .project;
            let perception_pending = execution
                .perception_queue
                .lock()
                .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
                .counts()
                .0;
            (profile, perception_pending)
        };
        self.with_rel(&token, |runtime| {
            let cva = &mut runtime.cva;
            let ids = cva.memory_ids();
            let (vector_pending, dream_pending) = rel_pending(cva, &ids, profile)?;
            Ok(crate::RuntimeBackgroundStatus {
                insomnia: cva.insomnia_stats(),
                reliquary_memories: cva.memory_stats(),
                reliquary_vectors: cva.memory_vector_stats(),
                phylactery_memories: None,
                phylactery_vectors: None,
                reliquary_vector_pending: vector_pending,
                phylactery_vector_pending: 0,
                reliquary_dream_pending: dream_pending,
                phylactery_dream_pending: 0,
                reliquary_perception_pending: perception_pending,
                phylactery_perception_pending: 0,
            })
        })
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
) -> Result<Vec<GraphRelationChange>, ReliquaryRuntimeHostError> {
    match (old_kind, new_kind) {
        (None, None) => Err(operation(
            "Knowledge relation mutation requires an old or new relation kind",
        )),
        (Some(kind), Some(next)) if kind == next => Ok(vec![GraphRelationChange {
            source,
            target,
            kind,
            active: true,
        }]),
        (old_kind, new_kind) => {
            let mut changes = Vec::with_capacity(2);
            if let Some(kind) = old_kind {
                changes.push(GraphRelationChange {
                    source,
                    target,
                    kind,
                    active: false,
                });
            }
            if let Some(kind) = new_kind {
                changes.push(GraphRelationChange {
                    source,
                    target,
                    kind,
                    active: true,
                });
            }
            Ok(changes)
        }
    }
}

fn resolve_memory_provenance(
    cva: &mut crate::Cva,
    memory: Memory,
) -> Result<MemoryProvenance, ReliquaryRuntimeHostError> {
    let source_episode = match memory.source_episode_id {
        Some(id) => Some(
            cva.episode(id)
                .cloned()
                .ok_or_else(|| operation("Memory source Episode is unavailable"))?,
        ),
        None => None,
    };
    let source_episode_turns = match source_episode.as_ref() {
        Some(episode) => cva.episode_turns(episode.id).map_err(operation)?,
        None => Vec::new(),
    };
    let source_turn = memory
        .source_node_id
        .as_deref()
        .map(|node_id| {
            source_episode_turns
                .iter()
                .find(|turn| turn.node_id == node_id)
                .cloned()
                .ok_or_else(|| {
                    operation("Memory source turn is unavailable from its source Episode")
                })
        })
        .transpose()?;
    let content_source_turn = resolve_source_turn(
        cva,
        memory.content_source_conversation_id.as_deref(),
        memory.content_source_node_id.as_deref(),
        "content source",
    )?;
    let grounding_source_turn = resolve_source_turn(
        cva,
        memory.grounding_source_conversation_id.as_deref(),
        memory.grounding_source_node_id.as_deref(),
        "grounding source",
    )?;
    Ok(MemoryProvenance {
        memory,
        source_episode,
        source_turn,
        source_episode_turns,
        content_source_turn,
        grounding_source_turn,
    })
}

fn resolve_source_turn(
    cva: &mut crate::Cva,
    conversation_id: Option<&str>,
    node_id: Option<&str>,
    label: &str,
) -> Result<Option<crate::ResolvedTurn>, ReliquaryRuntimeHostError> {
    let (conversation_id, node_id) = match (conversation_id, node_id) {
        (Some(conversation_id), Some(node_id)) => (conversation_id, node_id),
        (None, None) => return Ok(None),
        _ => {
            return Err(operation(format!(
                "Memory {label} provenance is incomplete"
            )));
        }
    };
    let turn = cva
        .conversation_turns(conversation_id, node_id)
        .map_err(operation)?
        .into_iter()
        .last()
        .filter(|turn| turn.node_id == node_id)
        .ok_or_else(|| operation(format!("Memory {label} turn is unavailable")))?;
    Ok(Some(turn))
}

fn rel_pending(
    cva: &mut crate::Cva,
    ids: &[MemoryId],
    profile: Option<crate::CompatibilityProfileId>,
) -> Result<(usize, usize), ReliquaryRuntimeHostError> {
    let mut vector_pending = 0;
    let mut dream_pending = 0;
    let now_ns = crate::dream_cooldown::unix_now_ns();
    for id in ids {
        let memory = cva.memory(*id).map_err(operation)?;
        if memory.archived {
            continue;
        }
        let has_vector = match profile {
            Some(profile) => {
                let body = cva.memory_body_id(*id).map_err(operation)?;
                cva.memory_vector_location(profile, body).is_some()
            }
            None => false,
        };
        if !has_vector {
            vector_pending += 1;
        } else if cva
            .dream_eligible_epoch(*id, now_ns)
            .map_err(operation)?
            .is_some()
        {
            dream_pending += 1;
        }
    }
    Ok((vector_pending, dream_pending))
}

#[cfg(test)]
#[path = "runtime_host_gateway_ambient_extended_tests.rs"]
mod extended_tests;
#[cfg(test)]
#[path = "runtime_host_gateway_project_tests.rs"]
mod project_tests;
#[cfg(test)]
#[path = "runtime_host_gateway_ambient_tests.rs"]
mod tests;
