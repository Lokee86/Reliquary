//! Versioned native historical import. Adapters normalize; Archive remains authoritative.
mod commit;
mod completion;
mod contract;
mod epoch_codec;
mod epoch_scope;
mod epoch_store;
mod prepare;
mod readiness;
mod reconciliation;
mod reconciliation_interval;
mod reconciliation_replay;
pub use contract::*;
pub(crate) use reconciliation::reconcile_import_epochs;
#[cfg(test)]
mod reconcile_tests;
#[cfg(test)]
mod sequential_completion_tests;
#[cfg(test)]
pub(crate) mod tests;
pub(crate) use epoch_scope::ImportEpochScope;
pub(crate) use epoch_store::ImportEpochStore;
pub(crate) use prepare::prepare;

/// Hermes full-session JSONL text profile. Active text rows with exported database IDs only.
pub fn normalize_hermes_jsonl(namespace: &str, bytes: &[u8]) -> Result<NativeImport, ImportError> {
    reliquary_import::normalize_hermes_jsonl(namespace, bytes).map_err(Into::into)
}

/// Normalize the provider-neutral structured agent interchange format.
pub fn normalize_agent(bytes: &[u8]) -> Result<NativeImport, ImportError> {
    reliquary_import::normalize_agent(bytes).map_err(Into::into)
}

/// Normalize the ChatGPT conversations.json mapping/text-v1 profile.
pub fn normalize_chatgpt(namespace: &str, bytes: &[u8]) -> Result<NativeImport, ImportError> {
    reliquary_import::normalize_chatgpt(namespace, bytes).map_err(Into::into)
}

/// Normalize the Claude conversations.json UUID/chat_messages/text-v1 profile.
pub fn normalize_claude(namespace: &str, bytes: &[u8]) -> Result<NativeImport, ImportError> {
    reliquary_import::normalize_claude(namespace, bytes).map_err(Into::into)
}
