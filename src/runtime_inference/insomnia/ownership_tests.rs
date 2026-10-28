use super::completion::decode_completion;
use super::test_support::test_path;
use crate::{
    Cva, EpisodeBoundary, EpisodeConfig, EpisodeId, EpisodeOrigin, InsomniaExtractor,
    InsomniaPriority, MemoryDraft, MemorySourceRef, Phylactery, SimulatedGeneralEndpoint,
};
use serde_json::{Value, json};

const SOURCE_PRINCIPAL: &str = "phy-00000000-0000-0000-0000-000000000123";

fn setup(name: &str) -> (Cva, crate::Episode) {
    let path = test_path(name);
    let mut cva = Cva::create(path).unwrap();
    cva.append_node_with_principal(
        "u0".into(),
        "c1".into(),
        None,
        "user".into(),
        Some(SOURCE_PRINCIPAL.into()),
        10,
        "I prefer concise answers.",
    )
    .unwrap();
    cva.append_node_with_principal(
        "a0".into(),
        "c1".into(),
        Some("u0".into()),
        "assistant".into(),
        Some(SOURCE_PRINCIPAL.into()),
        20,
        "Noted.",
    )
    .unwrap();
    let episode = cva
        .materialize_path_episodes(
            "c1",
            "a0",
            EpisodeConfig::default(),
            EpisodeOrigin::Live,
            Some((EpisodeBoundary::Inactivity, 100)),
        )
        .unwrap()
        .created[0]
        .clone();
    cva.queue_insomnia_episode(episode.id, InsomniaPriority::Live, 101)
        .unwrap();
    (cva, episode)
}

fn ledger() -> Value {
    json!({
        "turns": {"u0": [{
            "disposition": "retain",
            "authority_kind": "direct",
            "category": "preference",
            "type": "communication",
            "lifecycle": "current",
            "proposition": "The user prefers concise answers.",
            "authority_source_node_id": "",
            "grounding_source_node_id": "",
            "reason": "durable preference"
        }]},
        "evidence_requests": []
    })
}

fn user_extractor(title: &str, content: &str) -> InsomniaExtractor<SimulatedGeneralEndpoint> {
    InsomniaExtractor::new(SimulatedGeneralEndpoint::new(
        "wording-model",
        vec![
            ledger(),
            json!({"groups": {"g000": {"title": title, "content": content}}}),
        ],
    ))
    .with_ownership_endpoint(SimulatedGeneralEndpoint::new(
        "ownership-model",
        vec![json!({"groups": {"g000": {"ownership": "user"}}})],
    ))
}

#[test]
fn routed_user_memory_uses_owner_qualified_receipt_and_no_rel_provenance() {
    let (mut cva, episode) = setup("owner-routing.prj.rel");
    let source_owner = cva.owner_id().unwrap();
    let phy_path = test_path("owner-routing.phy");
    let mut phy = Phylactery::create(&phy_path).unwrap();
    let phy_owner = phy.owner_id().unwrap();
    let claim = cva
        .claim_insomnia_episode("worker", 110, 100)
        .unwrap()
        .unwrap();

    let result = cva
        .process_claimed_insomnia_episode_routed(
            &mut phy,
            &claim,
            &user_extractor("Concise responses", "The user prefers concise answers."),
            "private",
            110,
            120,
        )
        .unwrap();

    assert!(result.created.is_empty());
    assert_eq!(result.user_created.len(), 1);
    let memory = &result.user_created[0];
    assert!(memory.source_episode_id.is_none());
    assert!(memory.source_node_id.is_none());
    assert_eq!(memory.source_time_ns, Some(10));
    let source_ref = memory.source_ref.as_ref().unwrap();
    assert_eq!(source_ref.owner_id, source_owner);
    assert_eq!(source_ref.principal_id.as_deref(), Some(SOURCE_PRINCIPAL));
    assert_eq!(
        source_ref.source_episode_id,
        reliquary_memory_core::SourceEpisodeId((episode.id).0)
    );
    assert_eq!(source_ref.source_node_id, "u0");
    assert!(source_ref.content_source_conversation_id.is_none());
    assert!(source_ref.content_source_node_id.is_none());
    assert_eq!(cva.memory_stats().memories, 0);
    assert_eq!(phy.memory_stats().memories, 1);
    let attempt = &cva.insomnia_attempts(episode.id)[0];
    assert!(attempt.memory_ids.is_empty());
    assert_eq!(attempt.external_memory_refs.len(), 1);
    assert_eq!(attempt.external_memory_refs[0].owner_id, phy_owner);
    assert_eq!(attempt.external_memory_refs[0].memory_id, memory.id);
    let memory_id = memory.id;
    phy.sync().unwrap();
    drop(phy);
    let mut reopened = Phylactery::open(&phy_path).unwrap();
    let reopened_memory = reopened.memory(memory_id).unwrap();
    assert_eq!(reopened_memory.source_time_ns, Some(10));
    let reopened_ref = reopened_memory.source_ref.unwrap();
    assert_eq!(reopened_ref.owner_id, source_owner);
    assert_eq!(reopened_ref.principal_id.as_deref(), Some(SOURCE_PRINCIPAL));
    assert_eq!(
        reopened_ref.source_episode_id,
        reliquary_memory_core::SourceEpisodeId((episode.id).0)
    );
    assert_eq!(reopened_ref.source_node_id, "u0");
}

#[test]
fn routed_retry_reuses_phy_memory_after_wording_drift() {
    let (mut cva, episode) = setup("owner-routing-retry.prj.rel");
    let source_owner = cva.owner_id().unwrap();
    let phy_path = test_path("owner-routing-retry.phy");
    let mut phy = Phylactery::create(&phy_path).unwrap();
    let turns = cva.episode_turns(episode.id).unwrap();
    let extraction = user_extractor("Concise responses", "The user prefers concise answers.")
        .extract(&episode, &turns)
        .unwrap();
    let candidate = &extraction.candidates[0];
    let mutation_id = format!(
        "insomnia:{}:{}",
        reliquary_insomnia::hex(&episode.id.0),
        candidate.key
    );
    let (prewritten, _) = phy
        .publish_memory_with_source_ref(
            None,
            0,
            MemoryDraft {
                category: candidate.category.clone(),
                memory_type: candidate.memory_type.clone(),
                authority_kind: candidate.authority_kind.clone(),
                temporal_status: candidate.temporal_status.clone(),
                title: candidate.title.clone(),
                content: candidate.content.clone(),
                scope: "private".into(),
                lifecycle_state: "extracted".into(),
                archived: false,
                superseded_by: None,
                parent_id: None,
                source_node_id: None,
                content_source_conversation_id: None,
                content_source_node_id: None,
                grounding_source_conversation_id: None,
                grounding_source_node_id: None,
                source_episode_id: None,
                source_time_ns: Some(10),
                mutation_id,
                created_at_ns: episode.source_through_ns,
                updated_at_ns: 120,
            },
            MemorySourceRef {
                owner_id: source_owner,
                principal_id: Some(SOURCE_PRINCIPAL.into()),
                source_episode_id: reliquary_memory_core::SourceEpisodeId((episode.id).0),
                source_node_id: candidate.source_node_id.clone(),
                content_source_conversation_id: candidate.authority_source_conversation_id.clone(),
                content_source_node_id: candidate.authority_source_node_id.clone(),
                grounding_source_conversation_id: candidate
                    .grounding_source_conversation_id
                    .clone(),
                grounding_source_node_id: candidate.grounding_source_node_id.clone(),
            },
        )
        .unwrap();
    phy.sync().unwrap();

    let claim = cva
        .claim_insomnia_episode("worker", 110, 100)
        .unwrap()
        .unwrap();
    let result = cva
        .process_claimed_insomnia_episode_routed(
            &mut phy,
            &claim,
            &user_extractor(
                "Response brevity",
                "Concise responses are preferred by the user.",
            ),
            "private",
            110,
            120,
        )
        .unwrap();
    assert!(result.user_created.is_empty());
    assert_eq!(result.user_existing.len(), 1);
    assert_eq!(result.user_existing[0].id, prewritten.id);
    assert_eq!(phy.memory_stats().memories, 1);
}
