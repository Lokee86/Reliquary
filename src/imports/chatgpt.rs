use super::{ImportError, NativeImport};

/// Normalize the ChatGPT conversations.json mapping/text-v1 profile.
pub fn normalize_chatgpt(namespace: &str, bytes: &[u8]) -> Result<NativeImport, ImportError> {
    reliquary_import::normalize_chatgpt(namespace, bytes).map_err(Into::into)
}
