use super::tests_negative::Fixture;
use super::*;
use reliquary_archive::{Branch, EpisodeBoundary, EpisodeOrigin, TurnInput};
use reliquary_container::ReliquaryScope;
use std::io::Read;

#[test]
fn terminal_archive_tombstone_fences_writes_and_replay() {
    let source = Fixture::new("fenced-source");
    let forged = Fixture::new("fenced-forged");
    let owner = [190; 16];
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
                content: "original".into(),
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
    assert!(writer.tombstone_conversation("terminal").unwrap());
    let before = writer.container.latest_version();
    assert!(
        writer
            .finalize_episode(
                "terminal",
                "turn-1",
                "turn-1",
                EpisodeOrigin::Live,
                EpisodeBoundary::Explicit,
                2,
            )
            .is_err()
    );
    assert_eq!(writer.container.latest_version(), before);
    drop(writer);

    let opened = Container::open_read_only(&source.0).unwrap();
    let mut target = Container::create(&forged.0, opened.identity(), opened.owner_id()).unwrap();
    let mut cursor = opened.physical_cursor().unwrap();
    let mut original_turn = None;
    while let Some(mut frame) = cursor.next_frame().unwrap() {
        let mut bytes = Vec::new();
        frame.read_to_end(&mut bytes).unwrap();
        if bytes.starts_with(b"RELTURN2") {
            original_turn = Some(bytes.clone());
        }
        target.append(&bytes).unwrap();
    }
    target.append(&original_turn.unwrap()).unwrap();
    target.sync().unwrap();
    drop(target);
    assert!(RelSession::open_path(&source.0, &admission, [], []).is_ok());
    assert!(RelSession::open_path(&forged.0, &admission, [], []).is_err());
}
