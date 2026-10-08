use super::fixture::*;
use super::*;
use crate::multiplexing_journal::{ConversationId, SubmissionState};
use serde_json::json;
use std::sync::{Arc, Barrier};
use std::time::Instant;

#[test]
fn deterministic_topology_matrix_keeps_instance_selection_and_private_identity_separate() {
    assert_eq!(order(seed(), 16), order(seed(), 16));
    for (rel_count, phy_count, same_principal) in
        [(1, 1, true), (1, 2, false), (2, 1, true), (2, 2, false)]
    {
        let f = Fixture::new("topology", rel_count, phy_count);
        let first = attach(&f.g, &f.rels, Some(&f.phys[0]), "alice", true);
        let second_phy = if same_principal {
            &f.phys[0]
        } else {
            &f.phys[1]
        };
        let second_rels = if same_principal {
            f.rels.clone()
        } else {
            vec![f.rels[rel_count - 1].clone()]
        };
        let second = attach(
            &f.g,
            &second_rels,
            Some(second_phy),
            if same_principal { "alice" } else { "bob" },
            true,
        );
        let rel = &f.rels[0];
        let owner_before = Arc::clone(&f.g.rel_executions.read().unwrap()[rel].owner);
        let journal_before = f.g.conversation.journal(rel).unwrap();
        f.g.select_managed_conversation(&first, rel, "a", "main")
            .unwrap();
        f.g.select_managed_conversation(&second, &second_rels[0], "b", "main")
            .unwrap();
        assert_eq!(
            f.g.instance_managed_state(&first, rel)
                .unwrap()
                .conversation_id
                .as_deref(),
            Some("a")
        );
        assert_eq!(
            f.g.instance_managed_state(&second, &second_rels[0])
                .unwrap()
                .conversation_id
                .as_deref(),
            Some("b")
        );
        assert_eq!(f.g.mounted_phy_ids().unwrap().len(), phy_count);
        assert!(Arc::ptr_eq(
            &owner_before,
            &f.g.rel_executions.read().unwrap()[rel].owner
        ));
        assert!(Arc::ptr_eq(
            &journal_before,
            &f.g.conversation.journal(rel).unwrap()
        ));
        for phy in &f.phys {
            assert_eq!(
                f.g.phy_executions.read().unwrap()[phy]
                    .runtime
                    .lock()
                    .unwrap()
                    .worker_count(),
                3
            );
        }
        if !same_principal {
            assert!(
                f.g.authorize(&first, second_phy, OwnerGrant::UseOwnPhy)
                    .is_err()
            );
            if rel_count > 1 {
                assert!(f.g.authorize(&second, rel, OwnerGrant::ViewRel).is_err());
                assert!(f.g.dependency_closure_for(&second, rel).is_err());
            }
        }
        assert!(
            f.g.authorize(
                &first,
                "phy-00000000-0000-4000-8000-000000000999",
                OwnerGrant::UseOwnPhy
            )
            .is_err()
        );
        let viewer = attach(&f.g, &[rel.clone()], None, "viewer", false);
        let viewing = f.g.attach_conversation_viewer(&viewer, rel, "a").unwrap();
        assert!(
            f.g.submit_conversation(&viewer, rel, request(&f.g, rel, "a", 999, "root-a"))
                .is_err()
        );
        f.g.detach_conversation_viewer(viewing).unwrap();
        f.g.detach_instance(&viewer).unwrap();
        f.g.detach_instance(&second).unwrap();
        assert_eq!(f.g.mounted_phy_ids().unwrap().len(), phy_count);
        metric(
            "topology",
            json!({"rels":rel_count,"phys":phy_count,"same_principal":same_principal,"phy_workers_observed":3,"fixture":f.root}),
        );
    }
}

#[test]
fn one_held_owner_does_not_serialize_unrelated_rel_or_phy_and_reopen_rejects_old_tokens() {
    let f = Fixture::new("isolation", 2, 1);
    let instance = attach(&f.g, &f.rels, Some(&f.phys[0]), "alice", true);
    let token =
        f.g.authorize(&instance, &f.rels[0], OwnerGrant::ViewRel)
            .unwrap();
    let (entered_tx, entered_rx) = std::sync::mpsc::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let g = f.g.clone();
    let held = token.clone();
    let worker = std::thread::spawn(move || {
        g.with_rel(&held, |_| {
            entered_tx.send(()).unwrap();
            release_rx
                .recv_timeout(std::time::Duration::from_secs(10))
                .unwrap();
            Ok(())
        })
    });
    entered_rx
        .recv_timeout(std::time::Duration::from_secs(10))
        .unwrap();
    let start = Instant::now();
    f.g.conversation_summaries_for(&instance, &f.rels[1])
        .unwrap();
    f.g.phylactery_profile(&instance).unwrap();
    let unrelated_ns = start.elapsed().as_nanos() as u64;
    release_tx.send(()).unwrap();
    worker.join().unwrap().unwrap();
    f.g.quiesce_rel(&f.rels[0]).unwrap();
    assert!(f.g.with_rel(&token, |_| Ok(())).is_err());
    assert!(
        f.g.rel_executions.read().unwrap()[&f.rels[0]]
            .owner
            .lock()
            .unwrap()
            .workers_stopped()
    );
    assert!(
        !f.g.rel_executions.read().unwrap()[&f.rels[1]]
            .owner
            .lock()
            .unwrap()
            .workers_stopped()
    );
    f.g.conversation_summaries_for(&instance, &f.rels[1])
        .unwrap();
    f.g.phylactery_profile(&instance).unwrap();
    f.g.reopen_rel(&f.rels[0]).unwrap();
    assert!(f.g.with_rel(&token, |_| Ok(())).is_err());
    metric(
        "independent_owner",
        json!({"unrelated_read_pair_ns":unrelated_ns}),
    );
}

#[test]
fn seeded_pending_order_ignores_client_clock_and_survives_zero_viewer_restart() {
    let f = Fixture::new("queue", 1, 0);
    let owner = &f.rels[0];
    let writer = attach(&f.g, &[owner.clone()], None, "alice", true);
    let admissions = order(seed(), 12);
    let mut keys = Vec::new();
    let mut admitted_at = std::collections::BTreeMap::new();
    for index in admissions {
        let req = request(&f.g, owner, "a", index, "root-a");
        let accepted =
            f.g.submit_conversation(&writer, owner, req.clone())
                .unwrap();
        assert_eq!(
            f.g.submit_conversation(&writer, owner, req).unwrap().id,
            accepted.id
        );
        admitted_at.insert(
            accepted.id.submission_id.as_str().to_owned(),
            Instant::now(),
        );
        keys.push(accepted.id);
    }
    assert_eq!(
        f.g.conversation
            .journal(owner)
            .unwrap()
            .lock()
            .unwrap()
            .pending_order(&ConversationId::new("a").unwrap()),
        keys
    );
    f.g.detach_instance(&writer).unwrap();
    let cva = f.g.unmount_rel(owner).unwrap();
    cva.sync().unwrap();
    drop(cva);
    assert_eq!(
        f.g.mount_rel(crate::InteractionRuntime::new(
            crate::Cva::open(f.root.join("0.rel")).unwrap()
        ))
        .unwrap(),
        *owner
    );
    let begin = Instant::now();
    let mut last = "root-a".to_owned();
    let mut admission_to_commit = Vec::new();
    for expected in keys {
        let result =
            f.g.process_next_accepted_conversation_submission(owner, "a")
                .unwrap()
                .unwrap();
        assert_eq!(result.id, expected);
        assert_eq!(result.status.state, SubmissionState::Committed);
        assert_eq!(
            result
                .status
                .archive_receipt
                .as_ref()
                .unwrap()
                .parent_node_id
                .as_deref(),
            Some(last.as_str())
        );
        admission_to_commit.push(
            admitted_at[expected.submission_id.as_str()]
                .elapsed()
                .as_nanos() as u64,
        );
        last = expected.submission_id.as_str().into();
    }
    assert!(
        f.g.process_next_accepted_conversation_submission(owner, "a")
            .unwrap()
            .is_none()
    );
    metric(
        "accepted_queue",
        json!({"operations":12,"drain_ns":begin.elapsed().as_nanos() as u64,"admission_receipt_to_commit_ns":admission_to_commit,"latency_scope":"includes queued wait, detach, physical restart and commit"}),
    );
}

#[test]
fn cancel_finalize_races_have_one_winner_and_disk_replay_keeps_archive_version() {
    for index in order(seed(), 8) {
        let f = Fixture::new("race", 1, 0);
        let owner = &f.rels[0];
        let writer = attach(&f.g, &[owner.clone()], None, "alice", true);
        let admission =
            f.g.submit_conversation(&writer, owner, request(&f.g, owner, "a", index, "root-a"))
                .unwrap();
        let barrier = Arc::new(Barrier::new(3));
        let process = {
            let g = f.g.clone();
            let owner = owner.clone();
            let w = writer.clone();
            let b = barrier.clone();
            std::thread::spawn(move || {
                b.wait();
                g.process_next_conversation_submission(&w, &owner, "a")
            })
        };
        let cancel = {
            let g = f.g.clone();
            let owner = owner.clone();
            let w = writer.clone();
            let b = barrier.clone();
            std::thread::spawn(move || {
                b.wait();
                g.cancel_conversation_submission(&w, &owner, "a", &id(index))
            })
        };
        barrier.wait();
        let processed = process.join().unwrap();
        let cancelled = cancel.join().unwrap();
        assert!(processed.is_ok() || cancelled.is_ok());
        let state =
            f.g.conversation
                .journal(owner)
                .unwrap()
                .lock()
                .unwrap()
                .status(&admission.id)
                .unwrap()
                .state;
        assert!(matches!(
            state,
            SubmissionState::Committed | SubmissionState::Cancelled
        ));
        f.g.detach_instance(&writer).unwrap();
        let cva = f.g.unmount_rel(owner).unwrap();
        assert_eq!(
            cva.archive().has_node("a", &id(index)),
            state == SubmissionState::Committed
        );
        let version = cva.archive_version();
        cva.sync().unwrap();
        drop(cva);
        f.g.mount_rel(crate::InteractionRuntime::new(
            crate::Cva::open(f.root.join("0.rel")).unwrap(),
        ))
        .unwrap();
        assert!(
            f.g.process_next_accepted_conversation_submission(owner, "a")
                .unwrap()
                .is_none()
        );
        let cva = f.g.unmount_rel(owner).unwrap();
        assert_eq!(cva.archive_version(), version);
        assert_eq!(
            cva.archive().has_node("a", &id(index)),
            state == SubmissionState::Committed
        );
        drop(cva);
    }
}
