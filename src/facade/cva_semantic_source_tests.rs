use super::*;
use crate::{FragmentConfig, SimulatedEmbeddingEndpoint, VectorNormalization};

struct Fixture {
    path: std::path::PathBuf,
    cva: Option<Cva>,
}
impl Fixture {
    fn new() -> Self {
        let path =
            std::env::temp_dir().join(format!("semantic-source-{}.rel", uuid::Uuid::new_v4()));
        Self {
            cva: Some(Cva::create(&path).unwrap()),
            path,
        }
    }
    fn cva(&mut self) -> &mut Cva {
        self.cva.as_mut().unwrap()
    }
    fn turn(&mut self, id: &str, parent: Option<&str>, role: &str) -> Node {
        self.cva()
            .append_node(
                id.into(),
                "c".into(),
                parent.map(str::to_owned),
                role.into(),
                0,
                "genuine accepted content",
            )
            .unwrap()
    }
    fn generation(&mut self) -> VectorGeneration {
        let endpoint = SimulatedEmbeddingEndpoint::new(12, VectorNormalization::L2, 123);
        let profile = self
            .cva()
            .establish_compatibility_profile(&endpoint)
            .unwrap();
        self.cva()
            .build_archive_vector_generation(profile.id, &endpoint)
            .unwrap()
    }
    fn fragments(&mut self, leaf: &str, turns: usize) {
        self.cva()
            .materialize_path_fragments("c", leaf, FragmentConfig { turns, overlap: 0 }, true)
            .unwrap();
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        drop(self.cva.take());
        std::fs::remove_file(&self.path).unwrap();
    }
}
fn ready(outcome: SemanticSourceReadiness) -> AcceptedSemanticSource {
    match outcome {
        SemanticSourceReadiness::Ready(source) => source,
        _ => panic!("not ready"),
    }
}

#[test]
fn pending_is_not_empty_completion_and_non_turns_are_rejected() {
    let mut f = Fixture::new();
    let node = f.turn("n", None, "user");
    assert_eq!(
        f.cva()
            .prepare_accepted_semantic_source(&node, None)
            .unwrap(),
        SemanticSourceReadiness::Pending
    );
    let system = f.turn("sys", Some("n"), "system");
    assert!(
        f.cva()
            .prepare_accepted_semantic_source(&system, None)
            .is_err()
    );
    let mut conflict = node.clone();
    conflict.content_id.0[0] ^= 1;
    assert!(
        f.cva()
            .prepare_accepted_semantic_source(&conflict, None)
            .is_err()
    );
}

#[test]
fn exact_source_retries_and_reopen_preserve_selected_evidence_without_writes() {
    let mut f = Fixture::new();
    let node = f.turn("n", None, "assistant");
    f.fragments("n", 1);
    let generation = f.generation();
    let before = (
        f.cva().latest_global_version(),
        f.cva().archive_version(),
        f.cva().vector_version(),
    );
    let first = ready(
        f.cva()
            .prepare_accepted_semantic_source(&node, Some(generation.id))
            .unwrap(),
    );
    assert_eq!(first.activity_position, 1);
    assert_eq!(first.vectors.len(), 1);
    assert_eq!(first.owner_uuid, f.cva().owner_uuid().unwrap());
    assert_eq!(
        first.policy_digest,
        SemanticPolicy::default().digest().unwrap()
    );
    assert_eq!(
        before,
        (
            f.cva().latest_global_version(),
            f.cva().archive_version(),
            f.cva().vector_version()
        )
    );
    f.cva().sync().unwrap();
    drop(f.cva.take());
    f.cva = Some(Cva::open(&f.path).unwrap());
    let retry = ready(
        f.cva()
            .prepare_accepted_semantic_source(&node, Some(generation.id))
            .unwrap(),
    );
    assert_eq!(retry, first);
    f.turn("later", Some("n"), "user");
    assert_eq!(
        ready(
            f.cva()
                .prepare_accepted_semantic_source(&node, Some(generation.id))
                .unwrap()
        ),
        first
    );
}

#[test]
fn mixed_turn_generation_is_explicit_empty_not_a_genuine_turn_vector() {
    let mut f = Fixture::new();
    let first = f.turn("a", None, "user");
    f.turn("b", Some("a"), "assistant");
    f.fragments("b", 2);
    let generation = f.generation();
    let source = ready(
        f.cva()
            .prepare_accepted_semantic_source(&first, Some(generation.id))
            .unwrap(),
    );
    assert!(source.vectors.is_empty());
    assert_eq!(source.excluded_mixed_fragments, 1);
}

#[test]
fn stale_generation_cannot_claim_a_future_source_and_rotation_does_not_rebind() {
    let mut f = Fixture::new();
    let first = f.turn("a", None, "user");
    f.fragments("a", 1);
    let old = f.generation();
    let pinned = ready(
        f.cva()
            .prepare_accepted_semantic_source(&first, Some(old.id))
            .unwrap(),
    );
    let later = f.turn("b", Some("a"), "assistant");
    assert_eq!(
        f.cva()
            .prepare_accepted_semantic_source(&later, Some(old.id))
            .unwrap(),
        SemanticSourceReadiness::Pending
    );
    f.fragments("b", 1);
    f.generation();
    assert_eq!(
        ready(
            f.cva()
                .prepare_accepted_semantic_source(&first, Some(old.id))
                .unwrap()
        ),
        pinned
    );
}

#[test]
fn incomplete_mapping_stays_pending_even_with_one_valid_source_vector() {
    let mut f = Fixture::new();
    let node = f.turn("a", None, "user");
    f.turn("b", Some("a"), "assistant");
    f.fragments("b", 1);
    let old = f.generation();
    f.fragments("b", 2);
    // Existing generation is valid but does not cover the mixed fragment at this newer cut.
    let source_archive_version = f.cva().archive_version();
    let newer = f
        .cva()
        .publish_vector_generation(
            old.compatibility_profile_id,
            old.archive_vector_id,
            source_archive_version,
        )
        .unwrap();
    assert_eq!(
        f.cva()
            .prepare_accepted_semantic_source(&node, Some(newer.id))
            .unwrap(),
        SemanticSourceReadiness::Pending
    );
}

#[test]
fn candidate_search_filters_before_limit_and_deduplicates_targets() {
    use crate::freshness::semantic_policy::SemanticPolicy;
    let source_node = Node {
        id: "source".into(),
        conversation_id: "conversation".into(),
        parent_id: None,
        role: "user".into(),
        principal_id: None,
        timestamp_ns: 0,
        content_id: crate::ContentId([1; 32]),
    };
    let source = AcceptedSemanticSource {
        owner_uuid: [9; 16],
        node: source_node,
        activity_position: 10,
        generation: VectorGeneration {
            id: VectorGenerationId([1; 32]),
            compatibility_profile_id: crate::CompatibilityProfileId([2; 32]),
            archive_vector_id: crate::ArchiveVectorId([3; 32]),
            source_archive_version: 1,
            global_version: 1,
            vector_version: 1,
        },
        base_policy: crate::freshness::FreshnessPolicy::default(),
        semantic_policy: SemanticPolicy {
            candidate_limit: 1,
            ..SemanticPolicy::default()
        },
        policy_digest: [4; 32],
        source_digest: [5; 32],
        vectors: vec![
            SemanticSourceVector {
                fragment_id: FragmentId([1; 32]),
                row: 0,
                components: vec![1.0, 0.0],
            },
            SemanticSourceVector {
                fragment_id: FragmentId([2; 32]),
                row: 1,
                components: vec![0.9, 0.1],
            },
        ],
        excluded_mixed_fragments: 0,
        invalid_vectors: 0,
    };
    fn memory(id: u8, lifecycle: &str, source: Option<(&str, &str)>) -> crate::Memory {
        crate::Memory {
            id: crate::MemoryId([id; 32]),
            revision: 1,
            category: "fact".into(),
            memory_type: "project".into(),
            authority_kind: "unknown".into(),
            temporal_status: "unknown".into(),
            title: "title".into(),
            content: "content".into(),
            scope: "project".into(),
            lifecycle_state: lifecycle.into(),
            archived: false,
            superseded_by: None,
            parent_id: None,
            source_node_id: None,
            content_source_conversation_id: source.map(|(conversation, _)| conversation.into()),
            content_source_node_id: source.map(|(_, node)| node.into()),
            grounding_source_conversation_id: None,
            grounding_source_node_id: None,
            source_episode_id: None,
            source_time_ns: None,
            source_ref: None,
            temporal_inference: None,
            routing_metadata: None,
            mutation_id: format!("m{id}"),
            created_at_ns: 0,
            updated_at_ns: 0,
            global_version: 1,
            memory_version: 1,
        }
    }
    let target = |id, lifecycle, admitted, vector| SemanticTargetSnapshot {
        memory: memory(id, lifecycle, None),
        body_id: crate::MemoryBodyId([id; 32]),
        admitted_at_turn: admitted,
        vector,
    };
    let cohort = std::collections::BTreeSet::new();
    let mut provenance_target = target(4, "knowledge", Some(1), vec![1.0, 0.0]);
    provenance_target.memory.content_source_conversation_id = Some("conversation".into());
    provenance_target.memory.content_source_node_id = Some("source".into());
    let targets = vec![
        target(1, "knowledge", Some(1), vec![1.0, 0.0]),
        target(2, "knowledge", Some(20), vec![1.0, 0.0]),
        target(3, "extracted", Some(1), vec![1.0, 0.0]),
        provenance_target,
    ];
    let (effects, eligible, filtered) =
        plan_eligible_semantic_targets(&source, &cohort, targets).unwrap();
    assert_eq!(eligible, 1);
    assert_eq!(filtered, 3);
    assert_eq!(effects.len(), 1);
    assert_eq!(effects[0].memory_id, crate::MemoryId([1; 32]));
}
