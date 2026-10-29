mod chatgpt;
mod claude;
mod common;
mod hermes;

pub use chatgpt::normalize_chatgpt;
pub use claude::normalize_claude;
pub use common::normalize_agent;
pub use hermes::normalize_hermes_jsonl;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ImportError;

    #[test]
    fn agent_profile_preserves_the_provider_neutral_contract() {
        let bytes = br#"{"schema_version":1,"source_namespace":"acct","source_format":"agent-v1","conversations":[]}"#;
        let normalized = normalize_agent(bytes).expect("valid interchange");
        assert_eq!(normalized.source_namespace, "acct");
        assert_eq!(normalized.source_format, "agent-v1");
    }

    #[test]
    fn chatgpt_rejects_a_mapping_with_a_mismatched_message_id() {
        let bytes = br#"[{"id":"c","mapping":{"node":{"message":{"id":"different","author":{"role":"user"},"content":{"content_type":"text","parts":["hello"]},"create_time":1},"parent":null}}}]"#;
        assert!(matches!(
            normalize_chatgpt("acct", bytes),
            Err(ImportError::Invalid(_))
        ));
    }

    #[test]
    fn claude_normalizes_uuid_and_rfc3339_timestamp() {
        let bytes = br#"[{"uuid":"c","name":"title","chat_messages":[{"uuid":"m1","sender":"human","text":"hello","created_at":"2024-01-01T00:00:00Z"}]}]"#;
        let normalized = normalize_claude("acct", bytes).expect("valid Claude export");
        assert_eq!(normalized.conversations[0].messages[0].id, "m1");
        assert_eq!(
            normalized.conversations[0].messages[0].timestamp_ns,
            1_704_067_200_000_000_000
        );
    }

    #[test]
    fn hermes_rejects_rows_without_stable_message_ids() {
        let bytes = br#"{"id":"c","messages":[{"role":"user","timestamp":1,"content":"hello"}]}"#;
        assert!(matches!(
            normalize_hermes_jsonl("acct", bytes),
            Err(ImportError::Unsupported(_))
        ));
    }
}
