use super::*;
use crate::freshness_storage::{
    AcceptedImportEventEvidence, FreshnessEventProvenance, FreshnessReplayEntry,
    HistoricalImportFreshnessSettlement, ImportSemanticScope,
};
use crate::imports::{
    ImportBranch, ImportComplete, ImportConversation, ImportMessage, ImportStart, ImportWatermarks,
    NativeImport,
};
use crate::{Cva, EntityDraft, EntityGlobalId, FreshnessEventProof, MemoryDraft, MemoryId};
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

fn entity_draft(name: &str, mutation_id: &str, summary: &str, updated_at_ns: i64) -> EntityDraft {
    EntityDraft {
        canonical_name: name.into(),
        aliases: vec![format!("{name} alias")],
        kind: "project".into(),
        summary: summary.into(),
        mutation_id: mutation_id.into(),
        created_at_ns: 10,
        updated_at_ns,
    }
}

fn entity_import_evidence(cva: &Cva, global: u64, local: u64) -> Vec<u8> {
    cva.entities
        .retained_publication_at(global, local)
        .unwrap()
        .expect("Entity publication is accepted by the owner")
        .normalized_import_evidence()
        .unwrap()
}

fn append_entity_publication(
    cva: &mut Cva,
    publication: crate::entity_compact_codec::publication::EntityPublication,
    global: u64,
    local: u64,
) {
    let body = crate::entity_compact_codec::publication::encode(&publication).unwrap();
    let (appended_global, _) = cva.container.append_publication_envelope(&body).unwrap();
    assert_eq!(appended_global, global);
    assert_eq!(cva.entity_version() + 1, local);
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
        .bind_history_source(&left.container, fs::metadata(&left_path).unwrap().len())
        .unwrap();
    right
        .freshness
        .bind_history_source(&right.container, fs::metadata(&right_path).unwrap().len())
        .unwrap();
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
    completed_scope_inventory(false, false, false);
}

#[test]
fn inventory_derives_memory_envelope_evidence_from_real_completed_import() {
    completed_scope_inventory(true, false, false);
}

#[test]
fn inventory_normalizes_entity_envelopes_and_marker_bound_terminals_in_real_completed_import() {
    completed_scope_inventory(false, true, false);
}

#[test]
fn inventory_derives_relationship_envelope_evidence_from_real_completed_import() {
    completed_scope_inventory(false, false, true);
}

fn completed_scope_inventory(
    with_envelope: bool,
    with_entity_envelopes: bool,
    with_relationship_envelopes: bool,
) {
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
    let import_scope = cva
        .import_write_admission
        .as_ref()
        .map(|admission| admission.enter(&cva.container).unwrap());
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
    drop(import_scope);
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
    let import_scope = cva
        .import_write_admission
        .as_ref()
        .map(|admission| admission.enter(&cva.container).unwrap());
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
    drop(import_scope);

    let owner = cva.owner_uuid().unwrap();
    let turn = cva.rel_turn_count();
    let import_scope = cva
        .import_write_admission
        .as_ref()
        .map(|admission| admission.enter(&cva.container).unwrap());
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
    let envelope_evidence = if with_envelope {
        let current = cva.memory(memory).unwrap();
        let revision = current.revision;
        let source_ref = current.source_ref.clone();
        let temporal = current.temporal_inference.clone();
        let mut draft = crate::cva_reconcile_memory::memory_draft(current);
        draft.mutation_id = "completed-import-envelope-revision".into();
        draft.updated_at_ns += 1;
        let (published, changed) = cva
            .memories
            .publish_with_source_ref_and_temporal_inference(
                &mut cva.container,
                Some(memory),
                revision,
                draft,
                source_ref,
                temporal,
            )
            .unwrap();
        assert!(changed);
        let mut next = cva
            .memories
            .records()
            .iter()
            .find(|record| record.id == published.id && record.revision == published.revision)
            .unwrap()
            .clone();
        next.global_version = 0;
        next.memory_version = 0;
        Some(crate::memory_codec::encode_record(&next).unwrap())
    } else {
        None
    };
    let mut entity_evidence = Vec::new();
    if with_entity_envelopes {
        // Ordinary current creation and all later current Entity publication
        // shapes retain exact owner evidence in the completed import.
        let (legacy, changed) = cva
            .entities
            .publish_with_global_id(
                &mut cva.container,
                None,
                0,
                entity_draft(
                    "Marker-bound entity",
                    "marker-bound-entity-create",
                    "Accepted marker-bound description",
                    20,
                ),
                Some(EntityGlobalId([0x61; 16])),
            )
            .unwrap();
        assert!(changed);
        entity_evidence.push(entity_import_evidence(
            &cva,
            legacy.global_version,
            legacy.entity_version,
        ));

        // Step 183 consumes the new envelope format before Step 187 changes
        // the ordinary writer. Construct candidate publications directly and
        // admit them through the same rebuilt-store validation used by replay.
        let previous = cva
            .entities
            .records()
            .iter()
            .find(|record| record.id == legacy.id)
            .unwrap()
            .clone();
        let mut revised = previous.clone();
        revised.revision += 1;
        revised.global_version = cva.container.next_version_candidate();
        revised.entity_version = cva.entity_version() + 1;
        revised.mutation_id = "envelope-entity-revision".into();
        revised.summary = "Accepted envelope revision description".into();
        revised.updated_at_ns = 21;
        let delta =
            crate::entity_compact_contract::EntityDelta::between(&previous, &revised).unwrap();
        let global = revised.global_version;
        let local = revised.entity_version;
        append_entity_publication(
            &mut cva,
            crate::entity_compact_codec::publication::EntityPublication::Delta(delta.clone()),
            global,
            local,
        );
        let decoded = crate::entity_compact_apply::apply_delta(&previous, &delta).unwrap();
        assert_eq!(decoded, revised);
        cva.entities.insert_rebuilt(decoded).unwrap();
        entity_evidence.push(entity_import_evidence(&cva, global, local));

        let global = cva.container.next_version_candidate();
        let local = cva.entity_version() + 1;
        let direct = crate::entity_codec::DirectEntityTombstone {
            id: legacy.id,
            revision: revised.revision + 1,
        };
        append_entity_publication(
            &mut cva,
            crate::entity_compact_codec::publication::EntityPublication::DirectTombstone {
                id: direct.id,
                revision: direct.revision,
                entity_version: local,
            },
            global,
            local,
        );
        cva.entities
            .insert_direct_tombstone_rebuilt(direct, global, local)
            .unwrap();
        entity_evidence.push(entity_import_evidence(&cva, global, local));

        let make_entity = |id: crate::EntityId,
                           global_id: EntityGlobalId,
                           draft: EntityDraft,
                           global_version: u64,
                           entity_version: u64|
         -> crate::entity_model::EntityRecord {
            crate::entity_model::EntityRecord {
                id,
                global_id: Some(global_id),
                revision: 1,
                canonical_name: draft.canonical_name,
                aliases: draft.aliases,
                kind: draft.kind,
                summary: draft.summary,
                mutation_id: draft.mutation_id,
                created_at_ns: draft.created_at_ns,
                updated_at_ns: draft.updated_at_ns,
                global_version,
                entity_version,
            }
        };
        let merge_target_id = crate::EntityId([0x62; 32]);
        let merge_target_global = cva.container.next_version_candidate();
        let merge_target_local = cva.entity_version() + 1;
        let merge_target = make_entity(
            merge_target_id,
            EntityGlobalId([0x62; 16]),
            entity_draft(
                "Merged entity",
                "envelope-merge-target",
                "Accepted merge target description",
                22,
            ),
            merge_target_global,
            merge_target_local,
        );
        let creation =
            crate::entity_compact_contract::EntityCreation::new(merge_target.clone()).unwrap();
        append_entity_publication(
            &mut cva,
            crate::entity_compact_codec::publication::EntityPublication::Creation(creation),
            merge_target_global,
            merge_target_local,
        );
        cva.entities.insert_rebuilt(merge_target).unwrap();
        entity_evidence.push(entity_import_evidence(
            &cva,
            merge_target_global,
            merge_target_local,
        ));

        let survivor_id = crate::EntityId([0x63; 32]);
        let survivor_global = cva.container.next_version_candidate();
        let survivor_local = cva.entity_version() + 1;
        let survivor = make_entity(
            survivor_id,
            EntityGlobalId([0x63; 16]),
            entity_draft(
                "Surviving entity",
                "envelope-merge-survivor",
                "Accepted survivor description",
                23,
            ),
            survivor_global,
            survivor_local,
        );
        let creation =
            crate::entity_compact_contract::EntityCreation::new(survivor.clone()).unwrap();
        append_entity_publication(
            &mut cva,
            crate::entity_compact_codec::publication::EntityPublication::Creation(creation),
            survivor_global,
            survivor_local,
        );
        cva.entities.insert_rebuilt(survivor).unwrap();
        entity_evidence.push(entity_import_evidence(
            &cva,
            survivor_global,
            survivor_local,
        ));

        let global = cva.container.next_version_candidate();
        let local = cva.entity_version() + 1;
        let merge = crate::entity_codec::EntityTombstone {
            id: merge_target_id,
            revision: 2,
            replacement_id: survivor_id,
        };
        append_entity_publication(
            &mut cva,
            crate::entity_compact_codec::publication::EntityPublication::MergeTombstone {
                id: merge.id,
                revision: merge.revision,
                entity_version: local,
                replacement_id: merge.replacement_id,
            },
            global,
            local,
        );
        cva.entities
            .insert_tombstone_rebuilt(merge, global, local)
            .unwrap();
        entity_evidence.push(entity_import_evidence(&cva, global, local));
    }
    let mut relationship_evidence = Vec::new();
    if with_relationship_envelopes {
        // Construct valid Relationship candidates directly so this consumer
        // test does not depend on the ordinary writer cutover. The store's
        // rebuilt admission path validates each record and terminal position.
        let id = crate::RelationshipId([0x74; 32]);
        let make_relationship = |revision: u64,
                                 global_version: u64,
                                 relationship_version: u64,
                                 mutation_id: &str,
                                 summary: &str|
         -> crate::relationship_model::RelationshipRecord {
            crate::relationship_model::RelationshipRecord {
                id,
                revision,
                kind: "supports".into(),
                participants: vec![crate::RelationshipParticipant {
                    entity: crate::EntityRef {
                        owner_id: "stable-remote-owner".into(),
                        entity_id: crate::EntityId([0x75; 32]),
                    },
                    role: Some("source".into()),
                }],
                evidence: vec![crate::MemoryRef {
                    owner_id: "stable-remote-memory-owner".into(),
                    memory_id: MemoryId([0x76; 32]),
                }],
                summary: summary.into(),
                mutation_id: mutation_id.into(),
                created_at_ns: -12,
                updated_at_ns: 20 + revision as i64,
                global_version,
                relationship_version,
            }
        };
        let append_relationship = |cva: &mut Cva,
                                  publication: crate::relationship_compact_codec::publication::RelationshipPublication,
                                  expected_global: u64| {
            let body = crate::relationship_compact_codec::publication::encode(&publication).unwrap();
            let (global, _) = cva.container.append_publication_envelope(&body).unwrap();
            assert_eq!(global, expected_global);
        };
        let first_global = cva.container.next_version_candidate();
        let first_local = cva.relationship_version() + 1;
        let first = make_relationship(
            1,
            first_global,
            first_local,
            "envelope-relationship-create",
            "Accepted Relationship creation description",
        );
        let first_publication =
            crate::relationship_compact_codec::publication::RelationshipPublication::Creation(
                crate::relationship_compact_contract::RelationshipCreation::new(first.clone())
                    .unwrap(),
            );
        append_relationship(&mut cva, first_publication, first_global);
        cva.relationships.insert_rebuilt(first).unwrap();
        relationship_evidence.push(
            cva.relationships
                .retained_publication_at(first_global, first_local)
                .unwrap()
                .unwrap()
                .normalized_import_evidence()
                .unwrap(),
        );

        let next_global = cva.container.next_version_candidate();
        let next_local = cva.relationship_version() + 1;
        let next = make_relationship(
            2,
            next_global,
            next_local,
            "envelope-relationship-revision",
            "Accepted Relationship revised description",
        );
        let delta = crate::relationship_compact_contract::RelationshipDelta::between(
            &cva.relationships.records().last().unwrap(),
            &next,
        )
        .unwrap();
        let delta_publication =
            crate::relationship_compact_codec::publication::RelationshipPublication::Delta(
                delta.clone(),
            );
        append_relationship(&mut cva, delta_publication, next_global);
        let applied = crate::relationship_compact_apply::apply_delta(
            cva.relationships.records().last().unwrap(),
            &delta,
        )
        .unwrap();
        assert_eq!(applied, next);
        cva.relationships.insert_rebuilt(applied).unwrap();
        relationship_evidence.push(
            cva.relationships
                .retained_publication_at(next_global, next_local)
                .unwrap()
                .unwrap()
                .normalized_import_evidence()
                .unwrap(),
        );

        let terminal_global = cva.container.next_version_candidate();
        let terminal_local = cva.relationship_version() + 1;
        let tombstone = crate::relationship_codec::RelationshipTombstone { id, revision: 3 };
        append_relationship(
            &mut cva,
            crate::relationship_compact_codec::publication::RelationshipPublication::Tombstone {
                id,
                revision: tombstone.revision,
                relationship_version: terminal_local,
            },
            terminal_global,
        );
        cva.relationships
            .insert_tombstone_rebuilt(tombstone, terminal_global, terminal_local)
            .unwrap();
        relationship_evidence.push(
            cva.relationships
                .retained_publication_at(terminal_global, terminal_local)
                .unwrap()
                .unwrap()
                .normalized_import_evidence()
                .unwrap(),
        );
    }
    drop(import_scope);
    cva.reconstruct_historical_freshness(Default::default())
        .unwrap();
    cva.complete_native_import(&start.import_id, 200).unwrap();
    cva.sync().unwrap();
    drop(cva);

    let mut reopened = Cva::open(&path).unwrap();
    let epochs = super::inventory(&mut reopened).unwrap();
    let epoch = epochs.get(&start.import_id).unwrap();
    if let Some(expected) = envelope_evidence {
        assert!(
            epoch.owner_evidence.contains(&expected),
            "completed interval must retain canonical envelope publication facts"
        );
    }
    if with_entity_envelopes {
        assert_eq!(entity_evidence.len(), 6);
        for (index, expected) in entity_evidence.iter().enumerate() {
            assert!(
                epoch.owner_evidence.contains(expected),
                "completed epoch retains normalized Entity evidence {index}"
            );
        }
        let chunks = reopened.container.chunks().unwrap();
        let mut envelope_count = 0;
        for chunk in chunks.iter().take(epoch.end).skip(epoch.first) {
            let payload = reopened.container.read(*chunk).unwrap();
            envelope_count += usize::from(
                crate::entity_compact_codec::publication::coordinates(&payload)
                    .unwrap()
                    .is_some(),
            );
        }
        assert_eq!(
            envelope_count, 6,
            "All accepted Entity create/delta/direct/merge envelopes"
        );
    }
    if with_relationship_envelopes {
        assert_eq!(relationship_evidence.len(), 3);
        for (index, expected) in relationship_evidence.iter().enumerate() {
            assert!(
                epoch.owner_evidence.contains(expected),
                "completed epoch retains normalized Relationship evidence {index}"
            );
        }
        let chunks = reopened.container.chunks().unwrap();
        let mut relationship_envelopes = 0;
        for chunk in chunks.iter().take(epoch.end).skip(epoch.first) {
            let payload = reopened.container.read(*chunk).unwrap();
            relationship_envelopes += usize::from(
                crate::relationship_compact_codec::publication::coordinates(&payload)
                    .unwrap()
                    .is_some(),
            );
        }
        assert_eq!(
            relationship_envelopes, 3,
            "Relationship create/delta/terminal envelopes"
        );
        let descriptor = reopened
            .relationship_retention_descriptor(crate::RelationshipId([0x74; 32]))
            .unwrap();
        assert_eq!(descriptor.tombstone_revision, 3);
        assert_eq!(descriptor.tombstone_relationship_version, 3);
        assert_eq!(descriptor.revisions.len(), 2);
    }
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

#[test]
fn import_interval_classifier_rejects_malformed_and_foreign_publication_envelopes() {
    use reliquary_container::publication_envelope::{self, PublicationClock, PublicationEnvelope};

    for (name, body, malformed_physical) in [
        (
            "unknown-owner",
            b"CVAFORE1opaque foreign owner".as_slice(),
            false,
        ),
        ("malformed-entity", b"CVAENC02".as_slice(), false),
        ("malformed-relationship", b"CVARNC02".as_slice(), false),
        (
            "foreign-relationship-owner",
            b"CVAFORE1foreign relationship body".as_slice(),
            false,
        ),
        ("truncated-envelope", b"CVAPUB01".as_slice(), true),
    ] {
        let scratch = Scratch::new();
        let path = scratch.0.join(format!("{name}.rel"));
        let mut cva = Cva::create(&path).unwrap();
        let boundary = cva.container.chunks().unwrap().len();
        if malformed_physical {
            cva.container.append(body).unwrap();
        } else {
            let global = cva
                .container
                .allocate_version_with_transaction_time(Some(700))
                .unwrap();
            let envelope = publication_envelope::encode(PublicationEnvelope {
                global_version: global,
                clock: PublicationClock::BindReserved,
                body,
            })
            .unwrap();
            cva.container.append(&envelope).unwrap();
        }
        assert!(
            super::validate_replay_interval(&mut cva, boundary).is_err(),
            "strict import classification must reject {name}"
        );
    }
}
