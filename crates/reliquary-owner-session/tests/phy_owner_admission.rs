use reliquary_archive::{Archive, EpisodeBoundary, EpisodeOrigin, TurnInput};
use reliquary_container::{Container, ContainerIdentity, ReliquaryScope};
use reliquary_memory_core::{MemoryDraft, MemorySourceRef};
use reliquary_owner_session::{
    GraphAdmission, OwnerSessionError, PhySession, PhyWriter, RelSession, RelWriter,
};
use reliquary_vector_store::{
    ArchiveVectorStore, CompatibilityProfileStore, MemoryVectorStore, PackedVectorStore,
    VectorGenerationStore,
};
use sha2::{Digest, Sha256};
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
            "phy-rel-record-{}-{}.phy",
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

fn external_memory_record() -> Vec<u8> {
    let mut payload = b"RELXMEM2".to_vec();
    payload.extend_from_slice(&0_u64.to_le_bytes()); // body length
    payload.extend_from_slice(&0_u64.to_le_bytes()); // first global version
    payload.extend_from_slice(&0_u32.to_le_bytes()); // zero routed Memories
    payload.extend_from_slice(&0_i64.to_le_bytes()); // transaction time
    let digest = Sha256::digest(&payload);
    payload.extend_from_slice(&digest);
    payload
}

#[test]
fn rel_replay_rejects_unsupported_publication_carriers_without_mutation() {
    for payload in [external_memory_record(), b"RELINSC8\x01".to_vec()] {
        let path = Temp::new();
        let owner = [0x45; 16];
        drop(RelWriter::create_path(&path.0, ReliquaryScope::Project, owner).unwrap());
        let mut container = Container::open_writer(&path.0).unwrap();
        container.append(&payload).unwrap();
        container.sync().unwrap();
        drop(container);
        let before = fs::read(&path.0).unwrap();
        let admission = GraphAdmission::for_replay(owner, [], [], [], [], []);
        assert!(matches!(RelSession::open_path(&path.0, &admission, [], []),
            Err(OwnerSessionError::Record(message)) if message.contains("unsupported owner publication")));
        assert_eq!(fs::read(&path.0).unwrap(), before);
    }
}

#[test]
fn phy_rejects_wrapped_rel_only_records_without_mutation() {
    for (body, time) in [
        (b"RELTURN2".as_slice(), None),
        (b"RELXMEM2".as_slice(), Some(3)),
    ] {
        let path = Temp::new();
        let owner = [0x46; 16];
        drop(PhyWriter::create_path(&path.0, owner).unwrap());
        let mut container = Container::open_writer(&path.0).unwrap();
        container.append_publication_envelope(body, time).unwrap();
        container.sync().unwrap();
        drop(container);
        let before = fs::read(&path.0).unwrap();
        let admission = GraphAdmission::for_replay(owner, [], [], [], [], []);
        assert!(matches!(
            PhySession::open_path(&path.0, &admission, []),
            Err(OwnerSessionError::RelOnlyRecordInPhylactery(_))
        ));
        assert_eq!(fs::read(&path.0).unwrap(), before);
    }
}

#[test]
fn phy_owner_rejects_rel_external_memory_record_without_mutation() {
    let path = Temp::new();
    let owner = [41; 16];
    let mut container = Container::create(&path.0, ContainerIdentity::phylactery(), owner).unwrap();
    PackedVectorStore::default()
        .initialize(&mut container)
        .unwrap();
    CompatibilityProfileStore::default()
        .initialize(&mut container)
        .unwrap();
    MemoryVectorStore::default()
        .initialize(&mut container)
        .unwrap();
    container.append(&external_memory_record()).unwrap();
    container.sync().unwrap();
    drop(container);

    let before = fs::read(&path.0).unwrap();
    let admission = GraphAdmission::for_replay(owner, [], [], [], [], []);
    let error = match PhySession::open_path(&path.0, &admission, []) {
        Err(error) => error,
        Ok(_) => panic!("PhySession accepted a REL-only external Memory record"),
    };
    assert!(
        matches!(
            error,
            OwnerSessionError::RelOnlyRecordInPhylactery("external Memory reference")
        ),
        "unexpected Phy open error: {error:?}"
    );
    assert_eq!(fs::read(&path.0).unwrap(), before);
}

#[test]
fn phy_owner_rejects_future_rel_grouped_completion_without_mutation() {
    let path = Temp::new();
    let owner = [42; 16];
    let mut container = Container::create(&path.0, ContainerIdentity::phylactery(), owner).unwrap();
    PackedVectorStore::default()
        .initialize(&mut container)
        .unwrap();
    CompatibilityProfileStore::default()
        .initialize(&mut container)
        .unwrap();
    MemoryVectorStore::default()
        .initialize(&mut container)
        .unwrap();
    // Shape 1 is a valid RELINSC8 grouped-clock record without an embedded range.
    container.append(b"RELINSC8\x01").unwrap();
    container.sync().unwrap();
    drop(container);

    let before = fs::read(&path.0).unwrap();
    let admission = GraphAdmission::for_replay(owner, [], [], [], [], []);
    let error = match PhySession::open_path(&path.0, &admission, []) {
        Err(error) => error,
        Ok(_) => panic!("PhySession accepted a REL-only grouped completion"),
    };
    assert!(
        matches!(
            error,
            OwnerSessionError::RelOnlyRecordInPhylactery("grouped Insomnia completion v8")
        ),
        "unexpected Phy open error: {error:?}"
    );
    assert_eq!(fs::read(&path.0).unwrap(), before);
}

#[test]
fn phy_owner_rejects_unsupported_rel_freshness_versions_without_mutation() {
    for marker in [b"RELFRS07", b"RELFRS99"] {
        let path = Temp::new();
        let owner = [43; 16];
        let mut container =
            Container::create(&path.0, ContainerIdentity::phylactery(), owner).unwrap();
        PackedVectorStore::default()
            .initialize(&mut container)
            .unwrap();
        CompatibilityProfileStore::default()
            .initialize(&mut container)
            .unwrap();
        MemoryVectorStore::default()
            .initialize(&mut container)
            .unwrap();
        container.append(marker).unwrap();
        container.sync().unwrap();
        drop(container);

        let before = fs::read(&path.0).unwrap();
        let admission = GraphAdmission::for_replay(owner, [], [], [], [], []);
        let error = match PhySession::open_path(&path.0, &admission, []) {
            Err(error) => error,
            Ok(_) => panic!("PhySession accepted unsupported REL Freshness record {marker:?}"),
        };
        assert!(
            matches!(
                error,
                OwnerSessionError::RelOnlyRecordInPhylactery("Freshness record")
            ),
            "unexpected Phy open error: {error:?}"
        );
        assert_eq!(fs::read(&path.0).unwrap(), before);
    }
}

#[test]
fn rel_source_proof_checks_archive_evidence_and_phy_returns_durable_receipt() {
    let path = Temp::new();
    let owner = [44; 16];
    let mut container = Container::create(
        &path.0,
        ContainerIdentity::reliquary(ReliquaryScope::Project),
        owner,
    )
    .unwrap();
    PackedVectorStore::default()
        .initialize(&mut container)
        .unwrap();
    CompatibilityProfileStore::default()
        .initialize(&mut container)
        .unwrap();
    MemoryVectorStore::default()
        .initialize(&mut container)
        .unwrap();
    ArchiveVectorStore::default()
        .initialize(&mut container)
        .unwrap();
    VectorGenerationStore::empty()
        .initialize(&mut container)
        .unwrap();
    let mut archive = Archive::empty();
    archive
        .accept_turn(
            &mut container,
            TurnInput {
                id: "user-turn".into(),
                conversation_id: "conversation".into(),
                parent_id: None,
                role: "user".into(),
                principal_id: Some("principal-1".into()),
                timestamp_ns: 100,
                content: "I prefer tea in the morning".into(),
                attachments: vec![],
                project_attachments: vec![],
            },
        )
        .unwrap();
    archive
        .accept_turn(
            &mut container,
            TurnInput {
                id: "other-turn".into(),
                conversation_id: "conversation".into(),
                parent_id: Some("user-turn".into()),
                role: "user".into(),
                principal_id: Some("principal-1".into()),
                timestamp_ns: 200,
                content: "I like coffee in the afternoon".into(),
                attachments: vec![],
                project_attachments: vec![],
            },
        )
        .unwrap();
    archive
        .accept_turn(
            &mut container,
            TurnInput {
                id: "assistant-reference".into(),
                conversation_id: "previous-conversation".into(),
                parent_id: None,
                role: "assistant".into(),
                principal_id: None,
                timestamp_ns: 80,
                content: "Tea is the preferred morning drink.".into(),
                attachments: vec![],
                project_attachments: vec![],
            },
        )
        .unwrap();
    let episode_id = archive
        .put_episode(
            &mut container,
            "conversation",
            "user-turn",
            "user-turn",
            EpisodeOrigin::Live,
            EpisodeBoundary::Explicit,
            100,
        )
        .unwrap();
    container.sync().unwrap();
    drop(container);

    let graph = GraphAdmission::for_replay(owner, [], [], [], [], []);
    let container = Container::open_read_only(&path.0).unwrap();
    let rel = RelSession::open(&container, &graph, [], []).unwrap();
    let phy_path = Temp::new();
    let phy_owner = [45; 16];
    let mut phy_container =
        Container::create(&phy_path.0, ContainerIdentity::phylactery(), phy_owner).unwrap();
    PackedVectorStore::default()
        .initialize(&mut phy_container)
        .unwrap();
    CompatibilityProfileStore::default()
        .initialize(&mut phy_container)
        .unwrap();
    MemoryVectorStore::default()
        .initialize(&mut phy_container)
        .unwrap();
    reliquary_memory_core::MemoryStore::create(&mut phy_container).unwrap();
    phy_container.sync().unwrap();
    drop(phy_container);
    let phy_graph = GraphAdmission::for_replay(phy_owner, [], [], [], [], []);
    let mut phy = PhyWriter::open_path(&phy_path.0, &phy_graph, []).unwrap();
    let make_draft = |owner_id: String,
                      principal_id: String,
                      source_node_id: &str,
                      source_time_ns: i64| MemoryDraft {
        title: "Tea preference".into(),
        content: "Prefers tea in the morning".into(),
        category: "preference".into(),
        memory_type: "fact".into(),
        authority_kind: "direct".into(),
        temporal_status: "current".into(),
        scope: "user".into(),
        lifecycle_state: "extracted".into(),
        archived: false,
        parent_id: None,
        superseded_by: None,
        source_node_id: None,
        content_source_conversation_id: None,
        content_source_node_id: None,
        grounding_source_conversation_id: None,
        grounding_source_node_id: None,
        source_episode_id: None,
        source_ref: Some(MemorySourceRef {
            owner_id,
            principal_id: Some(principal_id),
            source_episode_id: episode_id.0,
            source_node_id: source_node_id.into(),
            content_source_conversation_id: None,
            content_source_node_id: None,
            grounding_source_conversation_id: None,
            grounding_source_node_id: None,
        }),
        source_time_ns: Some(source_time_ns),
        temporal_inference: None,
        mutation_id: "route-1".into(),
        created_at_ns: 101,
        updated_at_ns: 101,
    };
    let owner_id = owner
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    for (draft, quote) in [
        (
            make_draft(
                "forged-owner".into(),
                "principal-1".into(),
                "user-turn",
                100,
            ),
            "I prefer tea",
        ),
        (
            make_draft(
                owner_id.clone(),
                "forged-principal".into(),
                "user-turn",
                100,
            ),
            "I prefer tea",
        ),
        (
            make_draft(owner_id.clone(), "principal-1".into(), "user-turn", 100),
            "I prefer coffee",
        ),
        (
            make_draft(owner_id.clone(), "principal-1".into(), "other-turn", 200),
            "I like coffee",
        ),
    ] {
        rel.prove_routed_user_memory(&phy, &draft, quote, None)
            .expect_err("REL must reject draft data that differs from Archive evidence");
    }

    let mut adopted = make_draft(owner_id.clone(), "principal-1".into(), "user-turn", 80);
    adopted.authority_kind = "adoption".into();
    let source = adopted.source_ref.as_mut().unwrap();
    source.content_source_conversation_id = Some("previous-conversation".into());
    source.content_source_node_id = Some("assistant-reference".into());
    assert!(
        rel.prove_routed_user_memory(&phy, &adopted, "I prefer tea", None)
            .is_err()
    );
    assert!(
        rel.prove_routed_user_memory(&phy, &adopted, "I prefer tea", Some("I prefer coffee"))
            .is_err()
    );
    assert!(
        rel.prove_routed_user_memory(
            &phy,
            &adopted,
            "I prefer tea",
            Some("Tea is the preferred morning drink.")
        )
        .is_ok()
    );

    let draft = make_draft(owner_id, "principal-1".into(), "user-turn", 100);
    let proof = rel
        .prove_routed_user_memory(&phy, &draft, "I prefer tea", None)
        .expect("matching validated user draft can be sealed");
    let (revision, receipt) = phy
        .publish_routed_user_memory(proof)
        .expect("sealed draft publishes durably to PHY");
    assert_eq!(receipt.external_ref().memory_id, revision.record.id.0);
    assert!(receipt.matches_draft(&draft));
    let mut changed = draft;
    changed.title.push_str(" (changed)");
    assert!(!receipt.matches_draft(&changed));
}
