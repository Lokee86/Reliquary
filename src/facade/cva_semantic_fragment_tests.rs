//! Native N2 fragment-source correctness fixtures.
use super::*;

#[test]
fn semantic_fragment_excludes_a_whole_cohort_for_external_grounding_evidence() {
    let mut f = Fixture::new();
    f.turn("base", None, "user");
    let episode = f
        .cva()
        .materialize_path_episodes(
            "c",
            "base",
            crate::EpisodeConfig::default(),
            crate::EpisodeOrigin::Live,
            Some((crate::EpisodeBoundary::Explicit, 0)),
        )
        .unwrap()
        .created
        .remove(0);
    for n in 0..100 {
        f.turn(&format!("before-{n}"), None, "user");
    }
    f.cva().set_semantic_freshness_enabled(true).unwrap();
    f.turn("grounding", None, "user");
    let mut ids = Vec::new();
    for n in 0..2 {
        let mut draft = live_draft(&format!("grounded-cohort-{n}"));
        draft.source_episode_id = Some(episode.id);
        draft.source_node_id = Some("base".into());
        if n == 0 {
            draft.grounding_source_conversation_id = Some("c".into());
            draft.grounding_source_node_id = Some("grounding".into());
        }
        ids.push(f.cva().publish_memory(None, 0, draft).unwrap().0.id);
    }
    for n in 0..20 {
        f.turn(&format!("after-{n}"), None, "user");
    }
    f.turn("last", Some("grounding"), "assistant");
    f.fragments("last", 2);
    let profile = f
        .cva()
        .establish_compatibility_profile(&ConstantSemanticEndpoint)
        .unwrap();
    f.cva()
        .build_missing_memory_vectors(profile.id, &ConstantSemanticEndpoint)
        .unwrap();
    f.cva()
        .build_archive_vector_generation(profile.id, &ConstantSemanticEndpoint)
        .unwrap();
    for id in ids {
        assert_eq!(
            f.cva().freshness_record(id).unwrap().score_at(123).unwrap(),
            98
        );
    }
}

#[test]
fn semantic_mixed_fragment_availability_excludes_every_source_and_credits_once() {
    let mut f = Fixture::new();
    f.turn("base", None, "user");
    let target = f
        .cva()
        .publish_memory(None, 0, live_draft("mixed-target"))
        .unwrap()
        .0;
    let neighbor = f
        .cva()
        .publish_memory(None, 0, live_draft("mixed-neighbor"))
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
    let first = f.turn("mixed-first", None, "user");
    let mut self_draft = live_draft("earlier-source-memory");
    self_draft.content_source_conversation_id = Some("c".into());
    self_draft.content_source_node_id = Some(first.id.clone());
    let own = f.cva().publish_memory(None, 0, self_draft).unwrap().0;
    f.cva()
        .materialize_path_episodes(
            "c",
            &first.id,
            crate::EpisodeConfig::default(),
            crate::EpisodeOrigin::Live,
            Some((crate::EpisodeBoundary::Explicit, 0)),
        )
        .unwrap();
    let episode = f
        .cva()
        .episodes()
        .into_iter()
        .find(|e| e.end_node_id == first.id)
        .unwrap();
    let mut cohort_draft = live_draft("earlier-episode-cohort");
    cohort_draft.source_episode_id = Some(episode.id);
    cohort_draft.source_node_id = Some(first.id.clone());
    let cohort = f.cva().publish_memory(None, 0, cohort_draft).unwrap().0;
    let last = f.turn("mixed-last", Some(&first.id), "assistant");
    f.fragments(&last.id, 8);
    let fragment = f
        .cva()
        .fragments()
        .into_iter()
        .find(|f| f.end_node_id == last.id)
        .unwrap();
    assert_eq!(fragment.start_node_id, first.id);
    let endpoint = ConstantSemanticEndpoint;
    let profile = f.cva().establish_compatibility_profile(&endpoint).unwrap();
    let schema = crate::VectorSchema::new(2, crate::ScalarType::F32).unwrap();
    let rows = [[1.0_f32, 0.0], [0.0, 1.0], [1.0, 0.0], [1.0, 0.0]];
    let packed = f
        .cva()
        .put_packed_vectors(
            crate::PackedVectors::from_bytes(
                schema,
                rows.into_iter()
                    .flatten()
                    .flat_map(f32::to_le_bytes)
                    .collect(),
            )
            .unwrap(),
        )
        .unwrap();
    let bodies = [target.id, neighbor.id, own.id, cohort.id]
        .into_iter()
        .map(|id| f.cva().memories.current_body_id(id).unwrap())
        .collect();
    f.cva()
        .put_memory_vectors(profile.id, packed, bodies)
        .unwrap();
    let fragments_before = f.cva().fragments();
    let generation = f
        .cva()
        .build_archive_vector_generation(profile.id, &endpoint)
        .unwrap();
    assert_eq!(f.cva().fragments(), fragments_before); // No Freshness-only fragments.
    let prepared = ready(
        f.cva()
            .prepare_semantic_fragment(fragment.id, generation.id)
            .unwrap(),
    );
    assert_eq!(prepared.activity_position, 103);
    assert_eq!(prepared.vectors.len(), 1);
    assert_eq!(prepared.fragment.as_ref().unwrap().sources.len(), 2);
    let id = crate::freshness_storage::semantic_fragment_event_id(prepared.owner_uuid, fragment.id);
    let receipt = f.cva().freshness.event_receipt(&id).unwrap().unwrap();
    assert_eq!(
        receipt.2,
        std::collections::BTreeMap::from([(target.id, 10)])
    );
    assert_eq!(
        f.cva()
            .freshness_record(neighbor.id)
            .unwrap()
            .score_at(103)
            .unwrap(),
        90
    );
    assert_eq!(f.cva().freshness_record(own.id).unwrap().score, 100);
    assert_eq!(f.cva().freshness_record(cohort.id).unwrap().score, 100);
    let before = f.cva().latest_global_version();
    f.cva()
        .apply_accepted_semantic_reinforcement(&first, &Default::default())
        .unwrap();
    f.cva()
        .apply_accepted_semantic_reinforcement(&last, &Default::default())
        .unwrap();
    assert_eq!(f.cva().latest_global_version(), before);
    // Different generation/profile evidence never creates another fragment award.
    let rotated = f.generation();
    assert_ne!(
        generation.compatibility_profile_id,
        rotated.compatibility_profile_id
    );
    let before = f.cva().latest_global_version();
    assert_eq!(
        f.cva()
            .process_semantic_freshness(rotated.id, 64)
            .unwrap()
            .completed_fragments,
        0
    );
    assert_eq!(f.cva().latest_global_version(), before);
    drop(f.cva.take());
    f.cva = Some(Cva::open(&f.path).unwrap());
    assert_eq!(
        f.cva().freshness.event_receipt(&id).unwrap().unwrap(),
        receipt
    );
    assert_eq!(f.cva().semantic_freshness_pending_fragments().unwrap(), 0);
}

#[test]
fn semantic_overlap_and_nonactivity_context_do_not_repeat_fragment_credit() {
    let mut f = Fixture::new();
    f.cva().set_semantic_freshness_enabled(true).unwrap();
    let first = f.turn("first", None, "user");
    f.turn("context", Some("first"), "system");
    f.turn("second", Some("context"), "assistant");
    f.fragments("second", 8);
    let mixed = f
        .cva()
        .fragments()
        .into_iter()
        .find(|f| f.end_node_id == "second")
        .unwrap();
    f.fragments("second", 1);
    let before = f.cva().fragments().len();
    let generation = f.generation();
    assert_eq!(f.cva().fragments().len(), before);
    let source = ready(
        f.cva()
            .prepare_semantic_fragment(mixed.id, generation.id)
            .unwrap(),
    );
    assert_eq!(source.activity_position, 2);
    assert!(source.represents("c", "context"));
    let entries = f.cva().freshness_replay_entries().unwrap();
    let events: Vec<_> = entries
        .iter()
        .filter_map(|e| match e {
            crate::freshness_storage::FreshnessReplayEntry::Event {
                semantic: Some(e), ..
            } => Some(e),
            _ => None,
        })
        .collect();
    assert_eq!(events.len(), before - 1); // System-only fragment has no activity event.
    assert!(events.iter().all(|e| e.fragment.is_some()));
    assert_eq!(
        events
            .iter()
            .filter(|e| e.fragment.as_ref().unwrap().fragment_id == mixed.id)
            .count(),
        1
    );
    let version = f.cva().latest_global_version();
    f.cva()
        .apply_accepted_semantic_reinforcement(&first, &Default::default())
        .unwrap();
    assert_eq!(f.cva().latest_global_version(), version);
}

#[test]
fn semantic_fragment_waits_for_its_own_row_and_selected_archive_cut() {
    let mut f = Fixture::new();
    f.turn("first", None, "user");
    f.fragments("first", 1);
    let old = f.generation();
    f.cva().set_semantic_freshness_enabled(true).unwrap();
    f.turn("later", Some("first"), "assistant");
    f.fragments("later", 8);
    let mixed = f
        .cva()
        .fragments()
        .into_iter()
        .find(|f| f.start_node_id == "first" && f.end_node_id == "later")
        .unwrap();
    let before = f.cva().latest_global_version();
    assert_eq!(
        f.cva().prepare_semantic_fragment(mixed.id, old.id).unwrap(),
        SemanticSourceReadiness::Pending
    );
    let archive_version = f.cva().archive_version();
    let incomplete = f
        .cva()
        .publish_vector_generation(
            old.compatibility_profile_id,
            old.archive_vector_id,
            archive_version,
        )
        .unwrap();
    assert_eq!(
        f.cva()
            .prepare_semantic_fragment(mixed.id, incomplete.id)
            .unwrap(),
        SemanticSourceReadiness::Pending
    );
    assert_eq!(f.cva().latest_global_version(), before + 1);
    let fragment_count = f.cva().fragments().len();
    let batch = f
        .cva()
        .prepare_semantic_archive_batch(old.compatibility_profile_id)
        .unwrap()
        .unwrap();
    assert_eq!(batch.texts.len(), fragment_count);
    assert_eq!(f.cva().fragments().len(), fragment_count);
    assert_eq!(f.cva().semantic_freshness_pending_fragments().unwrap(), 1);
}
