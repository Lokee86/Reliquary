use crate::{
    Cva, EpisodeBoundary, EpisodeConfig, EpisodeOrigin, INSOMNIA_SYSTEM_PROMPT, InsomniaExtractor,
    InsomniaPriority, InsomniaWorkState, SimulatedGeneralEndpoint,
};
use reliquary_insomnia::{SYNTHESIS_SYSTEM_PROMPT, ledger_schema};
use serde_json::{Value, json};
use std::fs;
use std::path::PathBuf;

fn test_path(name: &str) -> PathBuf {
    let unique = uuid::Uuid::new_v4();
    let dir = std::env::temp_dir().join(format!("continuity-insomnia-extract-{unique}"));
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

fn omit() -> Value {
    json!({
        "disposition": "omit",
        "authority_kind": "none",
        "category": "none",
        "type": "none",
        "lifecycle": "none",
        "proposition": "",
        "authority_source_node_id": "",
        "grounding_source_node_id": "",
        "reason": "not durable state"
    })
}

fn retain(
    authority_kind: &str,
    category: &str,
    memory_type: &str,
    lifecycle: &str,
    proposition: &str,
    authority_source_node_id: &str,
    grounding_source_node_id: &str,
) -> Value {
    json!({
        "disposition": "retain",
        "authority_kind": authority_kind,
        "category": category,
        "type": memory_type,
        "lifecycle": lifecycle,
        "proposition": proposition,
        "authority_source_node_id": authority_source_node_id,
        "grounding_source_node_id": grounding_source_node_id,
        "reason": "durable state"
    })
}

fn ledger(turns: Value) -> Value {
    json!({"turns": turns, "evidence_requests": []})
}

fn wording(title: &str, content: &str) -> Value {
    json!({"groups": {"g000": {"title": title, "content": content}}})
}

#[test]
fn claimed_episode_extracts_and_publishes_authoritative_memory() {
    let path = test_path("publish.cva");
    let mut cva = Cva::create(&path).unwrap();
    append(
        &mut cva,
        "u0",
        None,
        "user",
        10,
        "Reliquary should keep all working-memory generation in Insomnia.",
    );
    append(&mut cva, "a0", Some("u0"), "assistant", 20, "Understood.");
    let episode = queue_episode(&mut cva, "a0");
    let claim = cva
        .claim_insomnia_episode("worker", 110, 100)
        .unwrap()
        .unwrap();
    let endpoint = SimulatedGeneralEndpoint::new(
        "test-model",
        vec![
            ledger(json!({
                "u0": [retain(
                    "direct",
                    "decision",
                    "project",
                    "current",
                    "Reliquary uses Insomnia as the sole authoritative generator of working memory.",
                    "",
                    ""
                )]
            })),
            wording(
                "Insomnia owns working-memory generation",
                "Reliquary uses Insomnia as the sole authoritative generator of working memory.",
            ),
        ],
    );
    let extractor = InsomniaExtractor::new(endpoint);
    let result = cva
        .process_claimed_insomnia_episode(&claim, &extractor, "private", 110, 120)
        .unwrap();
    assert_eq!(result.created.len(), 1);
    assert!(result.existing.is_empty());
    assert!(result.rejected.is_empty());
    let memory = &result.created[0];
    assert_eq!(
        memory.source_episode_id,
        Some(reliquary_memory_core::SourceEpisodeId(episode.id.0))
    );
    assert_eq!(memory.source_node_id.as_deref(), Some("u0"));
    assert_eq!(memory.temporal_status, "current");
    assert_eq!(
        cva.insomnia_work(episode.id).unwrap().state,
        InsomniaWorkState::Complete
    );

    cva.sync().unwrap();
    drop(cva);
    let mut reopened = Cva::open(&path).unwrap();
    assert_eq!(reopened.memory_stats().memories, 1);
    assert_eq!(
        reopened.memory(memory.id).unwrap().temporal_status,
        "current"
    );
    assert_eq!(reopened.insomnia_attempts(episode.id).len(), 1);
}

#[test]
fn assistant_cannot_become_user_authority_even_if_provider_bypasses_schema() {
    let path = test_path("authority.cva");
    let mut cva = Cva::create(&path).unwrap();
    append(&mut cva, "u0", None, "user", 10, "What should we do?");
    append(
        &mut cva,
        "a0",
        Some("u0"),
        "assistant",
        20,
        "Use Insomnia for all memory generation.",
    );
    let episode = queue_episode(&mut cva, "a0");
    let turns = cva.episode_turns(episode.id).unwrap();
    let extractor = InsomniaExtractor::new(SimulatedGeneralEndpoint::new(
        "test-model",
        vec![ledger(json!({
            "u0": [omit()],
            "a0": [retain(
                "direct",
                "decision",
                "project",
                "current",
                "Use Insomnia for all memory generation.",
                "",
                ""
            )]
        }))],
    ));
    let extraction = extractor.extract(&episode, &turns).unwrap();
    assert!(extraction.candidates.is_empty());
}

#[test]
fn explicit_adoption_can_use_assistant_content_with_user_authority() {
    let path = test_path("adoption.cva");
    let mut cva = Cva::create(&path).unwrap();
    append(
        &mut cva,
        "u0",
        None,
        "user",
        10,
        "What should the episode policy be?",
    );
    append(
        &mut cva,
        "a0",
        Some("u0"),
        "assistant",
        20,
        "Use a fifteen minute inactivity boundary.",
    );
    append(&mut cva, "u1", Some("a0"), "user", 30, "Remember that.");
    append(&mut cva, "a1", Some("u1"), "assistant", 40, "Recorded.");
    let episode = queue_episode(&mut cva, "a1");
    let claim = cva.claim_insomnia_episode("w", 110, 100).unwrap().unwrap();
    let extractor = InsomniaExtractor::new(SimulatedGeneralEndpoint::new(
        "test-model",
        vec![
            ledger(json!({
                "u0": [omit()],
                "u1": [retain(
                    "retention",
                    "decision",
                    "project",
                    "current",
                    "Episodes use a fifteen minute inactivity boundary.",
                    "a0",
                    ""
                )]
            })),
            wording(
                "Episode inactivity boundary",
                "Episodes use a fifteen minute inactivity boundary.",
            ),
            json!({
                "resolutions": [{"id":"u000","canonical_expression":""}]
            }),
        ],
    ));
    let result = cva
        .process_claimed_insomnia_episode(&claim, &extractor, "private", 110, 120)
        .unwrap();
    assert_eq!(result.created.len(), 1);
    assert_eq!(
        result.created[0].content_source_node_id.as_deref(),
        Some("a0")
    );
    assert_eq!(
        result.created[0].content_source_conversation_id.as_deref(),
        Some("c1")
    );
    assert_eq!(
        result.created[0].source_episode_id,
        Some(reliquary_memory_core::SourceEpisodeId(episode.id.0))
    );
}

#[test]
fn two_pass_selector_and_wording_contracts_preserve_tuned_policy() {
    for clause in [
        "Questions and requests are non-authoritative by default",
        "Pure execution/checkpoint receipts are omitted",
        "everything seems to be working OK so far",
        "Terse assent such as \"that works\"",
        "Archive evidence is supporting context, never independent user authority",
        "Prefer the FINAL user-authoritative source",
    ] {
        assert!(
            INSOMNIA_SYSTEM_PROMPT.contains(clause),
            "missing selector-contract clause: {clause}"
        );
    }
    for clause in [
        "Your ONLY job is to write one concise durable Memory title and content body",
        "Do not add, drop, merge, split, rename, or reorder groups",
        "Grounding, authority, metadata, and durable ownership are already final",
    ] {
        assert!(
            SYNTHESIS_SYSTEM_PROMPT.contains(clause),
            "missing wording-contract clause: {clause}"
        );
    }
}

#[test]
fn authority_kind_is_required_and_preserved_by_structured_selection() {
    let path = test_path("authority-kind.cva");
    let mut cva = Cva::create(&path).unwrap();
    append(
        &mut cva,
        "u0",
        None,
        "user",
        10,
        "I prefer concise answers.",
    );
    let episode = queue_episode(&mut cva, "u0");
    let turns = cva.episode_turns(episode.id).unwrap();
    let schema = ledger_schema(&turns, &[], true);
    let clause = &schema["properties"]["turns"]["properties"]["u0"]["items"];
    assert_eq!(
        clause["properties"]["authority_kind"]["enum"],
        json!(["direct", "correction", "adoption", "retention", "none"])
    );
    assert!(
        clause["required"]
            .as_array()
            .unwrap()
            .iter()
            .any(|field| field == "authority_kind")
    );

    let extractor = InsomniaExtractor::new(SimulatedGeneralEndpoint::new(
        "test-model",
        vec![
            ledger(json!({"u0": [retain(
                "direct",
                "preference",
                "communication",
                "current",
                "The user prefers concise answers.",
                "",
                ""
            )]})),
            wording("Concise responses", "The user prefers concise answers."),
        ],
    ));
    let extraction = extractor.extract(&episode, &turns).unwrap();
    assert_eq!(extraction.candidates.len(), 1);
    assert_eq!(extraction.candidates[0].authority_kind, "direct");
    assert_eq!(extraction.candidates[0].category, "preference");
    assert_eq!(extraction.candidates[0].memory_type, "communication");
    assert_eq!(extraction.candidates[0].temporal_status, "current");
}

#[test]
fn missing_adoption_provenance_is_a_structural_ledger_error() {
    let path = test_path("adoption-without-provenance.cva");
    let mut cva = Cva::create(&path).unwrap();
    append(&mut cva, "u0", None, "user", 10, "alright, that works");
    let episode = queue_episode(&mut cva, "u0");
    let turns = cva.episode_turns(episode.id).unwrap();
    let extractor = InsomniaExtractor::new(SimulatedGeneralEndpoint::new(
        "test-model",
        vec![ledger(json!({"u0": [retain(
            "adoption",
            "decision",
            "project",
            "current",
            "The server returns the updated Player state.",
            "",
            ""
        )]}))],
    ));
    let error = extractor.extract(&episode, &turns).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("adoption lacks assistant authority")
    );
}

#[test]
fn wording_pass_cannot_override_ledger_ownership() {
    let path = test_path("wording-ownership.cva");
    let mut cva = Cva::create(&path).unwrap();
    append(
        &mut cva,
        "u0",
        None,
        "user",
        10,
        "The server is authoritative for collision.",
    );
    append(&mut cva, "a0", Some("u0"), "assistant", 20, "Understood.");
    let episode = queue_episode(&mut cva, "a0");
    let turns = cva.episode_turns(episode.id).unwrap();
    let extractor = InsomniaExtractor::new(SimulatedGeneralEndpoint::new(
        "test-model",
        vec![
            ledger(json!({"u0": [retain(
                "direct",
                "constraint",
                "project",
                "current",
                "The server is authoritative for collision.",
                "",
                ""
            )]})),
            json!({"groups": {"g000": {
                "title": "Server collision authority",
                "content": "The server is authoritative for collision.",
                "authority_kind": "adoption",
                "category": "preference",
                "type": "other",
                "authority_source_node_id": "a0"
            }}}),
        ],
    ));
    let extraction = extractor.extract(&episode, &turns).unwrap();
    assert_eq!(extraction.candidates.len(), 1);
    let candidate = &extraction.candidates[0];
    assert_eq!(candidate.authority_kind, "direct");
    assert_eq!(candidate.category, "constraint");
    assert_eq!(candidate.memory_type, "project");
    assert!(candidate.authority_source_node_id.is_none());
}

#[test]
fn structurally_distinct_groups_from_one_turn_have_distinct_candidate_keys() {
    let path = test_path("group-identity.cva");
    let mut cva = Cva::create(&path).unwrap();
    append(
        &mut cva,
        "u0",
        None,
        "user",
        10,
        "The packet migration is strict now, and later it must remain byte-equivalent.",
    );
    let episode = queue_episode(&mut cva, "u0");
    let turns = cva.episode_turns(episode.id).unwrap();
    let extractor = InsomniaExtractor::new(SimulatedGeneralEndpoint::new(
        "test-model",
        vec![
            ledger(json!({"u0": [
                retain(
                    "direct",
                    "constraint",
                    "project",
                    "current",
                    "The packet migration is strict now.",
                    "",
                    ""
                ),
                retain(
                    "direct",
                    "constraint",
                    "project",
                    "future",
                    "The packet migration must remain byte-equivalent.",
                    "",
                    ""
                )
            ]})),
            json!({"groups": {
                "g000": {"title": "Strict packet migration", "content": "The packet migration is strict now."},
                "g001": {"title": "Packet byte equivalence", "content": "The packet migration must remain byte-equivalent."}
            }}),
        ],
    ));
    let extraction = extractor.extract(&episode, &turns).unwrap();
    assert_eq!(extraction.candidates.len(), 2);
    assert_eq!(extraction.candidates[0].temporal_status, "current");
    assert_eq!(extraction.candidates[1].temporal_status, "future");
    assert_ne!(extraction.candidates[0].key, extraction.candidates[1].key);
}

#[test]
fn missing_ledger_turn_gets_one_targeted_repair_and_extra_non_user_key_is_dropped() {
    let path = test_path("ledger-repair.cva");
    let mut cva = Cva::create(&path).unwrap();
    append(
        &mut cva,
        "u0",
        None,
        "user",
        10,
        "Default to concise answers.",
    );
    append(&mut cva, "a0", Some("u0"), "assistant", 20, "Understood.");
    append(&mut cva, "u1", Some("a0"), "user", 30, "What next?");
    append(&mut cva, "a1", Some("u1"), "assistant", 40, "Next step.");
    let episode = queue_episode(&mut cva, "a1");
    let turns = cva.episode_turns(episode.id).unwrap();
    let extractor = InsomniaExtractor::new(SimulatedGeneralEndpoint::new(
        "test-model",
        vec![
            ledger(json!({
                "u0": [retain(
                    "direct",
                    "instruction",
                    "communication",
                    "current",
                    "Default to concise answers.",
                    "",
                    ""
                )],
                "a0": [omit()]
            })),
            ledger(json!({"u1": [omit()]})),
            wording("Concise responses", "Default to concise answers."),
        ],
    ));

    let extraction = extractor.extract(&episode, &turns).unwrap();
    assert_eq!(extraction.candidates.len(), 1);
    assert_eq!(extraction.candidates[0].source_node_id, "u0");
    assert_eq!(extraction.candidates[0].category, "instruction");
}

#[test]
fn urgent_scheduling_does_not_force_a_memory() {
    let path = test_path("zero.cva");
    let mut cva = Cva::create(&path).unwrap();
    append(&mut cva, "u0", None, "user", 10, "hello");
    append(&mut cva, "a0", Some("u0"), "assistant", 20, "hi");
    let scheduled = cva
        .request_create_memory("c1", "a0", EpisodeConfig::default(), 30)
        .unwrap();
    let episode = scheduled.episodes.created[0].clone();
    let claim = cva.claim_insomnia_episode("w", 31, 100).unwrap().unwrap();
    let extractor = InsomniaExtractor::new(SimulatedGeneralEndpoint::new(
        "test-model",
        vec![ledger(json!({"u0": [omit()]}))],
    ));
    let result = cva
        .process_claimed_insomnia_episode(&claim, &extractor, "private", 31, 32)
        .unwrap();
    assert!(result.created.is_empty());
    assert_eq!(cva.memory_stats().memories, 0);
    assert_eq!(
        cva.insomnia_work(episode.id).unwrap().state,
        InsomniaWorkState::Complete
    );
}
