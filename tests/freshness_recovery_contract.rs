//! F3/F5 owner-boundary contracts for Dream settlement recovery.
//! These tests intentionally exercise only public Cva and DreamProcessor APIs.

use reliquary_memory::{
    Cva, DreamCandidateConfig, DreamProcessor, DreamRelationKind, DreamVerificationPolicy,
    GraphRelationKind, GraphRelationOrigin, MemoryDraft, MemoryId, SimulatedEmbeddingEndpoint,
    SimulatedGeneralEndpoint, VectorNormalization,
};
use serde_json::json;
use std::{fs, path::PathBuf};

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let path =
            std::env::temp_dir().join(format!("freshness-recovery-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn draft(key: &str, lifecycle: &str, content: &str) -> MemoryDraft {
    MemoryDraft {
        category: "fact".into(),
        memory_type: "project".into(),
        authority_kind: "direct".into(),
        temporal_status: "current".into(),
        title: key.into(),
        content: content.into(),
        scope: "project".into(),
        lifecycle_state: lifecycle.into(),
        archived: false,
        superseded_by: None,
        parent_id: None,
        source_node_id: None,
        content_source_conversation_id: None,
        content_source_node_id: None,
        grounding_source_conversation_id: None,
        grounding_source_node_id: None,
        source_episode_id: None,
        source_time_ns: Some(0),
        mutation_id: key.into(),
        created_at_ns: 1,
        updated_at_ns: 1,
    }
}

fn publish(rel: &mut Cva, key: &str, lifecycle: &str, content: &str) -> MemoryId {
    let (memory, changed) = rel
        .publish_memory(None, 0, draft(key, lifecycle, content))
        .unwrap();
    assert!(changed);
    memory.id
}

fn turns(rel: &mut Cva, from: usize, to: usize) {
    for n in from..to {
        rel.append_node(
            format!("freshness-turn-{n}"),
            "conversation".into(),
            None,
            if n % 2 == 0 { "user" } else { "assistant" }.into(),
            n as i64,
            "accepted owner activity",
        )
        .unwrap();
    }
}

#[test]
fn dream_duplicate_enrichment_reinforces_existing_canonical_only() {
    let fixture = Fixture::new();
    let mut rel = Cva::create(fixture.path("duplicate.rel")).unwrap();
    let canonical = publish(&mut rel, "canonical", "knowledge", "Same sentence.");
    turns(&mut rel, 0, 2_000);

    let source = publish(&mut rel, "duplicate-source", "extracted", "Same sentence.");
    let before_enrichment = rel.freshness_record(source).unwrap();
    let mut enriched = draft("duplicate-source-enriched", "extracted", "Same sentence.");
    enriched.title = "duplicate-source".into();
    let (source_memory, changed) = rel.publish_memory(Some(source), 1, enriched).unwrap();
    assert!(changed);
    assert_eq!(source_memory.revision, 2);
    assert_eq!(rel.freshness_record(source).unwrap(), before_enrichment);
    assert_eq!(before_enrichment.admitted_at_turn, None);
    assert_eq!(before_enrichment.score, 100);

    let endpoint = SimulatedEmbeddingEndpoint::new(8, VectorNormalization::L2, 17);
    let profile = rel.establish_compatibility_profile(&endpoint).unwrap();
    rel.build_missing_memory_vectors(profile.id, &endpoint)
        .unwrap();
    assert_eq!(
        rel.freshness_record(canonical)
            .unwrap()
            .score_at(2_000)
            .unwrap(),
        -100
    );

    let processor = DreamProcessor::new(
        SimulatedGeneralEndpoint::new(
            "test-classifier",
            vec![json!({
                "relation": "duplicate_of",
                "direction": "undirected",
                "evidence": [
                    {"side": "a", "quote": "Same sentence."},
                    {"side": "b", "quote": "Same sentence."}
                ]
            })],
        ),
        SimulatedGeneralEndpoint::new(
            "test-verifier",
            vec![json!({
                "relation_supported": "yes",
                "direction_supported": "yes",
                "evidence_supported": "yes"
            })],
        ),
    );
    let result = processor
        .process_memory(
            &mut rel,
            profile.id,
            source,
            DreamCandidateConfig {
                limit: 1,
                semantic_limit: 1,
                prior_semantic_quota: 0,
                lexical_limit: 1,
                temporal_limit: 0,
            },
            DreamVerificationPolicy::default(),
        )
        .unwrap();

    assert_eq!(result.candidate_count, 1);
    assert_eq!(
        result.pairs[0].classification.relation,
        DreamRelationKind::DuplicateOf
    );
    assert!(result.source.archived);
    assert!(!rel.memory(canonical).unwrap().archived);
    assert_eq!(
        rel.freshness_record(canonical)
            .unwrap()
            .score_at(2_000)
            .unwrap(),
        -50,
        "the first-cycle duplicate event reinforces the existing canonical representative by +50"
    );
    assert_eq!(
        rel.freshness_record(source).unwrap().admitted_at_turn,
        None,
        "an archived new duplicate is not admitted as an ordinary active Web Memory"
    );

    // A later graph-only rewire between established Memories does not create
    // another +50 event for the old canonical record.
    let other = publish(&mut rel, "other-existing", "knowledge", "Another fact.");
    let before_canonical = rel.freshness_record(canonical).unwrap();
    let before_other = rel.freshness_record(other).unwrap();
    rel.set_memory_relation_with_origin(
        canonical,
        other,
        GraphRelationKind::Topical,
        true,
        GraphRelationOrigin::Dream,
        rel.graph_version(),
    )
    .unwrap();
    assert_eq!(rel.freshness_record(canonical).unwrap(), before_canonical);
    assert_eq!(rel.freshness_record(other).unwrap(), before_other);
}
