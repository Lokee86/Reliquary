//! Bounded operation settings and transient coverage; no persistence authority.
use crate::{CompatibilityProfileId, VectorGenerationId};
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HistoricalEmbeddingLimits {
    pub sources: usize,
    pub fragments: usize,
    pub generation_rows: usize,
    pub missing_rows: usize,
    pub source_bytes: usize,
    pub text_bytes: usize,
    pub dimensions: usize,
    pub matrix_bytes: usize,
    pub components: usize,
    pub inspections: usize,
    pub temporary_bytes: usize,
    pub batch_inputs: usize,
    pub source_calls: usize,
}
impl Default for HistoricalEmbeddingLimits {
    fn default() -> Self {
        Self {
            sources: 65_536,
            fragments: 131_072,
            generation_rows: 131_072,
            missing_rows: 4_096,
            source_bytes: 256 * 1024,
            text_bytes: 16 * 1024 * 1024,
            dimensions: 4_096,
            matrix_bytes: 256 * 1024 * 1024,
            components: 67_108_864,
            inspections: 67_108_864,
            temporary_bytes: 512 * 1024 * 1024,
            batch_inputs: 16,
            source_calls: 256,
        }
    }
}
impl HistoricalEmbeddingLimits {
    pub(super) fn validate(self) -> Result<(), String> {
        let cap = Self::default();
        let pairs = [
            (self.sources, cap.sources),
            (self.fragments, cap.fragments),
            (self.generation_rows, cap.generation_rows),
            (self.missing_rows, cap.missing_rows),
            (self.source_bytes, cap.source_bytes),
            (self.text_bytes, cap.text_bytes),
            (self.dimensions, cap.dimensions),
            (self.matrix_bytes, cap.matrix_bytes),
            (self.components, cap.components),
            (self.inspections, cap.inspections),
            (self.temporary_bytes, cap.temporary_bytes),
            (self.batch_inputs, cap.batch_inputs),
            (self.source_calls, cap.source_calls),
        ];
        if pairs.iter().any(|&(n, max)| n == 0 || n > max) {
            return Err("zero or above-ceiling historical embedding limit".into());
        }
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HistoricalEmbeddingDisposition {
    Complete,
    Cancelled,
    ProviderError,
    InvalidOutput,
    BudgetExceeded,
    InputDrift,
    Unsupported,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HistoricalEmbeddingCoverage {
    pub fragment_id: crate::FragmentId,
    pub source_keys: Vec<(String, String)>,
    pub position: u64,
    pub reason: String,
    pub usable_rows: usize,
    pub mixed_fragments: usize,
    pub invalid_rows: usize,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HistoricalEmbeddingReport {
    pub disposition: HistoricalEmbeddingDisposition,
    pub owner_uuid: Option<[u8; 16]>,
    pub import_id: Option<[u8; 32]>,
    pub profile_id: Option<CompatibilityProfileId>,
    pub initial_generation: Option<VectorGenerationId>,
    pub final_generation: Option<VectorGenerationId>,
    pub initial_source_archive_version: Option<u64>,
    pub final_source_archive_version: Option<u64>,
    pub sources: usize,
    pub fragments: usize,
    pub missing_rows: usize,
    pub reused_rows: usize,
    pub new_rows: usize,
    /// Always zero: fill never creates fragment records.
    pub new_fragments: usize,
    /// A physical write was attempted; interrupted staging may survive reopen.
    pub write_attempted: bool,
    /// True after the final reported generation was synchronized.
    pub generation_synced: bool,
    pub attempted_calls: usize,
    pub completed_calls: usize,
    pub attempted_inputs: usize,
    pub completed_inputs: usize,
    pub text_bytes: usize,
    pub unavailable_fragments: usize,
    /// Present only after a full retained-coverage scan. Empty on an early stop.
    pub coverage: Vec<HistoricalEmbeddingCoverage>,
}
impl Default for HistoricalEmbeddingReport {
    fn default() -> Self {
        Self {
            disposition: HistoricalEmbeddingDisposition::Unsupported,
            owner_uuid: None,
            import_id: None,
            profile_id: None,
            initial_generation: None,
            final_generation: None,
            initial_source_archive_version: None,
            final_source_archive_version: None,
            sources: 0,
            fragments: 0,
            missing_rows: 0,
            reused_rows: 0,
            new_rows: 0,
            new_fragments: 0,
            write_attempted: false,
            generation_synced: false,
            attempted_calls: 0,
            completed_calls: 0,
            attempted_inputs: 0,
            completed_inputs: 0,
            text_bytes: 0,
            unavailable_fragments: 0,
            coverage: Vec::new(),
        }
    }
}
#[derive(Clone, Debug)]
pub struct HistoricalEmbeddingFailure {
    pub report: HistoricalEmbeddingReport,
    pub message: String,
}
impl std::fmt::Display for HistoricalEmbeddingFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "historical embedding {:?}: {}",
            self.report.disposition, self.message
        )
    }
}
impl std::error::Error for HistoricalEmbeddingFailure {}
pub(super) type Stop = (HistoricalEmbeddingDisposition, String);
pub(super) fn budget(message: &str) -> Stop {
    (
        HistoricalEmbeddingDisposition::BudgetExceeded,
        message.into(),
    )
}
pub(super) fn unsupported(error: impl std::fmt::Display) -> Stop {
    (
        HistoricalEmbeddingDisposition::Unsupported,
        error.to_string(),
    )
}
