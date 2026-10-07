use super::*;
use crate::{MemoryId, MemoryProvenance};

#[derive(Clone, Debug)]
pub enum RuntimeKnowledgeProvenanceResolution {
    None {
        memory_owner_id: String,
        provenance: Option<MemoryProvenance>,
    },
    Unresolved {
        memory_owner_id: String,
        source_owner_id: String,
    },
    Resolved {
        memory_owner_id: String,
        source_owner_id: String,
        provenance: MemoryProvenance,
    },
}

impl GatewayRuntime {
    pub fn read_memory_provenance_for_owner(
        &self,
        instance: &InstanceRuntime,
        owner_id: &str,
        memory_id: MemoryId,
    ) -> Result<MemoryProvenance, ReliquaryRuntimeHostError> {
        let token = self.semantic_token(instance, owner_id)?;
        match token.kind {
            OwnerKind::Rel => {
                let provenance = self.read_memory_provenance_for(instance, owner_id, memory_id)?;
                self.with_rel(&token, |_| Ok(provenance))
            }
            OwnerKind::Phy => Err(operation(
                "Phylactery Memories do not have Reliquary Archive provenance",
            )),
        }
    }

    pub fn resolve_knowledge_provenance_for_owner(
        &self,
        instance: &InstanceRuntime,
        owner_id: &str,
        memory_id: MemoryId,
    ) -> Result<RuntimeKnowledgeProvenanceResolution, ReliquaryRuntimeHostError> {
        let token = self.semantic_token(instance, owner_id)?;
        match token.kind {
            OwnerKind::Rel => {
                let provenance = self.read_memory_provenance_for(instance, owner_id, memory_id)?;
                let has_evidence = provenance.source_episode.is_some()
                    || provenance.source_turn.is_some()
                    || !provenance.source_episode_turns.is_empty()
                    || provenance.content_source_turn.is_some()
                    || provenance.grounding_source_turn.is_some();
                let result = if has_evidence {
                    RuntimeKnowledgeProvenanceResolution::Resolved {
                        memory_owner_id: owner_id.to_owned(),
                        source_owner_id: owner_id.to_owned(),
                        provenance,
                    }
                } else {
                    RuntimeKnowledgeProvenanceResolution::None {
                        memory_owner_id: owner_id.to_owned(),
                        provenance: Some(provenance),
                    }
                };
                self.with_rel(&token, |_| Ok(result))
            }
            OwnerKind::Phy => {
                let memory =
                    self.with_phy(&token, |phy| phy.memory(memory_id).map_err(operation))?;
                let result = match memory.source_ref.clone() {
                    None => RuntimeKnowledgeProvenanceResolution::None {
                        memory_owner_id: owner_id.to_owned(),
                        provenance: None,
                    },
                    Some(source_ref) => {
                        // A source reference does not confer authorization.
                        if self
                            .authorize(instance, &source_ref.owner_id, OwnerGrant::ViewRel)
                            .is_err()
                        {
                            RuntimeKnowledgeProvenanceResolution::Unresolved {
                                memory_owner_id: owner_id.to_owned(),
                                source_owner_id: source_ref.owner_id,
                            }
                        } else {
                            let source_owner_id = source_ref.owner_id.clone();
                            let provenance = self.resolve_external_memory_provenance_for(
                                instance,
                                &source_owner_id,
                                memory,
                            )?;
                            RuntimeKnowledgeProvenanceResolution::Resolved {
                                memory_owner_id: owner_id.to_owned(),
                                source_owner_id,
                                provenance,
                            }
                        }
                    }
                };
                // Prevent a private result from outliving grant or mount epoch.
                self.with_phy(&token, |_| Ok(result))
            }
        }
    }
}
