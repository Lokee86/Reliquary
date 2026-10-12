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
        birth_turn: admitted,
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
        target(5, "knowledge", Some(1), vec![0.0, 0.0]),
        target(6, "knowledge", Some(1), vec![f32::NAN, 0.0]),
        target(7, "knowledge", None, vec![1.0, 0.0]),
    ];
    let (effects, eligible, filtered) =
        plan_eligible_semantic_targets(&source, &cohort, targets).unwrap();
    assert_eq!(eligible, 1);
    assert_eq!(filtered, 6);
    let mut same_cut_admission = target(8, "knowledge", Some(10), vec![1.0, 0.0]);
    same_cut_admission.birth_turn = Some(1);
    assert_eq!(
        plan_eligible_semantic_targets(&source, &cohort, [same_cut_admission])
            .unwrap()
            .0
            .len(),
        1
    );
    assert_eq!(effects.len(), 1);
    assert_eq!(effects[0].memory_id, crate::MemoryId([1; 32]));
}

#[test]
fn semantic_live_activation_is_prospective_cancellable_until_completion() {
    let mut f = Fixture::new();
    let old = f.turn("old", None, "user");
    f.fragments("old", 1);
    let generation = f.generation();
    assert!(!f.cva().semantic_freshness_enabled());
    assert_eq!(
        f.cva()
            .process_semantic_freshness(generation.id, 64)
            .unwrap()
            .completed_turns,
        0
    );
    assert!(f.cva().set_semantic_freshness_enabled(true).unwrap());
    assert!(!f.cva().set_semantic_freshness_enabled(true).unwrap());
    assert!(f.cva().set_semantic_freshness_enabled(false).unwrap());
    assert!(f.cva().set_semantic_freshness_enabled(true).unwrap());
    f.cva().sync().unwrap();
    drop(f.cva.take());
    f.cva = Some(Cva::open(&f.path).unwrap());
    assert!(f.cva().semantic_freshness_enabled());
    let pending = f.turn("new", Some("old"), "assistant");
    assert_eq!(
        f.cva()
            .process_semantic_freshness(generation.id, 64)
            .unwrap()
            .pending_turns,
        1
    );
    let generation = f.generation(); // provider creates genuine turn fragments and completes
    assert_eq!(
        f.cva()
            .process_semantic_freshness(generation.id, 64)
            .unwrap()
            .completed_turns,
        0
    );
    let owner = f.cva().owner_uuid().unwrap();
    assert!(
        !f.cva()
            .freshness
            .contains_event(&crate::freshness_storage::semantic_live_event_id(
                owner,
                &old.conversation_id,
                &old.id
            ))
            .unwrap()
    );
    assert!(
        f.cva()
            .freshness
            .contains_event(&crate::freshness_storage::semantic_live_event_id(
                owner,
                &pending.conversation_id,
                &pending.id
            ))
            .unwrap()
    );
    assert!(f.cva().set_semantic_freshness_enabled(false).is_err());
    drop(f.cva.take());
    f.cva = Some(Cva::open(&f.path).unwrap());
    assert!(f.cva().set_semantic_freshness_enabled(false).is_err());
}

fn live_draft(key: &str) -> crate::MemoryDraft {
    crate::MemoryDraft {
        category: "fact".into(),
        memory_type: "project".into(),
        authority_kind: "direct".into(),
        temporal_status: "current".into(),
        title: key.into(),
        content: "meaning-target".into(),
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
        mutation_id: key.into(),
        created_at_ns: 0,
        updated_at_ns: 0,
    }
}

struct ConstantSemanticEndpoint;
impl crate::EmbeddingEndpoint for ConstantSemanticEndpoint {
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
        Ok(inputs.iter().map(|_| vec![1.0, 0.0]).collect())
    }
}

#[test]
fn semantic_live_accepted_turn_is_local_exactly_once_and_excludes_async_births() {
    let mut f = Fixture::new();
    f.turn("baseline", None, "user");
    let target = f
        .cva()
        .publish_memory(None, 0, live_draft("target"))
        .unwrap()
        .0;
    let neighbor = f
        .cva()
        .publish_memory(None, 0, live_draft("neighbor"))
        .unwrap()
        .0;
    f.cva()
        .set_memory_relation(
            target.id,
            neighbor.id,
            crate::GraphRelationKind::Topical,
            true,
            0,
        )
        .unwrap();
    for n in 0..100 {
        f.turn(&format!("age-{n}"), None, "user");
    }
    f.cva().set_semantic_freshness_enabled(true).unwrap();
    let source = f
        .cva()
        .ingest_turn(crate::IncomingTurn {
            id: "live".into(),
            conversation_id: "c".into(),
            parent_id: None,
            role: "user".into(),
            principal_id: None,
            timestamp_ns: 0,
            content: "genuine accepted content".into(),
            attachments: Vec::new(),
            project_attachments: Vec::new(),
        })
        .unwrap()
        .node;
    let cohort = f
        .cva()
        .publish_memory(None, 0, live_draft("source-cohort"))
        .unwrap()
        .0;
    let endpoint = ConstantSemanticEndpoint;
    let profile = f.cva().establish_compatibility_profile(&endpoint).unwrap();
    let schema = crate::VectorSchema::new(2, crate::ScalarType::F32).unwrap();
    let mut bytes = Vec::new();
    for vector in [[1.0_f32, 0.0], [0.0, 1.0], [1.0, 0.0]] {
        for value in vector {
            bytes.extend(value.to_le_bytes());
        }
    }
    let packed = f
        .cva()
        .put_packed_vectors(crate::PackedVectors::from_bytes(schema, bytes).unwrap())
        .unwrap();
    let bodies = [target.id, neighbor.id, cohort.id]
        .into_iter()
        .map(|id| f.cva().memories.current_body_id(id).unwrap())
        .collect();
    f.cva()
        .put_memory_vectors(profile.id, packed, bodies)
        .unwrap();
    let batch = f
        .cva()
        .prepare_semantic_archive_batch(profile.id)
        .unwrap()
        .unwrap();
    use crate::EmbeddingEndpoint;
    let vectors = endpoint
        .embed(crate::EmbeddingMode::Document, &batch.texts)
        .unwrap();
    f.cva()
        .commit_semantic_archive_batch(batch, vectors)
        .unwrap();
    assert_eq!(f.cva().freshness_record(target.id).unwrap().score, 100);
    assert_eq!(
        f.cva()
            .freshness_record(neighbor.id)
            .unwrap()
            .score_at(102)
            .unwrap(),
        90
    );
    assert_eq!(f.cva().freshness_record(cohort.id).unwrap().score, 100);
    let owner = f.cva().owner_uuid().unwrap();
    let id = crate::freshness_storage::semantic_live_event_id(owner, "c", &source.id);
    let receipt = f.cva().freshness.event_receipt(&id).unwrap().unwrap();
    assert_eq!(
        receipt.2,
        std::collections::BTreeMap::from([(target.id, 10)])
    );
    let before = f.cva().latest_global_version();
    let generation = f.cva().current_vector_generation(profile.id).unwrap();
    assert_eq!(
        f.cva()
            .process_semantic_freshness(generation.id, 64)
            .unwrap()
            .completed_turns,
        0
    );
    assert_eq!(f.cva().latest_global_version(), before);
    drop(f.cva.take());
    f.cva = Some(Cva::open(&f.path).unwrap());
    assert_eq!(
        f.cva().freshness.event_receipt(&id).unwrap().unwrap(),
        receipt
    );
    assert!(
        f.cva()
            .prepare_semantic_archive_batch(profile.id)
            .unwrap()
            .is_none()
    );
}

#[test]
fn semantic_live_runtime_worker_retries_ready_archive_without_ingest_shortcut() {
    let mut f = Fixture::new();
    f.turn("baseline", None, "user");
    let target = f
        .cva()
        .publish_memory(None, 0, live_draft("worker-target"))
        .unwrap()
        .0;
    for n in 0..100 {
        f.turn(&format!("worker-age-{n}"), None, "user");
    }
    f.cva().set_semantic_freshness_enabled(true).unwrap();
    f.turn("worker-live", None, "user");
    let cva = f.cva.take().unwrap();
    let host = crate::ReliquaryRuntimeHost::start(
        crate::InteractionRuntime::new(cva),
        crate::ReliquaryRuntimeRoutes::new(
            None,
            None,
            None,
            None,
            Some(std::sync::Arc::new(ConstantSemanticEndpoint)),
        ),
        crate::InsomniaWorkerConfig {
            workers: 1,
            ..Default::default()
        },
        crate::EpisodePolicy::default(),
    )
    .unwrap();
    let owner: [u8; 16] = uuid::Uuid::parse_str(
        host.owner_id()
            .unwrap()
            .unwrap()
            .strip_prefix("rel-")
            .unwrap(),
    )
    .unwrap()
    .into_bytes();
    let id = crate::freshness_storage::semantic_live_event_id(owner, "c", "worker-live");
    let start = std::time::Instant::now();
    while !host.semantic_event_completed_for_test(&id).unwrap()
        && start.elapsed() < std::time::Duration::from_secs(10)
    {
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(host.semantic_event_completed_for_test(&id).unwrap());
    assert_eq!(
        host.background_status().unwrap().reliquary_semantic_pending,
        0
    );
    let cva = host.into_cva().unwrap();
    f.cva = Some(cva);
    let owner = f.cva().owner_uuid().unwrap();
    let id = crate::freshness_storage::semantic_live_event_id(owner, "c", "worker-live");
    assert_eq!(
        f.cva().freshness.event_receipt(&id).unwrap().unwrap().2,
        std::collections::BTreeMap::from([(target.id, 10)])
    );
}

#[test]
fn semantic_live_activation_preserved_by_reclaim_and_divergent_reconcile() {
    let mut f = Fixture::new();
    f.turn("boundary", None, "user");
    f.cva().set_semantic_freshness_enabled(true).unwrap();
    let expected = f.cva().freshness.semantic_activation().cloned().unwrap();
    let right = f.path.with_extension("right.rel");
    let merged = f.path.with_extension("merged.rel");
    let reclaimed = f.path.with_extension("reclaimed.rel");
    f.cva().sync().unwrap();
    std::fs::copy(&f.path, &right).unwrap();
    f.turn("left-divergence", None, "assistant");
    {
        let mut other = Cva::open(&right).unwrap();
        other
            .append_node(
                "right-divergence".into(),
                "c".into(),
                None,
                "user".into(),
                0,
                "right",
            )
            .unwrap();
        other.sync().unwrap();
    }
    f.cva().reclaim_storage(&reclaimed).unwrap();
    let reclaimed_cva = Cva::open(&reclaimed).unwrap();
    assert_eq!(
        reclaimed_cva.freshness.semantic_activation(),
        Some(&expected)
    );
    drop(reclaimed_cva);
    drop(f.cva.take());
    Cva::reconcile(&f.path, &right, &merged).unwrap();
    {
        let mut result = Cva::open(&merged).unwrap();
        assert_eq!(result.freshness.semantic_activation(), Some(&expected));
        result.set_semantic_freshness_enabled(false).unwrap();
        result.set_semantic_freshness_enabled(true).unwrap();
        result.sync().unwrap();
        // Different prospective boundary is a conflict, never a silent enable.
    }
    let conflict = f.path.with_extension("conflict.rel");
    assert!(Cva::reconcile(&f.path, &merged, &conflict).is_err());
    f.cva = Some(Cva::open(&f.path).unwrap());
    for path in [right, merged, reclaimed] {
        std::fs::remove_file(path).unwrap();
    }
}

#[test]
#[ignore = "release measurement; run explicitly with --ignored --nocapture"]
fn semantic_release_measurement() {
    let mut f = Fixture::new();
    f.turn("base", None, "user");
    for n in 0..128 {
        f.cva()
            .publish_memory(None, 0, live_draft(&format!("bench-memory-{n}")))
            .unwrap();
    }
    for n in 0..100 {
        f.turn(&format!("bench-age-{n}"), None, "user");
    }
    f.cva().set_semantic_freshness_enabled(true).unwrap();
    for n in 0..140 {
        f.turn(&format!("bench-source-{n}"), None, "user");
    }
    let endpoint = ConstantSemanticEndpoint;
    let profile = f.cva().establish_compatibility_profile(&endpoint).unwrap();
    f.cva()
        .build_missing_memory_vectors(profile.id, &endpoint)
        .unwrap();
    let batch = f
        .cva()
        .prepare_semantic_archive_batch(profile.id)
        .unwrap()
        .unwrap();
    use crate::EmbeddingEndpoint;
    let vectors = endpoint
        .embed(crate::EmbeddingMode::Document, &batch.texts)
        .unwrap();
    let event_start = std::time::Instant::now();
    f.cva()
        .commit_semantic_archive_batch(batch, vectors)
        .unwrap();
    let generation = f.cva().current_vector_generation(profile.id).unwrap();
    loop {
        let report = f
            .cva()
            .process_semantic_freshness(generation.id, 64)
            .unwrap();
        if report.completed_turns == 0 {
            break;
        }
    }
    let event_ms = event_start.elapsed().as_secs_f64() * 1000.0;
    let node = f
        .cva()
        .archive
        .nodes
        .get("c", "bench-source-0")
        .unwrap()
        .clone();
    let source = ready(
        f.cva()
            .prepare_accepted_semantic_source(&node, Some(generation.id))
            .unwrap(),
    );
    let search_start = std::time::Instant::now();
    for _ in 0..32 {
        let result = f
            .cva()
            .search_eligible_semantic_targets(&source, &Default::default())
            .unwrap();
        assert_eq!(result.scanned_memories, 128);
        assert_eq!(result.effects.len(), 64);
    }
    let search_ms = search_start.elapsed().as_secs_f64() * 1000.0;
    let (receipts, bytes) = f.cva().freshness.recent_receipt_stats();
    assert!(receipts <= 128 && bytes <= 256 * 1024);
    let before = std::fs::metadata(&f.path).unwrap().len();
    f.cva()
        .apply_accepted_semantic_reinforcement(&node, &Default::default())
        .unwrap();
    assert_eq!(std::fs::metadata(&f.path).unwrap().len(), before);
    drop(f.cva.take());
    let open_start = std::time::Instant::now();
    f.cva = Some(Cva::open(&f.path).unwrap());
    let reopen_ms = open_start.elapsed().as_secs_f64() * 1000.0;
    f.cva()
        .apply_accepted_semantic_reinforcement(&node, &Default::default())
        .unwrap();
    assert_eq!(std::fs::metadata(&f.path).unwrap().len(), before);
    println!(
        "SEMANTIC_RELEASE targets=128 turns=140 search_iterations=32 search_ms={search_ms:.3} event_ms={event_ms:.3} reopen_ms={reopen_ms:.3} receipts={receipts} receipt_bytes={bytes}"
    );
}

#[test]
fn semantic_live_cancelled_activation_branch_cannot_be_silently_reenrolled() {
    let mut f = Fixture::new();
    f.turn("cut", None, "user");
    f.cva().set_semantic_freshness_enabled(true).unwrap();
    f.cva().sync().unwrap();
    let right = f.path.with_extension("cancelled.rel");
    let output = f.path.with_extension("cancel-conflict.rel");
    std::fs::copy(&f.path, &right).unwrap();
    f.turn("left", None, "user");
    {
        let mut other = Cva::open(&right).unwrap();
        other.set_semantic_freshness_enabled(false).unwrap();
        other.sync().unwrap();
    }
    drop(f.cva.take());
    assert!(Cva::reconcile(&f.path, &right, &output).is_err());
    f.cva = Some(Cva::open(&f.path).unwrap());
    f.cva().set_semantic_freshness_enabled(false).unwrap();
    f.cva().sync().unwrap();
    drop(f.cva.take());
    Cva::reconcile(&f.path, &right, &output).unwrap();
    {
        let merged = Cva::open(&output).unwrap();
        assert!(!merged.semantic_freshness_enabled());
        assert!(merged.freshness.semantic_activation_seen());
    }
    f.cva = Some(Cva::open(&f.path).unwrap());
    std::fs::remove_file(output).unwrap();
    std::fs::remove_file(right).unwrap();
}
