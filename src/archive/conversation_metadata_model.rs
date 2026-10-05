/// Durable conversation scheduling policy, owned by Archive metadata.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ConversationPolicy {
    #[default]
    SequentialChronological,
    VisibleAutoBranch,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConversationMetadata {
    pub conversation_id: String,
    pub title: Option<String>,
    pub active: bool,
    pub policy: ConversationPolicy,
    /// Independent of title/active revisions. Legacy metadata starts at 1.
    pub policy_revision: u64,
}

impl ConversationMetadata {
    pub(crate) fn new(conversation_id: &str) -> Self {
        Self {
            conversation_id: conversation_id.to_owned(),
            title: None,
            active: false,
            policy: ConversationPolicy::default(),
            policy_revision: 1,
        }
    }
}
