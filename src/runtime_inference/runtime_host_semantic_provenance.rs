use super::{
    ReliquaryRuntimeHost, ReliquaryRuntimeHostError, semantic_access::RuntimeSemanticOwnerKind,
};
use crate::{MemoryId, MemoryProvenance};

impl ReliquaryRuntimeHost {
    pub fn read_memory_provenance_for_owner(
        &self,
        owner_id: &str,
        memory_id: MemoryId,
    ) -> Result<MemoryProvenance, ReliquaryRuntimeHostError> {
        match self.semantic_owner(owner_id)?.kind {
            RuntimeSemanticOwnerKind::Reliquary => {
                self.read_reliquary_memory_provenance_for(owner_id, memory_id)
            }
            RuntimeSemanticOwnerKind::Phylactery => Err(ReliquaryRuntimeHostError::Operation(
                "Phylactery Memories do not have Reliquary Archive provenance".into(),
            )),
        }
    }

    pub fn resolve_knowledge_provenance_for_owner(
        &self,
        owner_id: &str,
        memory_id: MemoryId,
    ) -> Result<RuntimeKnowledgeProvenanceResolution, ReliquaryRuntimeHostError> {
        match self.semantic_owner(owner_id)?.kind {
            RuntimeSemanticOwnerKind::Reliquary => {
                let provenance = self.read_reliquary_memory_provenance_for(owner_id, memory_id)?;
                let has_evidence = provenance.source_episode.is_some()
                    || provenance.source_turn.is_some()
                    || !provenance.source_episode_turns.is_empty()
                    || provenance.content_source_turn.is_some()
                    || provenance.grounding_source_turn.is_some();
                if has_evidence {
                    Ok(RuntimeKnowledgeProvenanceResolution::Resolved {
                        memory_owner_id: owner_id.to_owned(),
                        source_owner_id: owner_id.to_owned(),
                        provenance,
                    })
                } else {
                    Ok(RuntimeKnowledgeProvenanceResolution::None {
                        memory_owner_id: owner_id.to_owned(),
                        provenance: Some(provenance),
                    })
                }
            }
            RuntimeSemanticOwnerKind::Phylactery => {
                let memory = self.knowledge_memory_for_owner(owner_id, memory_id)?;
                let Some(source_ref) = memory.source_ref.clone() else {
                    return Ok(RuntimeKnowledgeProvenanceResolution::None {
                        memory_owner_id: owner_id.to_owned(),
                        provenance: None,
                    });
                };
                if self.semantic_owner(&source_ref.owner_id).is_err() {
                    return Ok(RuntimeKnowledgeProvenanceResolution::Unresolved {
                        memory_owner_id: owner_id.to_owned(),
                        source_owner_id: source_ref.owner_id,
                    });
                }
                let source_owner_id = source_ref.owner_id.clone();
                let provenance =
                    self.resolve_external_memory_provenance_for(&source_owner_id, memory)?;
                Ok(RuntimeKnowledgeProvenanceResolution::Resolved {
                    memory_owner_id: owner_id.to_owned(),
                    source_owner_id,
                    provenance,
                })
            }
        }
    }
}
