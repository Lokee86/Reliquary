use crate::{
    Cva, EpisodeBoundary, EpisodeConfig, EpisodeOrigin, FragmentConfig, InsomniaExtractor,
    InsomniaPriority, SimulatedGeneralEndpoint,
};
use serde_json::json;
use std::fs;
use std::path::PathBuf;

fn test_path(name: &str) -> PathBuf {
    let unique = uuid::Uuid::new_v4();
    let dir = std::env::temp_dir().join(format!("continuity-insomnia-evidence-flow-{unique}"));
    fs::create_dir_all(&dir).unwrap();
    dir.join(name)
}

fn append(cva: &mut Cva, id: &str, parent: Option<&str>, role: &str, time: i64, content: &str) {
    cva.append_node(
        id.into(),
        "c1".into(),
        parent.map(str::to_owned),
        role.into(),
        time,
        content,
    )
    .unwrap();
}

fn queue_episode(cva: &mut Cva, leaf: &str) -> crate::Episode {
    let episode = cva
        .materialize_path_episodes(
            "c1",
            leaf,
            EpisodeConfig::default(),
            EpisodeOrigin::Live,
            Some((EpisodeBoundary::Inactivity, 100)),
        )
        .unwrap()
        .created
        .into_iter()
        .next()
        .unwrap();
    cva.queue_insomnia_episode(episode.id, InsomniaPriority::Live, 101)
        .unwrap();
    episode
}

fn omit(reason: &str) -> serde_json::Value {
    json!({
        "disposition":"omit", "authority_kind":"none", "category":"none",
        "type":"none", "lifecycle":"none", "proposition":"",
        "authority_source_node_id":"", "grounding_source_node_id":"",
        "reason":reason
    })
}

#[test]
fn historical_archive_search_can_supply_adopted_assistant_content() {
    let path = test_path("historical-evidence.cva");
    let mut cva = Cva::create(&path).unwrap();
    cva.append_node(
        "old-u".into(),
        "old".into(),
        None,
        "user".into(),
        1,
        "What are our deployment targets?",
    )
    .unwrap();
    cva.append_node(
        "old-a".into(),
        "old".into(),
        Some("old-u".into()),
        "assistant".into(),
        2,
        "The deployment target is Windows and Linux.",
    )
    .unwrap();
    cva.materialize_path_fragments("old", "old-a", FragmentConfig::default(), true)
        .unwrap();
    append(
        &mut cva,
        "u0",
        None,
        "user",
        10,
        "Remember the deployment target we discussed.",
    );
    append(&mut cva, "a0", Some("u0"), "assistant", 11, "Okay.");
    let episode = queue_episode(&mut cva, "a0");
    let claim = cva.claim_insomnia_episode("w", 110, 100).unwrap().unwrap();
    let extractor = InsomniaExtractor::new(SimulatedGeneralEndpoint::new(
        "test-model",
        vec![
            json!({
                "turns": {"u0": [omit("requires historical assistant evidence")]},
                "evidence_requests": [{
                    "kind": "archive_search", "conversation_id": "old", "node_id": "",
                    "start_node_id": "", "end_node_id": "", "query": "deployment target Windows Linux", "limit": 1
                }]
            }),
            json!({
                "turns": {"u0": [{
                    "disposition":"retain", "authority_kind":"retention", "category":"fact",
                    "type":"project", "lifecycle":"current",
                    "proposition":"The deployment target is Windows and Linux.",
                    "authority_source_node_id":"old-a", "grounding_source_node_id":"",
                    "reason":"user explicitly retains historical assistant-authored state"
                }]},
                "evidence_requests": []
            }),
            json!({"groups": {"g000": {
                "title":"Deployment targets",
                "content":"The deployment target is Windows and Linux."
            }}}),
        ],
    ));
    let result = cva
        .process_claimed_insomnia_episode(&claim, &extractor, "private", 110, 120)
        .unwrap();
    assert_eq!(result.created.len(), 1);
    assert_eq!(
        result.created[0].content_source_conversation_id.as_deref(),
        Some("old")
    );
    assert_eq!(
        result.created[0].content_source_node_id.as_deref(),
        Some("old-a")
    );
    assert_eq!(
        result.created[0].source_episode_id,
        Some(reliquary_memory_core::SourceEpisodeId(episode.id.0))
    );
}

#[test]
fn external_authority_source_must_have_been_returned_as_evidence() {
    let path = test_path("unsupplied-content-source.cva");
    let mut cva = Cva::create(&path).unwrap();
    cva.append_node(
        "old-u".into(),
        "old".into(),
        None,
        "user".into(),
        1,
        "What are our deployment targets?",
    )
    .unwrap();
    cva.append_node(
        "old-a".into(),
        "old".into(),
        Some("old-u".into()),
        "assistant".into(),
        2,
        "The deployment target is Windows and Linux.",
    )
    .unwrap();
    append(
        &mut cva,
        "u0",
        None,
        "user",
        10,
        "Remember the deployment target we discussed.",
    );
    let episode = queue_episode(&mut cva, "u0");
    let turns = cva.episode_turns(episode.id).unwrap();
    let extractor = InsomniaExtractor::new(SimulatedGeneralEndpoint::new(
        "test-model",
        vec![json!({
            "turns": {"u0": [{
                "disposition":"retain", "authority_kind":"retention", "category":"fact",
                "type":"project", "lifecycle":"current",
                "proposition":"The deployment target is Windows and Linux.",
                "authority_source_node_id":"old-a", "grounding_source_node_id":"",
                "reason":"invalid unsupplied historical authority"
            }]},
            "evidence_requests": []
        })],
    ));
    let error = extractor.extract(&episode, &turns).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("not an available assistant turn")
    );
    assert_eq!(cva.memory_stats().memories, 0);
}

#[test]
fn second_archive_evidence_round_is_rejected() {
    let path = test_path("second-evidence-round.cva");
    let mut cva = Cva::create(&path).unwrap();
    append(&mut cva, "u0", None, "user", 10, "Remember that.");
    let episode = queue_episode(&mut cva, "u0");
    let turns = cva.episode_turns(episode.id).unwrap();
    let response = json!({
        "turns": {"u0": [omit("requires evidence")]},
        "evidence_requests": [{
            "kind": "turn", "conversation_id": "c1", "node_id": "u0",
            "start_node_id": "", "end_node_id": "", "query": "", "limit": 0
        }]
    });
    let extractor = InsomniaExtractor::new(SimulatedGeneralEndpoint::new(
        "test-model",
        vec![response.clone(), response],
    ));
    let error = extractor
        .extract_with_evidence(&mut cva, &episode, &turns)
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("more than one archive-evidence round")
    );
}
