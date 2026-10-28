pub use reliquary_switchboard::EmbeddingEndpointError;
pub use reliquary_vector_store::{EmbeddingMode, VectorNormalization};
use sha2::{Digest, Sha256};

pub trait EmbeddingEndpoint {
    /// One request, no redirects/retries/fan-out, bounded output and a finite timeout.
    fn supports_bounded_embedding(&self) -> bool {
        false
    }
    fn embed_bounded(
        &self,
        _mode: EmbeddingMode,
        _inputs: &[String],
        _max_response_bytes: usize,
    ) -> Result<Vec<Vec<f32>>, EmbeddingEndpointError> {
        Err(EmbeddingEndpointError::InvalidConfiguration(
            "bounded embedding unsupported",
        ))
    }
    fn dimensions(&self) -> u32;
    fn normalization(&self) -> VectorNormalization;
    fn embed(
        &self,
        mode: EmbeddingMode,
        inputs: &[String],
    ) -> Result<Vec<Vec<f32>>, EmbeddingEndpointError>;
}

#[derive(Clone, Debug)]
pub struct SimulatedEmbeddingEndpoint {
    dimensions: u32,
    normalization: VectorNormalization,
    seed: u64,
    drift: f32,
}

impl SimulatedEmbeddingEndpoint {
    pub fn new(dimensions: u32, normalization: VectorNormalization, seed: u64) -> Self {
        Self {
            dimensions,
            normalization,
            seed,
            drift: 0.0,
        }
    }

    pub fn with_drift(mut self, drift: f32) -> Self {
        self.drift = drift;
        self
    }

    fn embed_one(
        &self,
        mode: EmbeddingMode,
        input: &str,
    ) -> Result<Vec<f32>, EmbeddingEndpointError> {
        let dimensions = usize::try_from(self.dimensions)
            .map_err(|_| EmbeddingEndpointError::InvalidConfiguration("dimensions overflow"))?;
        if dimensions == 0 || !self.drift.is_finite() {
            return Err(EmbeddingEndpointError::InvalidConfiguration(
                "invalid simulator configuration",
            ));
        }
        let mut output = Vec::with_capacity(dimensions);
        let mut block = 0_u32;
        while output.len() < dimensions {
            let mut hash = Sha256::new();
            hash.update(b"CONTINUITY-SIMULATED-EMBEDDING-V2\0");
            hash.update(self.seed.to_le_bytes());
            hash.update([mode.tag()]);
            hash.update(block.to_le_bytes());
            hash.update(input.as_bytes());
            let digest = hash.finalize();
            for raw in digest.chunks_exact(4) {
                if output.len() == dimensions {
                    break;
                }
                let value = u32::from_le_bytes(raw.try_into().expect("u32 width"));
                let base = (value as f64 / u32::MAX as f64 * 2.0 - 1.0) as f32;
                let sign = if output.len() % 2 == 0 { 1.0 } else { -1.0 };
                output.push(base + self.drift * sign);
            }
            block = block.checked_add(1).ok_or_else(|| {
                EmbeddingEndpointError::Failure("simulated block overflow".into())
            })?;
        }
        if self.normalization == VectorNormalization::L2 {
            let norm = output
                .iter()
                .map(|value| (*value as f64) * (*value as f64))
                .sum::<f64>()
                .sqrt();
            if norm == 0.0 {
                return Err(EmbeddingEndpointError::InvalidResponse("zero vector"));
            }
            for value in &mut output {
                *value = (*value as f64 / norm) as f32;
            }
        }
        Ok(output)
    }
}

impl EmbeddingEndpoint for SimulatedEmbeddingEndpoint {
    fn supports_bounded_embedding(&self) -> bool {
        true
    }
    fn embed_bounded(
        &self,
        mode: EmbeddingMode,
        inputs: &[String],
        max_response_bytes: usize,
    ) -> Result<Vec<Vec<f32>>, EmbeddingEndpointError> {
        let bytes = inputs
            .len()
            .checked_mul(self.dimensions as usize)
            .and_then(|n| n.checked_mul(4));
        if bytes.is_none_or(|n| n > max_response_bytes) {
            return Err(EmbeddingEndpointError::InvalidResponse(
                "bounded output exceeded",
            ));
        }
        self.embed(mode, inputs)
    }
    fn dimensions(&self) -> u32 {
        self.dimensions
    }

    fn normalization(&self) -> VectorNormalization {
        self.normalization
    }

    fn embed(
        &self,
        mode: EmbeddingMode,
        inputs: &[String],
    ) -> Result<Vec<Vec<f32>>, EmbeddingEndpointError> {
        inputs
            .iter()
            .map(|input| self.embed_one(mode, input))
            .collect()
    }
}

/// Project the provider route's normalization into vector semantics.
pub(crate) fn vector_normalization(value: crate::EmbeddingNormalization) -> VectorNormalization {
    match value {
        crate::EmbeddingNormalization::None => VectorNormalization::None,
        crate::EmbeddingNormalization::L2 => VectorNormalization::L2,
    }
}

impl EmbeddingEndpoint for reliquary_switchboard::OpenAiReadyEmbeddingEndpoint {
    fn supports_bounded_embedding(&self) -> bool {
        self.supports_bounded_provider_embedding()
    }
    fn embed_bounded(
        &self,
        mode: EmbeddingMode,
        inputs: &[String],
        max_response_bytes: usize,
    ) -> Result<Vec<Vec<f32>>, EmbeddingEndpointError> {
        self.embed_bounded_provider(provider_mode(mode), inputs, max_response_bytes)
    }
    fn dimensions(&self) -> u32 {
        self.provider_dimensions()
    }
    fn normalization(&self) -> VectorNormalization {
        vector_normalization(self.provider_normalization())
    }
    fn embed(
        &self,
        mode: EmbeddingMode,
        inputs: &[String],
    ) -> Result<Vec<Vec<f32>>, EmbeddingEndpointError> {
        self.embed_provider(provider_mode(mode), inputs)
    }
}
fn provider_mode(mode: EmbeddingMode) -> reliquary_switchboard::EmbeddingMode {
    match mode {
        EmbeddingMode::Query => reliquary_switchboard::EmbeddingMode::Query,
        EmbeddingMode::Document => reliquary_switchboard::EmbeddingMode::Document,
    }
}
