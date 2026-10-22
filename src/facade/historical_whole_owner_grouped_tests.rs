use super::whole_owner_at;
use crate::insomnia::completion::{InsomniaCompletion, encode_completion};
use crate::{Cva, EpisodeBoundary, EpisodeConfig, EpisodeOrigin, MemoryDraft};
use std::{fs, path::PathBuf};

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(".test-tmp")
            .join(format!("whole-owner-grouped-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
    fn path(&self) -> PathBuf {
        self.0.join("owner.rel")
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn whole_owner_cut_inside_grouped_publication_selects_individual_memory_versions() {
    let fixture = Fixture::new();
    let path = fixture.path();
    let mut cva = Cva::create_project(&path).unwrap();
    cva.append_node(
        "u".into(),
        "grouped-cut".into(),
        None,
        "user".into(),
        1,
        "remember two facts",
    )
    .unwrap();
    cva.append_node(
        "a".into(),
        "grouped-cut".into(),
        Some("u".into()),
        "assistant".into(),
        2,
        "noted",
    )
    .unwrap();
    let episode = cva
        .materialize_path_episodes(
            "grouped-cut",
            "a",
            EpisodeConfig::default(),
            EpisodeOrigin::Live,
            Some((EpisodeBoundary::Inactivity, 3)),
        )
        .unwrap()
        .created
        .into_iter()
        .next()
        .unwrap();

    let draft = |name: &str| MemoryDraft {
        category: "fact".into(),
        memory_type: "project".into(),
        authority_kind: "unknown".into(),
        temporal_status: "unknown".into(),
        title: name.into(),
        content: format!("Grouped {name}"),
        scope: "private".into(),
        lifecycle_state: "extracted".into(),
        archived: false,
        superseded_by: None,
        parent_id: None,
        source_node_id: Some("u".into()),
        content_source_conversation_id: None,
        content_source_node_id: None,
        grounding_source_conversation_id: None,
        grounding_source_node_id: None,
        source_episode_id: Some(episode.id),
        source_time_ns: None,
        mutation_id: name.into(),
        created_at_ns: 3,
        updated_at_ns: 4,
    };
    let batch = cva
        .memories
        .stage_grouped_publication(
            &mut cva.container,
            vec![
                (draft("grouped-one"), None, None),
                (draft("grouped-two"), None, None),
            ],
        )
        .unwrap();
    let ids: Vec<_> = batch.records.iter().map(|record| record.id).collect();
    let first_global = batch.records[0].global_version;
    assert_eq!(batch.records[1].global_version, first_global + 1);
    let turn = cva.rel_turn_count();
    let completion = InsomniaCompletion {
        episode_id: episode.id,
        attempt: 1,
        started_at_ns: 5,
        completed_at_ns: 6,
        transaction_time_ns: 600,
        extractor_model: "oracle-test".into(),
        extractor_version: "v1".into(),
        rejected_count: 0,
        memory_ids: ids.clone(),
        external_memory_refs: Vec::new(),
        global_version_start: batch.global_version_start,
        bodies: batch
            .bodies
            .into_iter()
            .map(|(id, bytes)| crate::insomnia::completion::InsomniaCompletionBody { id, bytes })
            .collect(),
        records: batch.records,
        routing_metadata: batch.routing_metadata,
        freshness_birth_turn: Some(turn),
        freshness_policy_version: Some(crate::freshness::FRESHNESS_POLICY_VERSION as u64),
        freshness_birth_memory_ids: ids.clone(),
    };
    cva.container
        .append(&encode_completion(&completion).unwrap())
        .unwrap();
    cva.container
        .commit_embedded_version_range_at(first_global, 2, Some(600))
        .unwrap();
    cva.sync().unwrap();
    drop(cva);

    let mut reopened = Cva::open(&path).unwrap();
    let before = whole_owner_at(&mut reopened, first_global - 1).unwrap();
    let middle = whole_owner_at(&mut reopened, first_global).unwrap();
    let end = whole_owner_at(&mut reopened, first_global + 1).unwrap();
    assert!(before.current_memories.is_empty());
    assert_eq!(
        middle.current_memories,
        std::collections::BTreeMap::from([(ids[0], 1)])
    );
    assert_eq!(
        end.current_memories,
        std::collections::BTreeMap::from([(ids[0], 1), (ids[1], 1)])
    );
    assert_ne!(middle.fingerprint, end.fingerprint);
    let middle_again = whole_owner_at(&mut reopened, first_global).unwrap();
    assert_eq!(middle.fingerprint, middle_again.fingerprint);
    let middle_fingerprint = middle.fingerprint;
    drop(reopened);

    let mut reopened = Cva::open(&path).unwrap();
    assert_eq!(
        middle_fingerprint,
        whole_owner_at(&mut reopened, first_global)
            .unwrap()
            .fingerprint
    );
    assert_eq!(
        end.current_memories,
        whole_owner_at(&mut reopened, first_global + 1)
            .unwrap()
            .current_memories
    );
}
