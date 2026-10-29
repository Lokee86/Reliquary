#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContextRole {
    User,
    Assistant,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContextTurn {
    pub message_id: String,
    pub role: ContextRole,
    pub content: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ContextView {
    pub turns: Vec<ContextTurn>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContextMessage {
    pub role: ContextRole,
    pub content: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContextRequest {
    pub instructions: String,
    pub messages: Vec<ContextMessage>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EvidenceKind {
    ReasoningSummary,
    Commentary,
    ReasoningTrace,
    ToolCall,
    ToolResult,
    Activity,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContextEvidence {
    pub kind: EvidenceKind,
    pub model_round: Option<u32>,
    pub correlation_id: Option<String>,
    pub name: Option<String>,
    pub content: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompactionCheckpoint {
    pub through_message_id: String,
    pub summary: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompactionBudget {
    pub trigger_tokens: u64,
    pub target_tokens: u64,
    pub raw_tail_tokens: u64,
    pub summary_tokens: u64,
    pub compaction_input_tokens: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompactionPlan {
    pub through_message_id: String,
    pub instructions: String,
    pub messages: Vec<ContextMessage>,
    pub summary_token_limit: u64,
}
