//! Runner-only process aborts preserve physical files without Drop/shutdown.
//! Durably staged intermediate states use existing source owners, not hooks.
use super::fixture::*;
use super::*;
use crate::multiplexing_journal::{DurableGenerationPhase, FinalizeCapture, SubmissionState};
use crate::{
    Branch, Cva, InteractionRole, InteractionRuntime, InteractionStreamRecord,
    InteractionStreamStatus,
};
use serde_json::json;
use sha2::Digest;
use std::path::PathBuf;

fn directory() -> PathBuf {
    PathBuf::from(
        std::env::var_os("RELIQUARY_P6_CRASH_DIR")
            .expect("runner supplies isolated crash directory"),
    )
}

#[test]
#[ignore = "runner invokes this in an isolated subprocess that must abort"]
fn p6_crash_child() {
    let root = directory();
    let mode = std::env::var("RELIQUARY_P6_CRASH_MODE").unwrap();
    std::fs::create_dir_all(&root).unwrap();
    let host = std::sync::Arc::new(super::host::DurableHost::at(&root.join("host.json")));
    let mut g = if mode.starts_with("host-") {
        host.lose_ack();
        super::host::gateway_host(&root, host.clone())
    } else {
        gateway(&root)
    };
    g.episode_policy.episode.max_input_bytes = 1;
    let owner = g.mount_rel(seeded(&root.join("owner.rel"))).unwrap();
    let w = attach(&g, &[owner.clone()], None, "alice", true);
    let token = g.authorize(&w, &owner, OwnerGrant::ViewRel).unwrap();
    let initial_turn_count = g
        .with_rel(&token, |r| Ok(r.cva().rel_turn_count()))
        .unwrap();
    let req = request(&g, &owner, "a", 300, "root-a");
    let accepted = g.submit_conversation(&w, &owner, req.clone()).unwrap();
    let mut generation = None;
    match mode.as_str() {
        "host-after-acceptance" | "host-cancel-before-replay" => {
            g.process_next_conversation_submission(&w, &owner, "a")
                .unwrap()
                .unwrap();
            if mode == "host-cancel-before-replay" {
                let operation = host.operation_id();
                host.set_status(&operation, HostOperationStatus::Unknown);
                g.cancel_conversation_generation(&w, &owner, "a", &operation)
                    .unwrap();
            }
        }
        "admission" => {}
        "finalize-intent" | "archive-before-receipt" => {
            let journal = g.conversation.journal(&owner).unwrap();
            let mut journal = journal.lock().unwrap();
            let revision = journal.conversation_revision(&accepted.id.conversation_id);
            journal
                .claim_finalize(
                    &accepted.id,
                    revision,
                    FinalizeCapture {
                        branch_id: "main".into(),
                        branch_label: None,
                        parent_node_id: Some("root-a".into()),
                        committed_timestamp_ns: 123,
                        verified_principal_id: "alice".into(),
                        policy_revision: 1,
                    },
                )
                .unwrap();
            drop(journal);
            if mode == "archive-before-receipt" {
                let t = g.authorize(&w, &owner, OwnerGrant::ManageRel).unwrap();
                g.with_rel(&t, |r| {
                    r.cva
                        .append_node_with_principal(
                            id(300),
                            "a".into(),
                            Some("root-a".into()),
                            "user".into(),
                            Some("alice".into()),
                            123,
                            &req.content,
                        )
                        .map_err(operation)?;
                    r.cva
                        .append_branch(Branch {
                            id: "main".into(),
                            conversation_id: "a".into(),
                            leaf_node_id: id(300),
                            canonical: true,
                        })
                        .map_err(operation)?;
                    r.cva.sync().map_err(operation)
                })
                .unwrap();
            }
        }
        "checkpoint" | "checkpoint-source-ahead" => {
            g.process_next_accepted_conversation_submission(&owner, "a")
                .unwrap()
                .unwrap();
            let started = g
                .start_conversation_generation(&owner, "a", &id(300))
                .unwrap()
                .status
                .generation
                .unwrap();
            g.checkpoint_conversation_generation(
                &owner,
                "a",
                &started.operation_id,
                1,
                "durable é prefix",
            )
            .unwrap();
            if mode == "checkpoint-source-ahead" {
                let t = g.authorize(&w, &owner, OwnerGrant::ManageRel).unwrap();
                g.with_rel(&t, |r| {
                    r.cva
                        .put_interaction_stream(InteractionStreamRecord {
                            message_id: started.generation_id.clone(),
                            session_id: "a".into(),
                            parent_message_id: Some(started.parent_node_id.clone()),
                            role: InteractionRole::Agent,
                            timestamp_ns: started.started_timestamp_ns,
                            content: "durable é prefix unacknowledged tail".into(),
                            status: InteractionStreamStatus::Streaming,
                        })
                        .map_err(operation)?;
                    r.cva.sync().map_err(operation)
                })
                .unwrap();
            }
            generation = Some(started);
        }
        _ => panic!("unknown crash boundary"),
    }
    let marker = json!({"owner":owner,"submission":id(300),"generation":generation,"mode":mode,"initial_turn_count":initial_turn_count});
    let bytes = serde_json::to_vec(&marker).unwrap();
    let mut file = std::fs::File::create(root.join("identity.json")).unwrap();
    std::io::Write::write_all(&mut file, &bytes).unwrap();
    file.sync_all().unwrap();
    println!("P6_CRASH_READY {}", serde_json::to_string(&marker).unwrap());
    std::io::Write::flush(&mut std::io::stdout()).unwrap();
    std::process::abort();
}

#[test]
#[ignore = "runner reopens physical files left by the abrupt crash child"]
fn p6_crash_recover() {
    let root = directory();
    let marker: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join("identity.json")).unwrap()).unwrap();
    let owner = marker["owner"].as_str().unwrap();
    let mode = marker["mode"].as_str().unwrap();
    let mut version = None;
    let mut semantic_state = None;
    for pass in 0..2 {
        let host = std::sync::Arc::new(super::host::DurableHost::at(&root.join("host.json")));
        if pass == 0 && mode.starts_with("host-") {
            let op = host.operation_id();
            host.set_status(
                &op,
                if mode == "host-cancel-before-replay" {
                    HostOperationStatus::Pending
                } else {
                    HostOperationStatus::Completed
                },
            );
        }
        let mut g = if mode.starts_with("host-") {
            super::host::gateway_host(&root, host.clone())
        } else {
            gateway(&root)
        };
        g.episode_policy.episode.max_input_bytes = 1;
        assert_eq!(
            g.mount_rel(InteractionRuntime::new(
                Cva::open(root.join("owner.rel")).unwrap()
            ))
            .unwrap(),
            owner
        );
        if mode.starts_with("host-") {
            let w = attach(&g, &[owner.into()], None, "alice", true);
            if pass == 0 {
                let result = g
                    .resume_conversation_submission(&w, owner, "a", &id(300))
                    .unwrap();
                assert_eq!(
                    result.status.state,
                    if mode == "host-cancel-before-replay" {
                        SubmissionState::Cancelled
                    } else {
                        SubmissionState::Committed
                    }
                );
                assert!(
                    host.calls()
                        .iter()
                        .all(|call| !call.starts_with("handoff:"))
                );
                if mode == "host-cancel-before-replay" {
                    assert!(host.calls().iter().any(|call| call.starts_with("cancel:")));
                }
            }
            assert!(
                g.process_next_accepted_conversation_submission(owner, "a")
                    .unwrap()
                    .is_none()
            );
        } else if marker["generation"].is_null() {
            if pass == 0 && (mode == "admission" || mode == "finalize-intent") {
                let result = g
                    .process_next_accepted_conversation_submission(owner, "a")
                    .unwrap()
                    .unwrap();
                assert_eq!(result.status.state, SubmissionState::Committed);
            } else {
                // Mount reconciliation may already finish the Archive/journal handoff.
                if let Some(result) = g
                    .process_next_accepted_conversation_submission(owner, "a")
                    .unwrap()
                {
                    assert_eq!(result.status.state, SubmissionState::Committed);
                }
            }
            assert!(
                g.process_next_accepted_conversation_submission(owner, "a")
                    .unwrap()
                    .is_none()
            );
        } else {
            let w = attach(&g, &[owner.into()], None, "viewer", false);
            let op = marker["generation"]["operation_id"].as_str().unwrap();
            let status = g
                .conversation_generation_status(&w, owner, "a", op)
                .unwrap()
                .status
                .generation
                .unwrap();
            assert_eq!(status.phase, DurableGenerationPhase::Interrupted);
            let expected = if mode == "checkpoint-source-ahead" {
                "durable é prefix unacknowledged tail"
            } else {
                "durable é prefix"
            };
            let checkpoint = status.checkpoint.as_ref().unwrap();
            assert_eq!(checkpoint.utf8_bytes, expected.len() as u64);
            assert_eq!(
                checkpoint.sequence,
                if mode == "checkpoint-source-ahead" {
                    2
                } else {
                    1
                }
            );
            let digest: [u8; 32] = sha2::Sha256::digest(expected.as_bytes()).into();
            assert_eq!(checkpoint.sha256, digest);
            assert_eq!(
                status.generation_id,
                marker["generation"]["generation_id"].as_str().unwrap()
            );
            let key = super::super::super::streams::StreamKey {
                rel_id: crate::multiplexing_journal::RelOwnerId::parse_canonical(owner).unwrap(),
                mount_epoch: g
                    .authorize(&w, owner, OwnerGrant::ViewRel)
                    .unwrap()
                    .mount_epoch,
                conversation_id: crate::multiplexing_journal::ConversationId::new("a").unwrap(),
                operation_id: status.operation_id.clone(),
                generation_id: status.generation_id.clone(),
                branch_id: status.branch_id.clone(),
            };
            let (snapshot, subscription) = g
                .subscribe_authoritative_conversation_stream(&w, key, 8)
                .unwrap();
            assert_eq!(snapshot.source.checkpoint.unwrap().text, expected);
            assert!(
                g.next_conversation_stream(&w, &subscription)
                    .unwrap()
                    .is_none()
            );
        }
        let cva = g.unmount_rel(owner).unwrap();
        assert_eq!(
            cva.archive().has_node("a", &id(300)),
            mode != "host-cancel-before-replay"
        );
        let committed = mode != "host-cancel-before-replay";
        let initial = marker["initial_turn_count"].as_u64().unwrap();
        assert_eq!(cva.rel_turn_count(), initial + u64::from(committed));
        assert_eq!(
            cva.activity_position_for_turn("a", &id(300)),
            committed.then_some(initial + 1)
        );
        let branches = cva.archive().branches();
        let main = branches
            .iter()
            .find(|b| b.conversation_id == "a" && b.id == "main")
            .unwrap();
        assert_eq!(
            main.leaf_node_id,
            if committed { id(300) } else { "root-a".into() }
        );
        assert_eq!(cva.episodes_for_conversation("a").is_empty(), !committed);
        assert!(cva.episodes_for_conversation("b").is_empty());
        let state = (
            cva.rel_turn_count(),
            cva.episodes(),
            cva.insomnia_stats(),
            branches,
        );
        if let Some(expected) = semantic_state.as_ref() {
            assert_eq!(
                &state, expected,
                "second recovery duplicated activity, Episode/work or branch publication"
            );
        }
        semantic_state = Some(state);
        if let Some(expected) = version {
            assert_eq!(
                cva.archive_version(),
                expected,
                "second recovery duplicated semantic publication"
            );
        }
        version = Some(cva.archive_version());
        cva.sync().unwrap();
        drop(cva);
        metric(
            "abrupt_recovery",
            json!({"mode":mode,"pass":pass,"archive_version":version}),
        );
    }
}
