use super::*;
use crate::freshness_storage::{
    AcceptedImportEventEvidence, FreshnessEventProvenance, FreshnessReplayEntry,
    HistoricalImportFreshnessSettlement, ImportSemanticScope,
};
use crate::imports::{
    ImportBranch, ImportComplete, ImportConversation, ImportMessage, ImportStart, ImportWatermarks,
    NativeImport,
};
use crate::{Cva, FreshnessEventProof, MemoryDraft, MemoryId};
use serde_json::{Map, json};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::PathBuf;

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(".test-tmp")
            .join(format!(
                "reconcile-freshness-authority-{}",
                uuid::Uuid::new_v4()
            ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn event(id: &str, turn: u64, graph: u64, memory: MemoryId) -> FreshnessReplayEntry {
    FreshnessReplayEntry::Event {
        event_id: id.into(),
        source_turn: turn,
        effects: BTreeMap::from([(memory, 3)]),
        proof: FreshnessEventProof {
            graph_version: graph,
            community_generation: Some(graph + 10),
            community_graph_version: Some(graph),
        },
        provenance: Some(FreshnessEventProvenance {
            producer_kind: 3,
            source_id: id.into(),
            origin_owner: [0xA1; 16],
            accepted_turn: turn,
            policy_version: 1,
        }),
        semantic: Some(crate::freshness_storage::SemanticEventEvidence {
            owner_uuid: [0xA1; 16],
            fragment: None,
            conversation_id: "conversation".into(),
            node_id: "node".into(),
            accepted_turn: turn,
            base_policy_digest: [1; 32],
            semantic_policy_version: 1,
            semantic_policy_digest: [2; 32],
            compatibility_profile_id: [3; 32],
            source_evidence_digest: [4; 32],
            target_evidence_digest: [5; 32],
        }),
    }
}

fn completed_epoch(
    events: Vec<FreshnessReplayEntry>,
    accepted_events: BTreeMap<String, FreshnessReplayEntry>,
) -> CompletedEpoch {
    let owner = [0xA1; 16];
    let import_id = [0xB2; 32];
    let semantic = ImportSemanticScope {
        owner,
        import_id,
        start_evidence_digest: [0xC3; 32],
        born_memories: BTreeSet::new(),
        affected_recipients: BTreeSet::new(),
        source_nodes: BTreeSet::new(),
        base_cut: 0,
        settlement_cut: 1,
        accepted_events: accepted_events
            .iter()
            .map(|(id, _)| {
                (
                    id.clone(),
                    AcceptedImportEventEvidence {
                        event_id: id.clone(),
                        payload_digest: [0xD4; 32],
                        accepted_turn: 1,
                        affected_recipients: BTreeSet::new(),
                    },
                )
            })
            .collect(),
        superseded: BTreeSet::new(),
        predecessor_digest: None,
    };
    CompletedEpoch {
        start: ImportStart {
            owner_uuid: owner,
            import_id: "epoch".into(),
            import_key: "key".into(),
            source_namespace: "fixture".into(),
            source_format: "fixture".into(),
            started_at_ns: 1,
            pre: ImportWatermarks::default(),
        },
        complete: ImportComplete {
            owner_uuid: owner,
            import_id: "epoch".into(),
            start_digest: String::new(),
            receipt_digest: "receipt".into(),
            completed_at_ns: 2,
            post: ImportWatermarks::default(),
            historical_freshness_settlement_digest: String::new(),
        },
        settlement: HistoricalImportFreshnessSettlement {
            epoch: None,
            semantic: Some(semantic),
            epoch_prefix_digest: [0; 32],
            owner,
            import_id,
            start_evidence_digest: [0xC3; 32],
            corpus_digest: [1; 32],
            manifest_digest: [2; 32],
            input_digest: [3; 32],
            cohort: BTreeSet::new(),
            base_cut: 0,
            settlement_cut: 1,
            global_version: 0,
            input_bytes: 0,
            archive_version: 0,
            graph_version: 0,
            memory_graph_version: 0,
            records: BTreeMap::new(),
            superseded: BTreeSet::new(),
        },
        first: 1,
        end: 2,
        completions: Vec::new(),
        archive_records: Vec::new(),
        events,
        accepted_events,
        dream: Vec::new(),
        owner_evidence: Vec::new(),
    }
}

#[test]
fn normalized_same_authority_keeps_accepted_membership_and_content() {
    let memory = MemoryId([0xEE; 32]);
    let original = event("accepted-id", 7, 12, memory);
    let mut rebased = event("accepted-id", 9, 18, memory);
    normalize_event_coordinates(&mut rebased);
    let mut canonical = original.clone();
    normalize_event_coordinates(&mut canonical);
    let one = BTreeMap::from([("accepted-id".to_string(), canonical.clone())]);
    let two = BTreeMap::from([("accepted-id".to_string(), rebased.clone())]);
    let base = completed_epoch(vec![canonical.clone()], one);
    let coordinate_rebase = completed_epoch(vec![rebased.clone()], two);
    assert!(same_authority(&base, &coordinate_rebase));

    let different_id = completed_epoch(
        vec![canonical.clone()],
        BTreeMap::from([("other-id".to_string(), canonical.clone())]),
    );
    assert!(!same_authority(&base, &different_id));

    let mut changed_effect = rebased.clone();
    let FreshnessReplayEntry::Event { effects, .. } = &mut changed_effect else {
        unreachable!()
    };
    effects.insert(memory, 4);
    let changed_causal_facts = completed_epoch(
        vec![canonical],
        BTreeMap::from([("accepted-id".to_string(), changed_effect)]),
    );
    assert!(!same_authority(&base, &changed_causal_facts));
}

#[test]
fn owner_requested_event_scan_expands_batches_and_fails_closed() {
    let scratch = Scratch::new();
    let path = scratch.0.join("owner.rel");
    let mut cva = Cva::create(&path).unwrap();
    let memory = cva
        .publish_memory(
            None,
            0,
            MemoryDraft {
                category: "fact".into(),
                memory_type: "project".into(),
                authority_kind: "direct".into(),
                temporal_status: "current".into(),
                title: "use".into(),
                content: "use body".into(),
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
                mutation_id: "authority-use".into(),
                created_at_ns: 1,
                updated_at_ns: 1,
            },
        )
        .unwrap()
        .0
        .id;
    let owner = cva.owner_uuid().unwrap();
    let batch = cva
        .freshness
        .prepare_accepted_use_batch(
            owner,
            "batch-use".into(),
            1,
            vec![memory],
            FreshnessEventProof {
                graph_version: cva.graph_version(),
                community_generation: None,
                community_graph_version: None,
            },
            vec![BTreeMap::from([(memory, 25)])],
        )
        .unwrap();
    let decoded = FreshnessReplayEntry::from_payload(&batch).unwrap();
    let requested = decoded
        .iter()
        .filter_map(|entry| match entry {
            FreshnessReplayEntry::Event { event_id, .. } => Some(event_id.clone()),
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(requested.len(), 1);
    cva.container.append(&batch).unwrap();
    cva.container.sync().unwrap();
    cva.freshness.ingest_appended(&batch).unwrap();
    let entries = cva
        .freshness
        .requested_import_event_entries(&requested)
        .unwrap();
    assert_eq!(entries.len(), 1);
    assert!(matches!(
        entries.values().next(),
        Some(FreshnessReplayEntry::Event { .. })
    ));
    assert!(
        cva.freshness
            .requested_import_event_entries(&BTreeSet::from(["missing".into()]))
            .is_err()
    );
}

#[test]
fn owner_scope_events_outside_epoch_interval_are_compared_by_normalized_facts() {
    let scratch = Scratch::new();
    let base_path = scratch.0.join("base.rel");
    let left_path = scratch.0.join("left.rel");
    let right_path = scratch.0.join("right.rel");
    let mut base = Cva::create(&base_path).unwrap();
    let memory = base
        .publish_memory(
            None,
            0,
            MemoryDraft {
                category: "fact".into(),
                memory_type: "project".into(),
                authority_kind: "direct".into(),
                temporal_status: "current".into(),
                title: "authority".into(),
                content: "body".into(),
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
                mutation_id: "authority-memory".into(),
                created_at_ns: 1,
                updated_at_ns: 1,
            },
        )
        .unwrap()
        .0
        .id;
    base.sync().unwrap();
    drop(base);
    fs::copy(&base_path, &left_path).unwrap();
    fs::copy(&base_path, &right_path).unwrap();
    let mut left = Cva::open(&left_path).unwrap();
    let mut right = Cva::open(&right_path).unwrap();
    let owner = left.owner_uuid().unwrap();
    let id = "accepted-before-epoch".to_string();
    let left_payload = left.freshness.encode_event(
        owner,
        id.clone(),
        0,
        FreshnessEventProof {
            graph_version: 1,
            community_generation: None,
            community_graph_version: None,
        },
        BTreeMap::from([(memory, 3)]),
        BTreeMap::new(),
    );
    let right_payload = right.freshness.encode_event(
        owner,
        id.clone(),
        0,
        FreshnessEventProof {
            graph_version: 2,
            community_generation: None,
            community_graph_version: None,
        },
        BTreeMap::from([(memory, 4)]),
        BTreeMap::new(),
    );
    left.container.append(&left_payload).unwrap();
    right.container.append(&right_payload).unwrap();
    left.container.sync().unwrap();
    right.container.sync().unwrap();
    left.freshness
        .bind_history_path(&left_path, fs::metadata(&left_path).unwrap().len());
    right
        .freshness
        .bind_history_path(&right_path, fs::metadata(&right_path).unwrap().len());
    let requested = BTreeSet::from([id]);
    let left_events = left
        .freshness
        .requested_import_event_entries(&requested)
        .unwrap();
    let right_events = right
        .freshness
        .requested_import_event_entries(&requested)
        .unwrap();
    assert_ne!(
        left_events, right_events,
        "same ID retains distinct immutable owner facts"
    );
    let left_events = left_events
        .into_iter()
        .map(|(id, mut event)| {
            normalize_event_coordinates(&mut event);
            (id, event)
        })
        .collect();
    let right_events = right_events
        .into_iter()
        .map(|(id, mut event)| {
            normalize_event_coordinates(&mut event);
            (id, event)
        })
        .collect();
    let left_epoch = completed_epoch(Vec::new(), left_events);
    let right_epoch = completed_epoch(Vec::new(), right_events);
    assert!(!same_authority(&left_epoch, &right_epoch));
}

#[test]
fn inventory_derives_accepted_event_authority_from_completed_owner_scope() {
    let scratch = Scratch::new();
    let path = scratch.0.join("completed-import.rel");
    let mut cva = Cva::create(&path).unwrap();
    let mut messages = Vec::new();
    let mut parent = None;
    for index in 0..8 {
        let user = format!("u{index}");
        let assistant = format!("a{index}");
        messages.push(ImportMessage {
            id: user.clone(),
            revision: Some("1".into()),
            parent_id: parent.clone(),
            replaces: None,
            deleted: false,
            role: "user".into(),
            timestamp_ns: index * 2 + 10,
            content: format!("Inventory authority fact {index}."),
            artifacts: Vec::new(),
        });
        messages.push(ImportMessage {
            id: assistant.clone(),
            revision: Some("1".into()),
            parent_id: Some(user),
            replaces: None,
            deleted: false,
            role: "assistant".into(),
            timestamp_ns: index * 2 + 11,
            content: "Acknowledged.".into(),
            artifacts: Vec::new(),
        });
        parent = Some(assistant);
    }
    cva.import_native(
        &NativeImport {
            schema_version: 1,
            source_namespace: "freshness-authority-inventory".into(),
            source_format: "test-v1".into(),
            conversations: vec![ImportConversation {
                id: "conversation".into(),
                title: None,
                messages,
                branches: vec![ImportBranch {
                    id: "main".into(),
                    leaf_message_id: parent.unwrap(),
                    canonical: true,
                }],
            }],
        },
        100,
    )
    .unwrap();
    let start = cva.open_import_epoch().unwrap().clone();
    cva.container.set_owner_write_scope(true);
    // Imported episodes can expose normalized node ids and turn membership
    // that differ from the canned live-episode fixture. Build the response
    // from the claimed episode's actual user turns, as readiness tests do.
    let claim = cva
        .claim_insomnia_episode("authority-worker", 110, 1_000)
        .unwrap()
        .unwrap();
    let turns = cva.episode_turns(claim.episode_id).unwrap();
    let mut turn_responses = Map::new();
    let mut groups = Map::new();
    for (index, turn) in turns.iter().filter(|turn| turn.role == "user").enumerate() {
        let clause = if index == 0 {
            json!({
                "disposition": "retain",
                "authority_kind": "direct",
                "category": "decision",
                "type": "project",
                "lifecycle": "current",
                "proposition": "Inventory authority completion invariant.",
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
        if index == 0 {
            groups.insert(
                "g000".into(),
                json!({
                    "title": "Inventory authority",
                    "content": "Inventory authority completion invariant."
                }),
            );
        }
    }
    let extractor = crate::InsomniaExtractor::new(crate::SimulatedGeneralEndpoint::new(
        "synthetic-import-authority",
        vec![
            json!({"turns": turn_responses, "evidence_requests": []}),
            json!({"groups": groups}),
        ],
    ));
    let result = cva
        .process_claimed_insomnia_episode(&claim, &extractor, "private", 110, 111)
        .unwrap();
    cva.container.set_owner_write_scope(false);
    let memory = result
        .created
        .first()
        .expect("test extraction creates an imported Memory")
        .id;
    // The real import completion validator requires a settled initial Dream
    // publication for an import-born Memory. Keep that admission path real;
    // otherwise inventory cannot reach the accepted-event authority check.
    let pass = crate::dream_freshness::initial_pass_id(cva.owner_uuid().unwrap(), memory);
    let graph_version = cva.graph_version();
    cva.container.set_owner_write_scope(true);
    cva.begin_initial_dream_pass(
        pass.clone(),
        memory,
        1,
        cva.rel_turn_count(),
        BTreeSet::new(),
        graph_version,
    )
    .unwrap();
    cva.reconcile_dream_lifecycle(memory).unwrap();
    cva.accept_initial_dream_settlement(&pass, cva.rel_turn_count())
        .unwrap();
    crate::dream_freshness::settle_initial_pass(&mut cva, memory, true, 120).unwrap();
    let dream_epoch = cva.dream_eligible_epoch(memory, 120).unwrap().unwrap_or(0);
    assert!(cva.mark_dream_processed(memory, dream_epoch, 120).unwrap());
    cva.container.set_owner_write_scope(false);

    let owner = cva.owner_uuid().unwrap();
    let turn = cva.rel_turn_count();
    cva.container.set_owner_write_scope(true);
    let payload = cva
        .freshness
        .prepare_accepted_use_batch(
            owner,
            "authority-use-in-import".into(),
            turn,
            vec![memory],
            FreshnessEventProof {
                graph_version: cva.graph_version(),
                community_generation: None,
                community_graph_version: None,
            },
            vec![BTreeMap::from([(memory, 25)])],
        )
        .unwrap();
    cva.container.append(&payload).unwrap();
    cva.container.sync().unwrap();
    cva.freshness.ingest_appended(&payload).unwrap();
    cva.container.set_owner_write_scope(false);
    cva.reconstruct_historical_freshness(Default::default())
        .unwrap();
    cva.complete_native_import(&start.import_id, 200).unwrap();
    cva.sync().unwrap();
    drop(cva);

    let mut reopened = Cva::open(&path).unwrap();
    let epochs = super::inventory(&mut reopened).unwrap();
    let epoch = epochs.get(&start.import_id).unwrap();
    let semantic_scope = epoch.settlement.semantic.as_ref().unwrap();
    assert_eq!(
        epoch.accepted_events.keys().collect::<BTreeSet<_>>(),
        semantic_scope
            .accepted_events
            .keys()
            .collect::<BTreeSet<_>>(),
        "inventory should retain the full canonical accepted-event membership"
    );
    let expected_use_events = FreshnessReplayEntry::from_payload(&payload)
        .unwrap()
        .into_iter()
        .filter_map(|entry| match entry {
            FreshnessReplayEntry::Event { event_id, .. } => Some(event_id),
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    assert!(!expected_use_events.is_empty());
    assert!(
        expected_use_events
            .iter()
            .all(|id| epoch.accepted_events.contains_key(id))
    );
    assert!(epoch.events.iter().any(|entry| matches!(
        entry,
        FreshnessReplayEntry::Event { event_id, .. }
            if epoch.accepted_events.contains_key(event_id)
    )));
}
