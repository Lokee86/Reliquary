//! Live fragment availability and immutable Archive source proof.
use crate::cva_semantic_source::{
    AcceptedSemanticSource, SemanticSourceReadiness, SemanticSourceVector,
};
use crate::freshness::semantic_policy::{SemanticPolicy, cosine_similarity};
use crate::freshness_storage::{SemanticEventEvidence, SemanticFragmentEvidence};
use crate::{Cva, CvaError, FragmentId, VectorGenerationId};
use sha2::{Digest, Sha256};

impl Cva {
    pub(crate) fn semantic_fragment_sources(
        &self,
        id: FragmentId,
    ) -> Result<Vec<(String, String)>, CvaError> {
        crate::owner_lifecycle::semantic_source::fragment_sources(&self.archive, id)
    }
    pub(crate) fn semantic_fragment_position(
        &self,
        id: FragmentId,
    ) -> Result<Option<u64>, CvaError> {
        crate::owner_lifecycle::semantic_source::fragment_position(&self.archive, id)
    }

    pub(crate) fn semantic_evidence_position(
        &self,
        evidence: &SemanticEventEvidence,
    ) -> Result<u64, CvaError> {
        if let Some(fragment) = &evidence.fragment {
            if self.semantic_fragment_sources(fragment.fragment_id)? != fragment.sources {
                return Err(CvaError::Freshness(
                    "conflicting semantic fragment sources".into(),
                ));
            }
            self.semantic_fragment_position(fragment.fragment_id)?
                .ok_or_else(|| CvaError::Freshness("fragment has no accepted activity".into()))
        } else {
            self.activity_position_for_turn(&evidence.conversation_id, &evidence.node_id)
                .ok_or_else(|| CvaError::Freshness("missing semantic source".into()))
        }
    }

    pub(crate) fn semantic_source_excludes_memory(
        &self,
        source: &AcceptedSemanticSource,
        memory: &crate::Memory,
    ) -> Result<bool, CvaError> {
        // Structured references carry the Episode conversation, avoiding bare-ID
        // collisions across conversations. Episode membership excludes its entire
        // publication cohort, including asynchronous outputs from earlier turns.
        let episode_id = memory
            .source_episode_id
            .or_else(|| memory.source_ref.as_ref().map(|r| r.source_episode_id));
        if let Some(id) = episode_id {
            let episode = self
                .episode(id)
                .ok_or_else(|| CvaError::Freshness("missing semantic target Episode".into()))?;
            let sources = source
                .fragment
                .as_ref()
                .map(|f| f.sources.clone())
                .unwrap_or_else(|| {
                    vec![(source.node.conversation_id.clone(), source.node.id.clone())]
                });
            for (conversation, node) in &sources {
                if conversation == &episode.conversation_id
                    && self.archive.episode_contains_node(id, node)?
                {
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }

    pub(crate) fn prepare_semantic_fragment(
        &mut self,
        id: FragmentId,
        generation_id: VectorGenerationId,
    ) -> Result<SemanticSourceReadiness, CvaError> {
        crate::owner_lifecycle::semantic_source::prepare_semantic_fragment(
            crate::owner_lifecycle::semantic_source::SemanticSourceReadParts {
                container: &self.container,
                archive: &self.archive,
                freshness: &self.freshness,
                generations: &self.vector_generations,
                compatibility_profiles: &self.compatibility_profiles,
                archive_vectors: &self.archive_vectors,
                packed_vectors: &self.packed_vectors,
            },
            id,
            generation_id,
        )
    }
}
