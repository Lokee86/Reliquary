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
fn semantic_completion_retry_after_rotation_is_read_only_and_reopens() {
    let mut f = Fixture::new();
    let node = f.turn("accepted", None, "user");
    f.fragments("accepted", 1);
    f.generation();
    let cohort = std::collections::BTreeSet::new();
    assert_eq!(
        f.cva()
            .apply_accepted_semantic_reinforcement(&node, &cohort)
            .unwrap(),
        0
    );
    let completed = f.cva().latest_global_version();
    assert_eq!(
        f.cva()
            .apply_accepted_semantic_reinforcement(&node, &cohort)
            .unwrap(),
        0
    );
    assert_eq!(f.cva().latest_global_version(), completed);
    f.turn("later", Some("accepted"), "assistant");
    f.fragments("later", 1);
    f.generation();
    let rotated = f.cva().latest_global_version();
    f.cva()
        .apply_accepted_semantic_reinforcement(&node, &cohort)
        .unwrap();
    assert_eq!(f.cva().latest_global_version(), rotated);
    f.cva().sync().unwrap();
    drop(f.cva.take());
    f.cva = Some(Cva::open(&f.path).unwrap());
    let reopened = f.cva().latest_global_version();
    f.cva()
        .apply_accepted_semantic_reinforcement(&node, &cohort)
        .unwrap();
    assert_eq!(f.cva().latest_global_version(), reopened);
    let mut forged = node;
    forged.content_id.0[0] ^= 1;
    assert!(
        f.cva()
            .apply_accepted_semantic_reinforcement(&forged, &cohort)
            .is_err()
    );
    assert_eq!(f.cva().latest_global_version(), reopened);
}

#[test]
fn semantic_evidence_survives_divergent_rebase_twice_merge_and_reclamation() {
    struct TestDir(std::path::PathBuf);
    impl TestDir {
        fn path(&self) -> &std::path::Path {
            &self.0
        }
    }
    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let dir =
        TestDir(std::env::temp_dir().join(format!("semantic-transform-{}", uuid::Uuid::new_v4())));
    std::fs::create_dir(&dir.0).unwrap();
    let left_path = dir.path().join("left.rel");
    let right_path = dir.path().join("right.rel");
    let merged_path = dir.path().join("merged.rel");
    let twice_path = dir.path().join("twice.rel");
    let reclaimed_path = dir.path().join("reclaimed.rel");
    let mut left = Cva::create(&left_path).unwrap();
    left.append_node("base".into(), "c".into(), None, "user".into(), 0, "base")
        .unwrap();
    let draft = crate::MemoryDraft {
        category: "fact".into(),
        memory_type: "project".into(),
        authority_kind: "direct".into(),
        temporal_status: "current".into(),
        title: "older target".into(),
        content: "older knowledge".into(),
        scope: "project".into(),
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
        mutation_id: "target".into(),
        created_at_ns: 1,
        updated_at_ns: 1,
    };
    let (target, _) = left.publish_memory(None, 0, draft).unwrap();
    for n in 0..100 {
        left.append_node(
            format!("shared-{n}"),
            "c".into(),
            None,
            "user".into(),
            n,
            "activity",
        )
        .unwrap();
    }
    left.sync().unwrap();
    std::fs::copy(&left_path, &right_path).unwrap();
    let mut right = Cva::open(&right_path).unwrap();
    left.append_node(
        "left".into(),
        "c".into(),
        Some("base".into()),
        "user".into(),
        0,
        "left",
    )
    .unwrap();
    let source = right
        .append_node(
            "right".into(),
            "c".into(),
            Some("base".into()),
            "assistant".into(),
            0,
            "right",
        )
        .unwrap();
    right
        .materialize_path_fragments(
            "c",
            "right",
            FragmentConfig {
                turns: 1,
                overlap: 0,
            },
            true,
        )
        .unwrap();
    let endpoint = SimulatedEmbeddingEndpoint::new(12, VectorNormalization::L2, 123);
    let profile = right.establish_compatibility_profile(&endpoint).unwrap();
    let generation = right
        .build_archive_vector_generation(profile.id, &endpoint)
        .unwrap();
    let prepared = ready(
        right
            .prepare_accepted_semantic_source(&source, Some(generation.id))
            .unwrap(),
    );
    let effects = std::collections::BTreeMap::from([(target.id, 3)]);
    let event_id =
        crate::freshness_storage::semantic_live_event_id(prepared.owner_uuid, "c", "right");
    let evidence = crate::freshness_storage::SemanticEventEvidence {
        owner_uuid: prepared.owner_uuid,
        conversation_id: "c".into(),
        node_id: "right".into(),
        accepted_turn: prepared.activity_position,
        base_policy_digest: freshness_policy_digest(prepared.base_policy),
        semantic_policy_version: prepared.semantic_policy.version,
        semantic_policy_digest: prepared.policy_digest,
        compatibility_profile_id: profile.id.0,
        source_evidence_digest: prepared.source_digest,
        target_evidence_digest: semantic_target_digest(&effects),
    };
    right
        .freshness_replay_semantic_event(event_id, evidence, effects)
        .unwrap();
    assert_eq!(right.freshness_record(target.id).unwrap().score, 93);
    let original = right
        .freshness_replay_entries()
        .unwrap()
        .into_iter()
        .find_map(|entry| {
            if let crate::freshness_storage::FreshnessReplayEntry::Event {
                semantic: Some(evidence),
                ..
            } = entry
            {
                Some(evidence)
            } else {
                None
            }
        })
        .unwrap();
    assert_eq!(original.accepted_turn, 102);
    let durable_before = std::fs::read(&right_path).unwrap();
    let event_id =
        crate::freshness_storage::semantic_live_event_id(original.owner_uuid, "c", "right");
    let mut conflict = original.clone();
    let changed_effects = std::collections::BTreeMap::from([(target.id, 4)]);
    conflict.target_evidence_digest = semantic_target_digest(&changed_effects);
    assert!(
        right
            .freshness_replay_semantic_event(event_id.clone(), conflict, changed_effects)
            .is_err()
    );
    let mut missing_cut = original.clone();
    missing_cut.accepted_turn += 1;
    assert!(
        right
            .freshness_replay_semantic_event(
                event_id,
                missing_cut,
                std::collections::BTreeMap::from([(target.id, 3)])
            )
            .is_err()
    );
    assert_eq!(std::fs::read(&right_path).unwrap(), durable_before);
    left.sync().unwrap();
    right.sync().unwrap();
    drop(left);
    drop(right);
    Cva::reconcile(&left_path, &right_path, &merged_path).unwrap();
    let mut merged = Cva::open(&merged_path).unwrap();
    assert_eq!(merged.activity_position_for_turn("c", "right"), Some(103));
    let rebased = merged
        .freshness_replay_entries()
        .unwrap()
        .into_iter()
        .find_map(|entry| {
            if let crate::freshness_storage::FreshnessReplayEntry::Event {
                semantic: Some(evidence),
                ..
            } = entry
            {
                Some(evidence)
            } else {
                None
            }
        })
        .unwrap();
    let mut expected = original;
    expected.accepted_turn = 103;
    assert_eq!(rebased, expected);
    assert_eq!(merged.freshness_record(target.id).unwrap().score, 93);
    let before = std::fs::metadata(&merged_path).unwrap().len();
    merged
        .apply_accepted_semantic_reinforcement(&source, &std::collections::BTreeSet::new())
        .unwrap();
    assert_eq!(std::fs::metadata(&merged_path).unwrap().len(), before);
    let entries = merged.freshness_replay_entries().unwrap();
    merged.reclaim_storage(&reclaimed_path).unwrap();
    assert_eq!(
        Cva::open(&reclaimed_path)
            .unwrap()
            .freshness_replay_entries()
            .unwrap(),
        entries
    );
    drop(merged);
    Cva::reconcile(&merged_path, &right_path, &twice_path).unwrap();
    let twice = Cva::open(&twice_path).unwrap();
    assert_eq!(twice.freshness_replay_entries().unwrap(), entries);
    assert_eq!(twice.freshness_record(target.id).unwrap().score, 93);
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
