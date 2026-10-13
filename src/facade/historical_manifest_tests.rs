use super::*;
use crate::MemoryDraft;

fn field<'a>(value: &'a encoding::Value, name: &str) -> &'a encoding::Value {
    let encoding::Value::Map(fields) = value else {
        panic!("not a map");
    };
    &fields.iter().find(|(key, _)| key == name).unwrap().1
}

#[test]
fn historical_manifest_pins_mixed_fragment_bindings_availability_and_generation() {
    use encoding::Value;
    let (_, mut cva, _) = historical_import_fixture("n3-manifest.rel");
    historical_events::vectors(&mut cva);
    let generation = *cva
        .vector_generations
        .generations()
        .iter()
        .max_by_key(|g| g.vector_version)
        .unwrap();
    let manifest = manifest::build_with_fill(&mut cva, true).unwrap();
    assert_eq!(
        field(&manifest.value, "schema_version"),
        &Value::Unsigned(2)
    );
    let proof = field(&manifest.value, "vector_proof");
    let Value::Array(records) = field(proof, "fragment_sources") else {
        panic!("not an array");
    };
    assert_eq!(records.len(), 16);
    for fragment in cva.fragments() {
        let record = records
            .iter()
            .find(|r| field(r, "fragment_id") == &Value::bytes(fragment.id.0))
            .unwrap();
        let position = cva
            .semantic_fragment_position(fragment.id)
            .unwrap()
            .unwrap();
        assert_eq!(field(record, "availability"), &Value::Unsigned(position));
        let Value::Array(nodes) = field(record, "represented_nodes") else {
            panic!("not an array");
        };
        assert_eq!(nodes.len(), 2);
        let evidence = field(record, "evidence");
        assert_eq!(
            field(evidence, "generation_id"),
            &Value::bytes(generation.id.0)
        );
        assert_eq!(
            field(evidence, "profile_id"),
            &Value::bytes(generation.compatibility_profile_id.0)
        );
        let Value::Array(rows) = field(evidence, "usable_rows") else {
            panic!("not an array");
        };
        assert_eq!(rows.len(), 1);
        assert_eq!(field(&rows[0], "fragment_id"), &Value::bytes(fragment.id.0));
    }
    assert_eq!(
        field(field(proof, "embedding_pass"), "contract_version"),
        &Value::Unsigned(2)
    );
}

#[test]
fn historical_generation_refresh_changes_proof_but_never_fragment_event_identity() {
    let (_, mut cva, _) = historical_import_fixture("n3-generation-proof.rel");
    historical_events::vectors(&mut cva);
    let first = cva.freeze_historical_import().unwrap();
    let generation = *cva
        .vector_generations
        .generations()
        .iter()
        .max_by_key(|g| g.vector_version)
        .unwrap();
    let set = cva.archive_vectors(generation.archive_vector_id).unwrap();
    let schema = crate::VectorSchema::new(2, crate::ScalarType::F32).unwrap();
    let bytes = (0..set.fragment_ids.len())
        .flat_map(|_| [0.8_f32, 0.6].into_iter().flat_map(f32::to_le_bytes))
        .collect();
    let packed = cva
        .put_packed_vectors(crate::PackedVectors::from_bytes(schema, bytes).unwrap())
        .unwrap();
    let binding = cva.put_archive_vectors(packed, set.fragment_ids).unwrap();
    cva.publish_vector_generation(
        generation.compatibility_profile_id,
        binding,
        cva.archive_version(),
    )
    .unwrap();
    let refreshed = cva.freeze_historical_import().unwrap();
    assert_ne!(
        first.checkpoint.manifest_digest,
        refreshed.checkpoint.manifest_digest
    );
    let a = &first.events.as_ref().unwrap().semantic.events;
    let b = &refreshed.events.as_ref().unwrap().semantic.events;
    assert_eq!(a.len(), 8);
    assert_eq!(
        a.iter().map(|e| e.event_id).collect::<Vec<_>>(),
        b.iter().map(|e| e.event_id).collect::<Vec<_>>()
    );
    assert!(
        a.iter()
            .zip(b)
            .all(|(a, b)| a.source_position == b.source_position && a.effects != b.effects)
    );
}
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
