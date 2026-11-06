use super::*;
use reliquary_archive::{Branch, TurnInput};
use reliquary_container::ReliquaryScope;
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(1);

struct Temp(PathBuf);

impl Temp {
    fn new(kind: &str) -> Self {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".owner-session-test-tmp");
        fs::create_dir_all(&root).unwrap();
        Self(root.join(format!(
            "historical-{kind}-{}-{}.rel",
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

fn turn(id: &str, parent: Option<&str>, role: &str, timestamp_ns: i64) -> TurnInput {
    TurnInput {
        id: id.into(),
        conversation_id: "history".into(),
        parent_id: parent.map(str::to_owned),
        role: role.into(),
        principal_id: (role == "user").then(|| "principal".into()),
        timestamp_ns,
        content: format!("retained {id}"),
        attachments: vec![],
        project_attachments: vec![],
    }
}

fn branch(leaf: &str) -> Branch {
    Branch {
        id: "main".into(),
        conversation_id: "history".into(),
        leaf_node_id: leaf.into(),
        canonical: true,
    }
}

#[test]
fn rel_proofs_are_stable_across_early_mid_terminal_and_restart_cuts() {
    let path = Temp::new("rel");
    let owner = [0x91; 16];
    let mut writer = RelWriter::create_path(&path.0, ReliquaryScope::Project, owner).unwrap();

    writer
        .accept_turn_and_advance_branch(turn("user-1", None, "user", 10), branch("user-1"))
        .unwrap();
    let early_cut = writer.read_session().current_historical_cut();
    let early = writer
        .read_session()
        .historical_proof(owner, &early_cut)
        .unwrap();

    writer
        .accept_turn_and_advance_branch(
            turn("assistant-1", Some("user-1"), "assistant", 20),
            branch("assistant-1"),
        )
        .unwrap();
    let mid_cut = writer.read_session().current_historical_cut();
    let mid = writer
        .read_session()
        .historical_proof(owner, &mid_cut)
        .unwrap();
    assert_ne!(early.digest, mid.digest);
    assert!(mid.retained_records > early.retained_records);

    writer.tombstone_conversation("history").unwrap();
    let terminal_cut = writer.read_session().current_historical_cut();
    let terminal = writer
        .read_session()
        .historical_proof(owner, &terminal_cut)
        .unwrap();
    assert_ne!(mid.digest, terminal.digest);
    assert_eq!(
        writer
            .read_session()
            .historical_proof(owner, &mid_cut)
            .unwrap(),
        mid
    );
    assert!(matches!(
        writer
            .read_session()
            .historical_proof([0x92; 16], &terminal_cut),
        Err(HistoricalProofError::WrongOwner)
    ));
    let mut future = terminal_cut.clone();
    future.archive_version += 1;
    assert!(matches!(
        writer.read_session().historical_proof(owner, &future),
        Err(HistoricalProofError::FutureCut("Archive"))
    ));
    let mut ambiguous = terminal_cut.clone();
    ambiguous.freshness.event_id_inclusive = Some("not-an-accepted-event".into());
    assert!(matches!(
        writer.read_session().historical_proof(owner, &ambiguous),
        Err(HistoricalProofError::InconsistentCut(_))
    ));
    drop(writer);

    let admission = GraphAdmission::for_replay(owner, [], [], [], [], []);
    let reopened = RelSession::open_path(&path.0, &admission, [], []).unwrap();
    assert_eq!(reopened.historical_proof(owner, &early_cut).unwrap(), early);
    assert_eq!(reopened.historical_proof(owner, &mid_cut).unwrap(), mid);
    assert_eq!(
        reopened.historical_proof(owner, &terminal_cut).unwrap(),
        terminal
    );
}

#[test]
fn phy_proofs_keep_private_ego_history_and_typed_boundaries() {
    let path = Temp::new("phy");
    let owner = [0xa1; 16];
    let identity = EgoIdentityId([0x33; 16]);
    let mut writer = PhyWriter::create_path(&path.0, owner).unwrap();
    writer
        .common
        .ego
        .create_identity(
            &mut writer.container,
            identity,
            "private".into(),
            "retained owner-only fact".into(),
        )
        .unwrap();
    drop(writer);

    let admission = GraphAdmission::for_replay(owner, [], [], [], [], []);
    let session = PhySession::open_path(&path.0, &admission, []).unwrap();
    let live_cut = session.current_historical_cut();
    let live = session.historical_proof(owner, live_cut).unwrap();
    assert_eq!(live.file_kind, FileKind::Phylactery);
    assert_eq!(live_cut.ego_version, 1);
    drop(session);

    let mut writer = PhyWriter::open_path(&path.0, &admission, []).unwrap();
    writer
        .common
        .ego
        .delete_identity(&mut writer.container, identity, 1)
        .unwrap();
    drop(writer);

    let session = PhySession::open_path(&path.0, &admission, []).unwrap();
    assert_eq!(session.historical_proof(owner, live_cut).unwrap(), live);
    let deleted_cut = session.current_historical_cut();
    let deleted = session.historical_proof(owner, deleted_cut).unwrap();
    assert_ne!(live.digest, deleted.digest);
    assert!(matches!(
        session.historical_proof([0xa2; 16], deleted_cut),
        Err(HistoricalProofError::WrongOwner)
    ));
    assert!(matches!(
        session.historical_proof(
            owner,
            HistoricalOwnerCut {
                vector_version: 1,
                ..deleted_cut
            }
        ),
        Err(HistoricalProofError::FutureCut("vector"))
    ));
}
