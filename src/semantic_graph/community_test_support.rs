use crate::{
    Cva, EpisodeBoundary, EpisodeConfig, EpisodeOrigin, GraphRelationChange, GraphRelationKind,
    MemoryDraft, MemoryId, Phylactery,
};
use std::collections::HashSet;
use std::{fs, path::PathBuf};

pub(crate) fn path(name: &str) -> PathBuf {
    let n = uuid::Uuid::new_v4();
    let dir = std::env::temp_dir().join(format!("reliquary-community-{n}"));
    fs::create_dir_all(&dir).unwrap();
    dir.join(name)
}

pub(crate) fn rel_episode(cva: &mut Cva) -> crate::Episode {
    cva.append_node(
        "u".into(),
        "community-test".into(),
        None,
        "user".into(),
        10,
        "group related memories",
    )
    .unwrap();
    cva.append_node(
        "a".into(),
        "community-test".into(),
        Some("u".into()),
        "assistant".into(),
        20,
        "ok",
    )
    .unwrap();
    cva.materialize_path_episodes(
        "community-test",
        "a",
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

pub(crate) fn rel_memory(cva: &mut Cva, episode: &crate::Episode, name: &str) -> MemoryId {
    cva.publish_memory(
        None,
        0,
        MemoryDraft {
            category: "fact".into(),
            memory_type: "project".into(),
            authority_kind: "unknown".into(),
            temporal_status: "unknown".into(),
            title: name.into(),
            content: format!("memory {name}"),
            scope: "private".into(),
            lifecycle_state: "knowledge".into(),
            archived: false,
            superseded_by: None,
            parent_id: None,
            source_node_id: Some("u".into()),
            content_source_conversation_id: None,
            content_source_node_id: None,
            grounding_source_conversation_id: None,
            grounding_source_node_id: None,
            source_episode_id: Some(reliquary_memory_core::SourceEpisodeId((episode.id).0)),
            source_time_ns: None,
            mutation_id: format!("community-{name}"),
            created_at_ns: 30,
            updated_at_ns: 30,
        },
    )
    .unwrap()
    .0
    .id
}

pub(crate) fn phy_memory(phy: &mut Phylactery, name: &str) -> MemoryId {
    phy.publish_memory(
        None,
        0,
        MemoryDraft {
            category: "fact".into(),
            memory_type: "user".into(),
            authority_kind: "retention".into(),
            temporal_status: "unknown".into(),
            title: name.into(),
            content: format!("user memory {name}"),
            scope: "private".into(),
            lifecycle_state: "knowledge".into(),
            archived: false,
            superseded_by: None,
            parent_id: None,
            source_node_id: None,
            content_source_conversation_id: None,
            content_source_node_id: None,
            grounding_source_conversation_id: None,
            grounding_source_node_id: None,
            source_episode_id: None,
            source_time_ns: None,
            mutation_id: format!("phy-community-{name}"),
            created_at_ns: 30,
            updated_at_ns: 30,
        },
    )
    .unwrap()
    .0
    .id
}

pub(crate) fn edge(source: MemoryId, target: MemoryId) -> GraphRelationChange {
    GraphRelationChange {
        source,
        target,
        kind: GraphRelationKind::Topical,
        active: true,
    }
}

pub(crate) fn member_sets(snapshot: &crate::CommunitySnapshot) -> HashSet<Vec<[u8; 32]>> {
    snapshot
        .communities
        .iter()
        .map(|community| {
            let mut members: Vec<_> = community.members.iter().map(|member| member.0).collect();
            members.sort();
            members
        })
        .collect()
}
