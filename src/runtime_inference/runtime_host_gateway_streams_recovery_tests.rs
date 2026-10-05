use super::*;

#[test]
fn join_racing_durable_checkpoint_gets_snapshot_or_later_delta_once() {
    let (g, i, key, seq, p) = setup("stream-join-race");
    struct Paused<'a> {
        p: &'a Provider,
        entered: std::sync::mpsc::SyncSender<()>,
        resume: Mutex<std::sync::mpsc::Receiver<()>>,
    }
    impl ConversationStreamProvider for Paused<'_> {
        fn snapshot(
            &self,
            key: &StreamKey,
        ) -> Result<StreamSourceSnapshot, ReliquaryRuntimeHostError> {
            let snap = self.p.snapshot(key)?;
            self.entered.send(()).unwrap();
            self.resume.lock().unwrap().recv().unwrap();
            Ok(snap)
        }
    }
    let (entered_tx, entered_rx) = std::sync::mpsc::sync_channel(1);
    let (resume_tx, resume_rx) = std::sync::mpsc::sync_channel(1);
    let paused = Paused {
        p: &p,
        entered: entered_tx,
        resume: Mutex::new(resume_rx),
    };
    std::thread::scope(|scope| {
        let join = scope.spawn(|| g.join_conversation_stream(&i, key.clone(), 2, &paused));
        entered_rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap();
        let writer = scope.spawn(|| {
            commit(
                &seq,
                &key,
                &p,
                8,
                21,
                "after cut",
                GenerationStatus::Running,
            )
        });
        resume_tx.send(()).unwrap();
        let (snap, sub) = join.join().unwrap().unwrap();
        writer.join().unwrap();
        assert_eq!(snap.cursor, 0);
        assert_eq!(snap.source.checkpoint.unwrap().text, "durable prefix");
        assert_eq!(event(&g, &i, &sub).cursor, 1);
        assert!(g.next_conversation_stream(&i, &sub).unwrap().is_none());
    });
}

#[test]
fn revocation_during_snapshot_prevents_registration_and_later_text_delivery() {
    let (g, i, key, _seq, p) = setup("stream-revoke-race");
    struct Paused<'a> {
        p: &'a Provider,
        entered: std::sync::mpsc::SyncSender<()>,
        resume: Mutex<std::sync::mpsc::Receiver<()>>,
    }
    impl ConversationStreamProvider for Paused<'_> {
        fn snapshot(
            &self,
            key: &StreamKey,
        ) -> Result<StreamSourceSnapshot, ReliquaryRuntimeHostError> {
            let snap = self.p.snapshot(key)?;
            self.entered.send(()).unwrap();
            self.resume.lock().unwrap().recv().unwrap();
            Ok(snap)
        }
    }
    let (entered_tx, entered_rx) = std::sync::mpsc::sync_channel(1);
    let (resume_tx, resume_rx) = std::sync::mpsc::sync_channel(1);
    let paused = Paused {
        p: &p,
        entered: entered_tx,
        resume: Mutex::new(resume_rx),
    };
    let entry = g.attachment_by_id(i.instance_id()).unwrap();
    std::thread::scope(|scope| {
        let join = scope.spawn(|| g.join_conversation_stream(&i, key.clone(), 2, &paused));
        entered_rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap();
        let revoke = scope.spawn(|| g.replace_instance_grants(&i, BTreeMap::new()));
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while entry.leases.snapshot().unwrap().0 == OwnerAdmission::Open {
            assert!(std::time::Instant::now() < deadline);
            std::thread::yield_now();
        }
        resume_tx.send(()).unwrap();
        assert!(join.join().unwrap().is_err());
        revoke.join().unwrap().unwrap();
    });
    assert!(g.join_conversation_stream(&i, key, 2, &p).is_err());
}

#[test]
fn refresh_detach_reopen_unmount_shutdown_fence_retained_stream_handles() {
    for mode in 0..5 {
        let (g, i, key, seq, p) = setup(&format!("stream-lifecycle-{mode}"));
        let (_, sub) = g.join_conversation_stream(&i, key.clone(), 2, &p).unwrap();
        commit(
            &seq,
            &key,
            &p,
            8,
            21,
            "queued secret",
            GenerationStatus::Running,
        );
        let owner = format!("rel-{}", key.rel_id.as_uuid());
        match mode {
            0 => g
                .replace_instance_grants(
                    &i,
                    BTreeMap::from([(
                        owner.clone(),
                        BTreeSet::from([OwnerGrant::ViewRel, OwnerGrant::ManageRel]),
                    )]),
                )
                .unwrap(),
            1 => {
                g.detach_instance(&i).unwrap();
            }
            2 => {
                g.quiesce_rel(&owner).unwrap();
                g.reopen_rel(&owner).unwrap();
            }
            3 => {
                drop(g.unmount_rel(&owner).unwrap());
            }
            _ => g.shutdown().unwrap(),
        }
        assert!(g.next_conversation_stream(&i, &sub).is_err());
        assert!(g.reconnect_conversation_stream(&i, &sub, 2, &p).is_err());
        if mode == 0 {
            let (_, fresh) = g.join_conversation_stream(&i, key.clone(), 2, &p).unwrap();
            assert!(g.next_conversation_stream(&i, &fresh).unwrap().is_none());
        }
        if mode == 2 {
            let token = g.authorize(&i, &owner, OwnerGrant::ViewRel).unwrap();
            let mut fresh_key = key.clone();
            fresh_key.mount_epoch = token.mount_epoch;
            let provider = Provider(Mutex::new(source(
                &fresh_key,
                Some(8),
                21,
                "durable prefixqueued secret",
                GenerationStatus::Interrupted,
            )));
            let (snap, _) = g
                .join_conversation_stream(&i, fresh_key, 2, &provider)
                .unwrap();
            assert_eq!(snap.cursor, 0);
            assert_eq!(
                snap.source.conversation.generations[0].status,
                GenerationStatus::Interrupted
            );
        }
    }
}

#[test]
fn recipient_limit_reconnect_reuses_slot_and_disconnect_frees_only_delivery() {
    let (g, i, key, seq, p) = setup("stream-registry-limit");
    let mut subs = Vec::new();
    for _ in 0..MAX_RECIPIENTS {
        subs.push(
            g.join_conversation_stream(&i, key.clone(), 1, &p)
                .unwrap()
                .1,
        );
    }
    assert!(g.join_conversation_stream(&i, key.clone(), 1, &p).is_err());
    assert!(
        g.reconnect_conversation_stream(&i, &subs[0], 0, &p)
            .is_err()
    );
    let (_, replacement) = g
        .reconnect_conversation_stream(&i, &subs[0], 1, &p)
        .unwrap();
    assert!(g.next_conversation_stream(&i, &subs[0]).is_err());
    commit(
        &seq,
        &key,
        &p,
        8,
        21,
        "persisted",
        GenerationStatus::Running,
    );
    assert_eq!(event(&g, &i, &replacement).cursor, 1);
    assert_eq!(event(&g, &i, &subs[1]).cursor, 1);
    g.disconnect_conversation_stream(&i, &replacement).unwrap();
    assert!(g.join_conversation_stream(&i, key, 1, &p).is_ok());
}

#[test]
fn stream_identity_and_text_bounds_are_enforced_before_delivery() {
    let (g, i, key, seq, p) = setup("stream-bounds");
    for n in 0..MAX_STREAMS {
        let mut scoped = key.clone();
        scoped.generation_id = format!("generation-{n}");
        let provider = Provider(Mutex::new(source(
            &scoped,
            Some(7),
            20,
            "prefix",
            GenerationStatus::Running,
        )));
        let (_, sub) = g
            .join_conversation_stream(&i, scoped, 1, &provider)
            .unwrap();
        g.disconnect_conversation_stream(&i, &sub).unwrap();
    }
    assert!(g.join_conversation_stream(&i, key.clone(), 1, &p).is_err());
    // Independent hub for boundary-length/digest checks.
    let (g, i, key, _seq, _) = setup("stream-text-bounds");
    let p = Provider(Mutex::new(source(
        &key,
        Some(7),
        20,
        &"x".repeat(MAX_TEXT_BYTES),
        GenerationStatus::Running,
    )));
    assert!(g.join_conversation_stream(&i, key.clone(), 1, &p).is_ok());
    let oversized = Provider(Mutex::new(source(
        &key,
        Some(7),
        20,
        &"x".repeat(MAX_TEXT_BYTES + 1),
        GenerationStatus::Running,
    )));
    assert!(
        g.join_conversation_stream(&i, key.clone(), 1, &oversized)
            .is_err()
    );
    let mut invalid = key;
    invalid.operation_id = "x".repeat(513);
    assert!(g.join_conversation_stream(&i, invalid, 1, &p).is_err());
    assert_eq!(seq.current_gate().unwrap().cursor(), 0);
}

#[test]
fn pending_generation_without_checkpoint_has_no_invented_text_prefix() {
    let (g, i, key, seq, _) = setup("stream-pending");
    let p = Provider(Mutex::new(source(
        &key,
        None,
        0,
        "",
        GenerationStatus::Pending,
    )));
    let (snap, sub) = g.join_conversation_stream(&i, key.clone(), 2, &p).unwrap();
    assert!(snap.source.checkpoint.is_none());
    commit(
        &seq,
        &key,
        &p,
        1,
        1,
        "first persisted text",
        GenerationStatus::Running,
    );
    assert_eq!(event(&g, &i, &sub).cursor, 1);
}

#[test]
fn byte_pressure_overflows_before_item_capacity_and_snapshot_recovers_full_prefix() {
    let (g, i, key, seq, p) = setup("stream-byte-limit");
    let (_, sub) = g
        .join_conversation_stream(&i, key.clone(), MAX_QUEUE, &p)
        .unwrap();
    for (n, revision) in [(8, 21), (9, 22)] {
        let mut gate = seq.gate(key.mount_epoch).unwrap();
        let text = "x".repeat(700_000);
        *p.0.lock().unwrap() = source(&key, Some(n), revision, &text, GenerationStatus::Running);
        gate.publish_stream(
            key.clone(),
            generation(&key, Some(n), revision, GenerationStatus::Running),
            Some(content(&key, n, None, &text, GenerationStatus::Running)),
        )
        .unwrap();
    }
    assert_eq!(
        g.next_conversation_stream(&i, &sub).unwrap(),
        Some(StreamDelivery::ResyncRequired {
            key: key.clone(),
            latest_cursor: 2
        })
    );
    let (snap, fresh) = g
        .reconnect_conversation_stream(&i, &sub, MAX_QUEUE, &p)
        .unwrap();
    assert_eq!(snap.source.checkpoint.unwrap().text.len(), 700_000);
    commit(&seq, &key, &p, 10, 23, "later", GenerationStatus::Running);
    assert_eq!(event(&g, &i, &fresh).cursor, 3);
}
