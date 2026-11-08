use reliquary_vector_store::EmbeddingMode;
use std::fmt;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EmbeddingEndpointError {
    InvalidConfiguration(&'static str),
    InvalidResponse(&'static str),
    Failure(String),
}
impl fmt::Display for EmbeddingEndpointError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "embedding endpoint error: {self:?}")
    }
}
impl std::error::Error for EmbeddingEndpointError {}

pub trait EmbeddingEndpoint {
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
    fn normalization(&self) -> reliquary_vector_store::VectorNormalization;
    fn embed(
        &self,
        mode: EmbeddingMode,
        inputs: &[String],
    ) -> Result<Vec<Vec<f32>>, EmbeddingEndpointError>;
}

/// Adapts Switchboard's provider endpoint contract to the vector validation
/// interface while preserving its bounded-response capability and errors.
pub struct SwitchboardEmbeddingEndpoint {
    inner: std::sync::Arc<dyn reliquary_switchboard::EmbeddingEndpoint>,
}

impl SwitchboardEmbeddingEndpoint {
    pub fn new(inner: std::sync::Arc<dyn reliquary_switchboard::EmbeddingEndpoint>) -> Self {
        Self { inner }
    }
}

impl EmbeddingEndpoint for SwitchboardEmbeddingEndpoint {
    fn supports_bounded_embedding(&self) -> bool {
        self.inner.supports_bounded_embedding()
    }
    fn embed_bounded(
        &self,
        mode: EmbeddingMode,
        inputs: &[String],
        max_response_bytes: usize,
    ) -> Result<Vec<Vec<f32>>, EmbeddingEndpointError> {
        self.inner
            .embed_bounded(switchboard_mode(mode), inputs, max_response_bytes)
            .map_err(Into::into)
    }
    fn dimensions(&self) -> u32 {
        self.inner.dimensions()
    }
    fn normalization(&self) -> reliquary_vector_store::VectorNormalization {
        match self.inner.normalization() {
            reliquary_switchboard::EmbeddingNormalization::None => {
                reliquary_vector_store::VectorNormalization::None
            }
            reliquary_switchboard::EmbeddingNormalization::L2 => {
                reliquary_vector_store::VectorNormalization::L2
            }
        }
    }
    fn embed(
        &self,
        mode: EmbeddingMode,
        inputs: &[String],
    ) -> Result<Vec<Vec<f32>>, EmbeddingEndpointError> {
        self.inner
            .embed(switchboard_mode(mode), inputs)
            .map_err(Into::into)
    }
}

fn switchboard_mode(mode: EmbeddingMode) -> reliquary_switchboard::EmbeddingMode {
    match mode {
        EmbeddingMode::Query => reliquary_switchboard::EmbeddingMode::Query,
        EmbeddingMode::Document => reliquary_switchboard::EmbeddingMode::Document,
    }
}

impl From<reliquary_switchboard::EmbeddingEndpointError> for EmbeddingEndpointError {
    fn from(error: reliquary_switchboard::EmbeddingEndpointError) -> Self {
        match error {
            reliquary_switchboard::EmbeddingEndpointError::InvalidConfiguration(m) => {
                Self::InvalidConfiguration(m)
            }
            reliquary_switchboard::EmbeddingEndpointError::InvalidResponse(m) => {
                Self::InvalidResponse(m)
            }
            reliquary_switchboard::EmbeddingEndpointError::Failure(m) => Self::Failure(m),
            reliquary_switchboard::EmbeddingEndpointError::HttpStatus(status) => {
                Self::Failure(format!("embedding provider returned HTTP {status}"))
            }
        }
    }
}

#[derive(Debug)]
pub enum ProbeError {
    Endpoint(EmbeddingEndpointError),
    InvalidProfile(&'static str),
    SizeOverflow,
}
impl From<EmbeddingEndpointError> for ProbeError {
    fn from(e: EmbeddingEndpointError) -> Self {
        Self::Endpoint(e)
    }
}
impl fmt::Display for ProbeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "vector probe error: {self:?}")
    }
}
impl std::error::Error for ProbeError {}
