use crate::memory_source_time::{memory_source_timestamp_ns, reliquary_memory_source_timestamp_ns};
use crate::{Cva, EpisodeBoundary, EpisodeConfig, EpisodeOrigin, MemoryDraft};
use std::fs;
use std::path::PathBuf;

fn test_path(name: &str) -> PathBuf {
    let unique = uuid::Uuid::new_v4();
    let dir = std::env::temp_dir().join(format!("continuity-memory-source-time-{unique}"));
    fs::create_dir_all(&dir).unwrap();
    dir.join(name)
}

fn seeded_episode(cva: &mut Cva) -> crate::Episode {
    cva.append_node(
        "u0".into(),
        "c1".into(),
        None,
        "user".into(),
        10,
        "Original source.",
    )
    .unwrap();
    cva.append_node(
        "a0".into(),
        "c1".into(),
        Some("u0".into()),
        "assistant".into(),
        20,
        "Response.",
    )
    .unwrap();
    cva.materialize_path_episodes(
        "c1",
        "a0",
        EpisodeConfig::default(),
        EpisodeOrigin::Live,
        Some((EpisodeBoundary::Inactivity, 30)),
    )
    .unwrap()
    .created
    .into_iter()
    .next()
    .unwrap()
}

fn draft(episode: &crate::Episode) -> MemoryDraft {
    MemoryDraft {
        category: "fact".into(),
        memory_type: "project".into(),
        authority_kind: "direct".into(),
        temporal_status: "unknown".into(),
        title: "Source chronology".into(),
        content: "Chronology precedence fixture.".into(),
        scope: "private".into(),
        lifecycle_state: "knowledge".into(),
        archived: false,
        superseded_by: None,
        parent_id: None,
        source_node_id: Some("u0".into()),
        content_source_conversation_id: None,
        content_source_node_id: None,
        grounding_source_conversation_id: None,
        grounding_source_node_id: None,
        source_episode_id: Some(episode.id),
        source_time_ns: Some(5),
        mutation_id: "memory-source-time".into(),
        created_at_ns: 100,
        updated_at_ns: 100,
    }
}

#[test]
fn memory_source_time_reads_persisted_source_time() {
    let path = test_path("persisted.cva");
    let mut cva = Cva::create(path).unwrap();
    let episode = seeded_episode(&mut cva);
    let memory = cva.publish_memory(None, 0, draft(&episode)).unwrap().0;

    assert_eq!(memory_source_timestamp_ns(&memory), Some(5));
}

#[test]
fn reliquary_source_time_prefers_content_source_node() {
    let path = test_path("content-source.cva");
    let mut cva = Cva::create(path).unwrap();
    let episode = seeded_episode(&mut cva);
    cva.append_node(
        "content0".into(),
        "c2".into(),
        None,
        "user".into(),
        70,
        "Quoted source content.",
    )
    .unwrap();

    let mut memory_draft = draft(&episode);
    memory_draft.content_source_conversation_id = Some("c2".into());
    memory_draft.content_source_node_id = Some("content0".into());
    let memory = cva.publish_memory(None, 0, memory_draft).unwrap().0;

    assert_eq!(
        reliquary_memory_source_timestamp_ns(cva.archive(), &memory),
        Some(70)
    );
}

#[test]
fn reliquary_source_time_falls_back_to_episode_source_node() {
    let path = test_path("episode-node.cva");
    let mut cva = Cva::create(path).unwrap();
    let episode = seeded_episode(&mut cva);
    let memory = cva.publish_memory(None, 0, draft(&episode)).unwrap().0;

    assert_eq!(
        reliquary_memory_source_timestamp_ns(cva.archive(), &memory),
        Some(10)
    );
}

#[test]
fn reliquary_source_time_falls_back_to_episode_source_through_time() {
    let path = test_path("episode-through.cva");
    let mut cva = Cva::create(path).unwrap();
    let episode = seeded_episode(&mut cva);
    let mut memory = cva.publish_memory(None, 0, draft(&episode)).unwrap().0;
    memory.source_node_id = None;

    assert_eq!(
        reliquary_memory_source_timestamp_ns(cva.archive(), &memory),
        Some(episode.source_through_ns)
    );
}

#[test]
fn reliquary_source_time_falls_back_to_persisted_memory_source_time() {
    let path = test_path("memory-fallback.cva");
    let mut cva = Cva::create(path).unwrap();
    let episode = seeded_episode(&mut cva);
    let mut memory = cva.publish_memory(None, 0, draft(&episode)).unwrap().0;
    memory.source_episode_id = None;
    memory.source_node_id = None;

    assert_eq!(
        reliquary_memory_source_timestamp_ns(cva.archive(), &memory),
        Some(5)
    );
}
