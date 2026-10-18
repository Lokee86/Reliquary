use super::*;
use crate::imports::{
    ImportCompletionOutcome, ImportCompletionPending, ImportReadinessOwner, ImportReadinessStatus,
    ImportVectorReadiness, NativeImport,
};
use crate::insomnia::completion_tests::{process, setup_origin};
use crate::{
    EntityResolutionReason, EpisodeOrigin, EpisodePolicy, GraphRelationKind, GraphRelationOrigin,
    InsomniaExtractor, InsomniaWorkState, MemoryDraft, MemoryEntityMention, MemoryEntityMentionKey,
    MemoryRoutingMetadata, MemoryTextField, Phylactery, SimulatedGeneralEndpoint,
};
use serde_json::{Map, json};
use std::collections::BTreeSet;

fn attributed_import(namespace: &str, with_branch: bool) -> NativeImport {
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
            timestamp_ns: 10 + index as i64 * 2,
            content: format!("Atomic Insomnia completion fact {index}."),
            artifacts: vec![],
        });
        messages.push(ImportMessage {
            id: assistant.clone(),
            revision: Some("1".into()),
            parent_id: Some(user),
            replaces: None,
            deleted: false,
            role: "assistant".into(),
            timestamp_ns: 11 + index as i64 * 2,
            content: "Acknowledged.".into(),
            artifacts: vec![],
        });
        parent = Some(assistant);
    }
    if !with_branch {
        messages.push(ImportMessage {
            id: "deleted-head".into(),
            revision: Some("1".into()),
            parent_id: parent.clone(),
            replaces: None,
            deleted: true,
            role: "user".into(),
            timestamp_ns: 30,
            content: String::new(),
            artifacts: vec![],
        });
    }
    NativeImport {
        schema_version: 1,
        source_namespace: namespace.into(),
        source_format: "readiness-test-v1".into(),
        conversations: vec![ImportConversation {
            id: "imported".into(),
            title: None,
            messages,
            branches: vec![ImportBranch {
                id: "main".into(),
                leaf_message_id: if with_branch {
                    parent.unwrap()
                } else {
                    "deleted-head".into()
                },
                canonical: true,
            }],
        }],
    }
}

fn process_import_episode(
    cva: &mut Cva,
    count: usize,
    now_ns: i64,
) -> crate::InsomniaProcessResult {
    cva.container.set_owner_write_scope(true);
    let claim = cva
        .claim_insomnia_episode("import-readiness-worker", now_ns, 1_000)
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
        let clause = if user_index < count {
            json!({
                "disposition": "retain",
                "authority_kind": "direct",
                "category": "decision",
                "type": "project",
                "lifecycle": "current",
                "proposition": format!("Imported Insomnia completion invariant {user_index}."),
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
        if user_index < count {
            groups.insert(
                format!("g{user_index:03}"),
                json!({
                    "title": format!("Imported completion {user_index}"),
                    "content": format!("Imported Insomnia completion invariant {user_index}.")
                }),
            );
        }
        user_index += 1;
    }
    assert_eq!(
        user_index, 8,
        "fixture import Episode must contain eight user turns"
    );
    let mut responses = vec![json!({"turns": turn_responses, "evidence_requests": []})];
    if count > 0 {
        responses.push(json!({"groups": groups}));
    }
    let extractor = InsomniaExtractor::new(SimulatedGeneralEndpoint::new(
        "synthetic-import-readiness",
        responses,
    ));
    let result = cva
        .process_claimed_insomnia_episode(&claim, &extractor, "private", now_ns, now_ns + 1)
        .unwrap();
    cva.container.set_owner_write_scope(false);
    result
}

fn commit_scoped_settlement(cva: &mut Cva) -> crate::HistoricalReconstructionReport {
    let report = cva
        .reconstruct_historical_freshness(HistoricalReconstructionOptions::default())
        .unwrap();
    assert!(
        report.committed,
        "fixture must append a real scoped settlement"
    );
    let settlement = cva
        .freshness
        .import_settlement(report.import_id)
        .expect("accepted import settlement");
    assert!(settlement.epoch.is_some());
    assert!(settlement.encode().starts_with(b"CVAFRS05"));
    report
}

fn complete_zero_memory_import_episode(cva: &mut Cva, now_ns: i64) {
    let scope = cva.import_epoch_scope().unwrap();
    assert_eq!(scope.episodes.len(), 1);
    cva.container.set_owner_write_scope(true);
    let result = process_import_episode(cva, 0, now_ns);
    cva.container.set_owner_write_scope(false);
    assert!(result.created.is_empty());
    let work = cva
        .insomnia_work(crate::EpisodeId(*scope.episodes.iter().next().unwrap()))
        .expect("completed Import Episode work");
    assert_eq!(work.state, InsomniaWorkState::Complete);
    let completed_scope = cva.import_epoch_scope().unwrap();
    let output = completed_scope
        .outputs
        .get(completed_scope.episodes.iter().next().unwrap())
        .expect("durable Import Episode completion");
    assert!(output.memory_ids.is_empty());
    assert!(output.external_memory_refs.is_empty());
    assert!(output.records.is_empty());
}

fn assert_readiness_pending(
    outcome: ImportCompletionOutcome,
    owner: ImportReadinessOwner,
    status: ImportReadinessStatus,
) {
    let ImportCompletionOutcome::Pending(ImportCompletionPending::Readiness(blocker)) = outcome
    else {
        panic!("expected an inspectable readiness blocker, got {outcome:?}");
    };
    assert_eq!(blocker.owner, owner);
    assert_eq!(blocker.status, status);
    assert!(!blocker.identity.is_empty());
}

fn settle_initial_dream_pass(cva: &mut Cva, id: crate::MemoryId) {
    cva.container.set_owner_write_scope(true);
    let result = (|| {
        let pass = crate::dream_freshness::initial_pass_id(cva.owner_uuid().unwrap(), id);
        cva.begin_initial_dream_pass(
            pass.clone(),
            id,
            1,
            cva.rel_turn_count(),
            BTreeSet::new(),
            cva.graph_version(),
        )?;
        cva.reconcile_dream_lifecycle(id)
            .map_err(|error| crate::CvaError::Freshness(error.to_string()))?;
        cva.accept_initial_dream_settlement(&pass, cva.rel_turn_count())?;
        crate::dream_freshness::settle_initial_pass(cva, id, true, 1)
    })();
    cva.container.set_owner_write_scope(false);
    result.unwrap();
}

fn revise_readiness_memory(
    cva: &mut Cva,
    id: crate::MemoryId,
    archived: bool,
    superseded_by: Option<crate::MemoryId>,
    mutation_id: &str,
) {
    let current = cva.memory(id).unwrap();
    let draft = MemoryDraft {
        category: current.category.clone(),
        memory_type: current.memory_type.clone(),
        authority_kind: current.authority_kind.clone(),
        temporal_status: current.temporal_status.clone(),
        title: current.title.clone(),
        content: current.content.clone(),
        scope: current.scope.clone(),
        lifecycle_state: current.lifecycle_state.clone(),
        archived,
        superseded_by,
        parent_id: current.parent_id,
        source_node_id: current.source_node_id.clone(),
        content_source_conversation_id: current.content_source_conversation_id.clone(),
        content_source_node_id: current.content_source_node_id.clone(),
        grounding_source_conversation_id: current.grounding_source_conversation_id.clone(),
        grounding_source_node_id: current.grounding_source_node_id.clone(),
        source_episode_id: current.source_episode_id,
        source_time_ns: current.source_time_ns,
        mutation_id: mutation_id.into(),
        created_at_ns: current.created_at_ns,
        updated_at_ns: current.updated_at_ns + 1,
    };
    cva.container.set_owner_write_scope(true);
    cva.publish_memory(Some(id), current.revision, draft)
        .unwrap();
    cva.container.set_owner_write_scope(false);
}

#[test]
fn nonempty_zero_output_import_completes_with_exact_scoped_settlement() {
    let fixture = Fixture::new();
    let path = fixture.rel();
    let mut cva = Cva::create(&path).unwrap();
    cva.import_native(&attributed_import("readiness/zero-output", true), 100)
        .unwrap();
    let start = cva.open_import_epoch().unwrap();
    let scope = cva.import_epoch_scope().unwrap();
    assert!(!scope.source_nodes.is_empty());
    assert_eq!(scope.episodes.len(), 1);
    assert!(scope.cohort.is_empty());

    complete_zero_memory_import_episode(&mut cva, 110);
    let report = commit_scoped_settlement(&mut cva);
    let settlement = cva.freshness.import_settlement(report.import_id).unwrap();
    assert_eq!(settlement.cohort, scope.cohort);
    assert_eq!(
        settlement.epoch.as_ref().unwrap().source_nodes,
        scope.source_nodes
    );

    let report_bytes = fs::read(&path).unwrap();
    let report_clocks = clocks(&cva);
    let readiness = cva.native_import_readiness(&start.import_id).unwrap();
    assert!(readiness.blocker.is_none());
    assert_eq!(
        readiness.vectors,
        ImportVectorReadiness::SupportedUnavailable
    );
    assert_eq!(fs::read(&path).unwrap(), report_bytes);
    assert_eq!(clocks(&cva), report_clocks);

    let before = watermarks(&cva);
    let outcome = cva.complete_native_import(&start.import_id, 200).unwrap();
    let ImportCompletionOutcome::Completed {
        complete,
        committed,
    } = outcome
    else {
        panic!("ready nonempty zero-output import remained pending: {outcome:?}");
    };
    assert!(committed);
    assert_eq!(complete.post, before);
    assert!(cva.open_import_epoch().is_none());
    assert!(cva.import_epoch_status(&start.import_id).is_some());
    assert!(cva.native_import_readiness(&start.import_id).is_err());
    cva.append_node(
        "after-import".into(),
        "live-after-import".into(),
        None,
        "user".into(),
        201,
        "write admission reopened",
    )
    .unwrap();
}

#[test]
fn imported_pending_insomnia_work_keeps_completion_byte_clock_and_fence_neutral() {
    let fixture = Fixture::new();
    let path = fixture.rel();
    let mut cva = Cva::create(&path).unwrap();
    cva.import_native(&attributed_import("readiness/pending", true), 100)
        .unwrap();
    let start = cva.open_import_epoch().unwrap();
    let episode = crate::EpisodeId(
        *cva.import_epoch_scope()
            .unwrap()
            .episodes
            .iter()
            .next()
            .unwrap(),
    );
    assert_eq!(
        cva.insomnia_work(episode).unwrap().state,
        InsomniaWorkState::Pending
    );
    let before_bytes = fs::read(&path).unwrap();
    let before_clocks = clocks(&cva);
    let before_statuses = cva.import_epoch_statuses();

    let outcome = cva.complete_native_import(&start.import_id, 200).unwrap();
    assert_readiness_pending(
        outcome,
        ImportReadinessOwner::Insomnia,
        ImportReadinessStatus::Pending,
    );
    assert_eq!(fs::read(&path).unwrap(), before_bytes);
    assert_eq!(clocks(&cva), before_clocks);
    assert_eq!(cva.import_epoch_statuses(), before_statuses);
    assert_eq!(cva.open_import_epoch(), Some(start));
    assert!(
        cva.append_node(
            "blocked".into(),
            "live".into(),
            None,
            "user".into(),
            201,
            "the ImportStart fence remains active",
        )
        .is_err()
    );
}

#[test]
fn imported_terminal_insomnia_work_is_failed_and_keeps_completion_neutral() {
    let fixture = Fixture::new();
    let path = fixture.rel();
    let mut cva = Cva::create(&path).unwrap();
    cva.import_native(&attributed_import("readiness/terminal", true), 100)
        .unwrap();
    let start = cva.open_import_epoch().unwrap();
    let scope = cva.import_epoch_scope().unwrap();
    let episode = crate::EpisodeId(*scope.episodes.iter().next().unwrap());
    cva.container.set_owner_write_scope(true);
    let claim = cva
        .claim_insomnia_episode("readiness-terminal-worker", 110, 1_000)
        .unwrap()
        .unwrap();
    assert_eq!(claim.episode_id, episode);
    cva.terminal_insomnia_episode(
        episode,
        claim.lease_token.unwrap(),
        110,
        111,
        "terminal test failure".into(),
    )
    .unwrap();
    cva.container.set_owner_write_scope(false);
    assert_eq!(
        cva.insomnia_work(episode).unwrap().state,
        InsomniaWorkState::Terminal
    );
    let before_bytes = fs::read(&path).unwrap();
    let before_clocks = clocks(&cva);
    let before_statuses = cva.import_epoch_statuses();

    let outcome = cva.complete_native_import(&start.import_id, 200).unwrap();
    assert_readiness_pending(
        outcome,
        ImportReadinessOwner::Insomnia,
        ImportReadinessStatus::Failed,
    );
    assert_eq!(fs::read(&path).unwrap(), before_bytes);
    assert_eq!(clocks(&cva), before_clocks);
    assert_eq!(cva.import_epoch_statuses(), before_statuses);
    assert_eq!(cva.open_import_epoch(), Some(start));
    assert!(
        cva.append_node(
            "blocked".into(),
            "live".into(),
            None,
            "user".into(),
            201,
            "terminal imported work keeps the fence active",
        )
        .is_err()
    );
}

#[test]
fn unrelated_processing_insomnia_work_does_not_block_import_completion() {
    let fixture = Fixture::new();
    let path = fixture.rel();
    let mut cva = Cva::create(&path).unwrap();

    // Start unrelated live work first and leave it Processing. The import's
    // exact Episode set must still be independently completed and settled.
    let mut parent = None;
    for index in 0..8 {
        let user = format!("live-u{index}");
        let assistant = format!("live-a{index}");
        cva.append_node(
            user.clone(),
            "unrelated-live".into(),
            parent.clone(),
            "user".into(),
            10 + index as i64 * 2,
            "Unrelated live work.",
        )
        .unwrap();
        cva.append_node(
            assistant.clone(),
            "unrelated-live".into(),
            Some(user),
            "assistant".into(),
            11 + index as i64 * 2,
            "Acknowledged.",
        )
        .unwrap();
        parent = Some(assistant);
    }
    let live = cva
        .finalize_explicit_path_and_queue(
            "unrelated-live",
            parent.as_deref().unwrap(),
            EpisodePolicy::default(),
            40,
        )
        .unwrap();
    let live_episode = live.episodes.created.first().unwrap().id;
    let live_claim = cva
        .claim_insomnia_episode("unrelated-worker", 41, 1_000)
        .unwrap()
        .unwrap();
    assert_eq!(live_claim.episode_id, live_episode);

    cva.import_native(&attributed_import("readiness/unrelated", true), 100)
        .unwrap();
    let start = cva.open_import_epoch().unwrap();
    let scope = cva.import_epoch_scope().unwrap();
    assert_eq!(scope.episodes.len(), 1);
    assert!(!scope.episodes.contains(&live_episode.0));

    cva.container.set_owner_write_scope(true);
    let result = process_import_episode(&mut cva, 0, 110);
    cva.container.set_owner_write_scope(false);
    assert!(result.created.is_empty());
    assert_eq!(
        cva.insomnia_work(live_episode).unwrap().state,
        InsomniaWorkState::Processing
    );
    let stats = cva.insomnia_stats();
    assert!(stats.processing >= 1);

    commit_scoped_settlement(&mut cva);
    let before = watermarks(&cva);
    let outcome = cva.complete_native_import(&start.import_id, 200).unwrap();
    assert!(matches!(
        outcome,
        ImportCompletionOutcome::Completed {
            committed: true,
            ..
        }
    ));
    assert_eq!(cva.import_epoch_status(&start.import_id).is_some(), true);
    assert_eq!(
        cva.insomnia_work(live_episode).unwrap().state,
        InsomniaWorkState::Processing
    );
    // Completion itself contributes the only post-settlement version advance.
    assert_eq!(cva.latest_global_version(), before.global_version);
}

#[test]
fn branchless_nonempty_import_is_rejected_without_creating_readiness_state() {
    let fixture = Fixture::new();
    let path = fixture.rel();
    let mut cva = Cva::create(&path).unwrap();
    let before_bytes = fs::read(&path).unwrap();
    let before_clocks = clocks(&cva);

    // A malformed source with unreachable non-deleted messages cannot create
    // an ImportStart fence or a receipt that could later pass vacuous readiness.
    let mut malformed = attributed_import("readiness/branchless", true);
    malformed.conversations[0].branches.clear();
    assert!(cva.import_native(&malformed, 100).is_err());
    assert_eq!(fs::read(&path).unwrap(), before_bytes);
    assert_eq!(clocks(&cva), before_clocks);
    assert!(cva.open_import_epoch().is_none());
    assert!(cva.native_import_receipts().is_empty());
}

#[test]
fn routed_phy_import_output_is_explicitly_unsupported_for_native_readiness() {
    let fixture = Fixture::new();
    let path = fixture.rel();
    let phy_path = fixture.0.join("routed.phy");
    let mut cva = Cva::create(&path).unwrap();
    let mut phy = Phylactery::create(&phy_path).unwrap();
    cva.import_native(&attributed_import("readiness/routed-phy", true), 100)
        .unwrap();
    let start = cva.open_import_epoch().unwrap();

    cva.container.set_owner_write_scope(true);
    let claim = cva
        .claim_insomnia_episode("routed-import-worker", 110, 1_000)
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
                "category": "preference",
                "type": "communication",
                "lifecycle": "current",
                "proposition": "The user prefers concise answers.",
                "authority_source_node_id": "",
                "grounding_source_node_id": "",
                "reason": "durable imported preference"
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
                "reason": "no additional durable fact"
            })
        };
        turn_responses.insert(turn.node_id.clone(), json!([clause]));
        if index == 0 {
            groups.insert(
                "g000".to_owned(),
                json!({
                    "title": "Concise responses",
                    "content": "The user prefers concise answers."
                }),
            );
        }
    }
    let extractor = InsomniaExtractor::new(SimulatedGeneralEndpoint::new(
        "synthetic-routed-import",
        vec![
            json!({"turns": turn_responses, "evidence_requests": []}),
            json!({"groups": groups}),
        ],
    ))
    .with_ownership_endpoint(SimulatedGeneralEndpoint::new(
        "synthetic-routed-ownership",
        vec![json!({"groups": {"g000": {"ownership": "user"}}})],
    ));
    let result = cva
        .process_claimed_insomnia_episode_routed(&mut phy, &claim, &extractor, "private", 110, 111)
        .unwrap();
    cva.container.set_owner_write_scope(false);
    assert!(result.created.is_empty());
    assert_eq!(result.user_created.len(), 1);
    let scope = cva.import_epoch_scope().unwrap();
    let output = scope
        .outputs
        .get(&claim.episode_id.0)
        .expect("retained routed completion");
    assert!(output.memory_ids.is_empty());
    assert_eq!(output.external_memory_refs.len(), 1);

    let readiness = cva.native_import_readiness(&start.import_id).unwrap();
    let blocker = readiness
        .blocker
        .expect("external PHY output is unsupported");
    assert_eq!(blocker.owner, ImportReadinessOwner::Memory);
    assert_eq!(blocker.status, ImportReadinessStatus::Unsupported);
    assert!(
        blocker.identity.contains(
            output
                .external_memory_refs
                .iter()
                .next()
                .unwrap()
                .owner_id
                .as_str()
        )
    );
    assert_eq!(
        readiness.vectors,
        ImportVectorReadiness::SupportedUnavailable
    );
}

#[test]
fn imported_insomnia_output_ids_without_memory_publication_evidence_are_rejected() {
    let fixture = Fixture::new();
    let path = fixture.rel();
    let mut cva = Cva::create(&path).unwrap();
    cva.import_native(&attributed_import("readiness/forged-output", true), 100)
        .unwrap();
    let start = cva.open_import_epoch().unwrap();
    let scope = cva.import_epoch_scope().unwrap();
    let episode = crate::EpisodeId(*scope.episodes.iter().next().unwrap());
    // Spoof the public mutation/source fields to resemble import-attributed
    // Memory. Only a retained grouped Insomnia completion record can prove it.
    let episode_hex = episode
        .0
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let (source_conversation_id, source_node_id) = scope
        .source_nodes
        .iter()
        .next()
        .expect("import source node");
    cva.container.set_owner_write_scope(true);
    let (unrelated, _) = cva
        .publish_memory(
            None,
            0,
            crate::MemoryDraft {
                category: "fact".into(),
                memory_type: "project".into(),
                authority_kind: "direct".into(),
                temporal_status: "current".into(),
                title: "Spoofed Memory".into(),
                content: "This Memory has caller-controlled import attribution.".into(),
                scope: "private".into(),
                lifecycle_state: "knowledge".into(),
                archived: false,
                superseded_by: None,
                parent_id: None,
                source_node_id: Some(source_node_id.clone()),
                source_episode_id: Some(episode),
                source_time_ns: Some(10),
                content_source_conversation_id: Some(source_conversation_id.clone()),
                content_source_node_id: Some(source_node_id.clone()),
                grounding_source_conversation_id: None,
                grounding_source_node_id: None,
                mutation_id: format!("insomnia:{episode_hex}:spoofed"),
                created_at_ns: 101,
                updated_at_ns: 101,
            },
        )
        .unwrap();
    let claim = cva
        .claim_insomnia_episode("forged-output-worker", 110, 1_000)
        .unwrap()
        .unwrap();
    cva.complete_insomnia_episode(
        episode,
        claim.lease_token.unwrap(),
        110,
        111,
        "test".into(),
        "test-v1".into(),
        vec![unrelated.id],
        0,
    )
    .unwrap();
    cva.container.set_owner_write_scope(false);
    let before_bytes = fs::read(&path).unwrap();
    let before_clocks = clocks(&cva);

    let outcome = cva.complete_native_import(&start.import_id, 200).unwrap();
    assert_readiness_pending(
        outcome,
        ImportReadinessOwner::Memory,
        ImportReadinessStatus::ConflictingEvidence,
    );
    assert_eq!(fs::read(&path).unwrap(), before_bytes);
    assert_eq!(clocks(&cva), before_clocks);
    assert_eq!(cva.open_import_epoch(), Some(start));
}

#[test]
fn import_born_memory_and_dream_completion_accepts_affected_freshness_result_cohort() {
    let (_path, mut cva, _) = setup_origin("readiness-nonzero-memory.rel", EpisodeOrigin::Live);
    let existing = process(&mut cva, 2, 110).created;
    let old = existing[0].id;
    let unrelated = existing[1].id;
    cva.freshness_admit(old, cva.rel_turn_count()).unwrap();
    settle_initial_dream_pass(&mut cva, old);
    cva.freshness_admit(unrelated, cva.rel_turn_count())
        .unwrap();
    let unrelated_memory = cva.memory(unrelated).unwrap();
    let unrelated_text = unrelated_memory
        .content
        .split_whitespace()
        .next()
        .expect("unrelated Memory content");
    let unrelated_start = unrelated_memory.content.find(unrelated_text).unwrap();
    let unrelated_mention = MemoryEntityMention {
        field: MemoryTextField::Content,
        start_byte: unrelated_start as u32,
        end_byte: (unrelated_start + unrelated_text.len()) as u32,
        text: unrelated_text.into(),
    };
    let unrelated_key = MemoryEntityMentionKey::new(unrelated, &unrelated_mention);
    let unrelated_body_id = cva.memory_body_id(unrelated).unwrap();
    cva.container.set_owner_write_scope(true);
    cva.memories
        .put_routing_metadata(
            &mut cva.container,
            MemoryRoutingMetadata {
                memory_id: unrelated,
                body_id: unrelated_body_id,
                entity_mentions: vec![unrelated_mention],
            },
        )
        .unwrap();
    cva.container.set_owner_write_scope(false);

    cva.import_native(&attributed_import("readiness/nonzero-memory", true), 200)
        .unwrap();
    let start = cva.open_import_epoch().unwrap();
    let result = process_import_episode(&mut cva, 1, 210);
    assert_eq!(result.created.len(), 1);
    let scope = cva.import_epoch_scope().unwrap();
    assert_eq!(scope.cohort.len(), 1);
    let imported_memory = *scope.cohort.iter().next().unwrap();

    // Force one exact routed mention so Perception readiness cannot be vacuous.
    let memory = cva.memory(imported_memory).unwrap();
    let mention_text = "Insomnia";
    let mention_start = memory.content.find(mention_text).unwrap();
    let mention = MemoryEntityMention {
        field: MemoryTextField::Content,
        start_byte: mention_start as u32,
        end_byte: (mention_start + mention_text.len()) as u32,
        text: mention_text.into(),
    };
    let mention_key = MemoryEntityMentionKey::new(imported_memory, &mention);
    let routing = MemoryRoutingMetadata {
        memory_id: imported_memory,
        body_id: cva.memory_body_id(imported_memory).unwrap(),
        entity_mentions: vec![mention],
    };
    cva.container.set_owner_write_scope(true);
    cva.memories
        .put_routing_metadata(&mut cva.container, routing)
        .unwrap();
    cva.container.set_owner_write_scope(false);

    // A real accepted initial Dream proof makes the import-born Memory ready.
    let pass = crate::dream_freshness::initial_pass_id(cva.owner_uuid().unwrap(), imported_memory);
    let graph_version = cva.graph_version();
    cva.container.set_owner_write_scope(true);
    cva.begin_initial_dream_pass(
        pass.clone(),
        imported_memory,
        1,
        cva.rel_turn_count(),
        BTreeSet::from([old]),
        graph_version,
    )
    .unwrap();
    cva.set_memory_relation_with_origin(
        imported_memory,
        old,
        GraphRelationKind::Topical,
        true,
        GraphRelationOrigin::Dream,
        graph_version,
    )
    .unwrap();
    cva.reconcile_dream_lifecycle(imported_memory).unwrap();
    cva.accept_initial_dream_settlement(&pass, cva.rel_turn_count())
        .unwrap();
    crate::dream_freshness::settle_initial_pass(&mut cva, imported_memory, true, 220).unwrap();
    let dream_epoch = cva
        .dream_eligible_epoch(imported_memory, 220)
        .unwrap()
        .unwrap_or(0);
    assert!(
        cva.mark_dream_processed(imported_memory, dream_epoch, 220)
            .unwrap()
    );
    cva.container.set_owner_write_scope(false);
    assert!(cva.initial_dream_pass_settled(&pass));
    assert!(cva.dream_was_processed(imported_memory));

    let pending_perception = cva.native_import_readiness(&start.import_id).unwrap();
    let blocker = pending_perception
        .blocker
        .expect("exact imported mention awaits Perception");
    assert_eq!(blocker.owner, ImportReadinessOwner::Perception);
    assert_eq!(blocker.status, ImportReadinessStatus::Pending);
    assert!(
        blocker
            .identity
            .contains(&format!("{:?}", MemoryTextField::Content))
    );

    // A persisted unresolved owner record remains a blocker until a terminal
    // resolution replaces its exact revision.
    cva.container.set_owner_write_scope(true);
    assert!(
        cva.put_entity_unresolved(
            mention_key,
            0,
            vec![],
            EntityResolutionReason::InsufficientEvidence,
            [3; 32],
            [4; 32],
            229,
        )
        .unwrap()
    );
    cva.container.set_owner_write_scope(false);
    let unresolved = cva.native_import_readiness(&start.import_id).unwrap();
    let blocker = unresolved
        .blocker
        .expect("persisted unresolved mention remains a Perception blocker");
    assert_eq!(blocker.owner, ImportReadinessOwner::Perception);
    assert_eq!(blocker.status, ImportReadinessStatus::Pending);

    // Persist a terminal owner-authored rejection, then readiness may advance.
    cva.container.set_owner_write_scope(true);
    assert!(
        cva.put_entity_rejected(mention_key, 1, EntityResolutionReason::GenericRole, 230,)
            .unwrap()
    );
    cva.container.set_owner_write_scope(false);
    assert!(matches!(
        &cva.entity_resolution(mention_key).unwrap().status,
        crate::MemoryEntityResolutionStatus::Rejected { .. }
    ));

    cva.container.set_owner_write_scope(true);
    assert!(
        cva.put_entity_unresolved(
            unrelated_key,
            0,
            vec![],
            EntityResolutionReason::InsufficientEvidence,
            [1; 32],
            [2; 32],
            240,
        )
        .unwrap()
    );
    cva.container.set_owner_write_scope(false);

    let report = commit_scoped_settlement(&mut cva);
    let settlement = cva.freshness.import_settlement(report.import_id).unwrap();
    assert!(settlement.cohort.contains(&imported_memory));
    assert!(
        settlement.cohort.contains(&old),
        "the import's real Dream effect updates a pre-existing eligible recipient"
    );
    assert!(!scope.cohort.contains(&old));

    // Unrelated pending Dream and Perception work is outside this import's scope.
    let unrelated_pass =
        crate::dream_freshness::initial_pass_id(cva.owner_uuid().unwrap(), unrelated);
    cva.container.set_owner_write_scope(true);
    cva.begin_initial_dream_pass(
        unrelated_pass.clone(),
        unrelated,
        1,
        cva.rel_turn_count(),
        BTreeSet::new(),
        cva.graph_version(),
    )
    .unwrap();
    cva.container.set_owner_write_scope(false);
    assert!(!cva.initial_dream_pass_settled(&unrelated_pass));
    assert!(matches!(
        &cva.entity_resolution(unrelated_key).unwrap().status,
        crate::MemoryEntityResolutionStatus::Pending(_)
    ));

    let ready = cva.native_import_readiness(&start.import_id).unwrap();
    assert!(ready.blocker.is_none());
    assert_eq!(ready.vectors, ImportVectorReadiness::SupportedUnavailable);

    let outcome = cva.complete_native_import(&start.import_id, 300).unwrap();
    let ImportCompletionOutcome::Completed { committed, .. } = outcome else {
        panic!("ready import-born Memory with accepted Dream evidence stayed pending: {outcome:?}");
    };
    assert!(committed);
    assert!(cva.open_import_epoch().is_none());

    drop(cva);
    let mut reopened = Cva::open(&_path).unwrap();
    let retry = reopened
        .complete_native_import(&start.import_id, 301)
        .unwrap();
    assert!(matches!(
        retry,
        ImportCompletionOutcome::Completed {
            committed: false,
            ..
        }
    ));
}

#[test]
fn pending_import_born_dream_pass_is_reported_by_exact_memory_identity() {
    let (path, mut cva, _) = setup_origin("readiness-dream-pending.rel", EpisodeOrigin::Live);
    // Existing Memories are irrelevant to this epoch; only its true birth should
    // be reported as the pending Dream identity.
    let old = process(&mut cva, 1, 110).created[0].id;
    cva.freshness_admit(old, cva.rel_turn_count()).unwrap();
    settle_initial_dream_pass(&mut cva, old);

    cva.import_native(&attributed_import("readiness/dream-pending", true), 200)
        .unwrap();
    let start = cva.open_import_epoch().unwrap();
    let result = process_import_episode(&mut cva, 1, 210);
    let imported_memory = result.created[0].id;
    // Dream's initial pass remains required for a superseded but non-archived
    // publication birth; the current Memory revision keeps its import source.
    revise_readiness_memory(
        &mut cva,
        imported_memory,
        false,
        Some(old),
        "readiness-superseded-before-dream",
    );
    let before_bytes = fs::read(&path).unwrap();
    let before_clocks = clocks(&cva);
    let pass_id =
        crate::dream_freshness::initial_pass_id(cva.owner_uuid().unwrap(), imported_memory);
    assert!(!cva.initial_dream_pass_settled(&pass_id));
    let readiness = cva.native_import_readiness(&start.import_id).unwrap();
    let blocker = readiness.blocker.unwrap();
    assert_eq!(blocker.owner, ImportReadinessOwner::Dream);
    assert_eq!(blocker.status, ImportReadinessStatus::Pending);
    assert_eq!(blocker.identity, pass_id);

    let outcome = cva.complete_native_import(&start.import_id, 300).unwrap();
    assert_readiness_pending(
        outcome,
        ImportReadinessOwner::Dream,
        ImportReadinessStatus::Pending,
    );
    assert_eq!(fs::read(&path).unwrap(), before_bytes);
    assert_eq!(clocks(&cva), before_clocks);
    assert_eq!(cva.open_import_epoch(), Some(start));
}

#[test]
fn archived_import_born_memory_is_exempt_from_initial_dream_pass() {
    let (_path, mut cva, _) = setup_origin("readiness-archived-dream.rel", EpisodeOrigin::Live);
    assert!(process(&mut cva, 0, 110).created.is_empty());
    cva.import_native(&attributed_import("readiness/archived-dream", true), 200)
        .unwrap();
    let start = cva.open_import_epoch().unwrap();
    let result = process_import_episode(&mut cva, 1, 210);
    let imported_memory = result.created[0].id;
    revise_readiness_memory(
        &mut cva,
        imported_memory,
        true,
        None,
        "readiness-archive-before-dream",
    );

    // Dream itself will not begin an initial pass for an archived source.
    // Readiness must not wait forever for work the owner intentionally omits.
    let readiness = cva.native_import_readiness(&start.import_id).unwrap();
    let blocker = readiness.blocker.expect("settlement is still absent");
    assert_eq!(blocker.owner, ImportReadinessOwner::Freshness);
    assert_eq!(blocker.status, ImportReadinessStatus::Missing);
}

#[test]
fn archived_import_memory_with_existing_pending_dream_pass_stays_blocked() {
    let (path, mut cva, _) =
        setup_origin("readiness-archived-pending-dream.rel", EpisodeOrigin::Live);
    assert!(process(&mut cva, 0, 110).created.is_empty());
    cva.import_native(
        &attributed_import("readiness/archived-pending-dream", true),
        200,
    )
    .unwrap();
    let start = cva.open_import_epoch().unwrap();
    let result = process_import_episode(&mut cva, 1, 210);
    let imported_memory = result.created[0].id;

    // Runtime Dream reuses an existing pending pass before checking whether a
    // new pass could be started for the now-archived source.
    cva.container.set_owner_write_scope(true);
    let pass =
        crate::dream_freshness::begin_initial_pass(&mut cva, imported_memory, std::iter::empty())
            .unwrap()
            .expect("live import birth starts its runtime Dream pass");
    cva.container.set_owner_write_scope(false);
    revise_readiness_memory(
        &mut cva,
        imported_memory,
        true,
        None,
        "readiness-archive-with-pending-dream",
    );
    assert_eq!(
        cva.pending_initial_dream_pass(imported_memory)
            .unwrap()
            .pass_id,
        pass.pass_id
    );

    let before_bytes = fs::read(&path).unwrap();
    let before_clocks = clocks(&cva);
    let readiness = cva.native_import_readiness(&start.import_id).unwrap();
    let blocker = readiness
        .blocker
        .expect("already-pending Dream pass remains an obligation");
    assert_eq!(blocker.owner, ImportReadinessOwner::Dream);
    assert_eq!(blocker.status, ImportReadinessStatus::Pending);
    assert_eq!(blocker.identity, pass.pass_id);

    let outcome = cva.complete_native_import(&start.import_id, 300).unwrap();
    assert_readiness_pending(
        outcome,
        ImportReadinessOwner::Dream,
        ImportReadinessStatus::Pending,
    );
    assert_eq!(fs::read(&path).unwrap(), before_bytes);
    assert_eq!(clocks(&cva), before_clocks);
    assert_eq!(cva.open_import_epoch(), Some(start));
    assert!(
        cva.append_node(
            "blocked-after-archive".into(),
            "live-after-import".into(),
            None,
            "user".into(),
            301,
            "the import fence remains active",
        )
        .is_err()
    );
    assert_eq!(fs::read(&path).unwrap(), before_bytes);
}

#[test]
fn ready_insomnia_and_vacuous_downstream_work_still_require_semantic_freshness_settlement() {
    let fixture = Fixture::new();
    let path = fixture.rel();
    let mut cva = Cva::create(&path).unwrap();
    cva.import_native(&attributed_import("readiness/missing-semantic", true), 100)
        .unwrap();
    let start = cva.open_import_epoch().unwrap();
    complete_zero_memory_import_episode(&mut cva, 110);
    let before_bytes = fs::read(&path).unwrap();
    let before_clocks = clocks(&cva);

    let outcome = cva.complete_native_import(&start.import_id, 200).unwrap();
    assert_eq!(
        outcome,
        ImportCompletionOutcome::Pending(ImportCompletionPending::MissingFreshnessSettlement,)
    );
    assert_eq!(fs::read(&path).unwrap(), before_bytes);
    assert_eq!(clocks(&cva), before_clocks);
    assert_eq!(cva.open_import_epoch(), Some(start));
}
