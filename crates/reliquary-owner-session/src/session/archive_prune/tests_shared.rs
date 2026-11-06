use super::tests::Fixture;
use super::*;
use reliquary_archive::{Branch, TurnInput};
use reliquary_container::ReliquaryScope;

#[test]
fn terminal_archive_prune_refuses_shared_text_and_file_content() {
    use reliquary_archive::AttachmentInput;
    for through_file in [false, true] {
        let source = Fixture::new("shared-source");
        let output = Fixture::new("shared-output");
        let owner = [184; 16];
        let grant = GraphAdmission::for_replay(owner, [], [], [], [], []);
        let text = "shared text is still needed".repeat(20);
        let mut writer = RelWriter::create_path(&source.0, ReliquaryScope::Project, owner).unwrap();
        for (conversation, content, file) in [
            ("terminal", text.clone(), false),
            (
                "retained",
                if through_file {
                    "other text".into()
                } else {
                    text.clone()
                },
                through_file,
            ),
        ] {
            writer
                .accept_turn_and_advance_branch(
                    TurnInput {
                        id: "node".into(),
                        conversation_id: conversation.into(),
                        parent_id: None,
                        role: "user".into(),
                        principal_id: None,
                        timestamp_ns: 1,
                        content,
                        attachments: if file {
                            vec![AttachmentInput {
                                filename: "shared.txt".into(),
                                mime_type: None,
                                bytes: text.as_bytes().to_vec(),
                            }]
                        } else {
                            Vec::new()
                        },
                        project_attachments: Vec::new(),
                    },
                    Branch {
                        id: "main".into(),
                        conversation_id: conversation.into(),
                        leaf_node_id: "node".into(),
                        canonical: true,
                    },
                )
                .unwrap();
        }
        writer.tombstone_conversation("terminal").unwrap();
        drop(writer);
        let before = fs::read(&source.0).unwrap();
        let plan = VerifiedOwnerCopyPlan::prepare(&source.0).unwrap();
        let err =
            prune_terminal_archive_copy(&plan, &source.0, &output.0, &["terminal".into()], &grant)
                .unwrap_err();
        assert!(
            err.contains("shared by retained turns or file bytes"),
            "{err}"
        );
        assert!(!output.0.exists());
        assert_eq!(fs::read(&source.0).unwrap(), before);
    }
}
