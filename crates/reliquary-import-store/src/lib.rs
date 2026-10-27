//! Durable import epoch values, codecs, and passive projection.
pub mod codec;
mod store;
mod values;
pub use store::{ImportEpochStore, import_id};
pub use values::*;

#[derive(Debug)]
pub enum ImportError {
    Epoch(&'static str),
    Json(serde_json::Error),
    Container(reliquary_container::ContainerError),
}
impl From<serde_json::Error> for ImportError {
    fn from(e: serde_json::Error) -> Self {
        Self::Json(e)
    }
}
impl From<reliquary_container::ContainerError> for ImportError {
    fn from(e: reliquary_container::ContainerError) -> Self {
        Self::Container(e)
    }
}
impl std::fmt::Display for ImportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for ImportError {}
