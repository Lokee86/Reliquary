use super::*;
use crate::{EmbeddingEndpoint, EmbeddingEndpointError, EmbeddingMode, VectorNormalization};
use std::cell::Cell;
use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
pub(super) struct Endpoint<'a> {
    calls: Cell<usize>,
    inputs: Cell<usize>,
    fail: usize,
    invalid: usize,
    pub(super) cancel_at: usize,
    cancel: &'a AtomicBool,
    drift: Option<(usize, std::path::PathBuf)>,
}
impl<'a> Endpoint<'a> {
    pub(super) fn new(cancel: &'a AtomicBool) -> Self {
        Self {
            calls: Cell::new(0),
            inputs: Cell::new(0),
            fail: 0,
            invalid: 0,
            cancel_at: 0,
            cancel,
            drift: None,
        }
    }
}
impl EmbeddingEndpoint for Endpoint<'_> {
    fn dimensions(&self) -> u32 {
        2
    }
    fn normalization(&self) -> VectorNormalization {
        VectorNormalization::L2
    }
    fn supports_bounded_embedding(&self) -> bool {
        true
    }
    fn embed(
        &self,
        _: EmbeddingMode,
        inputs: &[String],
    ) -> Result<Vec<Vec<f32>>, EmbeddingEndpointError> {
        Ok(vec![vec![1.0, 0.0]; inputs.len()])
    }
    fn embed_bounded(
        &self,
        mode: EmbeddingMode,
        inputs: &[String],
        bound: usize,
    ) -> Result<Vec<Vec<f32>>, EmbeddingEndpointError> {
        assert!(inputs.len() <= 16);
        assert!(bound >= inputs.len() * 8);
        let call = self.calls.get() + 1;
        self.calls.set(call);
        self.inputs.set(self.inputs.get() + inputs.len());
        if call > 2 {
            assert_eq!(mode, EmbeddingMode::Document);
            assert!(
                inputs
                    .iter()
                    .all(|s| s.starts_with("user: ") || s.starts_with("assistant: "))
            );
        }
        if self.cancel_at == call {
            self.cancel.store(true, Ordering::Release);
        }
        if let Some((at, path)) = &self.drift {
            if *at == call {
                fs::OpenOptions::new()
                    .append(true)
                    .open(path)
                    .unwrap()
                    .write_all(&0u64.to_le_bytes())
                    .unwrap();
            }
        }
        if self.fail == call {
            return Err(EmbeddingEndpointError::Failure(
                "injected provider failure".into(),
            ));
        }
        if self.invalid == call {
            return Ok(vec![vec![0.0, 0.0]; inputs.len()]);
        }
        self.embed(mode, inputs)
    }
}
fn request() -> HistoricalReconstructionOptions {
    HistoricalReconstructionOptions {
        fill_missing_embeddings: true,
    }
}
fn failure(error: HistoricalReconstructionError) -> HistoricalEmbeddingFailure {
    match error {
        HistoricalReconstructionError::EmbeddingFill(e) => *e,
        other => panic!("expected fill report: {other}"),
    }
}
fn assert_unaccepted(
    cva: &Cva,
    path: &std::path::Path,
    id: crate::MemoryId,
    before: FreshnessRecord,
) {
    assert!(cva.freshness.import_checkpoint().is_none());
    assert_eq!(cva.freshness_record(id), Some(before));
    assert!(!cva.semantic_freshness_enabled());
    assert!(!fs::read(path).unwrap().windows(8).any(|w| w == b"CVAFRS03"));
}
#[test]
fn historical_embedding_native_fill_manifest_reopen_and_accepted_retry() {
    let (path, mut cva, id) = historical_import_fixture("h8-fill.rel");
    let cancel = AtomicBool::new(false);
    let endpoint = Endpoint::new(&cancel);
    let profile = cva.establish_compatibility_profile(&endpoint).unwrap();
    cva.build_missing_memory_vectors(profile.id, &endpoint)
        .unwrap();
    let turns = cva.rel_turn_count();
    let graph = cva.graph_version();
    let pass = crate::dream_freshness::initial_pass_id(cva.owner_uuid().unwrap(), id);
    assert!(cva.initial_dream_pass_settled(&pass));
    let report = cva
        .reconstruct_historical_freshness_with_endpoint(
            request(),
            &endpoint,
            Default::default(),
            &cancel,
        )
        .unwrap();
    let fill = report.embedding.as_ref().unwrap();
    assert_eq!(fill.disposition, HistoricalEmbeddingDisposition::Complete);
    assert_eq!(
        (fill.new_rows, fill.new_fragments, fill.unavailable_sources),
        (32, 32, 0)
    );
    assert_eq!((fill.attempted_calls, fill.attempted_inputs), (4, 36));
    assert_eq!(endpoint.calls.get(), 4);
    assert_eq!(cva.rel_turn_count(), turns);
    assert_eq!(cva.graph_version(), graph);
    assert!(cva.initial_dream_pass_settled(&pass));
    assert!(!cva.semantic_freshness_enabled());
    assert_eq!(cva.freshness_record(id).unwrap().score, 100);
    drop(cva);
    let mut cva = Cva::open(&path).unwrap();
    let bytes = fs::read(&path).unwrap();
    cancel.store(true, Ordering::Release);
    let retry = cva
        .reconstruct_historical_freshness_with_endpoint(
            request(),
            &endpoint,
            HistoricalEmbeddingLimits {
                source_bytes: usize::MAX,
                ..Default::default()
            },
            &cancel,
        )
        .unwrap();
    assert!(!retry.committed);
    assert!(retry.embedding.is_none());
    assert_eq!(retry.manifest_digest, report.manifest_digest);
    assert_eq!(endpoint.calls.get(), 4);
    assert_eq!(fs::read(&path).unwrap(), bytes);
    assert!(cva.set_semantic_freshness_enabled(true).unwrap());
    cva.append_node(
        "next".into(),
        "live".into(),
        None,
        "user".into(),
        500,
        "Prospective source.",
    )
    .unwrap();
    assert_eq!(cva.semantic_freshness_pending_turns().unwrap(), 1);
}
#[test]
fn historical_embedding_native_partial_provider_failure_reopen_reuses_rows() {
    let (path, mut cva, id) = historical_import_fixture("h8-retry.rel");
    let cancel = AtomicBool::new(false);
    let mut endpoint = Endpoint::new(&cancel);
    cva.establish_compatibility_profile(&endpoint).unwrap();
    endpoint.fail = 4;
    let before = cva.freshness_record(id).unwrap();
    let stopped = failure(
        cva.reconstruct_historical_freshness_with_endpoint(
            request(),
            &endpoint,
            Default::default(),
            &cancel,
        )
        .unwrap_err(),
    );
    assert_eq!(
        stopped.report.disposition,
        HistoricalEmbeddingDisposition::ProviderError
    );
    assert_eq!(stopped.report.new_rows, 16);
    assert_unaccepted(&cva, &path, id, before);
    let g = cva
        .current_vector_generation(stopped.report.profile_id.unwrap())
        .unwrap();
    let old = cva.archive_vectors(g.archive_vector_id).unwrap();
    let packed = cva.packed_vectors(old.packed_vector_id).unwrap();
    let old_rows: BTreeMap<_, _> = old
        .fragment_ids
        .iter()
        .enumerate()
        .map(|(i, id)| (id.0, packed.row(i).unwrap().to_vec()))
        .collect();
    drop(cva);
    let mut cva = Cva::open(&path).unwrap();
    let endpoint = Endpoint::new(&cancel);
    let report = cva
        .reconstruct_historical_freshness_with_endpoint(
            request(),
            &endpoint,
            Default::default(),
            &cancel,
        )
        .unwrap();
    let fill = report.embedding.unwrap();
    assert_eq!(
        (fill.reused_rows, fill.new_rows, fill.attempted_calls),
        (16, 16, 3)
    );
    let g = cva
        .current_vector_generation(fill.profile_id.unwrap())
        .unwrap();
    let set = cva.archive_vectors(g.archive_vector_id).unwrap();
    let matrix = cva.packed_vectors(set.packed_vector_id).unwrap();
    assert!(set.fragment_ids.windows(2).all(|p| p[0].0 < p[1].0));
    for (i, id) in set.fragment_ids.iter().enumerate() {
        if let Some(expected) = old_rows.get(&id.0) {
            assert_eq!(matrix.row(i).unwrap(), expected);
        }
    }
}
#[test]
fn historical_embedding_native_cancellation_invalid_output_and_input_drift_stop() {
    for mode in 0..4 {
        let (path, mut cva, id) = historical_import_fixture(&format!("h8-stop-{mode}.rel"));
        let cancel = AtomicBool::new(false);
        let mut endpoint = Endpoint::new(&cancel);
        cva.establish_compatibility_profile(&endpoint).unwrap();
        let before = cva.freshness_record(id).unwrap();
        match mode {
            0 => cancel.store(true, Ordering::Release),
            1 => endpoint.cancel_at = 3,
            2 => endpoint.invalid = 3,
            _ => endpoint.drift = Some((3, path.clone())),
        }
        let stopped = failure(
            cva.reconstruct_historical_freshness_with_endpoint(
                request(),
                &endpoint,
                Default::default(),
                &cancel,
            )
            .unwrap_err(),
        );
        assert_eq!(
            stopped.report.disposition,
            match mode {
                0 | 1 => HistoricalEmbeddingDisposition::Cancelled,
                2 => HistoricalEmbeddingDisposition::InvalidOutput,
                _ => HistoricalEmbeddingDisposition::InputDrift,
            }
        );
        assert_eq!(stopped.report.new_rows, 0);
        assert_unaccepted(&cva, &path, id, before);
        if mode == 0 {
            assert_eq!(endpoint.calls.get(), 0);
        }
    }
}
#[test]
fn historical_embedding_native_budget_preflight_has_no_calls_or_writes() {
    let (path, mut cva, id) = historical_import_fixture("h8-budgets.rel");
    let cancel = AtomicBool::new(false);
    let endpoint = Endpoint::new(&cancel);
    cva.establish_compatibility_profile(&endpoint).unwrap();
    cva.sync().unwrap();
    let bytes = fs::read(&path).unwrap();
    let record = cva.freshness_record(id).unwrap();
    let limits = [
        HistoricalEmbeddingLimits {
            sources: 1,
            ..Default::default()
        },
        HistoricalEmbeddingLimits {
            fragments: 1,
            ..Default::default()
        },
        HistoricalEmbeddingLimits {
            generation_rows: 1,
            ..Default::default()
        },
        HistoricalEmbeddingLimits {
            missing_rows: 1,
            ..Default::default()
        },
        HistoricalEmbeddingLimits {
            new_fragments: 1,
            ..Default::default()
        },
        HistoricalEmbeddingLimits {
            source_bytes: 1,
            ..Default::default()
        },
        HistoricalEmbeddingLimits {
            text_bytes: 1,
            ..Default::default()
        },
        HistoricalEmbeddingLimits {
            dimensions: 1,
            ..Default::default()
        },
        HistoricalEmbeddingLimits {
            matrix_bytes: 1,
            ..Default::default()
        },
        HistoricalEmbeddingLimits {
            components: 1,
            ..Default::default()
        },
        HistoricalEmbeddingLimits {
            inspections: 1,
            ..Default::default()
        },
        HistoricalEmbeddingLimits {
            temporary_bytes: 1,
            ..Default::default()
        },
        HistoricalEmbeddingLimits {
            source_calls: 1,
            ..Default::default()
        },
        HistoricalEmbeddingLimits {
            batch_inputs: 17,
            ..Default::default()
        },
        HistoricalEmbeddingLimits {
            source_bytes: usize::MAX,
            ..Default::default()
        },
        HistoricalEmbeddingLimits {
            sources: 0,
            ..Default::default()
        },
    ];
    for limit in limits {
        let stopped = failure(
            cva.reconstruct_historical_freshness_with_endpoint(
                request(),
                &endpoint,
                limit,
                &cancel,
            )
            .unwrap_err(),
        );
        assert_eq!(
            stopped.report.disposition,
            HistoricalEmbeddingDisposition::BudgetExceeded
        );
        assert_eq!(endpoint.calls.get(), 0);
        assert_eq!(fs::read(&path).unwrap(), bytes);
        assert_unaccepted(&cva, &path, id, record);
    }
}
#[test]
fn historical_embedding_native_default_no_provider_and_zero_work_reuse() {
    let (path, mut cva, _) = historical_import_fixture("h8-no-fill.rel");
    let cancel = AtomicBool::new(false);
    let endpoint = Endpoint::new(&cancel);
    let no_fill = cva
        .reconstruct_historical_freshness_with_endpoint(
            Default::default(),
            &endpoint,
            Default::default(),
            &cancel,
        )
        .unwrap();
    assert!(no_fill.embedding.is_none());
    assert_eq!(endpoint.calls.get(), 0);
    drop(cva);
    let _ = fs::remove_file(path);
    let (path, mut cva, _) = historical_import_fixture("h8-zero-work.rel");
    historical_events::vectors(&mut cva);
    let bytes = fs::read(&path).unwrap();
    let filled = embedding::fill(&mut cva, &endpoint, Default::default(), &cancel).unwrap();
    assert_eq!(
        (
            filled.new_rows,
            filled.attempted_calls,
            filled.unavailable_sources
        ),
        (0, 2, 0)
    );
    let filled = manifest::build_with_fill(&mut cva, true).unwrap();
    assert_ne!(filled.digest, manifest::build(&mut cva).unwrap().digest);
    let encoded = filled.value.encoded().unwrap();
    assert!(
        encoded
            .windows("coverage_digest".len())
            .any(|s| s == b"coverage_digest")
    );
    assert_eq!(fs::read(&path).unwrap(), bytes);
}
