use reliquary_memory::{
    AcceptedMemoryUseReceipt, Cva, EpisodeBoundary, EpisodeConfig, EpisodeOrigin, GraphDirection,
    GraphRelationKind, GraphRelationOrigin, MemoryDraft, MemoryId,
};
use std::{fs, path::PathBuf};

struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let path =
            std::env::temp_dir().join(format!("freshness-boundary-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
    fn path(&self) -> PathBuf {
        self.0.join("owner.rel")
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn seed(cva: &mut Cva) -> reliquary_memory::Episode {
    cva.append_node(
        "source-node".into(),
        "conversation-1".into(),
        None,
        "user".into(),
        1,
        "source",
    )
    .unwrap();
    cva.append_node(
        "content-node".into(),
        "conversation-1".into(),
        Some("source-node".into()),
        "assistant".into(),
        2,
        "luna lexical boundary content",
    )
    .unwrap();
    cva.materialize_path_episodes(
        "conversation-1",
        "content-node",
        EpisodeConfig::default(),
        EpisodeOrigin::Live,
        Some((EpisodeBoundary::Explicit, 3)),
    )
    .unwrap()
    .created
    .into_iter()
    .next()
    .unwrap()
}

fn draft(name: &str, content: &str, episode: &reliquary_memory::Episode) -> MemoryDraft {
    MemoryDraft {
        category: "fact".into(),
        memory_type: "project".into(),
        authority_kind: "direct".into(),
        temporal_status: "current".into(),
        title: name.into(),
        content: content.into(),
        scope: "project".into(),
        lifecycle_state: "knowledge".into(),
        archived: false,
        superseded_by: None,
        parent_id: None,
        source_node_id: Some("source-node".into()),
        content_source_conversation_id: Some("conversation-1".into()),
        content_source_node_id: Some("content-node".into()),
        grounding_source_conversation_id: None,
        grounding_source_node_id: None,
        source_episode_id: Some(episode.id),
        source_time_ns: Some(2),
        mutation_id: name.into(),
        created_at_ns: 1,
        updated_at_ns: 1,
    }
}

fn publish(
    cva: &mut Cva,
    name: &str,
    content: &str,
    episode: &reliquary_memory::Episode,
) -> MemoryId {
    cva.publish_memory(None, 0, draft(name, content, episode))
        .unwrap()
        .0
        .id
}

fn turns(cva: &mut Cva, from: u64, to: u64) {
    for n in from..to {
        cva.append_node(
            format!("turn-{n}"),
            "conversation-1".into(),
            None,
            "user".into(),
            n as i64 + 10,
            "real REL activity",
        )
        .unwrap();
    }
}

#[test]
fn internal_retrieval_is_passive_and_reopenable() {
    let temp = Temp::new();
    let path = temp.path();
    let mut cva = Cva::create(&path).unwrap();
    let episode = seed(&mut cva);
    let source = publish(
        &mut cva,
        "source",
        "luna lexical boundary content",
        &episode,
    );
    let target = publish(&mut cva, "target", "graph target content", &episode);
    cva.set_memory_relation_with_origin(
        source,
        target,
        GraphRelationKind::Topical,
        true,
        GraphRelationOrigin::Dream,
        cva.graph_version(),
    )
    .unwrap();
    turns(&mut cva, 2, 300);
    cva.sync().unwrap();
    let bytes = fs::read(&path).unwrap();
    let clocks = (
        cva.rel_turn_count(),
        cva.latest_global_version(),
        cva.archive_version(),
        cva.memory_version(),
        cva.graph_version(),
    );
    let records = [source, target].map(|id| cva.freshness_record(id));
    assert_eq!(cva.memory_revision(source, 1).unwrap().id, source);
    assert_eq!(
        cva.memory(source).unwrap().source_node_id.as_deref(),
        Some("source-node")
    );
    assert_eq!(
        cva.memory(source)
            .unwrap()
            .content_source_node_id
            .as_deref(),
        Some("content-node")
    );
    assert_eq!(
        cva.search_memories("luna boundary", 8)
            .unwrap()
            .iter()
            .filter(|h| h.memory.id == source)
            .count(),
        1
    );
    assert!(
        cva.graph_neighbors(source, GraphDirection::Outgoing)
            .unwrap()
            .iter()
            .any(|n| n.memory_id == target)
    );
    assert!(
        cva.shortest_memory_path(source, target, 4)
            .unwrap()
            .is_some()
    );
    cva.dream_memory_context(source).unwrap();
    cva.dream_temporal_analysis(source).unwrap();
    // Memories were born at turn 2; projecting decay never rewrites their anchors.
    assert!(cva.ego_memory_freshness(&[source], 0).unwrap().is_empty());
    for turn in [2, 10, 100, 300] {
        let grades = cva
            .ego_memory_freshness(&[source, source, target], turn)
            .unwrap();
        assert_eq!(grades.len(), 2);
        assert!(
            grades
                .iter()
                .all(|g| g.score == 100 - ((turn - 2) / 10) as i16)
        );
    }
    assert_eq!(cva.episode_turns(episode.id).unwrap().len(), 2);
    let selected = cva
        .select_ego_routine_context(
            &[reliquary_memory::EgoFreshnessCandidate {
                memory_id: source,
                context_cost: 1,
            }],
            1,
            &Default::default(),
            300,
        )
        .unwrap();
    assert_eq!(selected, vec![source]);
    assert!(cva.memory(MemoryId([9; 32])).is_err());
    assert!(cva.search_memories("", 8).is_err());
    assert!(cva.ego_memory_freshness(&[source], 301).is_err());
    assert_eq!([source, target].map(|id| cva.freshness_record(id)), records);
    assert_eq!(cva.rel_turn_count(), clocks.0);
    assert_eq!(cva.latest_global_version(), clocks.1);
    assert_eq!(cva.archive_version(), clocks.2);
    assert_eq!(cva.memory_version(), clocks.3);
    assert_eq!(cva.graph_version(), clocks.4);
    assert_eq!(fs::read(&path).unwrap(), bytes);
    drop(cva);
    let mut reopened = Cva::open(&path).unwrap();
    assert_eq!(reopened.memory_revision(source, 1).unwrap().id, source);
    assert_eq!(reopened.rel_turn_count(), clocks.0);
    assert_eq!(
        [source, target].map(|id| reopened.freshness_record(id)),
        records
    );
    assert_eq!(
        reopened.ego_memory_freshness(&[source], 300).unwrap()[0].score,
        71
    );
    assert_eq!(fs::read(&path).unwrap(), bytes);
}

#[test]
fn accepted_use_is_exactly_once_bounded_and_propagates() {
    let temp = Temp::new();
    let path = temp.path();
    let mut cva = Cva::create(&path).unwrap();
    let episode = seed(&mut cva);
    let source = publish(&mut cva, "source", "accepted exact content", &episode);
    let neighbor = publish(&mut cva, "neighbor", "propagated graph content", &episode);
    let omitted = publish(
        &mut cva,
        "omitted",
        "retrieved but unaccepted candidate",
        &episode,
    );
    cva.set_memory_relation_with_origin(
        source,
        neighbor,
        GraphRelationKind::Topical,
        true,
        GraphRelationOrigin::Dream,
        cva.graph_version(),
    )
    .unwrap();
    turns(&mut cva, 2, 300);
    let owner = cva.owner_uuid().unwrap();
    let receipt = AcceptedMemoryUseReceipt {
        owner_uuid: owner,
        accepted_turn: 300,
        use_id: "exact-use".into(),
        memories: vec![source, source],
    };
    let before_scores = cva
        .ego_memory_freshness(&[source, neighbor, omitted], 300)
        .unwrap();
    assert!(before_scores.iter().all(|g| g.score == 71));
    cva.memory(omitted).unwrap();
    let omitted_record = cva.freshness_record(omitted);
    let before = fs::read(&path).unwrap();
    let semantic_clocks = (
        cva.rel_turn_count(),
        cva.latest_global_version(),
        cva.memory_version(),
        cva.graph_version(),
    );
    let event_chunks = |bytes: &[u8]| bytes.windows(8).filter(|w| *w == b"CVAFRS02").count();
    assert_eq!(
        cva.record_accepted_memory_use(receipt.clone(), 1).unwrap(),
        1
    );
    assert_eq!(
        cva.ego_memory_freshness(&[source], 300).unwrap()[0].score,
        96
    );
    // Unknown membership uses the existing outside-community hop cost (25 - 10).
    assert_eq!(
        cva.ego_memory_freshness(&[neighbor], 300).unwrap()[0].score,
        86
    );
    assert_eq!(cva.freshness_record(omitted), omitted_record);
    assert_eq!(
        (
            cva.rel_turn_count(),
            cva.latest_global_version(),
            cva.memory_version(),
            cva.graph_version()
        ),
        semantic_clocks
    );
    let after = fs::read(&path).unwrap();
    assert_eq!(event_chunks(&after) - event_chunks(&before), 1);
    let accepted_records = [source, neighbor, omitted].map(|id| cva.freshness_record(id));
    assert_eq!(
        cva.record_accepted_memory_use(receipt.clone(), 1).unwrap(),
        0
    );
    assert_eq!(fs::read(&path).unwrap(), after);
    assert!(
        cva.record_accepted_memory_use(
            AcceptedMemoryUseReceipt {
                owner_uuid: [7; 16],
                ..receipt.clone()
            },
            1
        )
        .is_err()
    );
    for rejected in [
        AcceptedMemoryUseReceipt {
            accepted_turn: 301,
            use_id: "future".into(),
            ..receipt.clone()
        },
        AcceptedMemoryUseReceipt {
            accepted_turn: 299,
            ..receipt.clone()
        },
        AcceptedMemoryUseReceipt {
            memories: vec![omitted],
            ..receipt.clone()
        },
        AcceptedMemoryUseReceipt {
            use_id: " ".into(),
            ..receipt.clone()
        },
    ] {
        assert!(cva.record_accepted_memory_use(rejected, 2).is_err());
        assert_eq!(fs::read(&path).unwrap(), after);
        assert_eq!(
            [source, neighbor, omitted].map(|id| cva.freshness_record(id)),
            accepted_records
        );
    }
    assert_eq!(
        cva.record_accepted_memory_use(
            AcceptedMemoryUseReceipt {
                use_id: "empty-unaccepted".into(),
                memories: vec![],
                ..receipt.clone()
            },
            1
        )
        .unwrap(),
        0
    );
    assert_eq!(fs::read(&path).unwrap(), after);
    let disconnected = MemoryId([9; 32]);
    let rejected_bytes = fs::read(&path).unwrap();
    assert!(
        cva.record_accepted_memory_use(
            AcceptedMemoryUseReceipt {
                memories: vec![disconnected],
                use_id: "other-use".into(),
                ..receipt.clone()
            },
            1
        )
        .is_err()
    );
    assert_eq!(fs::read(&path).unwrap(), rejected_bytes);
    assert!(cva.memory(disconnected).is_err());
    drop(cva);
    let mut reopened = Cva::open(&path).unwrap();
    assert_eq!(reopened.record_accepted_memory_use(receipt, 1).unwrap(), 0);
    assert_eq!(
        [source, neighbor, omitted].map(|id| reopened.freshness_record(id)),
        accepted_records
    );
    assert_eq!(fs::read(&path).unwrap(), after);
}

#[test]
fn phy_reads_share_the_passive_contract_without_an_activity_clock() {
    let temp = Temp::new();
    let path = temp.0.join("owner.phy");
    let mut rel = Cva::create(temp.path()).unwrap();
    let episode = seed(&mut rel);
    let mut input = draft("private", "private lexical content", &episode);
    input.source_node_id = None;
    input.source_episode_id = None;
    input.content_source_conversation_id = None;
    input.content_source_node_id = None;
    let mut phy = reliquary_memory::Phylactery::create(&path).unwrap();
    let memory = phy.publish_memory(None, 0, input).unwrap().0;
    phy.sync().unwrap();
    let bytes = fs::read(&path).unwrap();
    let clocks = (phy.memory_version(), phy.graph_version());
    assert_eq!(phy.memory(memory.id).unwrap(), memory);
    assert_eq!(
        phy.search_memories("private lexical", 8).unwrap()[0].memory,
        memory
    );
    phy.dream_memory_context(memory.id).unwrap();
    phy.dream_temporal_analysis(memory.id).unwrap();
    assert_eq!((phy.memory_version(), phy.graph_version()), clocks);
    assert_eq!(fs::read(&path).unwrap(), bytes);
    drop(phy);
    let mut phy = reliquary_memory::Phylactery::open(&path).unwrap();
    assert_eq!(phy.memory(memory.id).unwrap(), memory);
    assert_eq!(fs::read(&path).unwrap(), bytes);
}
