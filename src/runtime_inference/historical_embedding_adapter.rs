//! Runtime adapter for bounded historical embedding provider calls and owner reads.
use crate::imports::historical_embedding_model::{
    HistoricalEmbeddingLimits, HistoricalEmbeddingReport,
};
use crate::{
    CompatibilityProfile, CompatibilityProfileError, CompatibilityReport, EmbeddingEndpoint,
    EmbeddingEndpointError, EmbeddingMode, VectorGeneration, VectorNormalization,
};
use std::cell::RefCell;
use std::sync::atomic::{AtomicBool, Ordering};

struct HistoricalEmbeddingProvider<'a, E> {
    endpoint: &'a E,
    cancel: &'a AtomicBool,
    report: &'a RefCell<HistoricalEmbeddingReport>,
    limits: HistoricalEmbeddingLimits,
}

impl<E: EmbeddingEndpoint> EmbeddingEndpoint for HistoricalEmbeddingProvider<'_, E> {
    fn dimensions(&self) -> u32 {
        self.endpoint.dimensions()
    }

    fn normalization(&self) -> VectorNormalization {
        self.endpoint.normalization()
    }

    fn embed(
        &self,
        mode: EmbeddingMode,
        inputs: &[String],
    ) -> Result<Vec<Vec<f32>>, EmbeddingEndpointError> {
        let failure = |message: &str| EmbeddingEndpointError::Failure(message.into());
        if self.cancel.load(Ordering::Acquire) {
            return Err(failure("historical fill cancelled"));
        }
        let bytes = inputs
            .iter()
            .try_fold(0usize, |total, text| total.checked_add(text.len()))
            .ok_or_else(|| failure("historical byte budget overflow"))?;
        {
            let mut report = self.report.borrow_mut();
            if inputs.len() > self.limits.batch_inputs.max(2)
                || report.attempted_calls >= self.limits.source_calls + 2
                || report.attempted_inputs + inputs.len() > self.limits.missing_rows + 4
                || report
                    .text_bytes
                    .checked_add(bytes)
                    .is_none_or(|total| total > self.limits.text_bytes)
            {
                return Err(failure("historical provider budget exceeded"));
            }
            report.attempted_calls += 1;
            report.attempted_inputs += inputs.len();
            report.text_bytes += bytes;
        }
        // Generous JSON overhead allowance, still fixed before transport allocation.
        let cap = inputs
            .len()
            .checked_mul(self.dimensions() as usize)
            .and_then(|size| size.checked_mul(64))
            .and_then(|size| size.checked_add(4096))
            .ok_or_else(|| failure("historical response budget overflow"))?;
        let vectors = self.endpoint.embed_bounded(mode, inputs, cap)?;
        if vectors.len() != inputs.len()
            || vectors
                .iter()
                .any(|vector| vector.len() != self.dimensions() as usize)
        {
            return Err(EmbeddingEndpointError::InvalidResponse(
                "historical output shape",
            ));
        }
        let mut report = self.report.borrow_mut();
        report.completed_calls += 1;
        report.completed_inputs += inputs.len();
        Ok(vectors)
    }
}

impl crate::RelExecutionOwner {
    pub(crate) fn runtime_historical_embedding_supports_bounded<E: EmbeddingEndpoint>(
        &self,
        endpoint: &E,
    ) -> bool {
        endpoint.supports_bounded_embedding()
    }

    pub(crate) fn runtime_historical_embedding_dimensions<E: EmbeddingEndpoint>(
        &self,
        endpoint: &E,
    ) -> u32 {
        endpoint.dimensions()
    }

    pub(crate) fn runtime_historical_embedding_normalization<E: EmbeddingEndpoint>(
        &self,
        endpoint: &E,
    ) -> VectorNormalization {
        endpoint.normalization()
    }

    pub(crate) fn runtime_historical_embedding_verify_endpoint<E: EmbeddingEndpoint>(
        &self,
        profile: &CompatibilityProfile,
        endpoint: &E,
        cancel: &AtomicBool,
        report: &RefCell<HistoricalEmbeddingReport>,
        limits: HistoricalEmbeddingLimits,
    ) -> Result<CompatibilityReport, CompatibilityProfileError> {
        let provider = HistoricalEmbeddingProvider {
            endpoint,
            cancel,
            report,
            limits,
        };
        crate::compatibility_profile_probe::verify_endpoint(profile, &provider)
    }

    pub(crate) fn runtime_historical_embedding_batch<E: EmbeddingEndpoint>(
        &self,
        endpoint: &E,
        cancel: &AtomicBool,
        report: &RefCell<HistoricalEmbeddingReport>,
        limits: HistoricalEmbeddingLimits,
        mode: EmbeddingMode,
        inputs: &[String],
    ) -> Result<Vec<Vec<f32>>, EmbeddingEndpointError> {
        let provider = HistoricalEmbeddingProvider {
            endpoint,
            cancel,
            report,
            limits,
        };
        provider.embed(mode, inputs)
    }

    pub(crate) fn runtime_historical_embedding_validate_batch(
        &self,
        profile: &CompatibilityProfile,
        vectors: &[Vec<f32>],
        expected: usize,
    ) -> Result<(), CompatibilityProfileError> {
        crate::compatibility_profile_probe::validate_embedding_batch(profile, vectors, expected)
    }

    pub(crate) fn runtime_historical_embedding_vector_is_usable(&self, vector: &[f32]) -> bool {
        crate::freshness::historical_semantic::historical_embedding_vector_is_usable(vector)
    }

    pub(crate) fn runtime_historical_latest_vector_generation(&self) -> Option<VectorGeneration> {
        self.owner.vector_generation_latest_by_version()
    }

    pub(crate) fn runtime_historical_archive_vector_info(
        &self,
        id: crate::ArchiveVectorId,
    ) -> Option<crate::ArchiveVectorInfo> {
        self.owner.vector_archive_vector_info(id)
    }

    pub(crate) fn runtime_historical_packed_vector_info(
        &self,
        id: crate::PackedVectorId,
    ) -> Option<crate::PackedVectorInfo> {
        self.owner.vector_packed_vector_info(id)
    }

    pub(crate) fn runtime_historical_archive_vectors(
        &mut self,
        id: crate::ArchiveVectorId,
    ) -> Result<crate::ArchiveVectorSet, crate::ArchiveVectorError> {
        self.owner.get_archive_vectors(id)
    }

    pub(crate) fn runtime_historical_packed_vectors(
        &mut self,
        id: crate::PackedVectorId,
    ) -> Result<lodestone_packed::PackedVectors, crate::PackedVectorError> {
        self.owner.get_packed_vectors(id)
    }

    pub(crate) fn runtime_historical_source_nodes(&self) -> impl Iterator<Item = &crate::Node> {
        self.owner.native_historical_embedding_nodes()
    }

    pub(crate) fn runtime_historical_source_fragments(
        &self,
    ) -> impl Iterator<Item = &crate::Fragment> {
        self.owner.native_historical_embedding_fragments()
    }

    pub(crate) fn runtime_historical_embedding_node_count(&self) -> usize {
        self.owner.native_historical_embedding_node_count()
    }

    pub(crate) fn runtime_historical_embedding_fragment_count(&self) -> usize {
        self.owner.native_historical_embedding_fragment_count()
    }

    pub(crate) fn runtime_historical_embedding_fragment_text(
        &mut self,
        fragment: &crate::Fragment,
        max_bytes: usize,
    ) -> Result<String, crate::ArchiveError> {
        self.owner
            .native_historical_embedding_fragment_text(fragment, max_bytes)
    }
}
