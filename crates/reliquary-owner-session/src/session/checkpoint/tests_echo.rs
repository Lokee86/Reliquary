use super::*;
use crate::session::RelWriter;
use reliquary_archive::{EchoEvent, EchoEventKind};

#[test]
fn echo_checkpoint_keeps_verified_positions_and_restores_evidence_on_demand() {
    let directory = tempfile::tempdir().unwrap();
    let owner = directory.path().join("echo.rel");
    let private = directory.path().join("private");
    private_cache::test_directory(&private).unwrap();
    let slot = private.join("owner.checkpoint");
    let owner_id = [88; 16];
    let mut initial = RelWriter::create_current_path(&owner, owner_id).unwrap();
    let event = EchoEvent {
        conversation_id: "source-conversation".into(),
        message_id: "source-message".into(),
        sequence: 4,
        timestamp_ns: 17,
        model_round: None,
        kind: EchoEventKind::ReasoningSummary,
        correlation_id: None,
        name: None,
        content: "echo payload lives in the owner file ".repeat(4096),
    };
    initial
        .session
        .echo
        .put(&mut initial.container, event.clone())
        .unwrap();
    drop(initial);

    let lease =
        Container::prepare_checkpoint_open(&owner, true, FileKind::Reliquary, Some(owner_id))
            .unwrap();
    let extent = lease.extent();
    let (container, session, checkpoint) =
        open_leased(lease, Some(slot.clone()), extent, true).unwrap();
    assert!(!slot.exists());
    assert_eq!(
        session
            .echo_for_turn("source-conversation", "source-message")
            .unwrap(),
        [event.clone()]
    );
    let writer = RelWriter {
        container,
        session,
        checkpoint,
        verified_dream_import: None,
    };
    drop(writer);
    let checkpoint_size = std::fs::metadata(&slot).unwrap().len();
    assert!(checkpoint_size < event.content.len() as u64 / 4);

    let lease =
        Container::prepare_checkpoint_open(&owner, true, FileKind::Reliquary, Some(owner_id))
            .unwrap();
    let (container, session, checkpoint) = open_leased(lease, Some(slot), extent, true).unwrap();
    assert_eq!(checkpoint.unwrap().attempted_extent, Some(extent));
    assert_eq!(
        session
            .echo_for_conversation("source-conversation")
            .unwrap(),
        [event.clone()]
    );
    assert!(session.echo_for_conversation("absent").unwrap().is_empty());
    drop(container);
    assert!(
        session
            .echo_for_turn("source-conversation", "source-message")
            .is_err()
    );
    let detached = RelSession::open_path_current(&owner).unwrap();
    assert_eq!(
        detached
            .echo_for_turn("source-conversation", "source-message")
            .unwrap(),
        [event]
    );
}
