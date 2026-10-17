use super::historical_embedding::Endpoint;
use super::*;
use crate::{EmbeddingEndpoint, EmbeddingEndpointError, EmbeddingMode, VectorNormalization};
use std::sync::atomic::AtomicBool;

#[test]
fn historical_mixed_fragment_rendering_has_a_total_byte_bound() {
    let (_, mut cva, _) = historical_import_fixture("n3-render-bound.rel");
    for fragment in cva.fragments() {
        let ordinary = cva.fragment_text(fragment.id).unwrap();
        let bounded = cva
            .archive
            .fragment_text_bounded(&mut cva.container, &fragment, ordinary.len())
            .unwrap();
        assert_eq!(bounded, ordinary);
        assert!(
            cva.archive
                .fragment_text_bounded(&mut cva.container, &fragment, ordinary.len() - 1)
                .is_err()
        );
    }
}
#[test]
fn historical_embedding_native_mixed_rows_are_filled_or_reused_without_fragment_creation() {
    for retain_mixed in [false, true] {
        let (path, mut cva, _) = historical_import_fixture(&format!("h8-mixed-{retain_mixed}.rel"));
        for conversation in ["c1", "c2"] {
            cva.materialize_path_fragments(
                conversation,
                "a7",
                crate::FragmentConfig {
                    turns: 2,
                    overlap: 0,
                },
                true,
            )
            .unwrap();
        }
        let cancel = AtomicBool::new(false);
        let endpoint = Endpoint::new(&cancel);
        let profile = cva.establish_compatibility_profile(&endpoint).unwrap();
        let mixed = cva.fragments().len();
        if retain_mixed {
            historical_events::archive_generation_without_live(&mut cva, profile.id, &endpoint);
        }
        let report = embedding::fill(&mut cva, &endpoint, Default::default(), &cancel).unwrap();
        assert_eq!(report.new_rows, if retain_mixed { 0 } else { mixed });
        assert_eq!(report.new_fragments, 0);
        assert_eq!(cva.fragments().len(), mixed);
        assert_eq!(report.reused_rows, if retain_mixed { mixed } else { 0 });
        assert_eq!(report.unavailable_fragments, 0);
        if retain_mixed {
            assert!(report.coverage.iter().all(|s| s.mixed_fragments > 0));
        }
        let generation = cva.current_vector_generation(profile.id).unwrap();
        assert_eq!(
            cva.archive_vectors(generation.archive_vector_id)
                .unwrap()
                .fragment_ids
                .len(),
            mixed
        );
        assert!(cva.freshness.import_settlements().is_empty());
        drop(cva);
        let mut cva = Cva::open(&path).unwrap();
        let endpoint = Endpoint::new(&cancel);
        cva.reconstruct_historical_freshness_with_endpoint(
            HistoricalReconstructionOptions {
                fill_missing_embeddings: true,
            },
            &endpoint,
            Default::default(),
            &cancel,
        )
        .unwrap();
    }
}
#[test]
fn historical_embedding_native_existing_invalid_rows_are_not_reembedded() {
    let (path, mut cva, _) = historical_import_fixture("h8-invalid-retained.rel");
    historical_events::vectors(&mut cva);
    let profile = cva.compatibility_profiles().remove(0);
    let old = cva.current_vector_generation(profile.id).unwrap();
    let set = cva.archive_vectors(old.archive_vector_id).unwrap();
    let schema = crate::VectorSchema::new(2, crate::ScalarType::F32).unwrap();
    let packed =
        crate::PackedVectors::from_bytes(schema, vec![0; set.fragment_ids.len() * 8]).unwrap();
    let packed_id = cva.put_packed_vectors(packed).unwrap();
    let binding = cva
        .put_archive_vectors(packed_id, set.fragment_ids)
        .unwrap();
    let generation = cva
        .publish_vector_generation_without_live_semantic(profile.id, binding, cva.archive_version())
        .unwrap();
    cva.sync().unwrap();
    let bytes = fs::read(&path).unwrap();
    let cancel = AtomicBool::new(false);
    let endpoint = Endpoint::new(&cancel);
    let report = embedding::fill(&mut cva, &endpoint, Default::default(), &cancel).unwrap();
    assert_eq!(report.new_rows, 0);
    assert_eq!(report.final_generation, Some(generation.id));
    assert_eq!(report.unavailable_fragments, 0);
    assert!(
        report
            .coverage
            .iter()
            .all(|s| s.reason == "complete-unusable" && s.invalid_rows == 1)
    );
    assert_eq!(fs::read(&path).unwrap(), bytes);
}
struct UnboundedEndpoint;
impl EmbeddingEndpoint for UnboundedEndpoint {
    fn dimensions(&self) -> u32 {
        2
    }
    fn normalization(&self) -> VectorNormalization {
        VectorNormalization::L2
    }
    fn embed(
        &self,
        _: EmbeddingMode,
        _: &[String],
    ) -> Result<Vec<Vec<f32>>, EmbeddingEndpointError> {
        panic!("unbounded endpoint must not be called")
    }
}
#[test]
fn historical_embedding_native_unsupported_profile_endpoint_owner_and_clock_reject() {
    for mode in 0..3 {
        let (path, mut cva, _) = historical_import_fixture(&format!("h8-unsupported-{mode}.rel"));
        let cancel = AtomicBool::new(false);
        let endpoint = Endpoint::new(&cancel);
        cva.establish_compatibility_profile(&endpoint).unwrap();
        match mode {
            0 => {
                cva.establish_compatibility_profile(&crate::SimulatedEmbeddingEndpoint::new(
                    2,
                    VectorNormalization::L2,
                    123,
                ))
                .unwrap();
            }
            1 => {}
            _ => {
                cva.append_node(
                    "too-early".into(),
                    "other".into(),
                    None,
                    "user".into(),
                    -100,
                    "Noncanonical original acceptance.",
                )
                .unwrap();
            }
        }
        cva.sync().unwrap();
        let bytes = fs::read(&path).unwrap();
        let result = if mode == 1 {
            cva.reconstruct_historical_freshness_with_endpoint(
                HistoricalReconstructionOptions {
                    fill_missing_embeddings: true,
                },
                &UnboundedEndpoint,
                Default::default(),
                &cancel,
            )
        } else {
            cva.reconstruct_historical_freshness_with_endpoint(
                HistoricalReconstructionOptions {
                    fill_missing_embeddings: true,
                },
                &endpoint,
                Default::default(),
                &cancel,
            )
        };
        assert!(result.is_err());
        assert!(cva.freshness.import_settlements().is_empty());
        assert_eq!(fs::read(&path).unwrap(), bytes);
    }
}
#[test]
fn historical_embedding_native_cancel_after_probes_and_after_partial_fill_reopens() {
    // Cancellation after each opaque provider call sees no later dispatch.
    for call in [1, 2, 4] {
        let (path, mut cva, _) = historical_import_fixture(&format!("h8-cancel-call-{call}.rel"));
        let cancel = AtomicBool::new(false);
        let mut endpoint = Endpoint::new(&cancel);
        cva.establish_compatibility_profile(&endpoint).unwrap();
        endpoint.cancel_at = call;
        let error = cva
            .reconstruct_historical_freshness_with_endpoint(
                HistoricalReconstructionOptions {
                    fill_missing_embeddings: true,
                },
                &endpoint,
                HistoricalEmbeddingLimits {
                    batch_inputs: 8,
                    ..Default::default()
                },
                &cancel,
            )
            .unwrap_err();
        let HistoricalReconstructionError::EmbeddingFill(error) = error else {
            panic!("missing report");
        };
        assert_eq!(
            error.report.disposition,
            HistoricalEmbeddingDisposition::Cancelled
        );
        assert_eq!(error.report.attempted_calls, call);
        assert_eq!(error.report.new_rows, if call == 4 { 8 } else { 0 });
        assert!(cva.freshness.import_settlements().is_empty());
        drop(cva);
        let mut cva = Cva::open(path).unwrap();
        cancel.store(false, std::sync::atomic::Ordering::Release);
        let endpoint = Endpoint::new(&cancel);
        let retry = cva
            .reconstruct_historical_freshness_with_endpoint(
                HistoricalReconstructionOptions {
                    fill_missing_embeddings: true,
                },
                &endpoint,
                HistoricalEmbeddingLimits {
                    batch_inputs: 8,
                    ..Default::default()
                },
                &cancel,
            )
            .unwrap();
        assert_eq!(
            retry.embedding.unwrap().new_rows,
            if call == 4 { 8 } else { 16 }
        );
    }
}

struct UnnormalizedZero {
    calls: std::cell::Cell<usize>,
}
impl EmbeddingEndpoint for UnnormalizedZero {
    fn dimensions(&self) -> u32 {
        2
    }
    fn normalization(&self) -> VectorNormalization {
        VectorNormalization::None
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
        _: usize,
    ) -> Result<Vec<Vec<f32>>, EmbeddingEndpointError> {
        let call = self.calls.get() + 1;
        self.calls.set(call);
        if call > 2 {
            Ok(vec![vec![0.0, 0.0]; inputs.len()])
        } else {
            self.embed(mode, inputs)
        }
    }
}
#[test]
fn historical_embedding_native_unnormalized_zero_output_rejects_without_writes() {
    let (path, mut cva, id) = historical_import_fixture("h8-no-norm-zero.rel");
    let endpoint = UnnormalizedZero {
        calls: std::cell::Cell::new(0),
    };
    cva.establish_compatibility_profile(&endpoint).unwrap();
    cva.sync().unwrap();
    let bytes = fs::read(&path).unwrap();
    let record = cva.freshness_record(id);
    let cancel = AtomicBool::new(false);
    let error = cva
        .reconstruct_historical_freshness_with_endpoint(
            HistoricalReconstructionOptions {
                fill_missing_embeddings: true,
            },
            &endpoint,
            Default::default(),
            &cancel,
        )
        .unwrap_err();
    let HistoricalReconstructionError::EmbeddingFill(error) = error else {
        panic!("missing report");
    };
    assert_eq!(
        error.report.disposition,
        HistoricalEmbeddingDisposition::InvalidOutput
    );
    assert_eq!(endpoint.calls.get(), 3);
    assert!(!error.report.write_attempted);
    assert!(cva.freshness.import_settlements().is_empty());
    assert_eq!(cva.freshness_record(id), record);
    assert_eq!(fs::read(path).unwrap(), bytes);
}
