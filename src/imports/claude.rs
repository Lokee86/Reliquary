use super::{ImportError, NativeImport};

/// Normalize the Claude conversations.json UUID/chat_messages/text-v1 profile.
pub fn normalize_claude(namespace: &str, bytes: &[u8]) -> Result<NativeImport, ImportError> {
    reliquary_import::normalize_claude(namespace, bytes).map_err(Into::into)
}
