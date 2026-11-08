use super::*;
use reliquary_container::{ContainerIdentity, ReliquaryScope};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(1);
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".owner-session-test-tmp");
        fs::create_dir_all(&root).unwrap();
        Self(root.join(format!(
            "owner-session-{}-{}.rel",
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
fn initialize_vectors(container: &mut Container) {
    PackedVectorStore::default().initialize(container).unwrap();
    CompatibilityProfileStore::default()
        .initialize(container)
        .unwrap();
    ArchiveVectorStore::default().initialize(container).unwrap();
    MemoryVectorStore::default().initialize(container).unwrap();
    VectorGenerationStore::empty()
        .initialize(container)
        .unwrap();
    container.sync().unwrap();
}

#[test]
fn open_import_epoch_fences_mutation_after_writer_reopen() {
    let path = Temp::new();
    let owner = [0x71; 16];
    drop(RelWriter::create_path(&path.0, ReliquaryScope::Project, owner).unwrap());
    let mut container = Container::open_writer(&path.0).unwrap();
    let mut imports = crate::records::ImportEpochStore::open(&container).unwrap();
    let key = "ab".repeat(32);
    let start = crate::records::ImportStart {
        format_version: 2,
        owner_uuid: owner,
        import_id: crate::records::import_id(owner, &key),
        import_key: key,
        source_namespace: "source/account".into(),
        source_format: "json-v1".into(),
        started_at_ns: 1,
        pre: crate::records::ImportWatermarks::default(),
    };
    imports.append_start(&mut container, start.clone()).unwrap();
    drop(container);
    let before = fs::read(&path.0).unwrap();
    let admission = GraphAdmission::for_replay(owner, [], [], [], [], []);
    for _ in 0..2 {
        let mut writer = RelWriter::open_path(&path.0, admission.clone(), [], []).unwrap();
        assert!(
            writer
                .read_session()
                .import_status(&start.import_id)
                .is_some()
        );
        assert!(
            matches!(writer.claim_next_insomnia_input("worker", 2, 10, 1),
            Err(OwnerSessionError::Record(message)) if message.contains("fenced by an open import epoch"))
        );
        drop(writer);
        assert_eq!(fs::read(&path.0).unwrap(), before);
    }
    use sha2::{Digest, Sha256};
    let mut container = Container::open_writer(&path.0).unwrap();
    let mut imports = crate::records::ImportEpochStore::open(&container).unwrap();
    let settlement = crate::records::ImportFreshnessSettlement {
        owner_uuid: owner,
        import_id: start.import_id.clone(),
        start_digest: format!("{:x}", Sha256::digest(serde_json::to_vec(&start).unwrap())),
        receipt_digest: crate::records::hex_digest(b"fixture-receipt"),
        prefix_digest: crate::records::import_settlement::source_prefix_digest(&container).unwrap(),
        rel_turn: 0,
        global_version: container.latest_version(),
        memory_version: start.pre.memory_version,
        graph_version: start.pre.graph_version,
        freshness_state_digest: crate::records::hex_digest(b"empty-freshness"),
        freshness_memory_count: 0,
        replay_source_authority: None,
    };
    imports
        .append_settlement(&mut container, settlement.clone())
        .unwrap();
    let completion_version = container.latest_version();
    imports
        .append_complete(
            &mut container,
            crate::records::ImportComplete {
                format_version: 2,
                owner_uuid: owner,
                import_id: start.import_id.clone(),
                start_digest: settlement.start_digest.clone(),
                receipt_digest: settlement.receipt_digest.clone(),
                completed_at_ns: 2,
                post: crate::records::ImportWatermarks {
                    global_version: completion_version,
                    ..start.pre
                },
                historical_freshness_settlement_digest: settlement.digest().unwrap(),
            },
        )
        .unwrap();
    crate::records::ImportEpochStore::open(&container).expect("epoch physical replay");
    drop(container);
    let mut writer = RelWriter::open_path(&path.0, admission, [], []).unwrap();
    assert!(
        writer
            .claim_next_insomnia_input("worker", 3, 10, 1)
            .unwrap()
            .is_none()
    );
}

#[test]
fn rel_writer_owns_the_exclusive_container_lease() {
    let path = Temp::new();
    let mut container = Container::create(
        &path.0,
        ContainerIdentity::reliquary(ReliquaryScope::Project),
        [9; 16],
    )
    .unwrap();
    initialize_vectors(&mut container);
    drop(container);

    let admission = GraphAdmission::for_replay([9; 16], [], [], [], [], []);
    let writer = RelWriter::open_path(&path.0, admission, [], []).unwrap();
    assert_eq!(writer.owner_id(), [9; 16]);
    assert!(Container::open_writer(&path.0).is_err());
    drop(writer);

    let reopened = Container::open_writer(&path.0).unwrap();
    drop(reopened);
}

#[test]
fn accepted_turn_and_branch_are_idempotent_after_writer_reopen() {
    let path = Temp::new();
    let owner = [19; 16];
    let mut container = Container::create(
        &path.0,
        ContainerIdentity::reliquary(ReliquaryScope::Project),
        owner,
    )
    .unwrap();
    initialize_vectors(&mut container);
    drop(container);

    let admission = GraphAdmission::for_replay(owner, [], [], [], [], []);
    let turn = || reliquary_archive::TurnInput {
        id: "submission-1".to_owned(),
        conversation_id: "conversation-1".to_owned(),
        parent_id: None,
        role: "user".to_owned(),
        principal_id: Some("alice".to_owned()),
        timestamp_ns: 42,
        content: "durable content".to_owned(),
        attachments: Vec::new(),
        project_attachments: Vec::new(),
    };
    let branch = || reliquary_archive::Branch {
        id: "main".to_owned(),
        conversation_id: "conversation-1".to_owned(),
        leaf_node_id: "submission-1".to_owned(),
        canonical: true,
    };

    let mut writer = RelWriter::open_path(&path.0, admission.clone(), [], []).unwrap();
    writer
        .accept_turn_and_advance_branch(turn(), branch())
        .unwrap();
    drop(writer);

    let mut recovered = RelWriter::open_path(&path.0, admission, [], []).unwrap();
    recovered
        .accept_turn_and_advance_branch(turn(), branch())
        .unwrap();
    assert_eq!(recovered.session.archive.node_count(), 1);
    assert_eq!(
        recovered
            .session
            .archive
            .branch("conversation-1", "main")
            .unwrap()
            .leaf_node_id,
        "submission-1"
    );
}

#[test]
fn rel_session_reopens_native_archive_with_source_keyed_echo() {
    let path = Temp::new();
    let owner = [23; 16];
    let mut container = Container::create(
        &path.0,
        ContainerIdentity::reliquary(ReliquaryScope::Project),
        owner,
    )
    .unwrap();
    initialize_vectors(&mut container);
    let mut archive = Archive::empty();
    archive
        .accept_turn(
            &mut container,
            reliquary_archive::TurnInput {
                id: "assistant-1".into(),
                conversation_id: "conversation-echo".into(),
                parent_id: None,
                role: "assistant".into(),
                principal_id: None,
                timestamp_ns: 42,
                content: "accepted answer".into(),
                attachments: Vec::new(),
                project_attachments: Vec::new(),
            },
        )
        .unwrap();
    let event = reliquary_archive::EchoEvent {
        conversation_id: "conversation-echo".into(),
        message_id: "assistant-1".into(),
        sequence: 1,
        timestamp_ns: 43,
        model_round: Some(2),
        kind: reliquary_archive::EchoEventKind::ReasoningSummary,
        correlation_id: None,
        name: None,
        content: "answer grounded in the accepted turn".into(),
    };
    reliquary_archive::EchoStore::default()
        .put(&mut container, event.clone())
        .unwrap();
    container.sync().unwrap();
    drop(archive);
    drop(container);

    let container = Container::open_read_only(&path.0).unwrap();
    let admission = GraphAdmission::for_replay(owner, [], [], [], [], []);
    let session = RelSession::open(&container, &admission, [], []).unwrap();
    assert_eq!(
        session
            .archive_node("conversation-echo", "assistant-1")
            .unwrap()
            .role,
        "assistant"
    );
    assert_eq!(
        session.echo_for_turn("conversation-echo", "assistant-1"),
        vec![event.clone()]
    );
    assert_eq!(
        session.echo_for_conversation("conversation-echo"),
        vec![event]
    );
}

#[test]
fn bounded_insomnia_claim_returns_archive_verified_input() {
    let path = Temp::new();
    let owner = [31; 16];
    let mut container = Container::create(
        &path.0,
        ContainerIdentity::reliquary(ReliquaryScope::Project),
        owner,
    )
    .unwrap();
    initialize_vectors(&mut container);
    let mut archive = Archive::empty();
    archive
        .accept_turn(
            &mut container,
            reliquary_archive::TurnInput {
                id: "user-1".into(),
                conversation_id: "conversation-1".into(),
                parent_id: None,
                role: "user".into(),
                principal_id: Some("alice".into()),
                timestamp_ns: 10,
                content: "remember this".into(),
                attachments: Vec::new(),
                project_attachments: Vec::new(),
            },
        )
        .unwrap();
    archive
        .accept_turn(
            &mut container,
            reliquary_archive::TurnInput {
                id: "assistant-1".into(),
                conversation_id: "conversation-1".into(),
                parent_id: Some("user-1".into()),
                role: "assistant".into(),
                principal_id: None,
                timestamp_ns: 20,
                content: "I will".into(),
                attachments: Vec::new(),
                project_attachments: Vec::new(),
            },
        )
        .unwrap();
    archive
        .put_branch(
            &mut container,
            reliquary_archive::Branch {
                id: "main".into(),
                conversation_id: "conversation-1".into(),
                leaf_node_id: "assistant-1".into(),
                canonical: true,
            },
        )
        .unwrap();
    let episode_id = archive
        .put_episode(
            &mut container,
            "conversation-1",
            "user-1",
            "assistant-1",
            reliquary_archive::EpisodeOrigin::Live,
            reliquary_archive::EpisodeBoundary::Explicit,
            20,
        )
        .unwrap();
    let episode = archive.episode(episode_id).unwrap().clone();
    container.sync().unwrap();
    drop(archive);
    drop(container);

    let order = AcceptedEpisodeOrder {
        episode_id: WorkEpisodeId(episode_id.0),
        source_through_ns: episode.source_through_ns,
        conversation_id: episode.conversation_id.clone(),
        start_node_id: episode.start_node_id.clone(),
    };
    let admission = GraphAdmission::for_replay(owner, [], [], [], [], []);
    let mut forged = order.clone();
    forged.source_through_ns += 1;
    let reader = Container::open_read_only(&path.0).unwrap();
    assert!(matches!(
        RelSession::open(&reader, &admission, [forged], []),
        Err(OwnerSessionError::Insomnia(
            InsomniaError::UnacceptedEpisode
        ))
    ));
    drop(reader);
    let mut writer = RelWriter::open_path(&path.0, admission, [], []).unwrap();
    let claim = writer
        .claim_next_insomnia_input("worker-1", 100, 1_000, 4)
        .unwrap()
        .unwrap();
    assert_eq!(claim.episode, episode);
    assert_eq!(claim.turns.len(), 2);
    assert_eq!(claim.turns[0].node_id, "user-1");
    assert_eq!(claim.work.lease_owner.as_deref(), Some("worker-1"));
    assert!(claim.work.lease_token.is_some());
    assert_eq!(claim.started_at_ns, 100);
}

#[test]
fn rel_and_phy_replay_through_distinct_typed_aggregates() {
    let rel_path = Temp::new();
    let mut owner = [0; 16];
    owner[0] = 1;
    let mut rel = Container::create(
        &rel_path.0,
        ContainerIdentity::reliquary(ReliquaryScope::Project),
        owner,
    )
    .unwrap();
    initialize_vectors(&mut rel);
    let mut archive = Archive::empty();
    archive
        .accept_turn(
            &mut rel,
            reliquary_archive::TurnInput {
                id: "assistant-1".into(),
                conversation_id: "conversation-1".into(),
                parent_id: None,
                role: "assistant".into(),
                principal_id: None,
                timestamp_ns: 12,
                content: "replayed turn".into(),
                attachments: vec![],
                project_attachments: vec![],
            },
        )
        .unwrap();
    rel.sync().unwrap();
    let graph = GraphAdmission::for_replay(owner, [], [], [], [], []);
    let rel_session = RelSession::open(&rel, &graph, [], []).unwrap();
    assert_eq!(rel_session.owner_id(), owner);
    assert_eq!(
        rel_session
            .archive_node("conversation-1", "assistant-1")
            .unwrap()
            .role,
        "assistant"
    );
    assert_eq!(rel_session.accepted_turn_sources().len(), 1);
    assert!(matches!(
        PhySession::open(&rel, &graph, []),
        Err(OwnerSessionError::WrongFileKind { .. })
    ));

    let phy_path = Temp::new();
    let mut phy = Container::create(&phy_path.0, ContainerIdentity::phylactery(), owner).unwrap();
    PackedVectorStore::default().initialize(&mut phy).unwrap();
    CompatibilityProfileStore::default()
        .initialize(&mut phy)
        .unwrap();
    MemoryVectorStore::default().initialize(&mut phy).unwrap();
    phy.sync().unwrap();
    let phy_session = PhySession::open(&phy, &graph, []).unwrap();
    assert_eq!(phy_session.owner_id(), owner);
    assert_eq!(phy_session.graph_version(), 0);
    assert!(
        phy_session
            .memory_history(MemoryId([8; 32]))
            .unwrap()
            .is_none()
    );
    assert!(phy_session.entity(EntityId([8; 32])).is_none());
    assert!(phy_session.memory_vector_infos().is_empty());
    assert!(matches!(
        RelSession::open(&phy, &graph, [], []),
        Err(OwnerSessionError::WrongFileKind { .. })
    ));
}

#[test]
fn phy_private_history_and_clock_reopen_without_rel_or_principal() {
    let rel_path = Temp::new();
    let phy_path = Temp::new();
    let owner = [0x39; 16];
    drop(RelWriter::create_path(&rel_path.0, ReliquaryScope::Project, owner).unwrap());
    drop(PhyWriter::create_path(&phy_path.0, owner).unwrap());
    let identity = EgoIdentityId([0x18; 16]);
    let mut phy = Container::open_writer(&phy_path.0).unwrap();
    let mut ego = EgoStore::open(&phy).unwrap();
    ego.create_identity(
        &mut phy,
        identity,
        "private".into(),
        "private owner fact".into(),
    )
    .unwrap();
    phy.reserve_version(Some(5)).unwrap();
    phy.sync().unwrap();
    drop(ego);
    drop(phy);
    let graph = GraphAdmission::for_replay(owner, [], [], [], [], []);
    let phy = PhySession::open_path(&phy_path.0, &graph, []).unwrap();
    assert_eq!(
        phy.ego_identity(identity).unwrap().text,
        "private owner fact"
    );
    let rel = RelSession::open_path(&rel_path.0, &graph, [], []).unwrap();
    assert!(rel.ego_identity(identity).is_none());
    let phy_file = Container::open_read_only(&phy_path.0).unwrap();
    let rel_file = Container::open_read_only(&rel_path.0).unwrap();
    assert_eq!(phy_file.latest_version(), 1);
    assert_eq!(rel_file.latest_version(), 0);
}

#[test]
fn phy_rejects_rel_only_record_families_without_mutation() {
    for marker in [
        b"RELTURN2",
        b"RELFILE1",
        b"RELBRCH1",
        b"RELEPIS1",
        b"RELECHO1",
        b"RELFRS02",
        b"RELFRP01",
        b"RELIEPS1",
        b"RELIEPC1",
        b"RELIEPS2",
        b"RELIEPC2",
        b"RELIFRS1",
        b"RELIPLN1",
        b"CVAIMPT1",
        b"CVAFRG01",
        b"RELINSW1",
        b"RELINSR1",
        b"RELAVFM1",
        b"RELAVEC1",
        b"RELVGFM1",
        b"RELVGEN1",
        b"RELVGRC1",
        b"RELXIDX2",
    ] {
        let path = Temp::new();
        let owner = [7; 16];
        let mut phy = Container::create(&path.0, ContainerIdentity::phylactery(), owner).unwrap();
        phy.append(marker).unwrap();
        phy.sync().unwrap();
        drop(phy);
        let before = fs::read(&path.0).unwrap();
        let phy = Container::open_read_only(&path.0).unwrap();
        let admission = GraphAdmission::for_replay(owner, [], [], [], [], []);
        assert!(matches!(
            PhySession::open(&phy, &admission, []),
            Err(OwnerSessionError::RelOnlyRecordInPhylactery(_))
        ));
        drop(phy);
        assert_eq!(fs::read(&path.0).unwrap(), before);
    }
}

#[test]
fn graph_replay_requires_owner_scoped_closure_admission() {
    let path = Temp::new();
    let mut owner = [0; 16];
    owner[0] = 2;
    let mut container = Container::create(
        &path.0,
        ContainerIdentity::reliquary(ReliquaryScope::Project),
        owner,
    )
    .unwrap();
    initialize_vectors(&mut container);
    let foreign = GraphAdmission::for_replay([9; 16], [], [], [], [], []);
    assert!(matches!(
        RelSession::open(&container, &foreign, [], []),
        Err(OwnerSessionError::Graph(_))
    ));
}

#[test]
fn session_rejects_graph_endpoints_forged_by_caller_admission() {
    let path = Temp::new();
    let owner = [0x53; 16];
    let mut container = Container::create(
        &path.0,
        ContainerIdentity::reliquary(ReliquaryScope::Project),
        owner,
    )
    .unwrap();
    initialize_vectors(&mut container);
    let first = [1; 32];
    let second = [2; 32];
    let admission = GraphAdmission::for_changes(owner, [first, second], []);
    let mut graph = GraphStore::open(&container, &admission).unwrap();
    graph
        .put(
            &mut container,
            &admission,
            &[reliquary_graph::RelationChange {
                source: NodeRef::Memory(first),
                target: NodeRef::Memory(second),
                kind: reliquary_graph::RelationKind::Topical,
                active: true,
                origin: reliquary_graph::RelationOrigin::User,
            }],
        )
        .unwrap();
    let before = container.read_frames().unwrap().len();
    assert!(matches!(
        RelSession::open(&container, &admission, [], []),
        Err(OwnerSessionError::Graph(GraphError::InvalidClosure(_)))
    ));
    assert_eq!(container.read_frames().unwrap().len(), before);
}

#[test]
fn session_rejects_mention_of_missing_owner_memory() {
    let path = Temp::new();
    let owner = [0x54; 16];
    let mut container = Container::create(
        &path.0,
        ContainerIdentity::reliquary(ReliquaryScope::Project),
        owner,
    )
    .unwrap();
    initialize_vectors(&mut container);
    let mut perception = PerceptionStore::open(&container).unwrap();
    perception
        .put_mention_resolution(
            &mut container,
            MemoryEntityMentionKey {
                memory_id: [9; 32],
                field: reliquary_perception_store::MemoryTextField::Content,
                start_byte: 0,
                end_byte: 1,
            },
            0,
            1,
            reliquary_perception_store::MemoryEntityResolutionStatus::Unresolved {
                reason: reliquary_perception_store::EntityResolutionReason::InsufficientEvidence,
            },
        )
        .unwrap();
    let before = container.read_frames().unwrap().len();
    let admission = GraphAdmission::for_replay(owner, [], [], [], [], []);
    assert!(matches!(
        RelSession::open(&container, &admission, [], []),
        Err(OwnerSessionError::Record(_))
    ));
    assert_eq!(container.read_frames().unwrap().len(), before);
}

#[test]
fn new_rel_and_phy_writers_initialize_and_reopen_their_owners() {
    let rel_path = Temp::new();
    let rel_owner = [0x61; 16];
    let mut writer =
        RelWriter::create_path(&rel_path.0, ReliquaryScope::Project, rel_owner).unwrap();
    assert_eq!(writer.owner_id(), rel_owner);
    writer
        .accept_turn_and_advance_branch(
            reliquary_archive::TurnInput {
                id: "new-turn".into(),
                conversation_id: "new-conversation".into(),
                parent_id: None,
                role: "user".into(),
                principal_id: Some("principal".into()),
                timestamp_ns: 10,
                content: "persisted".into(),
                attachments: vec![],
                project_attachments: vec![],
            },
            reliquary_archive::Branch {
                id: "main".into(),
                conversation_id: "new-conversation".into(),
                leaf_node_id: "new-turn".into(),
                canonical: true,
            },
        )
        .unwrap();
    assert_eq!(
        writer.session.writer_lease_proof,
        writer.container.writer_lease_proof()
    );
    let before_invalid = writer.container.read_frames().unwrap().len();
    assert!(
        writer
            .finalize_episode(
                "new-conversation",
                "missing",
                "new-turn",
                reliquary_archive::EpisodeOrigin::Live,
                reliquary_archive::EpisodeBoundary::Explicit,
                11
            )
            .is_err()
    );
    assert_eq!(
        writer.container.read_frames().unwrap().len(),
        before_invalid
    );
    let episode = writer
        .finalize_episode(
            "new-conversation",
            "new-turn",
            "new-turn",
            reliquary_archive::EpisodeOrigin::Live,
            reliquary_archive::EpisodeBoundary::Explicit,
            11,
        )
        .unwrap();
    let before_retry = writer.container.read_frames().unwrap().len();
    assert_eq!(
        writer
            .finalize_episode(
                "new-conversation",
                "new-turn",
                "new-turn",
                reliquary_archive::EpisodeOrigin::Live,
                reliquary_archive::EpisodeBoundary::Explicit,
                11
            )
            .unwrap(),
        episode
    );
    assert_eq!(writer.container.read_frames().unwrap().len(), before_retry);
    let claim = writer
        .claim_next_insomnia_input("worker", 12, 100, 1)
        .unwrap()
        .unwrap();
    assert_eq!(claim.episode.id, episode);
    assert!(RelWriter::create_path(&rel_path.0, ReliquaryScope::Project, rel_owner).is_err());
    drop(writer);
    let admission = GraphAdmission::for_replay(rel_owner, [], [], [], [], []);
    let reopened = RelWriter::open_path(&rel_path.0, admission, [], []).unwrap();
    assert_eq!(reopened.read_session().memory_version(), 0);
    assert_eq!(
        reopened
            .read_session()
            .archive_node("new-conversation", "new-turn")
            .unwrap()
            .role,
        "user"
    );
    assert_eq!(
        reopened
            .read_session()
            .insomnia_work(WorkEpisodeId(episode.0))
            .unwrap()
            .state,
        reliquary_insomnia_store::InsomniaWorkState::Processing
    );
    drop(reopened);
    assert!(
        PhySession::open_path(
            &rel_path.0,
            &GraphAdmission::for_replay(rel_owner, [], [], [], [], []),
            []
        )
        .is_err()
    );

    let phy_path = Temp::new();
    let phy_owner = [0x62; 16];
    let writer = PhyWriter::create_path(&phy_path.0, phy_owner).unwrap();
    assert_eq!(writer.owner_id(), phy_owner);
    assert!(PhyWriter::create_path(&phy_path.0, phy_owner).is_err());
    drop(writer);
    let admission = GraphAdmission::for_replay(phy_owner, [], [], [], [], []);
    let reopened = PhyWriter::open_path(&phy_path.0, &admission, []).unwrap();
    assert_eq!(reopened.owner_id(), phy_owner);
    assert!(
        reopened
            .memory_history(MemoryId([0; 32]))
            .unwrap()
            .is_none()
    );
    drop(reopened);
    let file = Container::open_read_only(&phy_path.0).unwrap();
    assert!(matches!(
        RelSession::open(&file, &admission, [], []),
        Err(OwnerSessionError::WrongFileKind { .. })
    ));
}

#[test]
fn rel_open_rejects_caller_fabricated_episode_admission() {
    let path = Temp::new();
    let owner = [0x64; 16];
    let writer = RelWriter::create_path(&path.0, ReliquaryScope::Project, owner).unwrap();
    let admission = GraphAdmission::for_replay(owner, [], [], [], [], []);
    let forged = AcceptedEpisodeOrder {
        episode_id: WorkEpisodeId([0x77; 32]),
        source_through_ns: 1,
        conversation_id: "missing".into(),
        start_node_id: "missing".into(),
    };
    let before = writer.container.read_frames().unwrap().len();
    assert!(matches!(
        RelSession::open(&writer.container, &admission, [forged], []),
        Err(OwnerSessionError::Insomnia(
            InsomniaError::UnacceptedEpisode
        ))
    ));
    assert_eq!(writer.container.read_frames().unwrap().len(), before);
}

#[test]
fn poisoned_rel_writer_rejects_other_mutations_until_reopen() {
    let path = Temp::new();
    let mut writer = RelWriter::create_path(&path.0, ReliquaryScope::Project, [0x65; 16]).unwrap();
    assert!(
        writer
            .finish_insomnia_write::<()>(Err(InsomniaError::Container(
                ContainerError::WriteRecoveryRequired
            )))
            .is_err()
    );
    let before = writer.container.read_frames().unwrap().len();
    assert!(matches!(
        writer.claim_next_insomnia_input("worker", 1, 10, 1),
        Err(OwnerSessionError::Record(_))
    ));
    assert!(matches!(
        writer.queue_and_claim_insomnia_episode(
            reliquary_archive::EpisodeId([1; 32]),
            "worker",
            1,
            10
        ),
        Err(OwnerSessionError::Record(_))
    ));
    assert_eq!(writer.container.read_frames().unwrap().len(), before);
}

#[test]
fn dream_admission_cannot_name_absent_owner_history() {
    let path = Temp::new();
    let owner = [0x67; 16];
    let writer = RelWriter::create_path(&path.0, ReliquaryScope::Project, owner).unwrap();
    let graph = GraphAdmission::for_replay(owner, [], [], [], [], []);
    let admission = DreamBeginAdmission {
        owner_uuid: owner,
        pass_id: "forged".into(),
        source_id: MemoryId([9; 32]),
        turn: 1,
        graph_version_before: 0,
        preexisting_ids: Default::default(),
        memory_cut: 0,
    };
    let before = writer.container.read_frames().unwrap().len();
    assert!(matches!(
        RelSession::open(&writer.container, &graph, [], [admission]),
        Err(OwnerSessionError::Record(_))
    ));
    assert_eq!(writer.container.read_frames().unwrap().len(), before);
}
