use super::*;
use crate::freshness::{
    historical_dream::{HistoricalDreamCoverage, historical_dream_event_id},
    historical_propagation::HistoricalPropagationKind,
    historical_semantic::historical_semantic_event_id,
};
use crate::{
    EmbeddingEndpoint, EmbeddingEndpointError, EmbeddingMode, GraphRelationKind,
    GraphRelationOrigin, MemoryDraft, VectorNormalization,
};
struct ConstantEndpoint;
impl EmbeddingEndpoint for ConstantEndpoint {
    fn dimensions(&self) -> u32 {
        2
    }
    fn normalization(&self) -> VectorNormalization {
        VectorNormalization::L2
    }
    fn embed(
        &self,
        _: EmbeddingMode,
        texts: &[String],
    ) -> Result<Vec<Vec<f32>>, EmbeddingEndpointError> {
        Ok(vec![vec![1.0, 0.0]; texts.len()])
    }
}
fn vectors(cva: &mut Cva) {
    for conversation in ["c1", "c2"] {
        cva.materialize_path_fragments(
            conversation,
            "a7",
            crate::FragmentConfig {
                turns: 1,
                overlap: 0,
            },
            true,
        )
        .unwrap();
    }
    let profile = cva
        .establish_compatibility_profile(&ConstantEndpoint)
        .unwrap();
    cva.build_archive_vector_generation(profile.id, &ConstantEndpoint)
        .unwrap();
    cva.build_missing_memory_vectors(profile.id, &ConstantEndpoint)
        .unwrap();
}
#[test]
fn historical_native_semantic_source_cohort_is_excluded_before_roots_and_reopen() {
    let (path, mut cva, ids) = historical_import_fixture_count("h4-native-semantic.rel", 2);
    vectors(&mut cva);
    cva.sync().unwrap();
    let before = fs::read(&path).unwrap();
    let frozen = cva.freeze_historical_import().unwrap();
    let plan = frozen.events.as_ref().unwrap();
    assert_eq!(plan.semantic.events.len(), 16);
    assert!(plan.dream.events.is_empty());
    let cohort: BTreeSet<_> = ids.iter().copied().collect();
    for event in &plan.semantic.events {
        assert_eq!(event.source_key.conversation_id, "c2");
        assert!(event.source_position > 16);
        assert_eq!(
            event
                .effects
                .iter()
                .map(|e| e.memory_id)
                .collect::<BTreeSet<_>>(),
            cohort
        );
        assert_eq!(
            event.event_id,
            historical_semantic_event_id(
                cva.owner_uuid().unwrap(),
                frozen.import_id(),
                &event.source_key
            )
            .unwrap()
        );
        assert!(event.effects.iter().all(|e| e.bonus == 10));
    }
    // Every generating Episode source, including the admission boundary,
    // excludes both members of the verified complete grouped publication.
    assert!(
        !plan
            .semantic
            .events
            .iter()
            .any(|e| e.source_key.conversation_id == "c1")
    );
    assert_eq!(fs::read(&path).unwrap(), before);
    drop(cva);
    let mut reopened = Cva::open(&path).unwrap();
    let retried = reopened.freeze_historical_import().unwrap();
    assert_eq!(retried.events.as_ref(), frozen.events.as_ref());
    assert_eq!(fs::read(&path).unwrap(), before);
}
fn linked_source(cva: &mut Cva, target: MemoryId, duplicate: bool) -> MemoryId {
    let source = cva
        .publish_memory(
            None,
            0,
            MemoryDraft {
                category: "decision".into(),
                memory_type: "project".into(),
                authority_kind: "direct".into(),
                temporal_status: "current".into(),
                title: "new linked source".into(),
                content: "A verified later source.".into(),
                scope: "private".into(),
                lifecycle_state: "extracted".into(),
                archived: false,
                superseded_by: None,
                parent_id: None,
                source_node_id: None,
                source_episode_id: None,
                source_time_ns: None,
                content_source_conversation_id: Some("c2".into()),
                content_source_node_id: Some("a7".into()),
                grounding_source_conversation_id: None,
                grounding_source_node_id: None,
                mutation_id: format!("native-linked-source:{duplicate}"),
                created_at_ns: 400,
                updated_at_ns: 400,
            },
        )
        .unwrap()
        .0
        .id;
    let pass = crate::dream_freshness::initial_pass_id(cva.owner_uuid().unwrap(), source);
    cva.begin_initial_dream_pass(
        pass.clone(),
        source,
        1,
        cva.rel_turn_count(),
        BTreeSet::from([target]),
        cva.graph_version(),
    )
    .unwrap();
    cva.set_memory_relation_with_origin(
        source,
        target,
        if duplicate {
            GraphRelationKind::DuplicateOf
        } else {
            GraphRelationKind::Factual
        },
        true,
        GraphRelationOrigin::Dream,
        cva.graph_version(),
    )
    .unwrap();
    cva.reconcile_dream_lifecycle(source).unwrap();
    cva.accept_initial_dream_settlement(&pass, cva.rel_turn_count())
        .unwrap();
    let active = !cva.memory(source).unwrap().archived;
    crate::dream_freshness::settle_initial_pass(cva, source, active, 401).unwrap();
    source
}
#[test]
fn historical_native_actual_dream_no_link_duplicate_authority_and_event_order() {
    for duplicate in [false, true] {
        let (path, mut cva, target) =
            historical_import_fixture(&format!("h4-native-dream-{duplicate}.rel"));
        let source = linked_source(&mut cva, target, duplicate);
        vectors(&mut cva);
        cva.sync().unwrap();
        let before = fs::read(&path).unwrap();
        let frozen = cva.freeze_historical_import().unwrap();
        let plan = frozen.events.as_ref().unwrap();
        assert_eq!(
            plan.dream.coverage[&target],
            HistoricalDreamCoverage::VerifiedNoLink
        );
        assert_eq!(plan.dream.events.len(), 1);
        let event = &plan.dream.events[0];
        assert_eq!(event.source_id, source);
        assert_eq!(
            event.event_id,
            historical_dream_event_id(cva.owner_uuid().unwrap(), frozen.import_id(), source)
        );
        assert_eq!(event.propagation.position, 32);
        assert_eq!(event.propagation.roots, BTreeMap::from([(target, 50)]));
        assert!(event.propagation.credit_excluded.contains(&source));
        if duplicate {
            assert!(cva.memory(source).unwrap().archived);
            assert_eq!(frozen.initial[&source].admitted_at_turn, None);
        }
        assert!(
            plan.ordered
                .windows(2)
                .all(|p| (p[0].1.position, p[0].0) < (p[1].1.position, p[1].0))
        );
        let at_cut: Vec<_> = plan
            .ordered
            .iter()
            .filter(|(_, event)| event.position == 32)
            .collect();
        assert_eq!(at_cut.len(), 2);
        assert!(
            at_cut
                .iter()
                .any(|(_, e)| e.kind == HistoricalPropagationKind::Semantic)
        );
        assert!(
            at_cut
                .iter()
                .any(|(_, e)| e.kind == HistoricalPropagationKind::FirstCycleDream)
        );
        assert_eq!(fs::read(&path).unwrap(), before);
        drop(cva);
        let mut reopened = Cva::open(&path).unwrap();
        assert_eq!(
            reopened.freeze_historical_import().unwrap().events.as_ref(),
            frozen.events.as_ref()
        );
        assert_eq!(fs::read(&path).unwrap(), before);
    }
}
