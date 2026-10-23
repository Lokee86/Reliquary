//! Immutable semantic fragment evidence assembled from accepted producer facts.
use super::OwnerError;
use crate::freshness::semantic_policy::{SemanticPolicy, cosine_similarity};
use crate::freshness_storage::SemanticFragmentEvidence;
use crate::{FragmentId, Node, VectorGeneration, VectorGenerationId};
use sha2::{Digest, Sha256};
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct SemanticSourceVector {
    pub fragment_id: FragmentId,
    pub row: u64,
    pub components: Vec<f32>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct AcceptedSemanticSource {
    pub owner_uuid: [u8; 16],
    pub fragment: Option<crate::freshness_storage::SemanticFragmentEvidence>,
    pub node: Node,
    pub activity_position: u64,
    pub generation: VectorGeneration,
    pub base_policy: crate::freshness::FreshnessPolicy,
    pub semantic_policy: SemanticPolicy,
    pub policy_digest: [u8; 32],
    pub source_digest: [u8; 32],
    pub vectors: Vec<SemanticSourceVector>,
    pub invalid_vectors: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum SemanticSourceReadiness {
    Pending,
    Ready(AcceptedSemanticSource),
}

#[derive(Clone, Copy)]
pub(crate) struct SemanticSourceReadParts<'a> {
    pub(crate) container: &'a crate::Container,
    pub(crate) archive: &'a crate::Archive,
    pub(crate) freshness: &'a crate::freshness_storage::FreshnessStore,
    pub(crate) generations: &'a crate::vector_generation_store::VectorGenerationStore,
    pub(crate) compatibility_profiles:
        &'a crate::compatibility_profile_store::CompatibilityProfileStore,
    pub(crate) archive_vectors: &'a crate::archive_vector_store::ArchiveVectorStore,
    pub(crate) packed_vectors: &'a crate::packed_vector_store::PackedVectorStore,
}
pub(crate) fn fragment_sources(
    archive: &crate::Archive,
    id: FragmentId,
) -> Result<Vec<(String, String)>, OwnerError> {
    let fragment = archive
        .fragments
        .get(id)
        .ok_or_else(|| OwnerError::Freshness("missing semantic fragment".into()))?;
    let mut sources: Vec<_> = archive
        .fragment_nodes_for(fragment)?
        .into_iter()
        .map(|n| (n.conversation_id, n.id))
        .collect();
    sources.sort();
    Ok(sources)
}
pub(crate) fn prepare_semantic_fragment(
    parts: SemanticSourceReadParts<'_>,
    id: FragmentId,
    generation_id: VectorGenerationId,
) -> Result<SemanticSourceReadiness, OwnerError> {
    let owner_uuid = parts
        .container
        .owner_uuid()
        .ok_or(OwnerError::InvalidContainerIdentity(
            "semantic fragment owner",
        ))?;
    let fragment = parts
        .archive
        .fragments
        .get(id)
        .ok_or_else(|| OwnerError::Freshness("missing semantic fragment".into()))?
        .clone();
    let nodes = parts.archive.fragment_nodes_for(&fragment)?;
    let Some((activity_position, node)) = nodes
        .iter()
        .filter_map(|n| {
            parts
                .archive
                .activity_position_for_turn(&n.conversation_id, &n.id)
                .map(|at| (at, n))
        })
        .max_by_key(|(at, _)| *at)
    else {
        return Ok(SemanticSourceReadiness::Pending);
    };
    let node = node.clone();
    let generation = parts
        .generations
        .generation(generation_id)
        .ok_or(crate::VectorGenerationError::MissingGeneration)?;
    if parts
        .archive
        .fragments
        .archive_version(id)
        .is_none_or(|v| v > generation.source_archive_version)
        || parts
            .archive
            .activity_cut_at_archive_version(generation.source_archive_version)?
            < activity_position
    {
        return Ok(SemanticSourceReadiness::Pending);
    }
    let profile = parts
        .compatibility_profiles
        .get(generation.compatibility_profile_id)
        .ok_or_else(|| OwnerError::Freshness("missing semantic fragment profile".into()))?
        .clone();
    let set = parts
        .archive_vectors
        .get(parts.container, generation.archive_vector_id)?;
    let Some(row) = set.fragment_ids.iter().position(|f| *f == id) else {
        return Ok(SemanticSourceReadiness::Pending);
    };
    let packed = parts
        .packed_vectors
        .get(parts.container, set.packed_vector_id)?;
    if packed.schema().dimensions != profile.dimensions
        || packed.schema().scalar != crate::ScalarType::F32
    {
        return Err(OwnerError::Freshness(
            "incompatible semantic fragment matrix".into(),
        ));
    }
    let bytes = packed
        .row(row)
        .ok_or_else(|| OwnerError::Freshness("missing fragment row".into()))?;
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
            sources: fragment_sources(parts.archive, id)?,
        }),
        node,
        activity_position,
        generation,
        base_policy: parts.freshness.policy(),
        semantic_policy: policy,
        policy_digest: policy
            .digest()
            .map_err(|e| OwnerError::Freshness(e.to_string()))?,
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

pub(crate) fn fragment_position(
    archive: &crate::Archive,
    id: FragmentId,
) -> Result<Option<u64>, OwnerError> {
    Ok(fragment_sources(archive, id)?
        .iter()
        .filter_map(|(c, n)| archive.activity_position_for_turn(c, n))
        .max())
}

/// Immutable vector-evidence reader. It exposes accepted values, never stores or writers.
pub(crate) struct HistoricalVectorReadAccess<'a> {
    semantic: SemanticSourceReadParts<'a>,
    targets: &'a crate::memory_vector_store::MemoryVectorStore,
}
impl<'a> HistoricalVectorReadAccess<'a> {
    pub(crate) fn new(
        semantic: SemanticSourceReadParts<'a>,
        targets: &'a crate::memory_vector_store::MemoryVectorStore,
    ) -> Self {
        Self { semantic, targets }
    }
    pub(crate) fn generation(&self) -> Option<crate::VectorGeneration> {
        self.semantic
            .generations
            .generations()
            .iter()
            .max_by_key(|g| g.vector_version)
            .copied()
    }
    pub(crate) fn profiles(&self) -> Vec<crate::CompatibilityProfile> {
        self.semantic.compatibility_profiles.profiles()
    }
    pub(crate) fn fragment(
        &self,
        id: crate::FragmentId,
        generation: crate::VectorGenerationId,
    ) -> Result<SemanticSourceReadiness, OwnerError> {
        prepare_semantic_fragment(self.semantic, id, generation)
    }
    pub(crate) fn target_location(
        &self,
        profile: crate::CompatibilityProfileId,
        body: crate::MemoryBodyId,
    ) -> Option<crate::MemoryVectorLocation> {
        self.targets.location(profile, body)
    }
    pub(crate) fn target_set(
        &self,
        id: crate::MemoryVectorId,
    ) -> Result<crate::MemoryVectorSet, crate::MemoryVectorError> {
        self.targets.get(self.semantic.container, id)
    }
    pub(crate) fn packed(
        &self,
        id: crate::PackedVectorId,
    ) -> Result<lodestone_packed::PackedVectors, crate::PackedVectorError> {
        self.semantic
            .packed_vectors
            .get(self.semantic.container, id)
    }
}
