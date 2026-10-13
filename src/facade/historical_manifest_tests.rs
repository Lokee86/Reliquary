use super::*;
use crate::MemoryDraft;
fn singleton(cva: &mut Cva, name: &str, source: bool) -> MemoryId {
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
                content: format!("Direct native {name}"),
                scope: "private".into(),
                lifecycle_state: "knowledge".into(),
                archived: false,
                superseded_by: None,
                parent_id: None,
                source_node_id: None,
                source_episode_id: None,
                source_time_ns: None,
                content_source_conversation_id: source.then(|| "c1".into()),
                content_source_node_id: source.then(|| "u0".into()),
                grounding_source_conversation_id: None,
                grounding_source_node_id: None,
                mutation_id: format!("native-singleton:{name}"),
                created_at_ns: 400,
                updated_at_ns: 400,
            },
        )
        .unwrap()
        .0
        .id;
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
    cva.reconcile_dream_lifecycle(id).unwrap();
    cva.accept_initial_dream_settlement(&pass, cva.rel_turn_count())
        .unwrap();
    crate::dream_freshness::settle_initial_pass(cva, id, true, 401).unwrap();
    id
}
#[test]
fn historical_manifest_native_singletons_include_isolated_and_preserve_unsupported() {
    let (path, mut cva, _) = historical_import_fixture("h1-singletons.rel");
    let isolated = singleton(&mut cva, "isolated", true);
    let unsupported = singleton(&mut cva, "unsupported", false);
    let untouched = cva.freshness_record(unsupported);
    let manifest = manifest::build(&mut cva).unwrap();
    assert_eq!(manifest.initial[&isolated].admitted_at_turn, Some(1));
    assert!(!manifest.initial.contains_key(&unsupported));
    let prepared = cva
        .freeze_historical_import()
        .unwrap()
        .prepare_native()
        .unwrap();
    assert!(
        cva.reconstruct_historical_freshness(Default::default())
            .unwrap()
            .committed
    );
    assert_eq!(cva.freshness_record(unsupported), untouched);
    drop(cva);
    let reopened = Cva::open(&path).unwrap();
    assert_eq!(reopened.freshness_record(unsupported), untouched);
    assert_eq!(
        reopened.freshness_record(isolated),
        Some(prepared.records()[&isolated])
    );
}
#[test]
fn historical_manifest_missing_declared_source_rejects_without_projection() {
    let (path, mut cva, _) = historical_import_fixture("h1-declared-source.rel");
    let id = singleton(&mut cva, "declared", true);
    let memory = cva.memory(id).unwrap();
    let draft = MemoryDraft {
        category: memory.category,
        memory_type: memory.memory_type,
        authority_kind: memory.authority_kind,
        temporal_status: memory.temporal_status,
        title: memory.title,
        content: memory.content,
        scope: memory.scope,
        lifecycle_state: memory.lifecycle_state,
        archived: false,
        superseded_by: None,
        parent_id: None,
        source_node_id: None,
        source_episode_id: None,
        source_time_ns: None,
        content_source_conversation_id: Some("foreign-owner".into()),
        content_source_node_id: Some("missing".into()),
        grounding_source_conversation_id: None,
        grounding_source_node_id: None,
        mutation_id: "declared-source-drift".into(),
        created_at_ns: memory.created_at_ns,
        updated_at_ns: 402,
    };
    cva.sync().unwrap();
    let before = fs::read(&path).unwrap();
    assert!(
        cva.publish_memory(Some(id), memory.revision, draft)
            .is_err()
    );
    assert_eq!(fs::read(&path).unwrap(), before);
}
