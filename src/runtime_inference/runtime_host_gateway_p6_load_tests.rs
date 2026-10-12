use super::super::super::streams::{StreamDelivery, StreamKey, StreamPayload};
use super::super::super::subscriptions::RelDelivery;
use super::fixture::*;
use super::*;
use crate::multiplexing_journal::{ConversationId, RelOwnerId};
use serde_json::json;
use std::time::Instant;

#[test]
fn increasing_fanout_bounds_slow_recipients_and_preserves_fast_delivery() {
    let counts = std::env::var("RELIQUARY_P6_INSTANCES").unwrap_or_else(|_| "1,8,32".into());
    for count in counts.split(',').map(|n| n.parse::<usize>().unwrap()) {
        assert!((1..=256).contains(&count));
        let f = Fixture::new("fanout", 1, 0);
        let owner = &f.rels[0];
        let mut recipients = Vec::new();
        let mut delivered_cursor = 0;
        let start = Instant::now();
        for n in 0..count {
            let i = attach(&f.g, &[owner.clone()], None, &format!("viewer-{n}"), true);
            let (snapshot, sub) = f.g.subscribe_rel(&i, owner, None, 8).unwrap();
            if n == 0 {
                delivered_cursor = snapshot.watermark.cursor;
            }
            recipients.push((i, sub));
        }
        let startup_ns = start.elapsed().as_nanos() as u64;
        let (writer, _) = &recipients[0];
        let mut event_count = 0;
        let start = Instant::now();
        for n in 0..24 {
            f.g.create_knowledge_memory_for(
                writer,
                owner,
                format!("m-{n}"),
                "P6_MEMORY_PRIVATE_BODY".into(),
                "knowledge".into(),
                "fact".into(),
                n as i64 + 1,
            )
            .unwrap();
            while let Some(delivery) = f.g.next_rel_event(writer, &recipients[0].1).unwrap() {
                match delivery {
                    RelDelivery::Event(event) => {
                        assert!(!format!("{event:?}").contains("P6_MEMORY_PRIVATE_BODY"));
                        assert!(!format!("{event:?}").contains("P6_SOURCE_SECRET"));
                        assert_eq!(
                            event.cursor,
                            delivered_cursor + 1,
                            "fast recipient missed a publication"
                        );
                        delivered_cursor = event.cursor;
                        event_count += 1;
                    }
                    other => panic!("fast recipient overflowed: {other:?}"),
                }
            }
        }
        let publish_ns = start.elapsed().as_nanos() as u64;
        assert!(
            event_count >= 24,
            "each synchronized mutation must reach the fast recipient"
        );
        for (i, sub) in recipients.iter().skip(1) {
            assert!(matches!(
                f.g.next_rel_event(i, sub).unwrap(),
                Some(RelDelivery::ResyncRequired { .. })
            ));
            let snapshot = f.g.resync_rel(i, sub).unwrap();
            f.g.acknowledge_rel_resync(i, sub, snapshot.watermark)
                .unwrap();
            assert!(f.g.next_rel_event(i, sub).unwrap().is_none());
        }
        if let Some((i, sub)) = recipients.get(1) {
            f.g.create_knowledge_memory_for(
                writer,
                owner,
                "queued".into(),
                "SECRET".into(),
                "knowledge".into(),
                "fact".into(),
                100,
            )
            .unwrap();
            f.g.replace_instance_grants(i, std::collections::BTreeMap::new())
                .unwrap();
            assert!(f.g.next_rel_event(i, sub).is_err());
        }
        let (snapshot, old) = f.g.subscribe_rel(writer, owner, None, 8).unwrap();
        f.g.quiesce_rel(owner).unwrap();
        f.g.reopen_rel(owner).unwrap();
        assert!(f.g.next_rel_event(writer, &old).is_err());
        assert!(
            f.g.subscribe_rel(writer, owner, Some(snapshot.watermark), 8)
                .is_err()
        );
        f.g.subscribe_rel(writer, owner, None, 8).unwrap();
        metric(
            "fanout",
            json!({"instances":count,"capacity":8,"mutations":24,"fast_events":event_count,"slow_resyncs":count-1,"registration_ns":startup_ns,"sync_mutation_and_fanout_ns":publish_ns}),
        );
    }
}

#[test]
fn owner_lock_probe_measures_wait_and_hold_without_cross_owner_nesting() {
    let f = Fixture::new("lock-probe", 2, 0);
    let instance = attach(&f.g, &f.rels, None, "probe", true);
    let runtime = f.g.rel_executions.read().unwrap()[&f.rels[0]]
        .owner
        .lock()
        .unwrap()
        .runtime
        .as_ref()
        .unwrap()
        .clone();
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
    let b = barrier.clone();
    let g = f.g.clone();
    let owner = f.rels[0].clone();
    let i = instance.clone();
    let contender = std::thread::spawn(move || {
        b.wait();
        for _ in 0..100 {
            g.conversation_summaries_for(&i, &owner).unwrap();
        }
    });
    barrier.wait();
    let mut wait = Vec::new();
    let mut hold = Vec::new();
    for _ in 0..100 {
        let start = Instant::now();
        let guard = runtime.lock().unwrap();
        wait.push(start.elapsed().as_nanos() as u64);
        let start = Instant::now();
        std::hint::black_box(guard.cva().archive_version());
        drop(guard);
        hold.push(start.elapsed().as_nanos() as u64);
    }
    contender.join().unwrap();
    wait.sort();
    hold.sort();
    metric(
        "owner_lock_probe",
        json!({"samples":100,"wait_ns":wait,"probe_hold_ns":hold,"scope":"one direct owner read versus 100 public summary reads; not instrumentation of all production critical sections"}),
    );
}

#[test]
fn checkpoint_throughput_late_join_and_restart_preserve_exact_prefix_once() {
    let f = Fixture::new("checkpoint-load", 1, 0);
    let owner = &f.rels[0];
    let writer = attach(&f.g, &[owner.clone()], None, "alice", true);
    let accepted =
        f.g.submit_conversation(&writer, owner, request(&f.g, owner, "a", 200, "root-a"))
            .unwrap();
    f.g.process_next_accepted_conversation_submission(owner, "a")
        .unwrap()
        .unwrap();
    let generation =
        f.g.start_conversation_generation(owner, "a", accepted.id.submission_id.as_str())
            .unwrap()
            .status
            .generation
            .unwrap();
    let key = StreamKey {
        rel_id: RelOwnerId::parse_canonical(owner).unwrap(),
        mount_epoch: f
            .g
            .authorize(&writer, owner, OwnerGrant::ViewRel)
            .unwrap()
            .mount_epoch,
        conversation_id: ConversationId::new("a").unwrap(),
        operation_id: generation.operation_id.clone(),
        generation_id: generation.generation_id.clone(),
        branch_id: generation.branch_id.clone(),
    };
    let (_, early) =
        f.g.subscribe_authoritative_conversation_stream(&writer, key.clone(), 32)
            .unwrap();
    let mut text = String::new();
    let start = Instant::now();
    for sequence in 1..=16 {
        text.push_str(&"é".repeat(2048));
        f.g.checkpoint_conversation_generation(
            owner,
            "a",
            &generation.operation_id,
            sequence,
            &text,
        )
        .unwrap();
    }
    let checkpoint_ns = start.elapsed().as_nanos() as u64;
    let mut delivered = 0;
    let mut delivered_bytes = 0;
    while let Some(d) = f.g.next_conversation_stream(&writer, &early).unwrap() {
        if let StreamDelivery::Event(event) = d {
            if let StreamPayload::Checkpoint(c) = &event.payload {
                delivered += 1;
                delivered_bytes += c.text.len();
            }
        } else {
            panic!("bounded workload unexpectedly overflowed")
        }
    }
    assert_eq!(delivered, 16);
    assert_eq!(delivered_bytes, text.len());
    let start = Instant::now();
    let (snapshot, late) =
        f.g.subscribe_authoritative_conversation_stream(&writer, key, 32)
            .unwrap();
    let replay_ns = start.elapsed().as_nanos() as u64;
    assert_eq!(snapshot.source.checkpoint.unwrap().text, text);
    assert!(
        f.g.next_conversation_stream(&writer, &late)
            .unwrap()
            .is_none()
    );
    let (_, broad) = f.g.subscribe_rel(&writer, owner, None, 8).unwrap();
    let broad_debug = format!(
        "{:?}",
        f.g.subscribe_rel(&writer, owner, None, 8).unwrap().0
    );
    assert!(!broad_debug.contains(&text));
    assert!(f.g.next_rel_event(&writer, &broad).unwrap().is_none());
    f.g.detach_instance(&writer).unwrap();
    let start = Instant::now();
    let cva = f.g.unmount_rel(owner).unwrap();
    cva.sync().unwrap();
    drop(cva);
    f.g.mount_rel(crate::InteractionRuntime::new(
        crate::Cva::open(f.root.join("0.rel")).unwrap(),
    ))
    .unwrap();
    let restart_ns = start.elapsed().as_nanos() as u64;
    let observer = attach(&f.g, &[owner.clone()], None, "late", false);
    let status =
        f.g.conversation_generation_status(&observer, owner, "a", &generation.operation_id)
            .unwrap()
            .status
            .generation
            .unwrap();
    assert_eq!(
        status.phase,
        crate::multiplexing_journal::DurableGenerationPhase::Interrupted
    );
    assert_eq!(status.checkpoint.unwrap().utf8_bytes, text.len() as u64);
    metric(
        "checkpoints",
        json!({"checkpoints":16,"utf8_bytes":text.len(),"delta_delivery_bytes":delivered_bytes,"checkpoint_ns":checkpoint_ns,"late_join_replay_ns":replay_ns,"restart_and_reconcile_ns":restart_ns}),
    );
}

#[test]
fn bounded_topology_has_a_sampled_steady_state_window() {
    let f = Fixture::new("steady", 4, 2);
    let mut instances = Vec::new();
    let mut subscriptions = Vec::new();
    for index in 0..32 {
        let principal = if index % 2 == 0 { "alice" } else { "bob" };
        let i = attach(&f.g, &f.rels, Some(&f.phys[index % 2]), principal, false);
        let (_, sub) = f.g.subscribe_rel(&i, &f.rels[index % 4], None, 16).unwrap();
        subscriptions.push(sub);
        instances.push(i);
    }
    metric(
        "steady_begin",
        json!({"rels":4,"phys":2,"instances":32,"queue_capacity":16}),
    );
    // A measurement window for the external sampler, never a success threshold.
    std::thread::sleep(std::time::Duration::from_millis(250));
    metric(
        "steady_end",
        json!({"mounted_rels":f.g.mounted_rel_ids().unwrap().len(),"mounted_phys":f.g.mounted_phy_ids().unwrap().len(),"recipients":subscriptions.len()}),
    );
    std::hint::black_box(instances);
}
