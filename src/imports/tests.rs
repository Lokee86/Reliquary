use super::*;
use crate::{Cva, EpisodeConfig, EpisodeOrigin, InsomniaPriority};
use std::{collections::BTreeSet, fs, path::PathBuf};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("p5g-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn rel(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn input() -> NativeImport {
    NativeImport {
        schema_version: 1,
        source_namespace: "agent/account-1".into(),
        source_format: "structured-agent-v1".into(),
        conversations: vec![ImportConversation {
            id: "session".into(),
            title: Some("Imported".into()),
            messages: vec![
                message("u", None, "user", "question"),
                message("a", Some("u"), "assistant", "answer"),
            ],
            branches: vec![ImportBranch {
                id: "main".into(),
                leaf_message_id: "a".into(),
                canonical: true,
            }],
        }],
    }
}
fn message(id: &str, parent: Option<&str>, role: &str, content: &str) -> ImportMessage {
    ImportMessage {
        id: id.into(),
        revision: Some("1".into()),
        parent_id: parent.map(str::to_owned),
        replaces: None,
        deleted: false,
        role: role.into(),
        timestamp_ns: 42,
        content: content.into(),
        artifacts: vec![],
    }
}
fn text(cva: &mut Cva, receipt: &ImportReceipt, branch: usize) -> Vec<String> {
    let c = &receipt.conversations[0];
    c.branches[branch]
        .leaf_node_id
        .as_ref()
        .map(|leaf| {
            cva.conversation_turns(&c.conversation_id, leaf)
                .unwrap()
                .into_iter()
                .map(|t| t.content)
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn deterministic_reimport_survives_reopen_and_order_changes() {
    let f = Fixture::new();
    let path = f.rel("a.rel");
    let mut cva = Cva::create(&path).unwrap();
    let mut incoming = input();
    let receipt = cva.import_native(&incoming, 100).unwrap();
    let versions = (
        cva.archive_version(),
        cva.latest_global_version(),
        cva.rel_turn_count(),
    );
    let stats = cva.stats();
    let episodes = cva.episodes();
    incoming.conversations[0].messages.reverse();
    assert_eq!(receipt, cva.import_native(&incoming, 200).unwrap());
    assert_eq!(
        versions,
        (
            cva.archive_version(),
            cva.latest_global_version(),
            cva.rel_turn_count()
        )
    );
    drop(cva);
    let mut cva = Cva::open(path).unwrap();
    assert_eq!(receipt, cva.import_native(&incoming, 300).unwrap());
    assert_eq!(stats, cva.stats());
    assert_eq!(episodes, cva.episodes());
    assert_eq!(
        versions,
        (
            cva.archive_version(),
            cva.latest_global_version(),
            cva.rel_turn_count()
        )
    );
    assert_eq!(text(&mut cva, &receipt, 0), ["question", "answer"]);
    assert_eq!(cva.native_import_receipts(), vec![receipt]);
}

#[test]
fn import_epoch_start_is_durable_and_fences_unrelated_writes() {
    let f = Fixture::new();
    let path = f.rel("epoch-fence.rel");
    let mut cva = Cva::create(&path).unwrap();
    let receipt = cva.import_native(&input(), 100).unwrap();
    let start = cva.open_import_epoch().expect("import start");
    assert_eq!(start.import_key, receipt.import_key);
    assert!(cva.import_epoch_status(&start.import_id).is_some());
    let before = (
        cva.latest_global_version(),
        cva.archive_version(),
        fs::metadata(&path).unwrap().len(),
    );
    assert!(
        cva.append_node(
            "unrelated".into(),
            "live".into(),
            None,
            "user".into(),
            101,
            "must be fenced",
        )
        .is_err()
    );
    assert_eq!(
        before,
        (
            cva.latest_global_version(),
            cva.archive_version(),
            fs::metadata(&path).unwrap().len(),
        )
    );
    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        cva.with_import_write_scope(|_| panic!("scope cleanup probe"));
    }));
    assert!(panic.is_err());
    assert!(
        cva.set_rel_metadata(Some("still-fenced".into()), Vec::new())
            .is_err()
    );
    drop(cva);

    let mut reopened = Cva::open(&path).unwrap();
    assert_eq!(reopened.open_import_epoch(), Some(start));
    assert_eq!(reopened.import_native(&input(), 200).unwrap(), receipt);
    assert!(
        reopened
            .set_rel_metadata(Some("fenced".into()), Vec::new())
            .is_err()
    );
}

#[test]
fn open_epoch_rejects_freshness_policy_change_without_mutating_projection() {
    let f = Fixture::new();
    let path = f.rel("freshness-fence.rel");
    let mut cva = Cva::create(&path).unwrap();
    cva.import_native(&input(), 100).unwrap();
    let original_policy = cva.freshness_policy();
    let before = (
        cva.latest_global_version(),
        cva.archive_version(),
        fs::metadata(&path).unwrap().len(),
    );
    let changed_policy = crate::freshness::FreshnessPolicy {
        decay_turns_per_point: 5,
        access_principal: 30,
        linkage_principal: 60,
        local_hop_cost: 6,
        outside_hop_cost: 12,
    };
    assert!(
        cva.set_freshness_policy_before_enrollment(changed_policy)
            .is_err()
    );
    assert_eq!(cva.freshness_policy(), original_policy);
    assert_eq!(
        before,
        (
            cva.latest_global_version(),
            cva.archive_version(),
            fs::metadata(&path).unwrap().len(),
        )
    );
    drop(cva);

    let reopened = Cva::open(&path).unwrap();
    assert_eq!(reopened.freshness_policy(), original_policy);
    assert!(reopened.open_import_epoch().is_some());
}

#[test]
fn open_epoch_rejects_conflicting_import_input() {
    let f = Fixture::new();
    let mut cva = Cva::create(f.rel("epoch-conflict.rel")).unwrap();
    cva.import_native(&input(), 100).unwrap();
    let mut conflicting = input();
    conflicting.source_namespace = "agent/account-2".into();
    assert!(cva.import_native(&conflicting, 200).is_err());
}
#[test]
fn branches_queue_each_source_node_once() {
    let f = Fixture::new();
    let mut cva = Cva::create(f.rel("a.rel")).unwrap();
    let mut incoming = input();
    incoming.conversations[0]
        .messages
        .push(message("b", Some("u"), "assistant", "other answer"));
    incoming.conversations[0].branches.push(ImportBranch {
        id: "alternate".into(),
        leaf_message_id: "b".into(),
        canonical: false,
    });
    let receipt = cva.import_native(&incoming, 100).unwrap();
    let mut covered = BTreeSet::new();
    for episode in cva.episodes() {
        assert_eq!(episode.origin, EpisodeOrigin::Import);
        assert_eq!(
            cva.insomnia_work(episode.id).unwrap().priority,
            InsomniaPriority::Import
        );
        for turn in cva.episode_turns(episode.id).unwrap() {
            assert!(covered.insert(turn.node_id));
        }
    }
    assert_eq!(covered.len(), 3);
    assert_eq!(receipt.conversations[0].branches.len(), 2);
    // Canonical Archive activity counts unique user and assistant Nodes.
    assert_eq!(cva.rel_turn_count(), 3);
}
#[test]
fn edits_deletions_and_replacements_keep_historical_evidence() {
    let f = Fixture::new();
    let mut cva = Cva::create(f.rel("a.rel")).unwrap();
    let mut incoming = input();
    let original = cva.import_native(&incoming, 100).unwrap();
    incoming.conversations[0].messages[1].content = "edited".into();
    incoming.conversations[0].messages[1].revision = Some("2".into());
    let edited = cva.import_native(&incoming, 200).unwrap();
    assert_eq!(
        original.conversations[0].conversation_id,
        edited.conversations[0].conversation_id
    );
    assert_eq!(text(&mut cva, &original, 0), ["question", "answer"]);
    assert_eq!(text(&mut cva, &edited, 0), ["question", "edited"]);
    incoming.conversations[0].messages[1].deleted = true;
    incoming.conversations[0].messages[1].content.clear();
    incoming.conversations[0].messages[1].revision = Some("3".into());
    let deleted = cva.import_native(&incoming, 300).unwrap();
    assert_eq!(text(&mut cva, &deleted, 0), ["question"]);
    assert!(
        deleted.conversations[0]
            .messages
            .iter()
            .any(|m| m.source_id == "a" && m.deleted && m.node_id.is_none())
    );
    incoming.conversations[0].messages.push(message(
        "replacement",
        Some("u"),
        "assistant",
        "replacement",
    ));
    incoming.conversations[0]
        .messages
        .last_mut()
        .unwrap()
        .replaces = Some("a".into());
    incoming.conversations[0].branches[0].leaf_message_id = "replacement".into();
    let replacement = cva.import_native(&incoming, 400).unwrap();
    assert_eq!(text(&mut cva, &replacement, 0), ["question", "replacement"]);
    assert!(
        replacement.conversations[0]
            .messages
            .iter()
            .any(|m| m.replaces.as_deref() == Some("a"))
    );
    let versions = (cva.archive_version(), cva.latest_global_version());
    assert_eq!(cva.import_native(&input(), 500).unwrap(), original);
    assert_eq!(
        versions,
        (cva.archive_version(), cva.latest_global_version())
    );
    cva.sync().unwrap();
    drop(cva);
    let cva = Cva::open(f.rel("a.rel")).unwrap();
    assert_eq!(cva.native_import_receipts().len(), 4);
}
#[test]
fn new_snapshot_preserves_leased_and_completed_insomnia_work() {
    let f = Fixture::new();
    let path = f.rel("a.rel");
    let mut cva = Cva::create(&path).unwrap();
    let mut incoming = input();
    cva.import_native(&incoming, 100).unwrap();
    let claim = cva
        .claim_insomnia_episode("worker", 110, 1000)
        .unwrap()
        .unwrap();
    incoming.conversations[0].title = Some("Renamed".into());
    cva.import_native(&incoming, 120).unwrap();
    assert_eq!(cva.insomnia_work(claim.episode_id).unwrap(), &claim);
    let complete = cva
        .complete_insomnia_episode(
            claim.episode_id,
            claim.lease_token.unwrap(),
            110,
            130,
            "model".into(),
            "test".into(),
            vec![],
            0,
        )
        .unwrap();
    incoming.conversations[0].title = Some("Renamed again".into());
    cva.import_native(&incoming, 140).unwrap();
    assert_eq!(cva.insomnia_work(claim.episode_id).unwrap(), &complete);
    assert_eq!(cva.episodes().len(), 1);
    assert!(
        cva.claim_insomnia_episode("other", 150, 100)
            .unwrap()
            .is_none()
    );
    drop(cva);
    let mut cva = Cva::open(path).unwrap();
    cva.import_native(&incoming, 160).unwrap();
    assert_eq!(
        cva.insomnia_work(claim.episode_id).unwrap().state,
        crate::InsomniaWorkState::Complete
    );
    assert!(
        cva.claim_insomnia_episode("other", 170, 100)
            .unwrap()
            .is_none()
    );
}
#[test]
fn source_namespaces_and_rel_owners_remain_independent() {
    let f = Fixture::new();
    let mut left = Cva::create(f.rel("left.rel")).unwrap();
    let mut right = Cva::create(f.rel("right.rel")).unwrap();
    let original = left.import_native(&input(), 100).unwrap();
    assert!(right.native_import_receipts().is_empty());
    assert_eq!(right.stats().nodes, 0);
    let mut other = input();
    other.source_namespace = "agent/account-2".into();
    let second = left.import_native(&other, 200).unwrap();
    assert_ne!(
        original.conversations[0].conversation_id,
        second.conversations[0].conversation_id
    );
    assert_eq!(left.stats().nodes, 4);
    assert_eq!(right.import_native(&input(), 300).unwrap(), original);
    assert_eq!(right.native_import_receipts().len(), 1);
    assert_eq!(right.stats().nodes, 2);
    assert_eq!(left.native_import_receipts().len(), 2);
}
#[test]
fn artifact_identity_provenance_and_bytes_survive_reopen() {
    let f = Fixture::new();
    let mut cva = Cva::create(f.rel("a.rel")).unwrap();
    let mut incoming = input();
    incoming.conversations[0].messages[0]
        .artifacts
        .push(ImportArtifact {
            id: "upload-1".into(),
            source_ref: "provider://file-1".into(),
            filename: "source.txt".into(),
            mime_type: Some("text/plain".into()),
            bytes: b"artifact body".to_vec(),
        });
    let receipt = cva.import_native(&incoming, 100).unwrap();
    drop(cva);
    let mut cva = Cva::open(f.rel("a.rel")).unwrap();
    let c = &receipt.conversations[0];
    let m = c.messages.iter().find(|m| m.source_id == "u").unwrap();
    let files = cva.files_for_source(&c.conversation_id, m.node_id.as_ref().unwrap());
    assert_eq!(files.len(), 1);
    assert_eq!(cva.file_bytes(files[0].id).unwrap(), b"artifact body");
    assert_eq!(m.artifacts[0].source_ref, "provider://file-1");
    assert_eq!(
        m.artifacts[0].sha256,
        "9938be87d35f2a7a2b80237e8dc71806b209aaea8252f12c1b12949f61d40476"
    );
}
#[test]
fn malformed_input_and_revision_conflicts_do_not_publish() {
    let f = Fixture::new();
    let mut cva = Cva::create(f.rel("a.rel")).unwrap();
    cva.import_native(&input(), 100).unwrap();
    let versions = (cva.archive_version(), cva.latest_global_version());
    let mut bad = input();
    bad.conversations[0].messages[1].content = "changed same revision".into();
    assert!(cva.import_native(&bad, 200).is_err());
    bad = input();
    bad.conversations[0].messages[0].parent_id = Some("a".into());
    assert!(cva.import_native(&bad, 200).is_err());
    bad = input();
    bad.conversations[0].messages[0].deleted = true;
    assert!(cva.import_native(&bad, 200).is_err());
    bad = input();
    let duplicate = bad.conversations[0].messages[0].clone();
    bad.conversations[0].messages.push(duplicate);
    assert!(cva.import_native(&bad, 200).is_err());
    bad = input();
    bad.schema_version = 2;
    assert!(cva.import_native(&bad, 200).is_err());
    assert_eq!(
        versions,
        (cva.archive_version(), cva.latest_global_version())
    );
}
#[test]
fn interrupted_import_recovers_source_episode_and_queue_cuts() {
    for cut in 0..5 {
        let f = Fixture::new();
        let path = f.rel("a.rel");
        let incoming = input();
        let prepared = prepare(&incoming).unwrap();
        let conversation = &prepared.receipt.conversations[0];
        let mut cva = Cva::create(&path).unwrap();
        if cut >= 1 {
            cva.ingest_turn(prepared.turns[0].clone()).unwrap();
        }
        if cut >= 2 {
            cva.ingest_turn(prepared.turns[1].clone()).unwrap();
            cva.set_conversation_title(
                &conversation.conversation_id,
                conversation.title.clone().unwrap(),
            )
            .unwrap();
            let branch = &conversation.branches[0];
            cva.append_branch(crate::Branch {
                id: branch.branch_id.clone(),
                conversation_id: conversation.conversation_id.clone(),
                leaf_node_id: branch.leaf_node_id.clone().unwrap(),
                canonical: branch.canonical,
            })
            .unwrap();
            cva.materialize_path_episodes(
                &conversation.conversation_id,
                conversation.branches[0].leaf_node_id.as_ref().unwrap(),
                EpisodeConfig::default(),
                EpisodeOrigin::Import,
                Some((crate::EpisodeBoundary::ImportEnd, 100)),
            )
            .unwrap();
        }
        if cut >= 3 {
            for episode in cva.episodes() {
                cva.queue_insomnia_episode(episode.id, InsomniaPriority::Import, 100)
                    .unwrap();
            }
        }
        if cut >= 4 {
            cva.archive
                .put_import_receipt(&mut cva.container, prepared.receipt.clone())
                .unwrap();
        }
        cva.sync().unwrap();
        drop(cva);
        let mut cva = Cva::open(&path).unwrap();
        let receipt = cva.import_native(&incoming, 200).unwrap();
        assert_eq!(receipt, prepared.receipt);
        assert_eq!(cva.stats().nodes, 2);
        assert_eq!(cva.episodes().len(), 1);
        assert!(cva.insomnia_work(cva.episodes()[0].id).is_some());
        drop(cva);
        let mut cva = Cva::open(path).unwrap();
        let versions = (cva.archive_version(), cva.latest_global_version());
        cva.import_native(&incoming, 300).unwrap();
        assert_eq!(
            versions,
            (cva.archive_version(), cva.latest_global_version())
        );
    }
}
#[test]
fn imported_branch_can_continue_natively_without_invalidating_receipt() {
    let f = Fixture::new();
    let mut cva = Cva::create(f.rel("a.rel")).unwrap();
    let mut incoming = input();
    incoming.conversations[0]
        .messages
        .push(message("b", Some("u"), "assistant", "alternate"));
    incoming.conversations[0].branches.push(ImportBranch {
        id: "alternate".into(),
        leaf_message_id: "b".into(),
        canonical: false,
    });
    let receipt = cva.import_native(&incoming, 100).unwrap();
    let c = &receipt.conversations[0];
    let b = &c.branches[0];
    cva.append_node(
        "native".into(),
        c.conversation_id.clone(),
        b.leaf_node_id.clone(),
        "user".into(),
        200,
        "continue",
    )
    .unwrap();
    cva.append_branch(crate::Branch {
        id: b.branch_id.clone(),
        conversation_id: c.conversation_id.clone(),
        leaf_node_id: "native".into(),
        canonical: true,
    })
    .unwrap();
    cva.sync().unwrap();
    drop(cva);
    let mut cva = Cva::open(f.rel("a.rel")).unwrap();
    let versions = (cva.archive_version(), cva.latest_global_version());
    assert_eq!(cva.import_native(&incoming, 300).unwrap(), receipt);
    assert_eq!(
        versions,
        (cva.archive_version(), cva.latest_global_version())
    );
    incoming.conversations[0].title = Some("Changed source title".into());
    cva.import_native(&incoming, 350).unwrap();
    assert_eq!(
        cva.branches()
            .iter()
            .find(|branch| branch.id == b.branch_id)
            .unwrap()
            .leaf_node_id,
        "native"
    );
    let live = cva
        .finalize_explicit_path_and_queue(
            &c.conversation_id,
            "native",
            crate::EpisodePolicy::default(),
            400,
        )
        .unwrap();
    assert_eq!(live.episodes.created.len(), 1);
    let episode = &live.episodes.created[0];
    assert_eq!(episode.origin, EpisodeOrigin::Live);
    assert_eq!(
        cva.insomnia_work(episode.id).unwrap().priority,
        InsomniaPriority::ImmediateLive
    );
    assert_eq!(
        cva.episode_turns(episode.id)
            .unwrap()
            .iter()
            .map(|t| t.node_id.as_str())
            .collect::<Vec<_>>(),
        ["native"]
    );
    cva.sync().unwrap();
    drop(cva);
    let mut cva = Cva::open(f.rel("a.rel")).unwrap();
    cva.import_native(&incoming, 500).unwrap();
    assert!(
        cva.finalize_explicit_path_and_queue(
            &c.conversation_id,
            "native",
            crate::EpisodePolicy::default(),
            600,
        )
        .unwrap()
        .episodes
        .created
        .is_empty()
    );
}
#[test]
fn divergent_source_revision_conflicts_preserve_both_inputs() {
    let f = Fixture::new();
    let left = f.rel("left.rel");
    let right = f.rel("right.rel");
    let mut cva = Cva::create(&left).unwrap();
    cva.import_native(&input(), 100).unwrap();
    drop(cva);
    fs::copy(&left, &right).unwrap();
    let mut incoming = input();
    incoming.conversations[0].messages[1].revision = Some("2".into());
    incoming.conversations[0].messages[1].content = "left edit".into();
    let mut cva = Cva::open(&left).unwrap();
    cva.import_native(&incoming, 200).unwrap();
    drop(cva);
    incoming.conversations[0].messages[1].content = "right edit".into();
    let mut cva = Cva::open(&right).unwrap();
    cva.import_native(&incoming, 200).unwrap();
    drop(cva);
    let before_left = fs::read(&left).unwrap();
    let before_right = fs::read(&right).unwrap();
    assert!(matches!(
        Cva::reconcile(&left, &right, f.rel("merged.rel")),
        Err(crate::CvaReconcileError::Archive(
            crate::ArchiveError::InvalidField("conflicting import source revision")
        ))
    ));
    assert_eq!(fs::read(&left).unwrap(), before_left);
    assert_eq!(fs::read(&right).unwrap(), before_right);
    assert_eq!(Cva::open(left).unwrap().native_import_receipts().len(), 2);
    assert_eq!(Cva::open(right).unwrap().native_import_receipts().len(), 2);
}
#[test]
fn import_receipt_survives_reclaim_and_divergent_reconciliation() {
    let f = Fixture::new();
    let path = f.rel("a.rel");
    let mut cva = Cva::create(&path).unwrap();
    let original = cva.import_native(&input(), 100).unwrap();
    cva.reclaim_storage(f.rel("reclaimed.rel")).unwrap();
    assert_eq!(
        Cva::open(f.rel("reclaimed.rel"))
            .unwrap()
            .native_import_receipts(),
        vec![original.clone()]
    );
    drop(cva);
    fs::copy(&path, f.rel("b.rel")).unwrap();
    let mut a = Cva::open(&path).unwrap();
    let mut left = input();
    left.source_namespace = "left".into();
    a.import_native(&left, 200).unwrap();
    a.sync().unwrap();
    drop(a);
    let mut b = Cva::open(f.rel("b.rel")).unwrap();
    let mut right = input();
    right.source_namespace = "right".into();
    b.import_native(&right, 200).unwrap();
    b.sync().unwrap();
    drop(b);
    Cva::reconcile(&path, f.rel("b.rel"), f.rel("merged.rel")).unwrap();
    let merged = Cva::open(f.rel("merged.rel")).unwrap();
    assert_eq!(merged.native_import_receipts().len(), 3);
}

fn current_watermarks(cva: &Cva) -> ImportWatermarks {
    ImportWatermarks {
        global_version: cva.latest_global_version(),
        archive_version: cva.archive_version(),
        memory_version: cva.memory_version(),
        entity_version: cva.entities.entity_version(),
        relationship_version: cva.relationships.relationship_version(),
        graph_version: cva.graph.graph_version(),
        memory_graph_version: cva.graph.memory_graph_version(),
        vector_version: cva.vector_generations.vector_version(),
    }
}

fn completion_for(
    cva: &Cva,
    start: &ImportStart,
    receipt: &ImportReceipt,
    completed_at_ns: i64,
) -> ImportComplete {
    ImportComplete {
        owner_uuid: start.owner_uuid,
        import_id: start.import_id.clone(),
        start_digest: super::prepare::digest(start).unwrap(),
        receipt_digest: super::prepare::digest(receipt).unwrap(),
        completed_at_ns,
        post: current_watermarks(cva),
        historical_freshness_settlement_digest: "test-settlement-digest".into(),
    }
}

#[test]
fn native_only_rel_rebuilds_with_no_import_epoch_markers() {
    let f = Fixture::new();
    let path = f.rel("native-only.rel");
    let mut cva = Cva::create(&path).unwrap();
    assert!(cva.open_import_epoch().is_none());
    assert!(cva.import_epoch_statuses().is_empty());

    cva.append_node(
        "native".into(),
        "conversation".into(),
        None,
        "user".into(),
        10,
        "ordinary native write",
    )
    .unwrap();
    assert!(cva.import_epoch_statuses().is_empty());
    drop(cva);

    let reopened = Cva::open(&path).unwrap();
    assert!(reopened.open_import_epoch().is_none());
    assert!(reopened.import_epoch_statuses().is_empty());
}

#[test]
fn exact_import_retry_adds_no_marker_or_semantic_clock_change() {
    let f = Fixture::new();
    let path = f.rel("retry-no-extra-marker.rel");
    let mut cva = Cva::create(&path).unwrap();
    let receipt = cva.import_native(&input(), 100).unwrap();
    let start = cva.open_import_epoch().unwrap().clone();
    let before_len = fs::metadata(&path).unwrap().len();
    let before_clocks = current_watermarks(&cva);

    assert_eq!(cva.import_native(&input(), 200).unwrap(), receipt);
    assert_eq!(fs::metadata(&path).unwrap().len(), before_len);
    assert_eq!(current_watermarks(&cva), before_clocks);
    assert_eq!(
        cva.import_epoch_statuses(),
        vec![ImportEpochStatus::Open(start.clone())]
    );

    drop(cva);
    let mut reopened = Cva::open(&path).unwrap();
    let before_retry_len = fs::metadata(&path).unwrap().len();
    let before_retry_clocks = current_watermarks(&reopened);
    assert_eq!(reopened.import_native(&input(), 300).unwrap(), receipt);
    assert_eq!(fs::metadata(&path).unwrap().len(), before_retry_len);
    assert_eq!(current_watermarks(&reopened), before_retry_clocks);
    assert_eq!(reopened.open_import_epoch(), Some(start));
    assert_eq!(reopened.import_epoch_statuses().len(), 1);
}

#[test]
fn completed_epoch_rebuilds_and_completion_retry_is_exact() {
    let f = Fixture::new();
    let path = f.rel("completed-epoch-rebuild.rel");
    let mut cva = Cva::create(&path).unwrap();
    let receipt = cva.import_native(&input(), 100).unwrap();
    let start = cva.open_import_epoch().unwrap().clone();
    let complete = completion_for(&cva, &start, &receipt, 200);

    cva.container.set_owner_write_scope(true);
    cva.import_epochs
        .append_complete(&mut cva.container, complete.clone())
        .unwrap();
    cva.container.sync().unwrap();
    assert!(cva.open_import_epoch().is_none());

    let completed_len = fs::metadata(&path).unwrap().len();
    let completed_clocks = current_watermarks(&cva);
    cva.import_epochs
        .append_complete(&mut cva.container, complete.clone())
        .unwrap();
    let mut conflicting = complete.clone();
    conflicting.receipt_digest.push('x');
    assert!(
        cva.import_epochs
            .append_complete(&mut cva.container, conflicting)
            .is_err()
    );
    assert_eq!(fs::metadata(&path).unwrap().len(), completed_len);
    assert_eq!(current_watermarks(&cva), completed_clocks);

    drop(cva);
    let reopened = Cva::open(&path).unwrap();
    assert!(reopened.open_import_epoch().is_none());
    assert_eq!(
        reopened.import_epoch_statuses(),
        vec![ImportEpochStatus::Completed { start, complete }]
    );
    assert_eq!(reopened.native_import_receipts(), vec![receipt]);
}
