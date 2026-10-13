//! Live fragment availability and immutable Archive source proof.
use crate::cva_semantic_source::{
    AcceptedSemanticSource, SemanticSourceReadiness, SemanticSourceVector,
};
use crate::freshness::semantic_policy::{SemanticPolicy, cosine_similarity};
use crate::freshness_storage::{SemanticEventEvidence, SemanticFragmentEvidence};
use crate::{Cva, CvaError, FragmentId, VectorGenerationId};
use sha2::{Digest, Sha256};

impl AcceptedSemanticSource {
    pub(crate) fn represents(&self, conversation: &str, node: &str) -> bool {
        match &self.fragment {
            Some(fragment) => fragment
                .sources
                .iter()
                .any(|(c, n)| c == conversation && n == node),
            None => self.node.conversation_id == conversation && self.node.id == node,
        }
    }
}

impl Cva {
    pub(crate) fn semantic_fragment_sources(
        &self,
        id: FragmentId,
    ) -> Result<Vec<(String, String)>, CvaError> {
        let fragment = self
            .archive
            .fragments
            .get(id)
            .ok_or_else(|| CvaError::Freshness("missing semantic fragment".into()))?;
        let mut sources: Vec<_> = self
            .archive
            .fragment_nodes_for(fragment)?
            .into_iter()
            .map(|n| (n.conversation_id, n.id))
            .collect();
        sources.sort();
        Ok(sources)
    }

    pub(crate) fn semantic_fragment_position(
        &self,
        id: FragmentId,
    ) -> Result<Option<u64>, CvaError> {
        Ok(self
            .semantic_fragment_sources(id)?
            .iter()
            .filter_map(|(c, n)| self.activity_position_for_turn(c, n))
            .max())
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
        let owner_uuid = self.owner_uuid().ok_or(CvaError::InvalidContainerIdentity(
            "semantic fragment owner",
        ))?;
        let fragment = self
            .archive
            .fragments
            .get(id)
            .ok_or_else(|| CvaError::Freshness("missing semantic fragment".into()))?
            .clone();
        let nodes = self.archive.fragment_nodes_for(&fragment)?;
        let Some((activity_position, node)) = nodes
            .iter()
            .filter_map(|n| {
                self.activity_position_for_turn(&n.conversation_id, &n.id)
                    .map(|at| (at, n))
            })
            .max_by_key(|(at, _)| *at)
        else {
            return Ok(SemanticSourceReadiness::Pending);
        };
        let node = node.clone();
        let generation = self.vector_generation(generation_id)?;
        if self
            .archive
            .fragments
            .archive_version(id)
            .is_none_or(|v| v > generation.source_archive_version)
            || self
                .archive
                .activity_cut_at_archive_version(generation.source_archive_version)?
                < activity_position
        {
            return Ok(SemanticSourceReadiness::Pending);
        }
        let profile = self
            .compatibility_profiles
            .get(generation.compatibility_profile_id)
            .ok_or_else(|| CvaError::Freshness("missing semantic fragment profile".into()))?
            .clone();
        let set = self.archive_vectors(generation.archive_vector_id)?;
        let Some(row) = set.fragment_ids.iter().position(|f| *f == id) else {
            return Ok(SemanticSourceReadiness::Pending);
        };
        let packed = self.packed_vectors(set.packed_vector_id)?;
        if packed.schema().dimensions != profile.dimensions
            || packed.schema().scalar != crate::ScalarType::F32
        {
            return Err(CvaError::Freshness(
                "incompatible semantic fragment matrix".into(),
            ));
        }
        let bytes = packed
            .row(row)
            .ok_or_else(|| CvaError::Freshness("missing fragment row".into()))?;
        let components: Vec<f32> = bytes
            .chunks_exact(4)
            .map(|b| f32::from_le_bytes(b.try_into().expect("four bytes")))
            .collect();
        let invalid = cosine_similarity(&components, &components).is_err();
        let mut digest = Sha256::new();
        digest.update(b"reliquary:semantic-fragment-evidence:v1\0");
        digest.update(owner_uuid);
        digest.update(id.0);
        // Pin every represented Node, including non-activity context; positions
        // are resolved separately so reconciliation may rebase accepted order.
        for n in &nodes {
            for text in [&n.conversation_id, &n.id] {
                digest.update((text.len() as u64).to_le_bytes());
                digest.update(text.as_bytes());
            }
            digest.update(n.content_id.0);
        }
        digest.update(generation.id.0);
        digest.update(generation.compatibility_profile_id.0);
        digest.update((row as u64).to_le_bytes());
        digest.update(bytes);
        digest.update([u8::from(invalid)]);
        let policy = SemanticPolicy::default();
        Ok(SemanticSourceReadiness::Ready(AcceptedSemanticSource {
            owner_uuid,
            fragment: Some(SemanticFragmentEvidence {
                fragment_id: id,
                sources: self.semantic_fragment_sources(id)?,
            }),
            node,
            activity_position,
            generation,
            base_policy: self.freshness.policy(),
            semantic_policy: policy,
            policy_digest: policy
                .digest()
                .map_err(|e| CvaError::Freshness(e.to_string()))?,
            source_digest: digest.finalize().into(),
            vectors: if invalid {
                Vec::new()
            } else {
                vec![SemanticSourceVector {
                    fragment_id: id,
                    row: row as u64,
                    components,
                }]
            },
            invalid_vectors: usize::from(invalid),
        }))
    }
}
