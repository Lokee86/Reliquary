//! Negative closure and tamper cases for terminal Archive pruning.
use super::*;
use reliquary_archive::{Branch, EpisodeBoundary, EpisodeOrigin, TurnInput};
use reliquary_container::ReliquaryScope;
use reliquary_memory_core::{MemoryDraft, MemorySourceRef};
use std::{io::Read, path::PathBuf};

pub(super) struct Fixture(pub(super) PathBuf);
impl Fixture {
    pub(super) fn new(label: &str) -> Self {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".test-tmp");
        fs::create_dir_all(&dir).unwrap();
        Self(dir.join(format!(
            "archive-neg-{}-{}-{label}.rel",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        )))
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}
fn draft(episode: [u8; 32], mode: u8) -> MemoryDraft {
    MemoryDraft {
        title: "retained memory".into(),
        content: "memory remains available".into(),
        category: "fact".into(),
        memory_type: "semantic".into(),
        authority_kind: "user".into(),
        temporal_status: "current".into(),
        scope: "project".into(),
        lifecycle_state: "extracted".into(),
        archived: false,
        parent_id: None,
        superseded_by: None,
        source_node_id: None,
        content_source_conversation_id: (mode == 0).then_some("terminal".into()),
        content_source_node_id: None,
        grounding_source_conversation_id: None,
        grounding_source_node_id: None,
        source_episode_id: (mode == 1).then_some(episode),
        source_ref: (mode == 2).then_some(MemorySourceRef {
            owner_id: "owner".into(),
            principal_id: None,
            source_episode_id: episode,
            source_node_id: "turn-1".into(),
            content_source_conversation_id: None,
            content_source_node_id: None,
            grounding_source_conversation_id: None,
            grounding_source_node_id: None,
        }),
        source_time_ns: None,
        temporal_inference: None,
        mutation_id: format!("memory-{mode}"),
        created_at_ns: 1,
        updated_at_ns: 1,
    }
}

#[test]
fn terminal_archive_prune_rejects_all_memory_source_provenance_forms() {
    for mode in 0..3 {
        let source = Fixture::new("source");
        let output = Fixture::new("output");
        let owner = [187; 16];
        let admission = GraphAdmission::for_replay(owner, [], [], [], [], []);
        let mut writer = RelWriter::create_path(&source.0, ReliquaryScope::Project, owner).unwrap();
        writer
            .accept_turn_and_advance_branch(
                TurnInput {
                    id: "turn-1".into(),
                    conversation_id: "terminal".into(),
                    parent_id: None,
                    role: "user".into(),
                    principal_id: None,
                    timestamp_ns: 1,
                    content: "terminal history".repeat(100),
                    attachments: vec![],
                    project_attachments: vec![],
                },
                Branch {
                    id: "main".into(),
                    conversation_id: "terminal".into(),
                    leaf_node_id: "turn-1".into(),
                    canonical: true,
                },
            )
            .unwrap();
        let episode = writer
            .session
            .archive
            .put_episode(
                &mut writer.container,
                "terminal",
                "turn-1",
                "turn-1",
                EpisodeOrigin::Live,
                EpisodeBoundary::Explicit,
                2,
            )
            .unwrap();
        writer
            .session
            .common
            .memories
            .publish_versioned_creation(&mut writer.container, draft(episode.0, mode), 1)
            .unwrap();
        writer.session.writer_lease_proof = writer.container.writer_lease_proof();
        assert!(writer.tombstone_conversation("terminal").unwrap());
        drop(writer);
        let before = fs::read(&source.0).unwrap();
        let plan = VerifiedOwnerCopyPlan::prepare(&source.0).unwrap();
        let error = prune_terminal_archive_copy(
            &plan,
            &source.0,
            &output.0,
            &["terminal".into()],
            &admission,
        )
        .unwrap_err();
        assert!(error.contains("Memory provenance"), "{error}");
        assert_eq!(fs::read(&source.0).unwrap(), before);
        assert!(!output.0.exists());
    }
}

#[test]
fn terminal_archive_prune_replay_rejects_unbound_or_tampered_marker() {
    let source = Fixture::new("source");
    let output = Fixture::new("output");
    let forged = Fixture::new("forged");
    let owner = [188; 16];
    let admission = GraphAdmission::for_replay(owner, [], [], [], [], []);
    let mut writer = RelWriter::create_path(&source.0, ReliquaryScope::Project, owner).unwrap();
    writer
        .accept_turn_and_advance_branch(
            TurnInput {
                id: "turn-1".into(),
                conversation_id: "terminal".into(),
                parent_id: None,
                role: "user".into(),
                principal_id: None,
                timestamp_ns: 1,
                content: "terminal secret ".repeat(200),
                attachments: vec![],
                project_attachments: vec![],
            },
            Branch {
                id: "main".into(),
                conversation_id: "terminal".into(),
                leaf_node_id: "turn-1".into(),
                canonical: true,
            },
        )
        .unwrap();
    writer.tombstone_conversation("terminal").unwrap();
    drop(writer);
    let plan = VerifiedOwnerCopyPlan::prepare(&source.0).unwrap();
    prune_terminal_archive_copy(
        &plan,
        &source.0,
        &output.0,
        &["terminal".into()],
        &admission,
    )
    .unwrap();

    let opened = Container::open_read_only(&output.0).unwrap();
    let mut target = Container::create(&forged.0, opened.identity(), opened.owner_id()).unwrap();
    let mut cursor = opened.physical_cursor().unwrap();
    let mut saw_marker = false;
    while let Some(mut frame) = cursor.next_frame().unwrap() {
        let mut bytes = Vec::new();
        frame.read_to_end(&mut bytes).unwrap();
        if bytes.starts_with(b"RELTPR01") {
            saw_marker = true;
            bytes[8..40].copy_from_slice(&[0x42; 32]);
        }
        target.append(&bytes).unwrap();
    }
    assert!(saw_marker);
    target.sync().unwrap();
    drop(target);
    assert!(RelSession::open_path(&forged.0, &admission, [], []).is_err());
    assert!(RelSession::open_path(&output.0, &admission, [], []).is_ok());
}
#[test]
fn terminal_archive_prune_refuses_unprocessed_episode() {
    let source = Fixture::new("pending-source");
    let output = Fixture::new("pending-output");
    let owner = [189; 16];
    let admission = GraphAdmission::for_replay(owner, [], [], [], [], []);
    let mut writer = RelWriter::create_path(&source.0, ReliquaryScope::Project, owner).unwrap();
    writer
        .accept_turn_and_advance_branch(
            TurnInput {
                id: "turn-1".into(),
                conversation_id: "terminal".into(),
                parent_id: None,
                role: "user".into(),
                principal_id: None,
                timestamp_ns: 1,
                content: "unprocessed terminal source".repeat(200),
                attachments: vec![],
                project_attachments: vec![],
            },
            Branch {
                id: "main".into(),
                conversation_id: "terminal".into(),
                leaf_node_id: "turn-1".into(),
                canonical: true,
            },
        )
        .unwrap();
    writer
        .finalize_episode(
            "terminal",
            "turn-1",
            "turn-1",
            EpisodeOrigin::Live,
            EpisodeBoundary::Explicit,
            2,
        )
        .unwrap();
    writer.tombstone_conversation("terminal").unwrap();
    drop(writer);
    let before = fs::read(&source.0).unwrap();
    let plan = VerifiedOwnerCopyPlan::prepare(&source.0).unwrap();
    let error = prune_terminal_archive_copy(
        &plan,
        &source.0,
        &output.0,
        &["terminal".into()],
        &admission,
    )
    .unwrap_err();
    assert!(error.contains("unfinished Insomnia"), "{error}");
    assert_eq!(fs::read(&source.0).unwrap(), before);
    assert!(!output.0.exists());
}
