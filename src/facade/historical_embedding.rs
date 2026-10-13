//! Exclusive offline historical fill. Provider callbacks never hold runtime locks.
use super::embedding_model::*;
use super::embedding_plan;
use super::{fail, file_digest};
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
        Ok(Self {
            global: cva.latest_global_version(),
            bytes: std::fs::metadata(cva.container.path())
                .map_err(unsupported)?
                .len(),
            digest: file_digest(cva.container.path()).map_err(unsupported)?,
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
struct Provider<'a, E> {
    endpoint: &'a E,
    cancel: &'a AtomicBool,
    report: &'a std::cell::RefCell<HistoricalEmbeddingReport>,
    limits: HistoricalEmbeddingLimits,
}
impl<E: EmbeddingEndpoint> EmbeddingEndpoint for Provider<'_, E> {
    fn dimensions(&self) -> u32 {
        self.endpoint.dimensions()
    }
    fn normalization(&self) -> crate::VectorNormalization {
        self.endpoint.normalization()
    }
    fn embed(
        &self,
        mode: EmbeddingMode,
        inputs: &[String],
    ) -> Result<Vec<Vec<f32>>, crate::EmbeddingEndpointError> {
        let failure = |s: &str| crate::EmbeddingEndpointError::Failure(s.into());
        if self.cancel.load(Ordering::Acquire) {
            return Err(failure("historical fill cancelled"));
        }
        let bytes = inputs
            .iter()
            .try_fold(0usize, |n, s| n.checked_add(s.len()))
            .ok_or_else(|| failure("historical byte budget overflow"))?;
        {
            let mut report = self.report.borrow_mut();
            if inputs.len() > self.limits.batch_inputs.max(2)
                || report.attempted_calls >= self.limits.source_calls + 2
                || report.attempted_inputs + inputs.len() > self.limits.missing_rows + 4
                || report
                    .text_bytes
                    .checked_add(bytes)
                    .is_none_or(|n| n > self.limits.text_bytes)
            {
                return Err(failure("historical provider budget exceeded"));
            }
            report.attempted_calls += 1;
            report.attempted_inputs += inputs.len();
            report.text_bytes += bytes;
        }
        // Generous JSON overhead allowance, still fixed before transport allocation.
        let cap = inputs
            .len()
            .checked_mul(self.dimensions() as usize)
            .and_then(|n| n.checked_mul(64))
            .and_then(|n| n.checked_add(4096))
            .ok_or_else(|| failure("historical response budget overflow"))?;
        let vectors = self.endpoint.embed_bounded(mode, inputs, cap)?;
        if vectors.len() != inputs.len()
            || vectors
                .iter()
                .any(|v| v.len() != self.dimensions() as usize)
        {
            return Err(crate::EmbeddingEndpointError::InvalidResponse(
                "historical output shape",
            ));
        }
        let mut report = self.report.borrow_mut();
        report.completed_calls += 1;
        report.completed_inputs += inputs.len();
        Ok(vectors)
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
    let generation = cva
        .vector_generations
        .generations()
        .iter()
        .max_by_key(|g| g.vector_version)
        .copied();
    let timeline = cva.historical_freshness_timeline().map_err(unsupported)?;
    report.coverage.clear();
    report.unavailable_sources = 0;
    for (index, key) in timeline.activity_order.iter().enumerate() {
        let node = cva
            .archive
            .nodes
            .get(&key.conversation_id, &key.node_id)
            .cloned()
            .ok_or_else(|| unsupported("missing source"))?;
        let (reason, usable_rows, mixed_fragments, invalid_rows) = match cva
            .prepare_accepted_semantic_source(&node, generation.map(|g| g.id))
            .map_err(unsupported)?
        {
            crate::cva_semantic_source::SemanticSourceReadiness::Pending => {
                report.unavailable_sources += 1;
                ("missing-or-incomplete-retained-source-evidence", 0, 0, 0)
            }
            crate::cva_semantic_source::SemanticSourceReadiness::Ready(source) => (
                if source.vectors.is_empty() {
                    "complete-unusable"
                } else {
                    "complete-usable"
                },
                source.vectors.len(),
                source.excluded_mixed_fragments,
                source.invalid_vectors,
            ),
        };
        report.coverage.push(HistoricalEmbeddingCoverage {
            conversation_id: key.conversation_id.clone(),
            node_id: key.node_id.clone(),
            position: index as u64 + 1,
            reason: reason.into(),
            usable_rows,
            mixed_fragments,
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
        if !endpoint.supports_bounded_embedding() {
            return Err(unsupported(
                "endpoint cannot enforce bounded request contract",
            ));
        }
        if endpoint.dimensions() != plan.profile.dimensions
            || endpoint.normalization() != plan.profile.normalization
        {
            return Err(unsupported("incompatible configured endpoint"));
        }
        let cell = std::cell::RefCell::new(report.clone());
        let provider = Provider {
            endpoint,
            cancel,
            report: &cell,
            limits,
        };
        let compatibility =
            crate::compatibility_profile_probe::verify_endpoint(&plan.profile, &provider);
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
            let vectors = provider.embed(EmbeddingMode::Document, &texts);
            report = cell.borrow().clone();
            let vectors = vectors.map_err(|e| {
                if matches!(e, crate::EmbeddingEndpointError::InvalidResponse(_)) {
                    (HistoricalEmbeddingDisposition::InvalidOutput, e.to_string())
                } else {
                    provider_stop(e, cancel)
                }
            })?;
            check_cancel(cancel)?;
            crate::compatibility_profile_probe::validate_embedding_batch(
                &plan.profile,
                &vectors,
                batch.len(),
            )
            .map_err(|e| (HistoricalEmbeddingDisposition::InvalidOutput, e.to_string()))?;
            if vectors
                .iter()
                .any(|v| crate::freshness::semantic_policy::cosine_similarity(v, v).is_err())
            {
                return Err((
                    HistoricalEmbeddingDisposition::InvalidOutput,
                    "unusable genuine source row".into(),
                ));
            }
            plan.expected.verify(cva)?;
            for (row, components) in batch.iter().zip(vectors) {
                check_cancel(cancel)?;
                if row.materialize {
                    report.write_attempted = true;
                    if cva
                        .archive
                        .put_fragment(&mut cva.container, row.fragment.clone())
                        .map_err(unsupported)?
                    {
                        report.new_fragments += 1;
                    }
                }
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
                .publish_vector_generation(plan.profile.id, archive_id, cva.archive_version())
                .map_err(unsupported)?;
            report.generation_synced = false;
            report.final_generation = Some(generation.id);
            report.final_source_archive_version = Some(generation.source_archive_version);
            cva.container.sync().map_err(unsupported)?;
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
    use super::encoding::Value;
    let generation = cva
        .vector_generations
        .generations()
        .iter()
        .max_by_key(|g| g.vector_version)
        .copied()
        .ok_or_else(|| fail("completed fill lacks retained generation"))?;
    // Source entries include a derived coverage_digest; the fill digest commits
    // exactly node/position/mode/evidence, without hashing that derivative twice.
    let records = source_records
        .iter()
        .map(|record| match record {
            Value::Map(fields) => Ok(Value::Map(
                fields
                    .iter()
                    .filter(|(k, _)| k != "coverage_digest")
                    .cloned()
                    .collect(),
            )),
            _ => Err(fail("malformed embedding source coverage")),
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Value::map([
        ("contract_version", Value::Unsigned(1)),
        ("disposition", Value::text("complete")),
        (
            "profile_id",
            Value::bytes(generation.compatibility_profile_id.0),
        ),
        ("selected_generation_id", Value::bytes(generation.id.0)),
        (
            "source_archive_version",
            Value::Unsigned(generation.source_archive_version),
        ),
        (
            "coverage_digest",
            Value::bytes(
                Value::Array(records).digest("reliquary:historical-embedding-coverage:v1")?,
            ),
        ),
    ]))
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
