//! Integrated matrix fixtures retain native Import/Insomnia/Dream/vector authority.
use super::*;
use crate::{GraphRelationKind, GraphRelationOrigin, Memory, MemoryDraft};
use serde_json::json;

#[path = "historical_matrix_dream_tests.rs"]
mod dream;
#[path = "historical_matrix_guard_tests.rs"]
mod guards;
#[path = "historical_matrix_semantic_tests.rs"]
mod semantic;

fn draft(memory: Memory, mutation: &str) -> MemoryDraft {
    MemoryDraft {
        category: memory.category,
        memory_type: memory.memory_type,
        authority_kind: memory.authority_kind,
        temporal_status: memory.temporal_status,
        title: memory.title,
        content: memory.content,
        scope: memory.scope,
        lifecycle_state: memory.lifecycle_state,
        archived: memory.archived,
        superseded_by: memory.superseded_by,
        parent_id: memory.parent_id,
        source_node_id: memory.source_node_id,
        content_source_conversation_id: memory.content_source_conversation_id,
        content_source_node_id: memory.content_source_node_id,
        grounding_source_conversation_id: memory.grounding_source_conversation_id,
        grounding_source_node_id: memory.grounding_source_node_id,
        source_episode_id: memory.source_episode_id,
        source_time_ns: memory.source_time_ns,
        mutation_id: mutation.into(),
        created_at_ns: memory.created_at_ns,
        updated_at_ns: 2000,
    }
}
fn publish(cva: &mut Cva, name: &str, source: Option<(&str, &str)>, settle: bool) -> MemoryId {
    let id = cva
        .publish_memory(
            None,
            0,
            MemoryDraft {
                category: "decision".into(),
                memory_type: "project".into(),
                authority_kind: "direct".into(),
                temporal_status: "current".into(),
                title: name.into(),
                content: format!("Native matrix {name}."),
                scope: "private".into(),
                lifecycle_state: "knowledge".into(),
                archived: false,
                superseded_by: None,
                parent_id: None,
                source_node_id: None,
                source_episode_id: None,
                source_time_ns: None,
                content_source_conversation_id: source.map(|(c, _)| c.into()),
                content_source_node_id: source.map(|(_, n)| n.into()),
                grounding_source_conversation_id: None,
                grounding_source_node_id: None,
                mutation_id: format!("c9:{name}"),
                created_at_ns: 400,
                updated_at_ns: 400,
            },
        )
        .unwrap()
        .0
        .id;
    if settle {
        no_link(cva, id);
    }
    id
}
fn no_link(cva: &mut Cva, id: MemoryId) {
    let pass = crate::dream_freshness::initial_pass_id(cva.owner_uuid().unwrap(), id);
    cva.begin_initial_dream_pass(
        pass.clone(),
        id,
        1,
        cva.rel_turn_count(),
        Default::default(),
        cva.graph_version(),
    )
    .unwrap();
    cva.accept_initial_dream_settlement(&pass, cva.rel_turn_count())
        .unwrap();
    crate::dream_freshness::settle_initial_pass(cva, id, true, 1).unwrap();
}
fn edge(
    cva: &mut Cva,
    a: MemoryId,
    b: MemoryId,
    kind: GraphRelationKind,
    origin: GraphRelationOrigin,
) {
    cva.set_memory_relation_with_origin(a, b, kind, true, origin, cva.graph_version())
        .unwrap();
}
fn tail(cva: &mut Cva) {
    let mut parent = None;
    for index in 0..150 {
        for (prefix, role) in [("u", "user"), ("a", "assistant")] {
            let id = format!("{prefix}{index}");
            cva.append_node(
                id.clone(),
                "c3".into(),
                parent,
                role.into(),
                1000 + index * 2 + i64::from(role == "assistant"),
                if index == 149 && role == "user" {
                    "C9 signal"
                } else {
                    "C9 neutral"
                },
            )
            .unwrap();
            parent = Some(id);
        }
    }
    for episode in cva
        .materialize_path_episodes(
            "c3",
            "a149",
            EpisodeConfig::default(),
            EpisodeOrigin::Import,
            Some((EpisodeBoundary::Inactivity, 2000)),
        )
        .unwrap()
        .created
    {
        cva.queue_insomnia_episode(episode.id, InsomniaPriority::Import, 2001)
            .unwrap();
    }
    while let Some(claim) = cva.claim_insomnia_episode("c9", 2002, 1000).unwrap() {
        let turns = cva.episode_turns(claim.episode_id).unwrap();
        let entries: serde_json::Map<_, _> = turns
            .iter()
            .filter(|t| t.role == "user")
            .map(|t| {
                (
                    t.node_id.clone(),
                    json!([{"disposition":"omit","authority_kind":"none","category":"none",
            "type":"none","lifecycle":"none","proposition":"","authority_source_node_id":"",
            "grounding_source_node_id":"","reason":"matrix tail"}]),
                )
            })
            .collect();
        let extractor = crate::InsomniaExtractor::new(crate::SimulatedGeneralEndpoint::new(
            "c9-omission",
            vec![json!({"turns":entries,"evidence_requests":[]})],
        ));
        assert!(
            cva.process_claimed_insomnia_episode(&claim, &extractor, "private", 2002, 2003)
                .unwrap()
                .created
                .is_empty()
        );
    }
    assert_eq!(cva.rel_turn_count(), 332);
}
struct Endpoint;
impl crate::EmbeddingEndpoint for Endpoint {
    fn dimensions(&self) -> u32 {
        2
    }
    fn normalization(&self) -> crate::VectorNormalization {
        crate::VectorNormalization::L2
    }
    fn embed(
        &self,
        _: crate::EmbeddingMode,
        inputs: &[String],
    ) -> Result<Vec<Vec<f32>>, crate::EmbeddingEndpointError> {
        Ok(inputs
            .iter()
            .map(|text| {
                if text.contains("C9 sink") {
                    vec![-1.0, 0.0]
                } else if text.contains("C9 signal") || text.contains("Atomic completion") {
                    vec![1.0, 0.0]
                } else {
                    vec![0.0, 1.0]
                }
            })
            .collect())
    }
}
fn vectors(cva: &mut Cva, with_tail: bool) -> crate::VectorGeneration {
    for (conversation, leaf) in [("c1", "a7"), ("c2", "a7")]
        .into_iter()
        .chain(with_tail.then_some(("c3", "a149")))
    {
        cva.materialize_path_fragments(
            conversation,
            leaf,
            crate::FragmentConfig {
                turns: 2,
                overlap: 0,
            },
            true,
        )
        .unwrap();
    }
    let profile = cva.establish_compatibility_profile(&Endpoint).unwrap();
    cva.build_missing_memory_vectors(profile.id, &Endpoint)
        .unwrap();
    historical_events::archive_generation_without_live(cva, profile.id, &Endpoint)
}
fn accept_and_reopen(path: &std::path::Path, mut cva: Cva) -> Cva {
    assert!(
        cva.reconstruct_historical_freshness(Default::default())
            .unwrap()
            .committed
    );
    cva.sync().unwrap();
    drop(cva);
    let mut reopened = Cva::open(path).unwrap();
    let bytes = fs::read(path).unwrap();
    assert!(
        !reopened
            .reconstruct_historical_freshness(Default::default())
            .unwrap()
            .committed
    );
    assert_eq!(fs::read(path).unwrap(), bytes);
    reopened
}
fn rejected(cva: &mut Cva, path: &std::path::Path, expected: &str) {
    cva.sync().unwrap();
    let bytes = fs::read(path).unwrap();
    let records = cva.freshness_records_at(cva.rel_turn_count()).unwrap();
    let error = cva
        .reconstruct_historical_freshness(Default::default())
        .unwrap_err()
        .to_string();
    assert!(error.contains(expected), "{error}");
    assert!(cva.freshness.import_checkpoint().is_none());
    assert_eq!(fs::read(path).unwrap(), bytes);
    assert_eq!(
        cva.freshness_records_at(cva.rel_turn_count()).unwrap(),
        records
    );
}
