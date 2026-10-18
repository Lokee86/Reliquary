use crate::imports::{
    ImportBranch, ImportComplete, ImportCompletionOutcome, ImportCompletionPending,
    ImportConversation, ImportMessage, ImportStart, ImportWatermarks, NativeImport,
};
use crate::{Container, Cva, HistoricalReconstructionOptions};
use sha2::{Digest, Sha256};
use std::{fs, path::PathBuf};

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let path = std::env::current_dir()
            .unwrap()
            .join(".test-tmp")
            .join(uuid::Uuid::new_v4().to_string());
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn rel(&self) -> PathBuf {
        self.0.join("completion.rel")
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn empty_import(namespace: &str) -> NativeImport {
    NativeImport {
        schema_version: 1,
        source_namespace: namespace.into(),
        source_format: "completion-test-v1".into(),
        conversations: Vec::new(),
    }
}

fn nonempty_import() -> NativeImport {
    NativeImport {
        schema_version: 1,
        source_namespace: "completion-test/nonempty".into(),
        source_format: "completion-test-v1".into(),
        conversations: vec![ImportConversation {
            id: "conversation".into(),
            title: None,
            messages: vec![ImportMessage {
                id: "user".into(),
                revision: Some("1".into()),
                parent_id: None,
                replaces: None,
                deleted: false,
                role: "user".into(),
                timestamp_ns: 10,
                content: "hello".into(),
                artifacts: Vec::new(),
            }],
            branches: vec![ImportBranch {
                id: "main".into(),
                leaf_message_id: "user".into(),
                canonical: true,
            }],
        }],
    }
}

fn clocks(cva: &Cva) -> (u64, u64, u64, u64) {
    (
        cva.latest_global_version(),
        cva.archive_version(),
        cva.memory_version(),
        cva.rel_turn_count(),
    )
}

fn watermarks(cva: &Cva) -> ImportWatermarks {
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

fn actual_completion(cva: &Cva, start: &ImportStart, completed_at_ns: i64) -> ImportComplete {
    let receipt = cva
        .native_import_receipts()
        .into_iter()
        .find(|receipt| receipt.import_key == start.import_key)
        .unwrap();
    let settlement = cva
        .freshness
        .import_settlements()
        .iter()
        .find(|settlement| {
            settlement
                .import_id
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>()
                == start.import_id
        })
        .unwrap();
    ImportComplete {
        owner_uuid: start.owner_uuid,
        import_id: start.import_id.clone(),
        start_digest: super::prepare::digest(start).unwrap(),
        receipt_digest: super::prepare::digest(&receipt).unwrap(),
        completed_at_ns,
        post: watermarks(cva),
        historical_freshness_settlement_digest: super::settlement_digest(settlement),
    }
}

#[test]
fn empty_receipt_completes_only_with_real_freshness_settlement_and_exact_retry() {
    let fixture = Fixture::new();
    let path = fixture.rel();
    let mut cva = Cva::create(&path).unwrap();
    let receipt = cva
        .import_native(&empty_import("completion-test/empty"), 100)
        .unwrap();
    let start = cva.open_import_epoch().unwrap();
    let before_pending = fs::read(&path).unwrap();

    assert_eq!(
        cva.complete_native_import(&start.import_id, 200).unwrap(),
        ImportCompletionOutcome::Pending(ImportCompletionPending::MissingFreshnessSettlement)
    );
    assert_eq!(fs::read(&path).unwrap(), before_pending);
    assert!(
        cva.append_node(
            "live".into(),
            "live".into(),
            None,
            "user".into(),
            201,
            "allowed after pending"
        )
        .is_err()
    );

    let reconstructed = cva
        .reconstruct_historical_freshness(HistoricalReconstructionOptions::default())
        .unwrap();
    assert!(reconstructed.committed);
    let settlement = cva
        .freshness
        .import_settlement(reconstructed.import_id)
        .expect("accepted scoped freshness settlement");
    assert!(settlement.epoch.is_some());
    assert!(settlement.encode().starts_with(b"CVAFRS05"));
    let settlement_digest = Sha256::digest(settlement.encode())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();

    let before_complete = watermarks(&cva);
    let completed = cva.complete_native_import(&start.import_id, 300).unwrap();
    let (complete, committed) = match completed {
        ImportCompletionOutcome::Completed {
            complete,
            committed,
        } => (complete, committed),
        ImportCompletionOutcome::Pending(pending) => panic!("unexpected pending: {pending:?}"),
    };
    assert!(committed);
    assert_eq!(complete.post, before_complete);
    assert_eq!(watermarks(&cva), before_complete);
    assert_eq!(complete.owner_uuid, start.owner_uuid);
    assert_eq!(complete.import_id, start.import_id);
    assert_eq!(
        complete.start_digest,
        super::prepare::digest(&start).unwrap()
    );
    assert_eq!(
        complete.receipt_digest,
        super::prepare::digest(&receipt).unwrap()
    );
    assert_eq!(
        complete.historical_freshness_settlement_digest,
        settlement_digest
    );
    let after_first = fs::read(&path).unwrap();
    let after_clocks = clocks(&cva);

    // A valid live append moves the owner forward after completion. Retrying the
    // already accepted completion must preserve that later activity and marker.
    cva.append_node(
        "later".into(),
        "live".into(),
        None,
        "user".into(),
        400,
        "later activity",
    )
    .unwrap();
    let live_clocks = clocks(&cva);
    let marker_count = cva.import_epoch_statuses().len();
    drop(cva);

    let mut reopened = Cva::open(&path).unwrap();
    let before_retry_bytes = fs::read(&path).unwrap();
    let outcome = reopened
        .complete_native_import(&start.import_id, 500)
        .unwrap();
    assert_eq!(
        outcome,
        ImportCompletionOutcome::Completed {
            complete: complete.clone(),
            committed: false,
        }
    );
    assert_eq!(clocks(&reopened), live_clocks);
    assert_eq!(reopened.import_epoch_statuses().len(), marker_count);
    assert_eq!(fs::read(&path).unwrap(), before_retry_bytes);
    assert_ne!(after_first, before_retry_bytes);
    assert!(after_clocks.0 < live_clocks.0);
}

#[test]
fn nonempty_receipt_stays_pending_without_changing_readiness_or_bytes() {
    let fixture = Fixture::new();
    let path = fixture.rel();
    let mut cva = Cva::create(&path).unwrap();
    cva.import_native(&nonempty_import(), 100).unwrap();
    let start = cva.open_import_epoch().unwrap();
    let before_bytes = fs::read(&path).unwrap();
    let before_clocks = clocks(&cva);
    let before_statuses = cva.import_epoch_statuses();

    assert_eq!(
        cva.complete_native_import(&start.import_id, 200).unwrap(),
        ImportCompletionOutcome::Pending(ImportCompletionPending::AttributedReadinessUnavailable)
    );
    assert_eq!(fs::read(&path).unwrap(), before_bytes);
    assert_eq!(clocks(&cva), before_clocks);
    assert_eq!(cva.import_epoch_statuses(), before_statuses);
}

#[test]
fn unknown_id_and_pre_start_time_fail_without_writing() {
    let fixture = Fixture::new();
    let path = fixture.rel();
    let mut cva = Cva::create(&path).unwrap();
    cva.import_native(&empty_import("completion-test/errors"), 100)
        .unwrap();
    let start = cva.open_import_epoch().unwrap();
    let before_bytes = fs::read(&path).unwrap();
    let before_clocks = clocks(&cva);
    let before_statuses = cva.import_epoch_statuses();

    assert!(
        cva.complete_native_import("unknown-import-id", 200)
            .is_err()
    );
    assert!(cva.complete_native_import(&start.import_id, 99).is_err());
    assert_eq!(fs::read(&path).unwrap(), before_bytes);
    assert_eq!(clocks(&cva), before_clocks);
    assert_eq!(cva.import_epoch_statuses(), before_statuses);
}

#[test]
fn uncertain_completion_append_is_discovered_and_acknowledged_without_duplicate() {
    let fixture = Fixture::new();
    let path = fixture.rel();
    let mut cva = Cva::create(&path).unwrap();
    cva.import_native(&empty_import("completion-test/uncertain"), 100)
        .unwrap();
    let start = cva.open_import_epoch().unwrap();
    cva.reconstruct_historical_freshness(HistoricalReconstructionOptions::default())
        .unwrap();
    let before_completion_post = watermarks(&cva);
    let complete = actual_completion(&cva, &start, 200);
    assert_eq!(complete.post, before_completion_post);
    assert_eq!(watermarks(&cva), before_completion_post);

    // Model the uncertain boundary after the completion frame reached the
    // container but before the owner updated its in-memory epoch store or
    // reported success.
    let payload = super::epoch_codec::encode_complete(&complete).unwrap();
    cva.with_import_write_scope(|cva| cva.container.append(&payload).unwrap());
    let marker_bytes = fs::read(&path).unwrap();
    assert!(cva.open_import_epoch().is_some());
    assert!(
        cva.append_node(
            "blocked".into(),
            "live".into(),
            None,
            "user".into(),
            201,
            "fenced"
        )
        .is_err()
    );

    assert_eq!(
        cva.complete_native_import(&start.import_id, 300).unwrap(),
        ImportCompletionOutcome::Completed {
            complete: complete.clone(),
            committed: false,
        }
    );
    assert!(cva.open_import_epoch().is_none());
    assert_eq!(fs::read(&path).unwrap(), marker_bytes);
    cva.append_node(
        "unfenced".into(),
        "live".into(),
        None,
        "user".into(),
        301,
        "write allowed",
    )
    .unwrap();
}

#[test]
fn reopen_rejects_completion_with_wrong_settlement_digest_without_rewriting() {
    let fixture = Fixture::new();
    let path = fixture.rel();
    let mut cva = Cva::create(&path).unwrap();
    cva.import_native(&empty_import("completion-test/corrupt"), 100)
        .unwrap();
    let start = cva.open_import_epoch().unwrap();
    cva.reconstruct_historical_freshness(HistoricalReconstructionOptions::default())
        .unwrap();
    let mut complete = actual_completion(&cva, &start, 200);
    complete
        .historical_freshness_settlement_digest
        .push_str("corrupt");
    let payload = super::epoch_codec::encode_complete(&complete).unwrap();
    cva.with_import_write_scope(|cva| cva.container.append(&payload).unwrap());
    let corrupt_bytes = fs::read(&path).unwrap();
    drop(cva);

    assert!(Cva::open(&path).is_err());
    assert_eq!(fs::read(&path).unwrap(), corrupt_bytes);
}

#[test]
fn missing_receipt_stays_pending_without_changing_bytes() {
    let fixture = Fixture::new();
    let path = fixture.rel();
    let mut cva = Cva::create(&path).unwrap();
    let prepared = super::prepare::prepare(&empty_import("completion-test/no-receipt")).unwrap();
    let owner_uuid = cva.owner_uuid().unwrap();
    let start = ImportStart {
        owner_uuid,
        import_id: crate::imports::epoch_store::import_id(owner_uuid, &prepared.receipt.import_key),
        import_key: prepared.receipt.import_key,
        source_namespace: prepared.receipt.source_namespace,
        source_format: prepared.receipt.source_format,
        started_at_ns: 100,
        pre: watermarks(&cva),
    };
    let start_bytes = super::epoch_codec::encode_start(&start).unwrap();
    cva.with_import_write_scope(|cva| cva.container.append(&start_bytes).unwrap());
    let before_bytes = fs::read(&path).unwrap();

    assert_eq!(
        cva.complete_native_import(&start.import_id, 200).unwrap(),
        ImportCompletionOutcome::Pending(ImportCompletionPending::MissingReceipt)
    );
    assert_eq!(cva.open_import_epoch(), Some(start));
    assert!(
        cva.append_node(
            "blocked".into(),
            "live".into(),
            None,
            "user".into(),
            201,
            "fenced"
        )
        .is_err()
    );
    assert_eq!(fs::read(&path).unwrap(), before_bytes);
}

#[test]
fn retrying_an_older_completed_import_keeps_newer_open_import_fenced() {
    let fixture = Fixture::new();
    let path = fixture.rel();
    let mut cva = Cva::create(&path).unwrap();
    let import_a = empty_import("completion-test/older");
    cva.import_native(&import_a, 100).unwrap();
    let start_a = cva.open_import_epoch().unwrap();
    cva.reconstruct_historical_freshness(HistoricalReconstructionOptions::default())
        .unwrap();
    assert!(matches!(
        cva.complete_native_import(&start_a.import_id, 200).unwrap(),
        ImportCompletionOutcome::Completed {
            committed: true,
            ..
        }
    ));

    let import_b = empty_import("completion-test/newer");
    cva.import_native(&import_b, 300).unwrap();
    let start_b = cva.open_import_epoch().unwrap();
    assert_ne!(start_a.import_id, start_b.import_id);
    let before_retry_bytes = fs::read(&path).unwrap();

    assert!(matches!(
        cva.complete_native_import(&start_a.import_id, 400).unwrap(),
        ImportCompletionOutcome::Completed {
            committed: false,
            ..
        }
    ));
    assert_eq!(cva.open_import_epoch(), Some(start_b));
    assert!(
        cva.append_node(
            "blocked".into(),
            "live".into(),
            None,
            "user".into(),
            401,
            "still fenced"
        )
        .is_err()
    );
    assert_eq!(fs::read(&path).unwrap(), before_retry_bytes);
}

#[test]
fn reopen_rejects_completion_physically_before_its_settlement() {
    let fixture = Fixture::new();
    let path = fixture.rel();
    let mut cva = Cva::create(&path).unwrap();
    cva.import_native(&empty_import("completion-test/order"), 100)
        .unwrap();
    let start = cva.open_import_epoch().unwrap();
    cva.reconstruct_historical_freshness(HistoricalReconstructionOptions::default())
        .unwrap();
    let complete = actual_completion(&cva, &start, 200);
    let settlement = cva
        .freshness
        .import_settlement(
            start
                .import_id
                .as_bytes()
                .chunks_exact(2)
                .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
                .collect::<Vec<_>>()
                .try_into()
                .unwrap(),
        )
        .unwrap()
        .encode();

    let mut scan = Container::open_read_only_for_scan(&path).unwrap();
    let mut settlement_offset = None;
    scan.visit_payloads::<crate::ContainerError>(|object, payload| {
        if payload.starts_with(b"CVAFRS05") {
            settlement_offset = Some(u64::from_le_bytes(
                object.legacy_bytes()[..8].try_into().unwrap(),
            ));
        }
        Ok(())
    })
    .unwrap();
    drop(scan);
    drop(cva);

    let settlement_offset = settlement_offset.expect("physical scoped settlement frame");
    std::fs::OpenOptions::new()
        .write(true)
        .open(&path)
        .unwrap()
        .set_len(settlement_offset)
        .unwrap();
    let mut container = Container::open(&path).unwrap();
    container
        .append(&super::epoch_codec::encode_complete(&complete).unwrap())
        .unwrap();
    container.append(&settlement).unwrap();
    container.sync().unwrap();
    drop(container);
    let reordered_bytes = fs::read(&path).unwrap();

    assert!(Cva::open(&path).is_err());
    assert_eq!(fs::read(&path).unwrap(), reordered_bytes);
}
