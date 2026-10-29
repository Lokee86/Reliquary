use super::{ImportError, NativeImport};

/// Normalize the provider-neutral structured agent interchange format.
pub fn normalize_agent(bytes: &[u8]) -> Result<NativeImport, ImportError> {
    reliquary_import::normalize_agent(bytes).map_err(Into::into)
}
