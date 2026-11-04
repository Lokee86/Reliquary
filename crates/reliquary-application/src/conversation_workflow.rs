//! Native conversation publication through the canonical owner-session writer.
//! Gateway owns queue arbitration, cancellation and identity grants; this
//! workflow owns the semantic Archive command and validates it before commit.

use reliquary_owner_session::{
    OwnerSessionError, RelWriter,
    model::{Branch, TurnInput},
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeConversationCommit {
    pub node_id: String,
    pub conversation_id: String,
    pub parent_node_id: Option<String>,
    pub branch_id: String,
    pub principal_id: String,
    pub committed_timestamp_ns: i64,
    pub content: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublishedNativeTurn {
    pub node_id: String,
    pub parent_node_id: Option<String>,
    pub principal_id: Option<String>,
    pub committed_timestamp_ns: i64,
}

impl NativeConversationCommit {
    fn into_archive_parts(self) -> (TurnInput, Branch) {
        let branch = Branch {
            id: self.branch_id,
            conversation_id: self.conversation_id.clone(),
            leaf_node_id: self.node_id.clone(),
            canonical: true,
        };
        let turn = TurnInput {
            id: self.node_id,
            conversation_id: self.conversation_id,
            parent_id: self.parent_node_id,
            role: "user".to_owned(),
            principal_id: Some(self.principal_id),
            timestamp_ns: self.committed_timestamp_ns,
            content: self.content,
            attachments: Vec::new(),
            project_attachments: Vec::new(),
        };
        (turn, branch)
    }
}

/// Validate before the Gateway wins the durable queue finalization race.
pub fn validate_native_conversation_commit(
    writer: &RelWriter,
    command: &NativeConversationCommit,
) -> Result<(), OwnerSessionError> {
    let (turn, branch) = command.clone().into_archive_parts();
    writer.validate_turn_branch(&turn, &branch)
}

/// Publish through the owner session. No caller receives a store or Container.
pub fn commit_native_conversation_turn(
    writer: &mut RelWriter,
    command: NativeConversationCommit,
) -> Result<PublishedNativeTurn, OwnerSessionError> {
    let (turn, branch) = command.into_archive_parts();
    let node = writer.accept_turn_and_advance_branch(turn, branch)?;
    Ok(PublishedNativeTurn {
        node_id: node.id,
        parent_node_id: node.parent_id,
        principal_id: node.principal_id,
        committed_timestamp_ns: node.timestamp_ns,
    })
}
