use super::*;
use reliquary_archive::{Branch, TurnInput};
use reliquary_container::ReliquaryScope;
use reliquary_perception_store::{
    EntityDraft, RelationshipDraft, RelationshipEntityRef, RelationshipParticipant,
};
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

fn proof_memory(mutation_id: &str) -> MemoryDraft {
    MemoryDraft {
        title: mutation_id.into(),
        content: format!("content for {mutation_id}"),
        category: "fact".into(),
        memory_type: "semantic".into(),
        authority_kind: "direct".into(),
        temporal_status: "current".into(),
        scope: "private".into(),
        lifecycle_state: "knowledge".into(),
        archived: false,
        parent_id: None,
        superseded_by: None,
        source_node_id: None,
        content_source_conversation_id: None,
        content_source_node_id: None,
        grounding_source_conversation_id: None,
        grounding_source_node_id: None,
        source_episode_id: None,
        source_ref: None,
        source_time_ns: None,
        temporal_inference: None,
        mutation_id: mutation_id.into(),
        created_at_ns: 1,
        updated_at_ns: 1,
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

#[test]
fn phy_proof_distinguishes_converged_relationship_revision_histories() {
    let left = Temp::new("relationship-left");
    let right = Temp::new("relationship-right");
    let owner = [0xb1; 16];
    for (path, middle) in [(&left.0, "middle-left"), (&right.0, "middle-right")] {
        let mut writer = PhyWriter::create_path(path, owner).unwrap();
        let entity = writer
            .common
            .perception
            .publish_entity(
                &mut writer.container,
                EntityDraft {
                    canonical_name: "Subject".into(),
                    aliases: vec![],
                    kind: "person".into(),
                    summary: "stable entity".into(),
                    mutation_id: "subject-create".into(),
                    created_at_ns: 1,
                    updated_at_ns: 1,
                },
            )
            .unwrap();
        let draft = |mutation: &str, summary: &str, updated_at_ns| RelationshipDraft {
            kind: "knows".into(),
            participants: vec![RelationshipParticipant {
                entity: RelationshipEntityRef {
                    owner_id: "local".into(),
                    entity_id: entity.id,
                },
                role: Some("subject".into()),
            }],
            evidence: vec![],
            summary: summary.into(),
            mutation_id: mutation.into(),
            created_at_ns: 1,
            updated_at_ns,
        };
        let relationship = writer
            .common
            .perception
            .publish_relationship(
                &mut writer.container,
                draft("relationship-create", "initial", 1),
            )
            .unwrap();
        writer
            .common
            .perception
            .update_relationship(
                &mut writer.container,
                relationship.id,
                1,
                draft("relationship-middle", middle, 2),
            )
            .unwrap();
        writer
            .common
            .perception
            .update_relationship(
                &mut writer.container,
                relationship.id,
                2,
                draft("relationship-final", "same final", 3),
            )
            .unwrap();
    }
    let admission = GraphAdmission::for_replay(owner, [], [], [], [], []);
    let a = PhySession::open_path(&left.0, &admission, []).unwrap();
    let b = PhySession::open_path(&right.0, &admission, []).unwrap();
    let cut = a.current_historical_cut();
    assert_eq!(cut, b.current_historical_cut());
    assert_ne!(
        a.historical_proof(owner, cut).unwrap().digest,
        b.historical_proof(owner, cut).unwrap().digest
    );
}

#[test]
fn phy_proof_binds_cross_identity_perception_acceptance_order() {
    let left = Temp::new("entity-order-left");
    let right = Temp::new("entity-order-right");
    let owner = [0xb4; 16];
    for (path, order) in [
        (&left.0, ["entity-a", "entity-b"]),
        (&right.0, ["entity-b", "entity-a"]),
    ] {
        let mut writer = PhyWriter::create_path(path, owner).unwrap();
        for mutation in order {
            writer
                .common
                .perception
                .publish_entity(
                    &mut writer.container,
                    EntityDraft {
                        canonical_name: mutation.into(),
                        aliases: vec![],
                        kind: "concept".into(),
                        summary: format!("stable {mutation}"),
                        mutation_id: mutation.into(),
                        created_at_ns: 1,
                        updated_at_ns: 1,
                    },
                )
                .unwrap();
        }
    }
    let admission = GraphAdmission::for_replay(owner, [], [], [], [], []);
    let a = PhySession::open_path(&left.0, &admission, []).unwrap();
    let b = PhySession::open_path(&right.0, &admission, []).unwrap();
    let cut = a.current_historical_cut();
    assert_eq!(cut, b.current_historical_cut());
    assert_ne!(
        a.historical_proof(owner, cut).unwrap().digest,
        b.historical_proof(owner, cut).unwrap().digest
    );
}

#[test]
fn phy_proof_distinguishes_converged_graph_publication_histories() {
    let left = Temp::new("graph-left");
    let right = Temp::new("graph-right");
    let owner = [0xb2; 16];
    for (path, middle_active) in [(&left.0, false), (&right.0, true)] {
        let mut writer = PhyWriter::create_path(path, owner).unwrap();
        let first = writer
            .common
            .memories
            .publish_versioned_creation(&mut writer.container, proof_memory("graph-first"), 1)
            .unwrap()
            .revision
            .record
            .id;
        let second = writer
            .common
            .memories
            .publish_versioned_creation(&mut writer.container, proof_memory("graph-second"), 2)
            .unwrap()
            .revision
            .record
            .id;
        let admission = GraphAdmission::for_changes(owner, [first.0, second.0], []);
        let change = |active| reliquary_graph::RelationChange {
            source: NodeRef::Memory(first.0),
            target: NodeRef::Memory(second.0),
            kind: reliquary_graph::RelationKind::Topical,
            active,
            origin: reliquary_graph::RelationOrigin::User,
        };
        writer
            .common
            .graph
            .put(&mut writer.container, &admission, &[change(true)])
            .unwrap();
        writer
            .common
            .graph
            .put(&mut writer.container, &admission, &[change(middle_active)])
            .unwrap();
        writer
            .common
            .graph
            .put(&mut writer.container, &admission, &[change(true)])
            .unwrap();
    }
    let first = reliquary_memory_core::memory_id("graph-first");
    let second = reliquary_memory_core::memory_id("graph-second");
    let admission = GraphAdmission::for_replay(owner, [first.0, second.0], [], [], [], []);
    let a = PhySession::open_path(&left.0, &admission, []).unwrap();
    let b = PhySession::open_path(&right.0, &admission, []).unwrap();
    let cut = a.current_historical_cut();
    assert_eq!(cut, b.current_historical_cut());
    assert_ne!(
        a.historical_proof(owner, cut).unwrap().digest,
        b.historical_proof(owner, cut).unwrap().digest
    );
}

#[test]
fn phy_proof_distinguishes_converged_ego_revision_histories_and_reopens_stably() {
    let left = Temp::new("ego-left");
    let right = Temp::new("ego-right");
    let owner = [0xb3; 16];
    let identity = EgoIdentityId([0x45; 16]);
    for (path, middle) in [(&left.0, "middle-left"), (&right.0, "middle-right")] {
        let mut writer = PhyWriter::create_path(path, owner).unwrap();
        writer
            .common
            .ego
            .create_identity(
                &mut writer.container,
                identity,
                "private".into(),
                "initial".into(),
            )
            .unwrap();
        writer
            .common
            .ego
            .update_identity(
                &mut writer.container,
                identity,
                1,
                "private".into(),
                middle.into(),
            )
            .unwrap();
        writer
            .common
            .ego
            .update_identity(
                &mut writer.container,
                identity,
                2,
                "private".into(),
                "same final".into(),
            )
            .unwrap();
    }
    let admission = GraphAdmission::for_replay(owner, [], [], [], [], []);
    let a = PhySession::open_path(&left.0, &admission, []).unwrap();
    let b = PhySession::open_path(&right.0, &admission, []).unwrap();
    let cut = a.current_historical_cut();
    assert_eq!(cut, b.current_historical_cut());
    let proof_a = a.historical_proof(owner, cut).unwrap();
    let proof_b = b.historical_proof(owner, cut).unwrap();
    assert_ne!(proof_a.digest, proof_b.digest);
    drop(a);
    let reopened = PhySession::open_path(&left.0, &admission, []).unwrap();
    assert_eq!(reopened.historical_proof(owner, cut).unwrap(), proof_a);
}
#[test]
fn rel_proof_binds_intermediate_archive_branch_revisions() {
    let left = Temp::new("branch-history-left");
    let right = Temp::new("branch-history-right");
    let owner = [0xb5; 16];
    for (path, middle) in [(&left.0, "b"), (&right.0, "c")] {
        drop(RelWriter::create_path(path, ReliquaryScope::Project, owner).unwrap());
        let mut container = Container::open_writer_as(path, FileKind::Reliquary).unwrap();
        let mut archive = Archive::open(&container).unwrap();
        for (id, parent) in [
            ("a", None),
            ("b", Some("a")),
            ("c", Some("b")),
            ("d", Some("c")),
        ] {
            archive
                .accept_turn(&mut container, turn(id, parent, "user", 10))
                .unwrap();
        }
        for leaf in ["a", middle, "d"] {
            archive.put_branch(&mut container, branch(leaf)).unwrap();
        }
        container.sync().unwrap();
    }

    let grant = GraphAdmission::for_replay(owner, [], [], [], [], []);
    let first = RelSession::open_path(&left.0, &grant, [], []).unwrap();
    let second = RelSession::open_path(&right.0, &grant, [], []).unwrap();
    let cut = first.current_historical_cut();
    assert_eq!(cut, second.current_historical_cut());
    assert_eq!(
        first.archive.branch("history", "main"),
        second.archive.branch("history", "main")
    );
    assert_ne!(
        first.historical_proof(owner, &cut).unwrap().digest,
        second.historical_proof(owner, &cut).unwrap().digest
    );
}
