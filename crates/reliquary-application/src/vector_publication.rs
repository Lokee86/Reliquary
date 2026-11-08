//! Inference over immutable source cuts; publication is a separate owner call.
use crate::vector_workflow::{MAX_EMBEDDING_RESPONSE_BYTES, VectorWorkflowError};
use reliquary_owner_session::model::{
    CompatibilityProfile, CompatibilityProfileId, EmbeddingMode, PayloadState,
};
use reliquary_owner_session::{
    MemoryVectorInput, MemoryVectorRow, OwnerSessionError, PhyWriter, RelWriter,
};
use reliquary_vectors::{
    EmbeddingEndpoint, EmbeddingEndpointError, VectorNormalization, embed_batch_bounded,
    validate_embedding_batch,
};

struct Bounded<'a, E>(&'a E);
impl<E: EmbeddingEndpoint> EmbeddingEndpoint for Bounded<'_, E> {
    fn dimensions(&self) -> u32 {
        self.0.dimensions()
    }
    fn normalization(&self) -> VectorNormalization {
        self.0.normalization()
    }
    fn supports_bounded_embedding(&self) -> bool {
        self.0.supports_bounded_embedding()
    }
    fn embed(
        &self,
        mode: EmbeddingMode,
        inputs: &[String],
    ) -> Result<Vec<Vec<f32>>, EmbeddingEndpointError> {
        embed_batch_bounded(self.0, mode, inputs, MAX_EMBEDDING_RESPONSE_BYTES)
            .map_err(|e| EmbeddingEndpointError::Failure(e.to_string()))
    }
    fn embed_bounded(
        &self,
        mode: EmbeddingMode,
        inputs: &[String],
        bound: usize,
    ) -> Result<Vec<Vec<f32>>, EmbeddingEndpointError> {
        self.0.embed_bounded(mode, inputs, bound)
    }
}

pub fn prepare_compatibility_profile(
    endpoint: &impl EmbeddingEndpoint,
) -> Result<CompatibilityProfile, VectorWorkflowError> {
    if !endpoint.supports_bounded_embedding() {
        return Err(VectorWorkflowError::IncompatibleEndpoint);
    }
    reliquary_vectors::profile_from_endpoint(&Bounded(endpoint))
        .map_err(|e| VectorWorkflowError::Probe(e.to_string()))
}

/// Verify a live endpoint against an already accepted profile without
/// deriving or publishing a replacement identity.
pub fn verify_compatibility_profile(
    profile: &CompatibilityProfile,
    endpoint: &impl EmbeddingEndpoint,
) -> Result<(), VectorWorkflowError> {
    if !endpoint.supports_bounded_embedding() {
        return Err(VectorWorkflowError::IncompatibleEndpoint);
    }
    let report = reliquary_vectors::verify_endpoint(profile, &Bounded(endpoint))
        .map_err(|error| VectorWorkflowError::Probe(error.to_string()))?;
    if report.compatible {
        Ok(())
    } else {
        Err(VectorWorkflowError::IncompatibleEndpoint)
    }
}

/// Opaque, source-bound result. Neither the source binding nor vectors can be
/// replaced by a model-facing caller after inference.
pub struct PreparedMemoryVectors {
    owner: [u8; 16],
    profile: CompatibilityProfileId,
    rows: Vec<MemoryVectorRow>,
}
impl PreparedMemoryVectors {
    pub fn owner_id(&self) -> [u8; 16] {
        self.owner
    }
    pub fn profile_id(&self) -> CompatibilityProfileId {
        self.profile
    }
}

pub fn evaluate_memory_vectors(
    input: MemoryVectorInput,
    endpoint: &impl EmbeddingEndpoint,
) -> Result<PreparedMemoryVectors, VectorWorkflowError> {
    let mut rows = input.existing().to_vec();
    let mut by_body = std::collections::BTreeMap::<_, Vec<usize>>::new();
    for (index, row) in rows.iter().enumerate() {
        if row.is_none() {
            by_body
                .entry(input.sources()[index].record.body_id)
                .or_default()
                .push(index);
        }
    }
    let groups = by_body.into_values().collect::<Vec<_>>();
    let missing = groups.iter().map(|indices| indices[0]).collect::<Vec<_>>();
    if !missing.is_empty() {
        if !endpoint.supports_bounded_embedding() {
            return Err(VectorWorkflowError::IncompatibleEndpoint);
        }
        let report = reliquary_vectors::verify_endpoint(input.profile(), &Bounded(endpoint))
            .map_err(|e| VectorWorkflowError::Probe(e.to_string()))?;
        if !report.compatible {
            return Err(VectorWorkflowError::IncompatibleEndpoint);
        }
        let texts = missing
            .iter()
            .map(|i| match &input.sources()[*i].payload {
                PayloadState::Available(body) => Ok(format!("{}\n{}", body.title, body.content)),
                PayloadState::Absent => Err(VectorWorkflowError::BodyUnavailable(
                    input.sources()[*i].record.id,
                )),
            })
            .collect::<Result<Vec<_>, _>>()?;
        let vectors = embed_batch_bounded(
            endpoint,
            EmbeddingMode::Document,
            &texts,
            MAX_EMBEDDING_RESPONSE_BYTES,
        )
        .map_err(|e| VectorWorkflowError::Probe(e.to_string()))?;
        validate_embedding_batch(input.profile(), &vectors, missing.len())
            .map_err(|e| VectorWorkflowError::Probe(e.to_string()))?;
        for (indices, row) in groups.into_iter().zip(vectors) {
            for index in indices {
                rows[index] = Some(row.clone());
            }
        }
    }
    let rows = rows
        .into_iter()
        .map(|row| row.ok_or(VectorWorkflowError::StaleSnapshot))
        .collect::<Result<Vec<_>, _>>()?;
    validate_embedding_batch(input.profile(), &rows, input.sources().len())
        .map_err(|e| VectorWorkflowError::Probe(e.to_string()))?;
    Ok(PreparedMemoryVectors {
        owner: input.owner_id(),
        profile: input.profile().id,
        rows: input
            .sources()
            .iter()
            .zip(rows)
            .map(|(source, vector)| MemoryVectorRow {
                memory_id: source.record.id,
                revision: source.record.revision,
                body_id: source.record.body_id,
                vector,
            })
            .collect(),
    })
}

pub fn commit_rel_memory_vectors(
    writer: &mut RelWriter,
    prepared: &PreparedMemoryVectors,
) -> Result<bool, OwnerSessionError> {
    if writer.owner_id() != prepared.owner {
        return Err(OwnerSessionError::Record(
            "vector publication owner mismatch".into(),
        ));
    }
    writer.publish_memory_vectors(prepared.profile, &prepared.rows)
}

pub fn commit_native_import_memory_vectors(
    writer: &mut RelWriter,
    import_id: &str,
    prepared: &PreparedMemoryVectors,
) -> Result<bool, OwnerSessionError> {
    if writer.owner_id() != prepared.owner {
        return Err(OwnerSessionError::Record(
            "vector publication owner mismatch".into(),
        ));
    }
    writer.publish_native_import_memory_vectors(import_id, prepared.profile, &prepared.rows)
}

/// Evaluate all missing Archive rows in bounded provider calls before publishing
/// a single complete generation.
pub fn evaluate_archive_vectors(
    input: &reliquary_owner_session::ArchiveVectorInput,
    endpoint: &impl EmbeddingEndpoint,
) -> Result<Vec<reliquary_owner_session::ArchiveVectorRow>, VectorWorkflowError> {
    use reliquary_owner_session::{
        ArchiveVectorRow, MAX_ARCHIVE_VECTOR_BATCH_ROWS, MAX_ARCHIVE_VECTOR_RESPONSE_BYTES,
    };
    if input.sources().is_empty() {
        return Ok(Vec::new());
    }
    if !endpoint.supports_bounded_embedding()
        || !reliquary_vectors::verify_endpoint(input.profile(), &Bounded(endpoint))
            .map_err(|error| VectorWorkflowError::Probe(error.to_string()))?
            .compatible
    {
        return Err(VectorWorkflowError::IncompatibleEndpoint);
    }
    let mut rows = Vec::with_capacity(input.sources().len());
    for batch in input.sources().chunks(MAX_ARCHIVE_VECTOR_BATCH_ROWS) {
        let texts = batch
            .iter()
            .map(|source| source.text.clone())
            .collect::<Vec<_>>();
        let vectors = embed_batch_bounded(
            endpoint,
            EmbeddingMode::Document,
            &texts,
            MAX_ARCHIVE_VECTOR_RESPONSE_BYTES,
        )
        .map_err(|error| VectorWorkflowError::Probe(error.to_string()))?;
        validate_embedding_batch(input.profile(), &vectors, batch.len())
            .map_err(|error| VectorWorkflowError::Probe(error.to_string()))?;
        rows.extend(
            batch
                .iter()
                .zip(vectors)
                .map(|(source, vector)| ArchiveVectorRow {
                    fragment_id: source.fragment_id,
                    max_archive_version: source.max_archive_version,
                    vector,
                }),
        );
    }
    Ok(rows)
}
pub fn commit_phy_memory_vectors(
    writer: &mut PhyWriter,
    prepared: &PreparedMemoryVectors,
) -> Result<bool, OwnerSessionError> {
    if writer.owner_id() != prepared.owner {
        return Err(OwnerSessionError::Record(
            "vector publication owner mismatch".into(),
        ));
    }
    writer.publish_memory_vectors(prepared.profile, &prepared.rows)
}
