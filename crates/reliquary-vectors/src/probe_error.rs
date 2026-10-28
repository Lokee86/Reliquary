use crate::EmbeddingEndpointError;

#[derive(Debug)]
pub enum ProbeError {
    Endpoint(EmbeddingEndpointError),
    InvalidProfile(&'static str),
    SizeOverflow,
}

impl From<EmbeddingEndpointError> for ProbeError {
    fn from(value: EmbeddingEndpointError) -> Self {
        Self::Endpoint(value)
    }
}

impl std::fmt::Display for ProbeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "vector probe error: {self:?}")
    }
}
impl std::error::Error for ProbeError {}
