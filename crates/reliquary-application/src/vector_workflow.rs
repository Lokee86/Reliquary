//! Source-bound vector preparation over immutable owner reads.

use crate::EntityOwnerSession;
use reliquary_owner_session::model::{CompatibilityProfileId, EmbeddingMode};
use reliquary_owner_session::model::{MemoryBodyId, MemoryId, PayloadState};
use reliquary_vectors::{EmbeddingEndpoint, validate_embedding_batch, verify_endpoint};
use std::{collections::HashSet, fmt};

pub const MAX_MEMORY_EMBEDDING_BATCH: usize = 64;
pub const MAX_EMBEDDING_SOURCE_BYTES: usize = 128 << 10;
pub const MAX_EMBEDDING_RESPONSE_BYTES: usize = 8 << 20;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemoryEmbeddingSource {
    pub memory_id: MemoryId,
    pub revision: u64,
    pub body_id: MemoryBodyId,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MemoryEmbeddingBatch {
    pub owner_id: [u8; 16],
    pub profile_id: CompatibilityProfileId,
    pub sources: Vec<MemoryEmbeddingSource>,
    /// Rows correspond exactly to `sources` order.
    pub vectors: Vec<Vec<f32>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VectorWorkflowError {
    OwnerMismatch,
    EmptyBatch,
    BatchLimit,
    DuplicateMemory,
    MemoryUnavailable(MemoryId),
    MemoryTombstoned(MemoryId),
    MemoryArchived(MemoryId),
    BodyUnavailable(MemoryId),
    SourceTooLarge(MemoryId),
    MissingOrRetiredProfile,
    IncompatibleEndpoint,
    Probe(String),
    OwnerRead(String),
    StaleSnapshot,
}

impl fmt::Display for VectorWorkflowError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OwnerMismatch => f.write_str("vector source does not match authorized owner"),
            Self::EmptyBatch => f.write_str("vector source batch is empty"),
            Self::BatchLimit => f.write_str("vector source batch exceeds limit"),
            Self::DuplicateMemory => f.write_str("vector source repeats a Memory"),
            Self::MemoryUnavailable(id) => write!(f, "Memory {id:?} is unavailable"),
            Self::MemoryTombstoned(id) => write!(f, "Memory {id:?} is tombstoned"),
            Self::MemoryArchived(id) => write!(f, "Memory {id:?} is archived"),
            Self::BodyUnavailable(id) => write!(f, "Memory {id:?} body is unavailable"),
            Self::SourceTooLarge(id) => write!(f, "Memory {id:?} exceeds embedding source limit"),
            Self::MissingOrRetiredProfile => f.write_str("vector profile is missing or retired"),
            Self::IncompatibleEndpoint => {
                f.write_str("embedding endpoint is incompatible with profile")
            }
            Self::Probe(error) => write!(f, "embedding validation failed: {error}"),
            Self::OwnerRead(error) => write!(f, "owner Memory read failed: {error}"),
            Self::StaleSnapshot => f.write_str("vector batch no longer matches owner state"),
        }
    }
}

impl std::error::Error for VectorWorkflowError {}

/// Read source bodies and profile, then call the embedding provider after all
/// owner reads have completed. This prepares in-memory rows only; no vector
/// store publication is claimed. Authorization is the caller's responsibility.
pub fn prepare_memory_embeddings<E: EmbeddingEndpoint>(
    owner: &EntityOwnerSession<'_>,
    authorized_owner_id: [u8; 16],
    profile_id: CompatibilityProfileId,
    memory_ids: &[MemoryId],
    endpoint: &E,
) -> Result<MemoryEmbeddingBatch, VectorWorkflowError> {
    let owner_id = owner.owner_id();
    if owner_id != authorized_owner_id {
        return Err(VectorWorkflowError::OwnerMismatch);
    }
    if memory_ids.is_empty() {
        return Err(VectorWorkflowError::EmptyBatch);
    }
    if memory_ids.len() > MAX_MEMORY_EMBEDDING_BATCH {
        return Err(VectorWorkflowError::BatchLimit);
    }
    let mut seen = HashSet::with_capacity(memory_ids.len());
    let mut sources = Vec::with_capacity(memory_ids.len());
    for &memory_id in memory_ids {
        if !seen.insert(memory_id) {
            return Err(VectorWorkflowError::DuplicateMemory);
        }
        let history = owner
            .memory_history(memory_id)
            .map_err(|error| VectorWorkflowError::OwnerRead(error.to_string()))?
            .ok_or(VectorWorkflowError::MemoryUnavailable(memory_id))?;
        if history.tombstone.is_some() {
            return Err(VectorWorkflowError::MemoryTombstoned(memory_id));
        }
        let revision = history
            .revisions
            .last()
            .ok_or(VectorWorkflowError::MemoryUnavailable(memory_id))?;
        if revision.record.archived {
            return Err(VectorWorkflowError::MemoryArchived(memory_id));
        }
        let body = match &revision.payload {
            PayloadState::Available(body) => body,
            PayloadState::Absent => return Err(VectorWorkflowError::BodyUnavailable(memory_id)),
        };
        let text = format!("{}\n{}", body.title, body.content);
        if text.len() > MAX_EMBEDDING_SOURCE_BYTES {
            return Err(VectorWorkflowError::SourceTooLarge(memory_id));
        }
        sources.push(MemoryEmbeddingSource {
            memory_id,
            revision: revision.record.revision,
            body_id: revision.record.body_id,
            text,
        });
    }

    let profile = owner
        .active_compatibility_profile(profile_id)
        .ok_or(VectorWorkflowError::MissingOrRetiredProfile)?;
    let report = verify_endpoint(&profile, endpoint)
        .map_err(|error| VectorWorkflowError::Probe(error.to_string()))?;
    if !report.compatible {
        return Err(VectorWorkflowError::IncompatibleEndpoint);
    }

    let texts = sources
        .iter()
        .map(|source| source.text.clone())
        .collect::<Vec<_>>();
    let vectors = reliquary_vectors::embed_batch_bounded(
        endpoint,
        EmbeddingMode::Document,
        &texts,
        MAX_EMBEDDING_RESPONSE_BYTES,
    )
    .map_err(|error| VectorWorkflowError::Probe(error.to_string()))?;
    validate_embedding_batch(&profile, &vectors, sources.len())
        .map_err(|error| VectorWorkflowError::Probe(error.to_string()))?;
    Ok(MemoryEmbeddingBatch {
        owner_id,
        profile_id,
        sources,
        vectors,
    })
}

/// Check an embedding proposal against a freshly opened owner snapshot before
/// publication. It does not write vectors or profile records.
pub fn recheck_memory_embedding_batch(
    owner: &EntityOwnerSession<'_>,
    authorized_owner_id: [u8; 16],
    batch: &MemoryEmbeddingBatch,
) -> Result<(), VectorWorkflowError> {
    if owner.owner_id() != authorized_owner_id || batch.owner_id != authorized_owner_id {
        return Err(VectorWorkflowError::OwnerMismatch);
    }
    if owner
        .active_compatibility_profile(batch.profile_id)
        .is_none()
    {
        return Err(VectorWorkflowError::MissingOrRetiredProfile);
    }
    for source in &batch.sources {
        let history = owner
            .memory_history(source.memory_id)
            .map_err(|error| VectorWorkflowError::OwnerRead(error.to_string()))?
            .ok_or(VectorWorkflowError::StaleSnapshot)?;
        let Some(current) = history.revisions.last() else {
            return Err(VectorWorkflowError::StaleSnapshot);
        };
        if history.tombstone.is_some()
            || current.record.archived
            || current.record.revision != source.revision
            || current.record.body_id != source.body_id
        {
            return Err(VectorWorkflowError::StaleSnapshot);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::EntityOwnerSession;
    use reliquary_container::{Container, ContainerIdentity};
    use reliquary_memory_core::{MemoryDraft, MemoryStore};
    use reliquary_owner_session::{GraphAdmission, PhySession};
    use reliquary_vector_store::{
        CompatibilityProfileStore, EmbeddingMode, MemoryVectorStore, PackedVectorStore,
        VectorNormalization,
    };
    use reliquary_vectors::{EmbeddingEndpointError, profile_from_endpoint};
    use std::{
        fs,
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
        sync::{Arc, Mutex},
    };

    static NEXT: AtomicU64 = AtomicU64::new(1);
    struct Temp(PathBuf);
    impl Temp {
        fn new() -> Self {
            let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".test-tmp");
            fs::create_dir_all(&root).unwrap();
            Self(root.join(format!(
                "vector-workflow-{}-{}.phy",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            )))
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = fs::remove_file(&self.0);
        }
    }

    #[derive(Clone, Default)]
    struct RecordingEmbedding(Arc<Mutex<Vec<(EmbeddingMode, Vec<String>)>>>);
    impl EmbeddingEndpoint for RecordingEmbedding {
        fn supports_bounded_embedding(&self) -> bool {
            true
        }
        fn embed_bounded(
            &self,
            mode: EmbeddingMode,
            inputs: &[String],
            max_response_bytes: usize,
        ) -> Result<Vec<Vec<f32>>, EmbeddingEndpointError> {
            let vectors = vec![vec![1.0, 0.0]; inputs.len()];
            if vectors.len() * 2 * std::mem::size_of::<f32>() > max_response_bytes {
                return Err(EmbeddingEndpointError::InvalidResponse("limit"));
            }
            self.0.lock().unwrap().push((mode, inputs.to_vec()));
            Ok(vectors)
        }
        fn dimensions(&self) -> u32 {
            2
        }
        fn normalization(&self) -> VectorNormalization {
            VectorNormalization::L2
        }
        fn embed(
            &self,
            mode: EmbeddingMode,
            inputs: &[String],
        ) -> Result<Vec<Vec<f32>>, EmbeddingEndpointError> {
            self.0.lock().unwrap().push((mode, inputs.to_vec()));
            Ok(vec![vec![1.0, 0.0]; inputs.len()])
        }
    }

    fn memory_draft() -> MemoryDraft {
        MemoryDraft {
            title: "Vector source title".into(),
            content: "Exact source body for the selected owner.".into(),
            category: "fact".into(),
            memory_type: "project".into(),
            authority_kind: "direct".into(),
            temporal_status: "current".into(),
            scope: "project".into(),
            lifecycle_state: "knowledge".into(),
            archived: false,
            parent_id: None,
            superseded_by: None,
            source_node_id: None,
            content_source_conversation_id: None,
            content_source_node_id: None,
            grounding_source_conversation_id: None,
            grounding_source_node_id: None,
            source_episode_id: None,
            source_ref: None,
            source_time_ns: Some(4),
            temporal_inference: None,
            mutation_id: "vector-workflow-memory".into(),
            created_at_ns: 1,
            updated_at_ns: 1,
        }
    }

    fn owner(
        profile: reliquary_vector_store::CompatibilityProfile,
    ) -> (Temp, PhySession, MemoryId) {
        let path = Temp::new();
        let owner_id = [107; 16];
        let mut container =
            Container::create(&path.0, ContainerIdentity::phylactery(), owner_id).unwrap();
        PackedVectorStore::default()
            .initialize(&mut container)
            .unwrap();
        let mut profiles = CompatibilityProfileStore::default();
        profiles.initialize(&mut container).unwrap();
        let profile_id = profiles.put(&mut container, profile).unwrap();
        MemoryVectorStore::default()
            .initialize(&mut container)
            .unwrap();
        let mut memories = MemoryStore::create(&mut container).unwrap();
        let memory_id = memories
            .publish(&mut container, None, 0, memory_draft())
            .unwrap()
            .revision
            .record
            .id;
        container.sync().unwrap();
        drop(memories);
        drop(container);
        let admission = GraphAdmission::for_replay(owner_id, [], [], [], [], []);
        let session = PhySession::open_path(&path.0, &admission, []).unwrap();
        assert!(session.active_compatibility_profile(profile_id).is_some());
        (path, session, memory_id)
    }

    #[test]
    fn embeds_current_owner_body_against_active_profile_without_writing_vectors() {
        let endpoint = RecordingEmbedding::default();
        let profile = profile_from_endpoint(&endpoint).unwrap();
        let (temp, owner, memory) = owner(profile.clone());
        let proposal = prepare_memory_embeddings(
            &EntityOwnerSession::Phy(&owner),
            owner.owner_id(),
            profile.id,
            &[memory],
            &endpoint,
        )
        .unwrap();
        assert_eq!(proposal.owner_id, owner.owner_id());
        assert_eq!(proposal.sources.len(), 1);
        assert_eq!(proposal.sources[0].memory_id, memory);
        assert_eq!(proposal.sources[0].revision, 1);
        assert_eq!(proposal.vectors, vec![vec![1.0, 0.0]]);
        assert!(proposal.sources[0].text.contains("Exact source body"));
        assert!(
            recheck_memory_embedding_batch(
                &EntityOwnerSession::Phy(&owner),
                owner.owner_id(),
                &proposal,
            )
            .is_ok()
        );
        assert!(owner.memory_vector_infos().is_empty());
        assert!(endpoint.0.lock().unwrap().iter().any(|(mode, inputs)| {
            *mode == EmbeddingMode::Document
                && inputs
                    == &[
                        "Vector source title\nExact source body for the selected owner.".to_owned(),
                    ]
        }));
        drop(owner);
        drop(temp);
    }
}
