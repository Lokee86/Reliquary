use super::{ImportError, NativeImport};

/// Hermes full-session JSONL text profile. Active text rows with exported database IDs only.
pub fn normalize_hermes_jsonl(namespace: &str, bytes: &[u8]) -> Result<NativeImport, ImportError> {
    reliquary_import::normalize_hermes_jsonl(namespace, bytes).map_err(Into::into)
}
