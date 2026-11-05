//! Native conversation publication through the canonical owner-session writer.
//! Gateway owns queue arbitration, cancellation and identity grants; this
//! workflow owns the semantic Archive command and validates it before commit.

use reliquary_owner_session::{
    OwnerSessionError, RelWriter,
    model::{Branch, EpisodeBoundary, EpisodeId, EpisodeOrigin, TurnInput},
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
    pub(crate) fn into_archive_parts(self) -> (TurnInput, Branch) {
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

/// Finalize an accepted native conversation range for durable Insomnia work.
pub fn finalize_native_episode(
    writer: &mut RelWriter,
    conversation_id: &str,
    start_node_id: &str,
    end_node_id: &str,
    boundary: EpisodeBoundary,
    finalized_at_ns: i64,
) -> Result<EpisodeId, OwnerSessionError> {
    writer.finalize_episode(
        conversation_id,
        start_node_id,
        end_node_id,
        EpisodeOrigin::Live,
        boundary,
        finalized_at_ns,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use reliquary_container::ReliquaryScope;
    use reliquary_owner_session::GraphAdmission;
    use std::{
        fs,
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
    };
    static NEXT: AtomicU64 = AtomicU64::new(1);
    struct Temp(PathBuf);
    impl Temp {
        fn new() -> Self {
            let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".test-tmp");
            fs::create_dir_all(&dir).unwrap();
            Self(dir.join(format!(
                "conversation-{}-{}.rel",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            )))
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = fs::remove_file(&self.0);
        }
    }
    #[test]
    fn native_turn_to_episode_work_reopens_without_caller_admission() {
        let path = Temp::new();
        let owner = [0x69; 16];
        let mut writer = RelWriter::create_path(&path.0, ReliquaryScope::Project, owner).unwrap();
        let command = NativeConversationCommit {
            node_id: "turn".into(),
            conversation_id: "conversation".into(),
            parent_node_id: None,
            branch_id: "main".into(),
            principal_id: "user".into(),
            committed_timestamp_ns: 10,
            content: "source evidence".into(),
        };
        commit_native_conversation_turn(&mut writer, command).unwrap();
        let episode = finalize_native_episode(
            &mut writer,
            "conversation",
            "turn",
            "turn",
            EpisodeBoundary::Explicit,
            11,
        )
        .unwrap();
        drop(writer);
        let admission = GraphAdmission::for_replay(owner, [], [], [], [], []);
        let mut writer = RelWriter::open_path(&path.0, admission, [], []).unwrap();
        let claim = writer
            .claim_next_insomnia_input("worker", 12, 100, 1)
            .unwrap()
            .unwrap();
        assert_eq!(claim.episode.id, episode);
        assert_eq!(claim.turns[0].content, "source evidence");
    }
}
