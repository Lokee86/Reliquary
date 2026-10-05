use super::super::{OwnerAdmission, events::RelSequencer, observation::*};
use super::*;
use crate::runtime_host_test_support::test_path;
use crate::{Cva, InteractionRuntime};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Mutex;

struct Provider(Mutex<StreamSourceSnapshot>);
impl ConversationStreamProvider for Provider {
    fn snapshot(&self, _: &StreamKey) -> Result<StreamSourceSnapshot, ReliquaryRuntimeHostError> {
        Ok(self.0.lock().unwrap().clone())
    }
}
fn identity(key: &StreamKey) -> GenerationIdentity {
    GenerationIdentity {
        operation_id: key.operation_id.clone(),
        generation_id: key.generation_id.clone(),
        branch_id: key.branch_id.clone(),
    }
}
fn generation(
    key: &StreamKey,
    sequence: Option<u64>,
    revision: u64,
    status: GenerationStatus,
) -> GenerationObservation {
    GenerationObservation {
        identity: identity(key),
        status,
        transition_revision: revision,
        durable_checkpoint_sequence: sequence,
    }
}
fn content(
    key: &StreamKey,
    sequence: u64,
    after: Option<u64>,
    text: &str,
    status: GenerationStatus,
) -> GenerationCheckpointContent {
    let utf8_bytes = text.len() as u64;
    let sha256 = Sha256::digest(text.as_bytes()).into();
    GenerationCheckpointContent {
        observation: GenerationCheckpointObservation {
            rel_id: key.rel_id,
            mount_epoch: key.mount_epoch,
            conversation_id: key.conversation_id.clone(),
            generation: identity(key),
            durable_checkpoint_sequence: sequence,
            status,
            content: match after {
                Some(after_checkpoint_sequence) => CheckpointContentIdentity::Delta {
                    after_checkpoint_sequence,
                    utf8_bytes,
                    sha256,
                },
                None => CheckpointContentIdentity::Prefix { utf8_bytes, sha256 },
            },
        },
        text: text.into(),
    }
}
fn source(
    key: &StreamKey,
    sequence: Option<u64>,
    revision: u64,
    text: &str,
    status: GenerationStatus,
) -> StreamSourceSnapshot {
    StreamSourceSnapshot {
        conversation: ConversationObservationSnapshot {
            rel_id: key.rel_id,
            mount_epoch: key.mount_epoch,
            conversation_id: key.conversation_id.clone(),
            selected_branch_id: None,
            selected_head_node_id: None,
            policy: ConversationPolicy::SequentialChronological,
            policy_revision: 1,
            branches: vec![BranchObservation {
                branch_id: key.branch_id.clone(),
                head_node_id: None,
                fork_parent_node_id: None,
                display_label: None,
            }],
            submissions: vec![],
            generations: vec![generation(key, sequence, revision, status)],
            host_operations: vec![],
            durable_journal_sequence: 500 + revision,
        },
        checkpoint: sequence.map(|n| content(key, n, None, text, status)),
    }
}
fn setup(
    label: &str,
) -> (
    GatewayRuntime,
    Arc<InstanceRuntime>,
    StreamKey,
    Arc<RelSequencer>,
    Provider,
) {
    let gateway = GatewayRuntime::new(
        test_path(&format!("{label}-state")),
        Default::default(),
        crate::InsomniaWorkerConfig {
            workers: 0,
            ..Default::default()
        },
        Default::default(),
    );
    let owner = gateway
        .mount_rel(InteractionRuntime::new(
            Cva::create_project(test_path(&format!("{label}.rel"))).unwrap(),
        ))
        .unwrap();
    let instance = gateway
        .attach_instance(
            label.into(),
            None,
            BTreeMap::from([(
                owner.clone(),
                BTreeSet::from([OwnerGrant::ViewRel, OwnerGrant::ManageRel]),
            )]),
        )
        .unwrap();
    let token = gateway
        .authorize(&instance, &owner, OwnerGrant::ViewRel)
        .unwrap();
    let seq = Arc::clone(
        &gateway
            .rel_executions
            .read()
            .unwrap()
            .get(&owner)
            .unwrap()
            .sequencer,
    );
    let key = StreamKey {
        rel_id: RelOwnerId::parse_canonical(&owner).unwrap(),
        mount_epoch: token.mount_epoch,
        conversation_id: ConversationId::new("conversation-a").unwrap(),
        operation_id: "operation-a".into(),
        generation_id: "generation-a".into(),
        branch_id: "branch-a".into(),
    };
    let provider = Provider(Mutex::new(source(
        &key,
        Some(7),
        20,
        "durable prefix",
        GenerationStatus::Running,
    )));
    (gateway, instance, key, seq, provider)
}
/// Deterministic P3 stand-in: stage text separately, then persist/update the source
/// and publish together under the mounted gate. No production P3 policy is added.
fn commit(
    seq: &RelSequencer,
    key: &StreamKey,
    p: &Provider,
    n: u64,
    revision: u64,
    suffix: &str,
    status: GenerationStatus,
) {
    let mut gate = seq.gate(key.mount_epoch).unwrap();
    let mut src = p.0.lock().unwrap();
    let previous = src.conversation.generations[0]
        .durable_checkpoint_sequence
        .unwrap_or(0);
    let mut prefix = src
        .checkpoint
        .as_ref()
        .map(|c| c.text.clone())
        .unwrap_or_default();
    prefix.push_str(suffix);
    *src = source(key, Some(n), revision, &prefix, status);
    gate.publish_stream(
        key.clone(),
        generation(key, Some(n), revision, status),
        Some(content(key, n, Some(previous), suffix, status)),
    )
    .unwrap();
}
fn event(g: &GatewayRuntime, i: &InstanceRuntime, sub: &StreamSubscription) -> Arc<StreamEvent> {
    match g.next_conversation_stream(i, sub).unwrap().unwrap() {
        StreamDelivery::Event(e) => e,
        other => panic!("expected event: {other:?}"),
    }
}

#[test]
fn late_join_gets_prefix_once_then_only_later_durable_deltas() {
    let (g, i, key, seq, p) = setup("stream-late");
    let (snap, early) = g.join_conversation_stream(&i, key.clone(), 4, &p).unwrap();
    assert_eq!(snap.cursor, 0);
    assert_eq!(snap.source.checkpoint.unwrap().text, "durable prefix");
    assert!(g.next_conversation_stream(&i, &early).unwrap().is_none());
    // Staged provider text is invisible until a checkpoint is synchronized.
    let _staged = "uncommitted token";
    assert!(g.next_conversation_stream(&i, &early).unwrap().is_none());
    commit(&seq, &key, &p, 8, 21, " Δ", GenerationStatus::Running);
    let (snap, late) = g.join_conversation_stream(&i, key.clone(), 4, &p).unwrap();
    assert_eq!(snap.cursor, 1);
    assert_eq!(snap.source.checkpoint.unwrap().text, "durable prefix Δ");
    assert!(g.next_conversation_stream(&i, &late).unwrap().is_none());
    commit(&seq, &key, &p, 9, 22, " fin", GenerationStatus::Completed);
    assert_eq!(event(&g, &i, &early).cursor, 1);
    for sub in [&early, &late] {
        let e = event(&g, &i, sub);
        assert_eq!(e.cursor, 2);
        match &e.payload {
            StreamPayload::Checkpoint(c) => {
                assert_eq!(c.text, " fin");
                assert_eq!(c.observation.status, GenerationStatus::Completed);
                assert_eq!(c.observation.durable_checkpoint_sequence, 9);
            }
            _ => panic!("missing checkpoint"),
        }
        assert!(g.next_conversation_stream(&i, sub).unwrap().is_none());
    }
    assert_eq!(seq.gate(key.mount_epoch).unwrap().cursor(), 0);
}

#[test]
fn exact_scope_separates_conversation_operation_generation_branch_and_rel() {
    let (g, i, key, seq, p) = setup("stream-scopes");
    let (_, original) = g.join_conversation_stream(&i, key.clone(), 2, &p).unwrap();
    let mut others = Vec::new();
    for part in 0..4 {
        let mut other = key.clone();
        match part {
            0 => other.conversation_id = ConversationId::new("conversation-b").unwrap(),
            1 => other.operation_id = "operation-b".into(),
            2 => other.generation_id = "generation-b".into(),
            _ => other.branch_id = "branch-b".into(),
        }
        let provider = Provider(Mutex::new(source(
            &other,
            Some(7),
            20,
            "other",
            GenerationStatus::Running,
        )));
        let (_, sub) = g.join_conversation_stream(&i, other, 2, &provider).unwrap();
        others.push(sub);
    }
    let other_owner = g
        .mount_rel(InteractionRuntime::new(
            Cva::create_project(test_path("stream-other.rel")).unwrap(),
        ))
        .unwrap();
    let mut foreign = key.clone();
    foreign.rel_id = RelOwnerId::parse_canonical(&other_owner).unwrap();
    assert!(g.join_conversation_stream(&i, foreign, 2, &p).is_err());
    commit(&seq, &key, &p, 8, 21, "only a", GenerationStatus::Running);
    assert_eq!(event(&g, &i, &original).key, key);
    for sub in &others {
        assert!(g.next_conversation_stream(&i, sub).unwrap().is_none());
    }
}

#[test]
fn stream_cursors_do_not_consume_rel_cursor_or_archive_watermark() {
    let (g, i, key, seq, p) = setup("stream-clocks");
    let (_, sub) = g.join_conversation_stream(&i, key.clone(), 2, &p).unwrap();
    seq.gate(key.mount_epoch)
        .unwrap()
        .publish(
            super::super::events::RelEventClass::ArchiveChanged,
            DurableWatermarks {
                archive_version: Some(100),
                ..Default::default()
            },
            vec![],
        )
        .unwrap();
    commit(&seq, &key, &p, 8, 21, "x", GenerationStatus::Running);
    assert_eq!(event(&g, &i, &sub).cursor, 1);
    let gate = seq.gate(key.mount_epoch).unwrap();
    assert_eq!(gate.cursor(), 1);
    assert_eq!(gate.watermarks().archive_version, Some(100));
}

#[test]
fn overflow_latches_until_fresh_prefix_replaces_only_that_recipient() {
    let (g, i, key, seq, p) = setup("stream-overflow");
    let (_, slow) = g.join_conversation_stream(&i, key.clone(), 1, &p).unwrap();
    let (_, fast) = g.join_conversation_stream(&i, key.clone(), 4, &p).unwrap();
    for (n, revision) in [(8, 21), (9, 22), (10, 23)] {
        commit(&seq, &key, &p, n, revision, "x", GenerationStatus::Running);
    }
    for _ in 0..2 {
        assert_eq!(
            g.next_conversation_stream(&i, &slow).unwrap(),
            Some(StreamDelivery::ResyncRequired {
                key: key.clone(),
                latest_cursor: 3
            })
        );
    }
    let (snap, replacement) = g.reconnect_conversation_stream(&i, &slow, 2, &p).unwrap();
    assert_eq!(snap.cursor, 3);
    assert_eq!(snap.source.checkpoint.unwrap().text, "durable prefixxxx");
    assert!(g.next_conversation_stream(&i, &slow).is_err());
    for expected in 1..=3 {
        assert_eq!(event(&g, &i, &fast).cursor, expected);
    }
    commit(&seq, &key, &p, 11, 24, "later", GenerationStatus::Running);
    assert_eq!(event(&g, &i, &replacement).cursor, 4);
    g.disconnect_conversation_stream(&i, &replacement).unwrap();
    assert!(g.next_conversation_stream(&i, &replacement).is_err());
    assert_eq!(
        p.0.lock()
            .unwrap()
            .checkpoint
            .as_ref()
            .unwrap()
            .observation
            .durable_checkpoint_sequence,
        11
    );
}

#[test]
fn failed_reader_bounds_and_snapshot_validation_preserve_existing_queue() {
    let (g, i, key, seq, p) = setup("stream-failures");
    let (_, sub) = g.join_conversation_stream(&i, key.clone(), 2, &p).unwrap();
    commit(&seq, &key, &p, 8, 21, "x", GenerationStatus::Running);
    struct Failure;
    impl ConversationStreamProvider for Failure {
        fn snapshot(
            &self,
            _: &StreamKey,
        ) -> Result<StreamSourceSnapshot, ReliquaryRuntimeHostError> {
            Err(operation("injected reader error"))
        }
    }
    assert!(
        g.reconnect_conversation_stream(&i, &sub, 2, &Failure)
            .is_err()
    );
    for capacity in [0, MAX_QUEUE + 1] {
        assert!(
            g.reconnect_conversation_stream(&i, &sub, capacity, &p)
                .is_err()
        );
    }
    let current = p.0.lock().unwrap().clone();
    for variant in 0..9 {
        let mut bad = current.clone();
        match variant {
            0 => bad.conversation.conversation_id = ConversationId::new("wrong").unwrap(),
            1 => bad.conversation.mount_epoch += 1,
            2 => bad.conversation.generations.clear(),
            3 => bad
                .conversation
                .generations
                .push(bad.conversation.generations[0].clone()),
            4 => bad.checkpoint.as_mut().unwrap().text.push_str("tampered"),
            5 => bad.checkpoint = None,
            6 => {
                bad.checkpoint
                    .as_mut()
                    .unwrap()
                    .observation
                    .generation
                    .operation_id = "wrong".into()
            }
            7 => bad.checkpoint = Some(content(&key, 8, Some(7), "x", GenerationStatus::Running)),
            _ => bad.conversation.generations[0].durable_checkpoint_sequence = Some(9),
        }
        assert!(
            g.reconnect_conversation_stream(&i, &sub, 2, &Provider(Mutex::new(bad)))
                .is_err()
        );
    }
    assert_eq!(event(&g, &i, &sub).cursor, 1);
}

#[test]
fn terminal_and_interrupted_status_are_delivered_without_cancellation_authority() {
    for status in [
        GenerationStatus::Interrupted,
        GenerationStatus::Cancelled,
        GenerationStatus::Completed,
    ] {
        let (g, _manager, key, seq, p) = setup(&format!("stream-status-{status:?}"));
        let owner = format!("rel-{}", key.rel_id.as_uuid());
        let viewer = g
            .attach_instance(
                "view-only".into(),
                None,
                BTreeMap::from([(owner.clone(), BTreeSet::from([OwnerGrant::ViewRel]))]),
            )
            .unwrap();
        let (_, sub) = g
            .join_conversation_stream(&viewer, key.clone(), 2, &p)
            .unwrap();
        assert!(g.authorize(&viewer, &owner, OwnerGrant::ManageRel).is_err());
        assert!(
            g.authorize(&viewer, &owner, OwnerGrant::SubmitConversation)
                .is_err()
        );
        {
            let mut gate = seq.gate(key.mount_epoch).unwrap();
            *p.0.lock().unwrap() = source(&key, Some(7), 21, "durable prefix", status);
            gate.publish_stream(key.clone(), generation(&key, Some(7), 21, status), None)
                .unwrap();
        }
        assert_eq!(
            event(&g, &viewer, &sub).payload,
            StreamPayload::Status(generation(&key, Some(7), 21, status))
        );
        let (snap, _) = g
            .join_conversation_stream(&viewer, key.clone(), 2, &p)
            .unwrap();
        assert_eq!(snap.source.conversation.generations[0].status, status);
        assert_eq!(snap.source.checkpoint.unwrap().text, "durable prefix");
    }
}

#[test]
fn gap_or_invalid_durable_fact_closes_distribution_instead_of_fake_continuity() {
    for variant in 0..4 {
        let (g, i, key, seq, p) = setup(&format!("stream-gap-{variant}"));
        let (_, sub) = g.join_conversation_stream(&i, key.clone(), 2, &p).unwrap();
        let mut gate = seq.gate(key.mount_epoch).unwrap();
        let result = match variant {
            0 => gate.publish_stream(
                key.clone(),
                generation(&key, Some(9), 21, GenerationStatus::Running),
                Some(content(&key, 9, Some(8), "gap", GenerationStatus::Running)),
            ),
            1 => gate.publish_stream(
                key.clone(),
                generation(&key, Some(7), 20, GenerationStatus::Running),
                Some(content(
                    &key,
                    7,
                    None,
                    "duplicate",
                    GenerationStatus::Running,
                )),
            ),
            2 => gate.publish_stream(
                key.clone(),
                generation(&key, Some(8), 21, GenerationStatus::Running),
                None,
            ),
            _ => {
                let mut bad = content(&key, 8, Some(7), "bad hash", GenerationStatus::Running);
                bad.observation.content = CheckpointContentIdentity::Delta {
                    after_checkpoint_sequence: 7,
                    utf8_bytes: 8,
                    sha256: [0; 32],
                };
                gate.publish_stream(
                    key.clone(),
                    generation(&key, Some(8), 21, GenerationStatus::Running),
                    Some(bad),
                )
            }
        };
        assert!(result.is_err());
        drop(gate);
        assert!(seq.gate(key.mount_epoch).is_err());
        assert!(g.next_conversation_stream(&i, &sub).is_err());
    }
}

#[test]
fn snapshot_provider_must_share_publication_cut_and_never_invent_current_content() {
    let (g, i, key, seq, p) = setup("stream-provider-cut");
    let (_, sub) = g.join_conversation_stream(&i, key.clone(), 2, &p).unwrap();
    *p.0.lock().unwrap() = source(&key, Some(8), 21, "unsignalled", GenerationStatus::Running);
    assert!(g.join_conversation_stream(&i, key.clone(), 2, &p).is_err());
    assert!(g.reconnect_conversation_stream(&i, &sub, 2, &p).is_err());
    assert_eq!(seq.gate(key.mount_epoch).unwrap().cursor(), 0);
}

#[path = "runtime_host_gateway_streams_recovery_tests.rs"]
mod recovery;
