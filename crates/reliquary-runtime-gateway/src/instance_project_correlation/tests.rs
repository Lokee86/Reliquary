use super::*;
use reliquary_container::ReliquaryScope;
use reliquary_owner_session::model::ProjectRepositoryKind;
use sha2::{Digest, Sha256};
use std::{fs, process::Command};

struct ContextWords;
impl reliquary_application::ContextTokenCounter for ContextWords {
    fn count_text(&self, text: &str) -> u64 {
        text.split_whitespace().count() as u64
    }
    fn count_message(&self, message: &reliquary_application::ContextMessage) -> u64 {
        self.count_text(&message.content).saturating_add(1)
    }
}

fn git(root: &Path, args: &[&str]) -> String {
    let result = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    String::from_utf8(result.stdout).unwrap().trim().to_owned()
}

#[cfg(windows)]
fn make_fixture_writable(root: &Path) {
    for entry in fs::read_dir(root).unwrap() {
        let entry = entry.unwrap();
        if entry.file_type().unwrap().is_dir() {
            make_fixture_writable(&entry.path());
        } else {
            let mut permissions = entry.metadata().unwrap().permissions();
            if permissions.readonly() {
                permissions.set_readonly(false);
                fs::set_permissions(entry.path(), permissions).unwrap();
            }
        }
    }
}

#[test]
fn project_grant_proofs_gate_native_commit_and_survive_reopen() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(".test-tmp")
        .join(format!("project-{}", Uuid::new_v4()));
    let repo = root.join("repo");
    fs::create_dir_all(&repo).unwrap();
    git(&repo, &["init", "-q"]);
    git(&repo, &["config", "user.name", "fixture"]);
    git(&repo, &["config", "user.email", "fixture@example.invalid"]);
    git(&repo, &["config", "warlock.repositoryId", "project-repo"]);
    fs::write(repo.join("source.txt"), "source authority").unwrap();
    fs::write(repo.join("other.txt"), "other source").unwrap();
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-qm", "source"]);
    let revision = git(&repo, &["rev-parse", "HEAD"]);
    let repository = ProjectRepositoryRef {
        kind: ProjectRepositoryKind::Git,
        repository_id: "project-repo".into(),
        project_path: ".".into(),
    };
    let path = root.join("owner.rel");
    let raw = [155; 16];
    drop(RelWriter::create_path(&path, ReliquaryScope::Project, raw).unwrap());
    let gateway = Gateway::new();
    let admission = GraphAdmission::for_replay(raw, [], [], [], [], []);
    let owner = gateway
        .mount_writable_reliquary(&path, &admission, [], [])
        .unwrap();
    let alice = gateway.instance("alice", [owner], []).unwrap();
    let bob = gateway.instance("bob", [owner], []).unwrap();
    let request = ProjectCorrelationRequest {
        authorized_owner_id: raw,
        authorized_principal_id: "alice".into(),
        repository: repository.clone(),
        revision: revision.clone(),
        path: "source.txt".into(),
        expected_content_hash: Sha256::digest(b"source authority").into(),
        filename: "source.txt".into(),
        mime_type: None,
    };
    let source = TrustedProjectSource::new("alice", owner, repository.clone(), &repo);
    assert!(
        bob.prepare_project_correlation(owner, request.clone(), &source)
            .is_err()
    );
    let denied = gateway.instance("alice", [], []).unwrap();
    assert!(
        denied
            .prepare_project_correlation(owner, request.clone(), &source)
            .is_err()
    );
    let proof = alice
        .prepare_project_correlation(owner, request.clone(), &source)
        .unwrap();
    let mut queue = alice
        .open_conversation_queue(root.join("queue"), owner)
        .unwrap();
    let id =
        ConversationSubmissionId::new("conversation", SubmissionId::new("source-turn").unwrap())
            .unwrap();
    alice
        .submit_conversation(
            &mut queue,
            "conversation",
            id.submission_id.clone(),
            "Use this file.",
            0,
        )
        .unwrap();
    let capture = FinalizeCapture {
        branch_id: "main".into(),
        parent_node_id: None,
        committed_timestamp_ns: 10,
        verified_principal_id: "alice".into(),
    };
    let version = queue.conversation_revision("conversation");
    source.revoke();
    assert!(
        gateway
            .finalize_project_correlated_submission(
                &alice,
                &mut queue,
                &id,
                version,
                capture.clone(),
                vec![proof]
            )
            .is_err()
    );
    assert_eq!(queue.status(&id).unwrap().state, QueueState::Pending);
    let source = TrustedProjectSource::new("alice", owner, repository.clone(), &repo);
    let proof = alice
        .prepare_project_correlation(owner, request.clone(), &source)
        .unwrap();
    let other_instance = gateway.instance("alice", [owner], []).unwrap();
    assert!(
        gateway
            .finalize_project_correlated_submission(
                &other_instance,
                &mut queue,
                &id,
                version,
                capture.clone(),
                vec![proof.clone()]
            )
            .is_err()
    );
    let committed = gateway
        .finalize_project_correlated_submission(
            &alice,
            &mut queue,
            &id,
            version,
            capture.clone(),
            vec![proof.clone()],
        )
        .unwrap();
    assert_eq!(committed.state, QueueState::Committed);
    assert_eq!(
        gateway
            .finalize_project_correlated_submission(
                &alice,
                &mut queue,
                &id,
                version,
                capture.clone(),
                vec![proof]
            )
            .unwrap(),
        committed
    );
    let attachments = alice
        .with_writable_reliquary(owner, |rel| {
            rel.archive_source_attachments("conversation", "source-turn")
        })
        .unwrap();
    assert_eq!(attachments.len(), 1);
    assert!(attachments[0].bytes.is_empty());
    let attachment_id = attachments[0].id;
    let context = alice
        .prepare_context_workflow(
            owner,
            "conversation",
            reliquary_application::ContextView {
                turns: vec![reliquary_application::ContextTurn {
                    message_id: "source-turn".into(),
                    role: reliquary_application::ContextRole::User,
                    content: "Use this file.".into(),
                }],
            },
            &[],
            &[crate::ProjectContextAttachmentRef {
                conversation_id: "conversation".into(),
                source_node_id: "source-turn".into(),
                attachment_id,
            }],
            &[&source],
            None,
            0,
            reliquary_application::CompactionBudget {
                trigger_tokens: 50,
                target_tokens: 40,
                raw_tail_tokens: 20,
                summary_tokens: 10,
                compaction_input_tokens: 80,
            },
            &ContextWords,
        )
        .unwrap();
    let context_request = alice.context_request(&context).unwrap();
    assert_eq!(context_request.messages.len(), 2);
    assert!(
        context_request.messages[1]
            .content
            .contains("source authority")
    );
    assert_eq!(
        alice
            .verify_project_attachment(owner, "conversation", "source-turn", attachment_id, &source)
            .unwrap()
            .bytes,
        b"source authority"
    );
    assert!(
        bob.verify_project_attachment(owner, "conversation", "source-turn", attachment_id, &source)
            .is_err()
    );
    source.revoke();
    assert!(alice.context_request(&context).is_err());
    assert!(
        alice
            .verify_project_attachment(owner, "conversation", "source-turn", attachment_id, &source)
            .is_err()
    );
    assert_eq!(
        alice
            .with_writable_reliquary(owner, |rel| rel.rel_turn_count())
            .unwrap(),
        1
    );
    drop(other_instance);
    drop(denied);
    drop(bob);
    drop(alice);
    drop(gateway);
    drop(queue);
    fs::write(repo.join("source.txt"), "edited later").unwrap();
    let gateway = Gateway::new();
    gateway.mount_reliquary(&path, &admission, [], []).unwrap();
    let alice = gateway.instance("alice", [owner], []).unwrap();
    let source = TrustedProjectSource::new("alice", owner, repository, &repo);
    let proof = alice
        .prepare_project_correlation(owner, request.clone(), &source)
        .unwrap();
    let mut queue = alice
        .open_conversation_queue(root.join("queue"), owner)
        .unwrap();
    assert_eq!(
        gateway
            .finalize_project_correlated_submission(
                &alice,
                &mut queue,
                &id,
                version,
                capture.clone(),
                vec![proof]
            )
            .unwrap(),
        committed
    );
    assert_eq!(
        alice
            .verify_project_attachment(owner, "conversation", "source-turn", attachment_id, &source)
            .unwrap()
            .bytes,
        b"source authority"
    );
    let mut changed = request;
    changed.path = "other.txt".into();
    changed.filename = "other.txt".into();
    changed.expected_content_hash = Sha256::digest(b"other source").into();
    let changed = alice
        .prepare_project_correlation(owner, changed, &source)
        .unwrap();
    assert!(
        gateway
            .finalize_project_correlated_submission(
                &alice,
                &mut queue,
                &id,
                version,
                capture,
                vec![changed]
            )
            .is_err()
    );
    assert_eq!(alice.reliquary(owner).unwrap().rel_turn_count(), 1);
    drop(alice);
    drop(gateway);
    drop(queue);
    #[cfg(windows)]
    make_fixture_writable(&root);
    fs::remove_dir_all(root).unwrap();
}
