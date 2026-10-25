//! Exclusive offline historical fill. Provider callbacks never hold runtime locks.
use super::embedding_model::*;
use super::embedding_plan;
use super::fail;
use crate::{Cva, CvaError, EmbeddingEndpoint, EmbeddingMode, VectorGeneration};
use std::sync::atomic::{AtomicBool, Ordering};

pub(super) fn check_cancel(cancel: &AtomicBool) -> Result<(), Stop> {
    if cancel.load(Ordering::Acquire) {
        Err((
            HistoricalEmbeddingDisposition::Cancelled,
            "cancelled before acceptance".into(),
        ))
    } else {
        Ok(())
    }
}
pub(super) struct Expected {
    global: u64,
    bytes: u64,
    digest: [u8; 32],
}
impl Expected {
    pub(super) fn read(cva: &Cva) -> Result<Self, Stop> {
        let (bytes, digest) = cva
            .runtime_historical_backing_facts()
            .map_err(unsupported)?;
        Ok(Self {
            global: cva.latest_global_version(),
            bytes,
            digest,
        })
    }
    pub(super) fn verify(&self, cva: &Cva) -> Result<(), Stop> {
        let current = Self::read(cva)?;
        if self.global != current.global
            || self.bytes != current.bytes
            || self.digest != current.digest
        {
            Err((
                HistoricalEmbeddingDisposition::InputDrift,
                "ImportInputDrift during embedding".into(),
            ))
        } else {
            Ok(())
        }
    }
}
fn provider_stop(error: impl std::fmt::Display, cancel: &AtomicBool) -> Stop {
    if cancel.load(Ordering::Acquire) {
        (HistoricalEmbeddingDisposition::Cancelled, error.to_string())
    } else {
        (
            HistoricalEmbeddingDisposition::ProviderError,
            error.to_string(),
        )
    }
}
pub(super) fn coverage(cva: &mut Cva, report: &mut HistoricalEmbeddingReport) -> Result<(), Stop> {
    let generation = cva.runtime_historical_latest_vector_generation();
    report.coverage.clear();
    report.unavailable_fragments = 0;
    for fragment in cva.fragments() {
        let Some(position) = cva
            .semantic_fragment_position(fragment.id)
            .map_err(unsupported)?
        else {
            continue;
        };
        let sources = cva
            .semantic_fragment_sources(fragment.id)
            .map_err(unsupported)?;
        let activity_turns = sources
            .iter()
            .filter(|(c, n)| cva.activity_position_for_turn(c, n).is_some())
            .count();
        let result = generation
            .map(|g| cva.prepare_semantic_fragment(fragment.id, g.id))
            .transpose()
            .map_err(unsupported)?;
        let (reason, usable_rows, invalid_rows) = match result {
            None | Some(crate::cva_semantic_source::SemanticSourceReadiness::Pending) => {
                report.unavailable_fragments += 1;
                ("missing-or-incomplete-fragment-evidence", 0, 0)
            }
            Some(crate::cva_semantic_source::SemanticSourceReadiness::Ready(source)) => (
                if source.vectors.is_empty() {
                    "complete-unusable"
                } else {
                    "complete-usable"
                },
                source.vectors.len(),
                source.invalid_vectors,
            ),
        };
        report.coverage.push(HistoricalEmbeddingCoverage {
            fragment_id: fragment.id,
            source_keys: sources,
            position,
            reason: reason.into(),
            usable_rows,
            mixed_fragments: usize::from(activity_turns > 1),
            invalid_rows,
        });
    }
    Ok(())
}
pub(super) fn fill<E: EmbeddingEndpoint>(
    cva: &mut Cva,
    endpoint: &E,
    limits: HistoricalEmbeddingLimits,
    cancel: &AtomicBool,
) -> Result<HistoricalEmbeddingReport, HistoricalEmbeddingFailure> {
    let mut report = HistoricalEmbeddingReport::default();
    let result = (|| -> Result<(), Stop> {
        let mut plan = embedding_plan::prepare(cva, limits, cancel, &mut report)?;
        check_cancel(cancel)?;
        if !cva.runtime_historical_embedding_supports_bounded(endpoint) {
            return Err(unsupported(
                "endpoint cannot enforce bounded request contract",
            ));
        }
        if cva.runtime_historical_embedding_dimensions(endpoint) != plan.profile.dimensions
            || cva.runtime_historical_embedding_normalization(endpoint)
                != plan.profile.normalization
        {
            return Err(unsupported("incompatible configured endpoint"));
        }
        let cell = std::cell::RefCell::new(report.clone());
        let compatibility = cva.runtime_historical_embedding_verify_endpoint(
            &plan.profile,
            endpoint,
            cancel,
            &cell,
            limits,
        );
        report = cell.borrow().clone();
        let compatibility = compatibility.map_err(|e| {
            if matches!(
                e,
                crate::CompatibilityProfileError::Endpoint(
                    crate::EmbeddingEndpointError::InvalidResponse(_)
                ) | crate::CompatibilityProfileError::InvalidProfile(_)
            ) {
                (HistoricalEmbeddingDisposition::InvalidOutput, e.to_string())
            } else {
                provider_stop(e, cancel)
            }
        })?;
        check_cancel(cancel)?;
        if !compatibility.compatible {
            return Err(unsupported("incompatible endpoint probes"));
        }
        plan.expected.verify(cva)?;
        for batch in plan.missing.chunks(limits.batch_inputs) {
            check_cancel(cancel)?;
            plan.expected.verify(cva)?;
            *cell.borrow_mut() = report.clone();
            let texts: Vec<_> = batch.iter().map(|row| row.text.clone()).collect();
            let vectors = cva.runtime_historical_embedding_batch(
                endpoint,
                cancel,
                &cell,
                limits,
                EmbeddingMode::Document,
                &texts,
            );
            report = cell.borrow().clone();
            let vectors = vectors.map_err(|e| {
                if matches!(e, crate::EmbeddingEndpointError::InvalidResponse(_)) {
                    (HistoricalEmbeddingDisposition::InvalidOutput, e.to_string())
                } else {
                    provider_stop(e, cancel)
                }
            })?;
            check_cancel(cancel)?;
            cva.runtime_historical_embedding_validate_batch(&plan.profile, &vectors, batch.len())
                .map_err(|e| (HistoricalEmbeddingDisposition::InvalidOutput, e.to_string()))?;
            if vectors
                .iter()
                .any(|v| !cva.runtime_historical_embedding_vector_is_usable(v))
            {
                return Err((
                    HistoricalEmbeddingDisposition::InvalidOutput,
                    "unusable genuine source row".into(),
                ));
            }
            plan.expected.verify(cva)?;
            for (row, components) in batch.iter().zip(vectors) {
                check_cancel(cancel)?;
                plan.rows.insert(
                    row.fragment.id.0,
                    components.into_iter().flat_map(f32::to_le_bytes).collect(),
                );
            }
            check_cancel(cancel)?;
            let schema = crate::VectorSchema::new(plan.profile.dimensions, crate::ScalarType::F32)
                .map_err(unsupported)?;
            let ids = plan.rows.keys().map(|id| crate::FragmentId(*id)).collect();
            let bytes = plan
                .rows
                .values()
                .flat_map(|row| row.iter().copied())
                .collect();
            let packed = crate::PackedVectors::from_bytes(schema, bytes).map_err(unsupported)?;
            report.write_attempted = true;
            let packed_id = cva.put_packed_vectors(packed).map_err(unsupported)?;
            check_cancel(cancel)?;
            let archive_id = cva
                .put_archive_vectors(packed_id, ids)
                .map_err(unsupported)?;
            check_cancel(cancel)?;
            let generation: VectorGeneration = cva
                .publish_vector_generation_without_live_semantic(
                    plan.profile.id,
                    archive_id,
                    cva.archive_version(),
                )
                .map_err(unsupported)?;
            report.generation_synced = false;
            report.final_generation = Some(generation.id);
            report.final_source_archive_version = Some(generation.source_archive_version);
            cva.runtime_sync_public_owner().map_err(unsupported)?;
            report.generation_synced = true;
            report.new_rows += batch.len();
            plan.expected = Expected::read(cva)?;
        }
        check_cancel(cancel)?;
        plan.expected.verify(cva)?;
        coverage(cva, &mut report)?;
        check_cancel(cancel)?;
        // Retained source evidence is now complete for the bounded fill lane.
        // Readiness/manifest planning starts again from current owner authority.
        cva.historical_reconstruction_owner().map_err(unsupported)?;
        report.disposition = HistoricalEmbeddingDisposition::Complete;
        Ok(())
    })();
    match result {
        Ok(()) => Ok(report),
        Err((disposition, message)) => {
            report.disposition = disposition;
            Err(HistoricalEmbeddingFailure { report, message })
        }
    }
}
pub(super) fn cancelled_error(
    report: HistoricalEmbeddingReport,
) -> super::reconstruction::HistoricalReconstructionError {
    let mut report = report;
    report.disposition = HistoricalEmbeddingDisposition::Cancelled;
    super::reconstruction::HistoricalReconstructionError::EmbeddingFill(Box::new(
        HistoricalEmbeddingFailure {
            report,
            message: "cancelled before checkpoint acceptance".into(),
        },
    ))
}
pub(super) fn manifest_pass(
    cva: &Cva,
    source_records: &[super::encoding::Value],
) -> Result<super::encoding::Value, CvaError> {
    let generation = cva.runtime_historical_latest_vector_generation();
    crate::imports::historical_manifest_plan::embedding_pass(generation, source_records)
        .map_err(CvaError::from)
}

pub(super) fn owner_error(
    error: CvaError,
    mut report: HistoricalEmbeddingReport,
) -> super::reconstruction::HistoricalReconstructionError {
    let message = error.to_string();
    report.disposition = if message.contains("ImportInputDrift") {
        HistoricalEmbeddingDisposition::InputDrift
    } else {
        HistoricalEmbeddingDisposition::Unsupported
    };
    super::reconstruction::HistoricalReconstructionError::EmbeddingFill(Box::new(
        HistoricalEmbeddingFailure { report, message },
    ))
}

/// Only the explicit offline embedding operation may use this temporary bypass.
/// Provider transport remains at the authorized endpoint adapter.
pub(super) fn fill_open_epoch<E: EmbeddingEndpoint>(
    cva: &mut Cva,
    endpoint: &E,
    limits: HistoricalEmbeddingLimits,
    cancel: &AtomicBool,
) -> Result<HistoricalEmbeddingReport, HistoricalEmbeddingFailure> {
    let mut scope = cva.runtime_historical_embedding_write_scope();
    fill(&mut scope, endpoint, limits, cancel)
}
