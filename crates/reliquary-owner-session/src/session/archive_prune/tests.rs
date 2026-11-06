use super::*;
use reliquary_archive::{Branch, TurnInput};
use reliquary_container::ReliquaryScope;
use std::path::PathBuf;

pub(super) struct Fixture(pub(super) PathBuf);
impl Fixture {
    pub(super) fn new(suffix: &str) -> Self {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".test-tmp");
        fs::create_dir_all(&dir).unwrap();
        Self(dir.join(format!(
                "archive-prune-{}-{}-{suffix}.rel",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            )))
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

#[test]
fn terminal_archive_prune_retains_structure_clock_and_proof() {
    let source = Fixture::new("source");
    let output = Fixture::new("output");
    let refused = Fixture::new("refused");
    let owner = [181; 16];
    let grant = GraphAdmission::for_replay(owner, [], [], [], [], []);
    let mut writer = RelWriter::create_path(&source.0, ReliquaryScope::Project, owner).unwrap();
    writer
        .accept_turn_and_advance_branch(
            TurnInput {
                id: "turn-1".into(),
                conversation_id: "terminal".into(),
                parent_id: None,
                role: "user".into(),
                principal_id: Some("owner".into()),
                timestamp_ns: 9,
                content: "private terminal transcript ".repeat(200),
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
    drop(writer);
    let plan = VerifiedOwnerCopyPlan::prepare(&source.0).unwrap();
    assert!(
        prune_terminal_archive_copy(&plan, &source.0, &refused.0, &["terminal".into()], &grant)
            .is_err()
    );
    assert!(!refused.0.exists());
    let mut writer = RelWriter::open_path(&source.0, grant.clone(), [], []).unwrap();
    assert!(writer.tombstone_conversation("terminal").unwrap());
    let source_version = writer.session.archive.archive_version();
    let source_cut = writer.read_session().current_historical_cut();
    let source_proof = writer
        .read_session()
        .historical_proof(owner, &source_cut)
        .unwrap();
    drop(writer);
    let plan = VerifiedOwnerCopyPlan::prepare(&source.0).unwrap();
    let before = fs::read(&source.0).unwrap();
    let report =
        prune_terminal_archive_copy(&plan, &source.0, &output.0, &["terminal".into()], &grant)
            .unwrap();
    assert_eq!((report.pruned_conversations, report.pruned_turns), (1, 1));
    assert!(report.transformed_bytes < report.source_bytes);
    assert_eq!(fs::read(&source.0).unwrap(), before);
    let reopened = RelSession::open_path(&output.0, &grant, [], []).unwrap();
    assert_eq!(reopened.archive_version(), source_version);
    assert!(reopened.archive_node("terminal", "turn-1").is_some());
    assert!(
        reopened
            .resolved_archive_turn("terminal", "turn-1")
            .is_err()
    );
    assert!(reopened.conversation_tombstone("terminal").is_some());
    assert!(
        reopened
            .retention_manifest(&report.manifest_digest)
            .is_some()
    );
    let pruned_proof = reopened.historical_proof(owner, &source_cut).unwrap();
    assert_eq!(
        source_proof.history_generation,
        pruned_proof.history_generation
    );
    assert_eq!(source_proof.retained_records, pruned_proof.retained_records);
    assert_ne!(source_proof.digest, pruned_proof.digest);
    assert!(pruned_proof.unavailable_payloads > source_proof.unavailable_payloads);
}
#[test]
fn terminal_archive_prune_preserves_episode_and_attachment_history() {
    use reliquary_archive::{AttachmentInput, EpisodeBoundary, EpisodeOrigin};
    let source = Fixture::new("multi-source");
    let output = Fixture::new("multi-output");
    let owner = [183; 16];
    let grant = GraphAdmission::for_replay(owner, [], [], [], [], []);
    let mut writer = RelWriter::create_path(&source.0, ReliquaryScope::Project, owner).unwrap();
    let private = "terminal text should disappear ".repeat(250);
    let attachment_bytes = b"retained attachment content".repeat(20);
    for (conversation, id, parent, timestamp, content, with_file) in [
        ("terminal", "turn-a", None, 10, private.clone(), true),
        (
            "terminal",
            "turn-b",
            Some("turn-a"),
            11,
            "second terminal turn".repeat(100),
            false,
        ),
        (
            "retained",
            "turn-c",
            None,
            12,
            "unselected content".repeat(100),
            false,
        ),
    ] {
        writer
            .accept_turn_and_advance_branch(
                TurnInput {
                    id: id.into(),
                    conversation_id: conversation.into(),
                    parent_id: parent.map(str::to_owned),
                    role: "user".into(),
                    principal_id: None,
                    timestamp_ns: timestamp,
                    content,
                    attachments: if with_file {
                        vec![AttachmentInput {
                            filename: "private.pdf".into(),
                            mime_type: Some("application/pdf".into()),
                            bytes: attachment_bytes.clone(),
                        }]
                    } else {
                        Vec::new()
                    },
                    project_attachments: Vec::new(),
                },
                Branch {
                    id: "main".into(),
                    conversation_id: conversation.into(),
                    leaf_node_id: id.into(),
                    canonical: true,
                },
            )
            .unwrap();
    }
    let episode_id = writer
        .finalize_episode(
            "terminal",
            "turn-a",
            "turn-b",
            EpisodeOrigin::Live,
            EpisodeBoundary::Explicit,
            12,
        )
        .unwrap();
    let work_id = reliquary_insomnia_store::EpisodeId(episode_id.0);
    let order = writer
        .session
        .insomnia
        .accepted_order(work_id)
        .unwrap()
        .clone();
    writer
        .session
        .insomnia
        .queue(
            &mut writer.container,
            order,
            reliquary_insomnia_store::InsomniaPriority::Live,
            20,
        )
        .unwrap();
    let work = writer
        .session
        .insomnia
        .claim_episode(&mut writer.container, work_id, "test", 21, 10)
        .unwrap();
    writer
        .session
        .insomnia
        .terminal(
            &mut writer.container,
            work_id,
            work.lease_token.unwrap(),
            22,
            "source retired".into(),
        )
        .unwrap();
    writer.session.writer_lease_proof = writer.container.writer_lease_proof();
    assert!(writer.tombstone_conversation("terminal").unwrap());
    drop(writer);

    let before = fs::read(&source.0).unwrap();
    let reference = RelSession::open_path(&source.0, &grant, [], []).unwrap();
    let plan = VerifiedOwnerCopyPlan::prepare(&source.0).unwrap();
    let report =
        prune_terminal_archive_copy(&plan, &source.0, &output.0, &["terminal".into()], &grant)
            .unwrap();
    assert_eq!((report.pruned_conversations, report.pruned_turns), (1, 2));
    assert_eq!(fs::read(&source.0).unwrap(), before);
    let after_bytes = fs::read(&output.0).unwrap();
    assert!(
        !after_bytes
            .windows(private.len())
            .any(|bytes| bytes == private.as_bytes())
    );
    let reopened = RelSession::open_path(&output.0, &grant, [], []).unwrap();
    assert_eq!(
        reopened.archive.node_count(),
        reference.archive.node_count()
    );
    assert_eq!(
        reopened.archive.episode(episode_id),
        reference.archive.episode(episode_id)
    );
    assert_eq!(reopened.archive.branches(), reference.archive.branches());
    assert_eq!(reopened.archive.pruned_turns().len(), 2);
    assert_eq!(
        reopened.archive.files_for_source("terminal", "turn-a"),
        reference.archive.files_for_source("terminal", "turn-a"),
    );
    assert_eq!(
        reopened.archive.files_for_source("terminal", "turn-a")[0].bytes,
        attachment_bytes
    );
    assert_eq!(
        reopened
            .archive
            .resolved_turn("retained", "turn-c")
            .unwrap(),
        reference
            .archive
            .resolved_turn("retained", "turn-c")
            .unwrap(),
    );
    assert!(
        reopened
            .archive
            .resolved_turn("terminal", "turn-a")
            .is_err()
    );
    assert!(reopened.archive.episode_turns(episode_id).is_err());
    assert!(
        reopened
            .retention_manifest(&report.manifest_digest)
            .is_some()
    );
}
