use crate::{
    Cva, CvaRelation, EmbeddingEndpoint, EmbeddingEndpointError, EmbeddingMode, VectorNormalization,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RelReconciliationReport {
    pub canonical_changed: bool,
    pub vector_rebuild_required: bool,
    pub vector_rebuild_completed: bool,
    pub candidates: Vec<RelReconciliationCandidateReport>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RelReconciliationCandidateReport {
    pub path: String,
    pub status: String,
    pub relation: Option<String>,
    pub conflict_kind: Option<String>,
    pub message: Option<String>,
}

struct SharedEmbeddingEndpoint(Arc<dyn EmbeddingEndpoint + Send + Sync>);

impl EmbeddingEndpoint for SharedEmbeddingEndpoint {
    fn dimensions(&self) -> u32 {
        self.0.dimensions()
    }

    fn normalization(&self) -> VectorNormalization {
        self.0.normalization()
    }

    fn embed(
        &self,
        mode: EmbeddingMode,
        inputs: &[String],
    ) -> Result<Vec<Vec<f32>>, EmbeddingEndpointError> {
        self.0.embed(mode, inputs)
    }
}

pub(crate) fn reconcile_rel_file_unmounted(
    canonical: &Path,
    endpoint: Option<&Arc<dyn EmbeddingEndpoint + Send + Sync>>,
) -> Result<RelReconciliationReport, String> {
    let mut report = reconcile_rel_siblings(canonical)?;
    recover_reconciliation_vectors(canonical, &mut report, endpoint)?;
    Ok(report)
}

fn reconcile_rel_siblings(canonical: &Path) -> Result<RelReconciliationReport, String> {
    let canonical_cva = Cva::open(canonical).map_err(|error| error.to_string())?;
    let owner_id = canonical_cva
        .owner_id()
        .ok_or_else(|| "Reliquary does not have durable owner identity".to_string())?;
    drop(canonical_cva);

    let mut candidates = discover_candidates(canonical, &owner_id)?;
    candidates.sort();
    let mut report = RelReconciliationReport::default();
    for candidate in candidates {
        reconcile_candidate(canonical, &candidate, &mut report);
    }
    Ok(report)
}

fn discover_candidates(canonical: &Path, owner_id: &str) -> Result<Vec<PathBuf>, String> {
    let parent = canonical.parent().unwrap_or_else(|| Path::new("."));
    let canonical_real = fs::canonicalize(canonical).map_err(|error| error.to_string())?;
    let extension = canonical.extension().map(|value| value.to_os_string());
    let mut matches = Vec::new();

    for entry in fs::read_dir(parent).map_err(|error| error.to_string())? {
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => continue,
        };
        let path = entry.path();
        if !entry.file_type().is_ok_and(|kind| kind.is_file())
            || !same_extension(&path, extension.as_deref())
        {
            continue;
        }
        if fs::canonicalize(&path).is_ok_and(|real| real == canonical_real) {
            continue;
        }
        let Ok(candidate) = Cva::open(&path) else {
            continue;
        };
        if candidate.owner_id().as_deref() == Some(owner_id) {
            matches.push(path);
        }
    }
    Ok(matches)
}

fn reconcile_candidate(canonical: &Path, candidate: &Path, report: &mut RelReconciliationReport) {
    let comparison = match Cva::compare(canonical, candidate) {
        Ok(comparison) => comparison,
        Err(error) => {
            report
                .candidates
                .push(error_report(candidate, error.to_string()));
            return;
        }
    };
    let relation = relation_name(comparison.relation).to_string();
    match comparison.relation {
        CvaRelation::Identical => {
            report
                .candidates
                .push(candidate_report(candidate, "identical", relation))
        }
        CvaRelation::LeftExtendsRight => report
            .candidates
            .push(candidate_report(candidate, "stale", relation)),
        CvaRelation::RightExtendsLeft | CvaRelation::Diverged => {
            match Cva::reconcile_and_promote(canonical, candidate) {
                Ok(result) => {
                    report.canonical_changed |= result.canonical_change_required;
                    report.vector_rebuild_required |= result.vector_rebuild_required;
                    let status = if !result.canonical_change_required {
                        "absorbed"
                    } else if comparison.relation == CvaRelation::RightExtendsLeft {
                        "fast_forwarded"
                    } else {
                        "merged"
                    };
                    report
                        .candidates
                        .push(candidate_report(candidate, status, relation));
                }
                Err(error) => {
                    if let Some(conflict) = error.conflict() {
                        report.candidates.push(RelReconciliationCandidateReport {
                            path: candidate.display().to_string(),
                            status: "conflict".into(),
                            relation: Some(relation),
                            conflict_kind: Some(conflict.kind().into()),
                            message: Some(error.to_string()),
                        });
                    } else {
                        report
                            .candidates
                            .push(error_report(candidate, error.to_string()));
                    }
                }
            }
        }
    }
}

fn recover_reconciliation_vectors(
    path: &Path,
    report: &mut RelReconciliationReport,
    endpoint: Option<&Arc<dyn EmbeddingEndpoint + Send + Sync>>,
) -> Result<(), String> {
    let mut cva = Cva::open(path).map_err(|error| error.to_string())?;
    if !report.vector_rebuild_required && has_retired_vector_state(&cva) {
        report.vector_rebuild_required = true;
    }
    if !report.vector_rebuild_required {
        return Ok(());
    }
    let Some(endpoint) = endpoint else {
        return Ok(());
    };

    let endpoint = SharedEmbeddingEndpoint(Arc::clone(endpoint));
    let recovery = cva
        .rebuild_derived_vectors(&endpoint)
        .map_err(|error| format!("reconciliation vector recovery failed: {error}"))?;
    let memory_count = cva.memory_stats().memories;
    let has_fragments = !cva.fragments().is_empty();
    cva.sync().map_err(|error| error.to_string())?;
    drop(cva);

    let reopened = Cva::open(path).map_err(|error| {
        format!("reconciliation vector recovery validation failed on reopen: {error}")
    })?;
    if memory_count > 0 && reopened.memory_vector_stats().bindings < memory_count {
        return Err("reconciliation vector recovery left Memory vectors incomplete".into());
    }
    if has_fragments {
        let generation = reopened
            .current_vector_generation(recovery.compatibility_profile_id)
            .ok_or_else(|| {
                "reconciliation vector recovery did not publish an Archive vector generation"
                    .to_string()
            })?;
        if generation.source_archive_version != reopened.archive_version() {
            return Err(
                "reconciliation vector recovery published a stale Archive vector generation".into(),
            );
        }
    }

    report.vector_rebuild_completed = true;
    Ok(())
}

fn has_retired_vector_state(cva: &Cva) -> bool {
    !cva.active_memory_compatibility_profiles().is_empty()
        && (cva.memory_stats().memories > 0 || !cva.fragments().is_empty())
        && cva.packed_vector_stats().objects == 0
        && cva.memory_vector_stats().objects == 0
        && cva.archive_vector_stats().objects == 0
        && cva.vector_generation_stats().generations == 0
}

fn same_extension(path: &Path, extension: Option<&std::ffi::OsStr>) -> bool {
    match (path.extension(), extension) {
        (Some(left), Some(right)) => left.eq_ignore_ascii_case(right),
        (None, None) => true,
        _ => false,
    }
}

fn relation_name(relation: CvaRelation) -> &'static str {
    match relation {
        CvaRelation::Identical => "identical",
        CvaRelation::LeftExtendsRight => "canonical_extends_candidate",
        CvaRelation::RightExtendsLeft => "candidate_extends_canonical",
        CvaRelation::Diverged => "diverged",
    }
}

fn candidate_report(
    path: &Path,
    status: &str,
    relation: String,
) -> RelReconciliationCandidateReport {
    RelReconciliationCandidateReport {
        path: path.display().to_string(),
        status: status.into(),
        relation: Some(relation),
        conflict_kind: None,
        message: None,
    }
}

fn error_report(path: &Path, message: String) -> RelReconciliationCandidateReport {
    RelReconciliationCandidateReport {
        path: path.display().to_string(),
        status: "error".into(),
        relation: None,
        conflict_kind: None,
        message: Some(message),
    }
}
