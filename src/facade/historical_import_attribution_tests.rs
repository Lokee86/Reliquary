use super::*;
use crate::imports::{ImportBranch, ImportConversation, ImportMessage, NativeImport};
use crate::insomnia::completion_tests::{process, setup_origin};
use crate::{
    EpisodeOrigin, GraphRelationKind, GraphRelationOrigin, InsomniaExtractor, MemoryDraft,
    SimulatedGeneralEndpoint,
};
use serde_json::{Map, json};

fn imported_snapshot(id: &str) -> NativeImport {
    let mut messages = Vec::new();
    let mut parent = None;
    for index in 0..8 {
        let user = format!("u{index}");
        let assistant = format!("a{index}");
        messages.push(message(
            &user,
            parent.as_deref(),
            "user",
            &format!("Imported fact {index}."),
        ));
        messages.push(message(
            &assistant,
            Some(&user),
            "assistant",
            "Acknowledged.",
        ));
        parent = Some(assistant);
    }
    NativeImport {
        schema_version: 1,
        source_namespace: format!("attribution/{id}"),
        source_format: "attribution-test-v1".into(),
        conversations: vec![ImportConversation {
            id: id.into(),
            title: None,
            messages,
            branches: vec![ImportBranch {
                id: "main".into(),
                leaf_message_id: parent.unwrap(),
                canonical: true,
            }],
        }],
    }
}

fn message(id: &str, parent_id: Option<&str>, role: &str, content: &str) -> ImportMessage {
    ImportMessage {
        id: id.into(),
        revision: Some("1".into()),
        parent_id: parent_id.map(str::to_owned),
        replaces: None,
        deleted: false,
        role: role.into(),
        timestamp_ns: 100,
        content: content.into(),
        artifacts: vec![],
    }
}

fn settle_initial_pass(cva: &mut Cva, id: crate::MemoryId, _now_ns: i64) {
    cva.container.set_owner_write_scope(true);
    let result = (|| {
        let pass = crate::dream_freshness::initial_pass_id(cva.owner_uuid().unwrap(), id);
        cva.begin_initial_dream_pass(
            pass.clone(),
            id,
            1,
            cva.rel_turn_count(),
            Default::default(),
            cva.graph_version(),
        )?;
        cva.reconcile_dream_lifecycle(id)
            .map_err(|error| fail(&error.to_string()))?;
        cva.accept_initial_dream_settlement(&pass, cva.rel_turn_count())?;
        crate::dream_freshness::settle_initial_pass(cva, id, true, 1)
    })();
    cva.container.set_owner_write_scope(false);
    result.unwrap();
}

fn process_import_episode(cva: &mut Cva, now_ns: i64) {
    cva.container.set_owner_write_scope(true);
    let claim = cva
        .claim_insomnia_episode("import-attribution-worker", now_ns, 1_000)
        .unwrap()
        .unwrap();
    assert!(
        cva.import_epoch_scope()
            .unwrap()
            .episodes
            .contains(&claim.episode_id.0)
    );
    let turns = cva.episode_turns(claim.episode_id).unwrap();
    let mut turn_responses = Map::new();
    let mut groups = Map::new();
    let mut user_index = 0;
    for turn in turns.iter().filter(|turn| turn.role == "user") {
        let clause = if user_index == 0 {
            json!({
                "disposition": "retain",
                "authority_kind": "direct",
                "category": "decision",
                "type": "project",
                "lifecycle": "current",
                "proposition": "Imported attribution fact 0.",
                "authority_source_node_id": "",
                "grounding_source_node_id": "",
                "reason": "synthetic durable state"
            })
        } else {
            json!({
                "disposition": "omit",
                "authority_kind": "none",
                "category": "none",
                "type": "none",
                "lifecycle": "none",
                "proposition": "",
                "authority_source_node_id": "",
                "grounding_source_node_id": "",
                "reason": "synthetic omission"
            })
        };
        turn_responses.insert(turn.node_id.clone(), json!([clause]));
        if user_index == 0 {
            groups.insert(
                "g000".into(),
                json!({
                    "title": "Imported attribution",
                    "content": "Imported attribution fact 0."
                }),
            );
        }
        user_index += 1;
    }
    assert_eq!(
        user_index, 8,
        "fixture Episode must contain eight user turns"
    );
    let endpoint = SimulatedGeneralEndpoint::new(
        "synthetic-import-attribution",
        vec![
            json!({"turns": turn_responses, "evidence_requests": []}),
            json!({"groups": groups}),
        ],
    );
    let extractor = InsomniaExtractor::new(endpoint);
    let result = cva
        .process_claimed_insomnia_episode(&claim, &extractor, "private", now_ns, now_ns + 1)
        .unwrap();
    cva.container.set_owner_write_scope(false);
    assert_eq!(
        result.created.len(),
        1,
        "fixture must publish one import birth"
    );
}

fn field<'a>(value: &'a encoding::Value, name: &str) -> &'a encoding::Value {
    let encoding::Value::Map(fields) = value else {
        panic!("expected proof map")
    };
    &fields
        .iter()
        .find(|(key, _)| key == name)
        .unwrap_or_else(|| panic!("missing proof field {name}"))
        .1
}

fn unrelated_memory(cva: &mut Cva) -> crate::MemoryId {
    cva.publish_memory(
        None,
        0,
        MemoryDraft {
            category: "decision".into(),
            memory_type: "project".into(),
            authority_kind: "direct".into(),
            temporal_status: "current".into(),
            title: "Unrelated post-start memory".into(),
            content: "This Memory is outside the current import cohort.".into(),
            scope: "private".into(),
            lifecycle_state: "knowledge".into(),
            archived: false,
            superseded_by: None,
            parent_id: None,
            source_node_id: None,
            source_episode_id: None,
            source_time_ns: None,
            content_source_conversation_id: None,
            content_source_node_id: None,
            grounding_source_conversation_id: None,
            grounding_source_node_id: None,
            mutation_id: "historical-import-attribution-unrelated".into(),
            created_at_ns: 230,
            updated_at_ns: 230,
        },
    )
    .unwrap()
    .0
    .id
}

#[test]
fn scoped_admission_keeps_prestart_recipient_and_only_import_births_in_cohort() {
    let (path, mut cva, _) = setup_origin(
        "historical-import-attribution-live.rel",
        EpisodeOrigin::Live,
    );
    let old = process(&mut cva, 1, 110).created[0].id;
    cva.freshness_admit(old, cva.rel_turn_count()).unwrap();
    cva.record_accepted_memory_use(
        crate::AcceptedMemoryUseReceipt {
            owner_uuid: cva.owner_uuid().unwrap(),
            accepted_turn: cva.rel_turn_count(),
            use_id: "before-native-import".into(),
            memories: vec![old],
        },
        120,
    )
    .unwrap();
    settle_initial_pass(&mut cva, old, 121);
    cva.put_conversation_compaction("c1".into(), "a6".into(), "old mutable summary".into(), None)
        .unwrap();

    cva.import_native(&imported_snapshot("attributed-source"), 200)
        .unwrap();
    let scope = cva.import_epoch_scope().unwrap();
    assert_eq!(scope.episodes.len(), 1);
    assert!(!scope.cohort.contains(&old));
    process_import_episode(&mut cva, 210);
    let scope = cva.import_epoch_scope().unwrap();
    for id in scope.cohort.iter().copied() {
        settle_initial_pass(&mut cva, id, 220);
    }
    let receipt = cva
        .native_import_receipts()
        .into_iter()
        .find(|receipt| receipt.import_key == scope.start.import_key)
        .unwrap();
    let imported_conversation = receipt.conversations[0].conversation_id.clone();
    let imported_leaf = receipt.conversations[0]
        .messages
        .iter()
        .rev()
        .find_map(|message| message.node_id.clone())
        .unwrap();
    cva.container.set_owner_write_scope(true);
    cva.materialize_path_fragments("c1", "a7", crate::FragmentConfig::default(), true)
        .unwrap();
    cva.materialize_path_fragments(
        &imported_conversation,
        &imported_leaf,
        crate::FragmentConfig::default(),
        true,
    )
    .unwrap();
    let unrelated = unrelated_memory(&mut cva);
    let graph_version = cva.graph_version();
    cva.set_memory_relation_with_origin(
        old,
        unrelated,
        GraphRelationKind::Factual,
        true,
        GraphRelationOrigin::User,
        graph_version,
    )
    .unwrap();
    cva.refresh_communities_leiden().unwrap();
    cva.container.set_owner_write_scope(false);

    let admission = input::admission_proof_scoped(&mut cva, Some(&scope)).unwrap();
    let recipients: std::collections::BTreeSet<_> = admission.initial.keys().copied().collect();
    let base = cva.historical_import_base(&scope).unwrap();
    let base_ids: std::collections::BTreeSet<_> = base.keys().copied().collect();
    let expected: std::collections::BTreeSet<_> = scope.cohort.union(&base_ids).copied().collect();
    assert!(
        recipients.contains(&old),
        "pre-start recipient proof must survive replay"
    );
    assert_eq!(recipients, expected);
    assert!(
        !admission
            .memory_inputs
            .keys()
            .any(|id| *id == old && scope.cohort.contains(id))
    );
    assert!(
        scope
            .cohort
            .iter()
            .all(|id| admission.memory_inputs.contains_key(id))
    );

    let manifest = manifest::build_with_fill(&mut cva, false).unwrap();
    assert!(manifest.epoch);
    assert_eq!(manifest.cohort, scope.cohort);
    assert_eq!(manifest.base_cut, scope.base_cut);
    assert_eq!(manifest.start_evidence_digest, scope.start_digest);
    assert_eq!(
        manifest
            .memory_inputs
            .keys()
            .copied()
            .collect::<std::collections::BTreeSet<_>>(),
        expected
    );
    assert!(
        manifest.superseded.is_empty(),
        "the accepted use predates ImportStart"
    );
    let fragment_sources = field(field(&manifest.value, "vector_proof"), "fragment_sources");
    let encoding::Value::Array(fragment_sources) = fragment_sources else {
        panic!("vector fragment proof must be an array")
    };
    assert!(!fragment_sources.is_empty());
    for fragment in fragment_sources {
        let encoding::Value::Text(conversation) = field(fragment, "conversation_id") else {
            panic!("fragment conversation must be text")
        };
        assert_eq!(conversation, &imported_conversation);
        let encoding::Value::Array(represented) = field(fragment, "represented_nodes") else {
            panic!("fragment represented nodes must be an array")
        };
        assert!(!represented.is_empty());
        for node in represented {
            let encoding::Value::Text(conversation) = field(node, "conversation_id") else {
                panic!("source conversation must be text")
            };
            let encoding::Value::Text(node_id) = field(node, "node_id") else {
                panic!("source node ID must be text")
            };
            assert!(
                scope
                    .source_nodes
                    .contains(&(conversation.clone(), node_id.clone()))
            );
        }
    }
    let source_records = field(&manifest.value, "source_records");
    let encoding::Value::Array(source_records) = source_records else {
        panic!("source records must be an array")
    };
    let expected_activity_sources: std::collections::BTreeSet<_> = scope
        .source_nodes
        .iter()
        .filter(|(conversation, node)| cva.activity_position_for_turn(conversation, node).is_some())
        .cloned()
        .collect();
    let mut actual_source_records = std::collections::BTreeSet::new();
    for record in source_records {
        let node = field(record, "node");
        let encoding::Value::Text(conversation) = field(node, "conversation_id") else {
            panic!("source conversation must be text")
        };
        let encoding::Value::Text(node_id) = field(node, "node_id") else {
            panic!("source node ID must be text")
        };
        assert!(
            scope
                .source_nodes
                .contains(&(conversation.clone(), node_id.clone()))
        );
        actual_source_records.insert((conversation.clone(), node_id.clone()));
    }
    assert_eq!(actual_source_records, expected_activity_sources);
    let graph = field(&manifest.value, "graph_proof");
    let encoding::Value::Array(relations) = field(graph, "relationships") else {
        panic!("graph relationships must be an array")
    };
    assert!(relations.iter().all(|relation| {
        field(relation, "source_id") != &encoding::Value::Bytes(unrelated.0.to_vec())
            && field(relation, "target_id") != &encoding::Value::Bytes(unrelated.0.to_vec())
    }));
    let community = field(&manifest.value, "community_proof");
    if let encoding::Value::Array(communities) = field(community, "membership") {
        for entry in communities {
            let encoding::Value::Array(members) = field(entry, "members") else {
                panic!("community members must be an array")
            };
            assert!(!members.contains(&encoding::Value::Bytes(unrelated.0.to_vec())));
        }
    }

    // First freeze has no affected old recipient; later provisional evidence
    // below expands only the affected result set before settlement publication.
    let frozen = cva.freeze_historical_import().unwrap();
    let prepared = frozen.prepare_native().unwrap();
    assert_eq!(prepared.settlement.cohort, scope.cohort);
    assert_eq!(
        prepared.settlement.start_evidence_digest,
        scope.start_digest
    );
    assert!(
        !prepared.settlement.cohort.contains(&old),
        "an unrelated pre-start recipient is not an import effect recipient"
    );
    let frozen_recipients: std::collections::BTreeSet<_> =
        frozen.initial_records().keys().copied().collect();
    assert_eq!(frozen_recipients, prepared.settlement.cohort);
    assert!(cva.freshness.import_settlements().is_empty());

    // A provisional import-source effect on an existing recipient belongs to
    // this result without turning that recipient into an import birth.
    let (conversation_id, node_id) = scope.source_nodes.iter().next().unwrap().clone();
    let evidence = crate::freshness_storage::SemanticEventEvidence {
        owner_uuid: scope.start.owner_uuid,
        conversation_id,
        node_id,
        fragment: None,
        accepted_turn: cva.rel_turn_count(),
        base_policy_digest: crate::freshness::historical_planner::HISTORICAL_POLICY_DIGEST,
        semantic_policy_version: crate::freshness::semantic_policy::SEMANTIC_POLICY_VERSION,
        semantic_policy_digest: crate::freshness::semantic_policy::SemanticPolicy::default()
            .digest()
            .unwrap(),
        compatibility_profile_id: [81; 32],
        source_evidence_digest: [82; 32],
        target_evidence_digest: [83; 32],
    };
    let event_id = evidence.event_id();
    let payload = cva
        .freshness
        .encode_semantic_event(scope.start.owner_uuid, evidence, BTreeMap::from([(old, 3)]))
        .unwrap();
    cva.container.set_owner_write_scope(true);
    cva.container.append(&payload).unwrap();
    cva.container.sync().unwrap();
    cva.freshness.ingest_appended(&payload).unwrap();
    cva.container.set_owner_write_scope(false);

    cva.container.set_owner_write_scope(true);
    cva.record_accepted_memory_use(
        crate::AcceptedMemoryUseReceipt {
            owner_uuid: cva.owner_uuid().unwrap(),
            accepted_turn: cva.rel_turn_count(),
            use_id: "during-epoch-before-freeze".into(),
            memories: vec![old],
        },
        280,
    )
    .unwrap();
    cva.container.set_owner_write_scope(false);
    let affected = cva.freeze_historical_import().unwrap();
    let mut expected_result = scope.cohort.clone();
    expected_result.insert(old);
    assert_eq!(affected.settlement.cohort, expected_result);
    assert_eq!(
        affected.settlement.superseded,
        std::collections::BTreeSet::from([event_id])
    );
    assert_eq!(affected.initial_records().get(&old), base.get(&old));
    assert!(!affected.initial_records().contains_key(&unrelated));
    assert!(!cva.import_epoch_scope().unwrap().cohort.contains(&old));
    let untouched = cva.freshness_record(unrelated).unwrap();
    let prepared = affected.prepare_native().unwrap();
    assert!(prepared.settlement.encode().starts_with(b"CVAFRS05"));
    let mut expected_old = base[&old];
    expected_old
        .reinforce_with_policy(
            25,
            prepared.settlement.settlement_cut,
            &FreshnessPolicy::default_v1(),
        )
        .unwrap();
    assert_eq!(
        prepared.records()[&old],
        expected_old,
        "retain legitimate same-cut interval use while replacing provisional import credit"
    );
    assert!(cva.commit_historical_import(&prepared).unwrap());
    assert_eq!(cva.freshness_record(unrelated).unwrap(), untouched);
    assert_eq!(
        cva.freshness
            .records_at(prepared.settlement.settlement_cut)
            .unwrap()[&old],
        prepared.records()[&old]
    );
    assert!(cva.open_import_epoch().is_some());
    let accepted = std::fs::read(&path).unwrap();
    assert!(!cva.commit_historical_import(&prepared).unwrap());
    assert_eq!(std::fs::read(&path).unwrap(), accepted);
    // Same-cut accepted use after the settlement must apply once, survive reopen,
    // and remain intact on an exact settlement retry.
    cva.container.set_owner_write_scope(true);
    cva.record_accepted_memory_use(
        crate::AcceptedMemoryUseReceipt {
            owner_uuid: cva.owner_uuid().unwrap(),
            accepted_turn: cva.rel_turn_count(),
            use_id: "after-epoch-settlement".into(),
            memories: vec![old],
        },
        300,
    )
    .unwrap();
    cva.put_conversation_compaction(
        "c1".into(),
        "a7".into(),
        "replacement mutable summary".into(),
        Some("a6"),
    )
    .unwrap();
    cva.container.set_owner_write_scope(false);
    assert_eq!(
        immutable_prefix_digest(&path, prepared.settlement.input_bytes).unwrap(),
        prepared.settlement.epoch_prefix_digest
    );
    let after_use = cva.freshness.records_at(cva.rel_turn_count()).unwrap();
    drop(cva);
    let mut cva = Cva::open(&path).unwrap();
    assert_eq!(
        cva.freshness.records_at(cva.rel_turn_count()).unwrap(),
        after_use
    );
    assert!(
        !cva.reconstruct_historical_freshness(Default::default())
            .unwrap()
            .committed
    );
    assert_eq!(
        cva.freshness.records_at(cva.rel_turn_count()).unwrap(),
        after_use
    );

    // Scope authentication uses retained source paths after a terminal source.
    // Full Cva reopen of completed Insomnia work under a deleted conversation is
    // a separate owner-lifecycle gap; normal import completion is still fenced.
    cva.container.set_owner_write_scope(true);
    cva.tombstone_conversation(&imported_conversation).unwrap();
    cva.container.sync().unwrap();
    cva.container.set_owner_write_scope(false);
    cva.validate_import_settlement_scope(prepared.settlement.epoch.as_ref().unwrap())
        .unwrap();
    drop(cva);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn zero_result_import_has_empty_scoped_birth_and_source_sets() {
    let path = std::env::current_dir()
        .unwrap()
        .join(".test-tmp")
        .join(format!(
            "historical-import-attribution-empty-{}.rel",
            uuid::Uuid::new_v4()
        ));
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut cva = Cva::create(&path).unwrap();
    let empty = NativeImport {
        schema_version: 1,
        source_namespace: "attribution/empty".into(),
        source_format: "attribution-test-v1".into(),
        conversations: vec![],
    };
    cva.import_native(&empty, 200).unwrap();
    let scope = cva.import_epoch_scope().unwrap();
    assert!(scope.source_nodes.is_empty());
    assert!(scope.episodes.is_empty());
    assert!(scope.cohort.is_empty());
    assert!(cva.historical_import_base(&scope).unwrap().is_empty());
    let frozen = cva.freeze_historical_import().unwrap();
    assert!(frozen.epoch);
    assert!(frozen.settlement.cohort.is_empty());
    assert!(frozen.initial_records().is_empty());
    assert!(cva.freshness.import_settlements().is_empty());
    let prepared = frozen.prepare_native().unwrap();
    assert_eq!(prepared.settlement.settlement_cut, 0);
    let before = std::fs::read(&path).unwrap();
    let mut forged = prepared.clone();
    forged.settlement.epoch.as_mut().unwrap().start_prefix += 1;
    assert!(cva.commit_historical_import(&forged).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), before);
    let encoded = prepared.settlement.encode();
    assert_eq!(
        HistoricalImportFreshnessSettlement::decode(&encoded).unwrap(),
        prepared.settlement
    );
    for end in 0..encoded.len() {
        assert!(HistoricalImportFreshnessSettlement::decode(&encoded[..end]).is_err());
    }
    for at in [0, 8, encoded.len() / 2, encoded.len() - 1] {
        let mut corrupt = encoded.clone();
        corrupt[at] ^= 1;
        assert!(HistoricalImportFreshnessSettlement::decode(&corrupt).is_err());
    }

    assert!(cva.commit_historical_import(&prepared).unwrap());
    assert!(cva.open_import_epoch().is_some());
    assert!(cva.freshness.records().is_empty());
    let accepted = std::fs::read(&path).unwrap();
    assert!(
        !cva.reconstruct_historical_freshness(Default::default())
            .unwrap()
            .committed
    );
    assert_eq!(std::fs::read(&path).unwrap(), accepted);
    assert!(
        !cva.reconstruct_historical_freshness(HistoricalReconstructionOptions {
            fill_missing_embeddings: true,
        })
        .unwrap()
        .committed
    );
    drop(cva);
    let cva = Cva::open(&path).unwrap();
    assert_eq!(cva.freshness.import_settlements().len(), 1);
    assert!(cva.freshness.records().is_empty());

    drop(cva);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn tombstoned_prestart_memory_is_proven_but_never_an_import_recipient() {
    let (path, mut cva, _) = setup_origin(
        "historical-import-attribution-tombstone.rel",
        EpisodeOrigin::Live,
    );
    let id = process(&mut cva, 1, 110).created[0].id;
    cva.tombstone_memory(id, 1).unwrap();
    cva.import_native(
        &NativeImport {
            schema_version: 1,
            source_namespace: "attribution/tombstone-empty".into(),
            source_format: "attribution-test-v1".into(),
            conversations: vec![],
        },
        200,
    )
    .unwrap();
    let scope = cva.import_epoch_scope().unwrap();
    assert!(
        cva.historical_import_base(&scope)
            .unwrap()
            .contains_key(&id)
    );
    let proof = input::admission_proof_scoped(&mut cva, Some(&scope)).unwrap();
    assert!(proof.initial.is_empty());
    assert!(proof.memory_inputs.is_empty());
    assert_eq!(proof.memories.len(), 1);
    assert_eq!(
        field(&proof.memories[0], "status"),
        &encoding::Value::Text("tombstoned".into())
    );
    let frozen = cva.freeze_historical_import().unwrap();
    assert!(frozen.settlement.cohort.is_empty());
    assert!(frozen.initial_records().is_empty());
    assert!(
        cva.reconstruct_historical_freshness(Default::default())
            .unwrap()
            .committed
    );
    assert!(
        cva.freshness
            .import_settlements()
            .last()
            .unwrap()
            .records
            .is_empty()
    );
    assert!(cva.memory(id).is_err());
    assert!(cva.memory_revision(id, 1).is_ok());
    drop(cva);
    let mut reopened = Cva::open(&path).unwrap();
    assert!(
        reopened
            .freeze_historical_import()
            .unwrap()
            .settlement
            .cohort
            .is_empty()
    );
    assert!(reopened.memory(id).is_err());
    drop(reopened);
    std::fs::remove_file(path).unwrap();
}
