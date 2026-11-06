use super::*;
use crate::VerifiedOwnerCopyPlan;
use crate::staged_copy::StagedCopy;
use sha2::{Digest, Sha256};
use std::{fs, path::Path};

#[cfg(test)]
mod acceptance_tests;
mod replay;
#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RelHistoryRelation {
    Identical,
    LeftExtendsRight,
    RightExtendsLeft,
    Diverged,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RelHistoryComparison {
    pub owner_id: [u8; 16],
    pub history_generation: [u8; 32],
    pub common_record_count: usize,
    pub left_record_count: usize,
    pub right_record_count: usize,
    pub relation: RelHistoryRelation,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RelReconciliationError {
    DestinationExists,
    WrongFileKind,
    OwnerOrScopeMismatch,
    HistoryGenerationConflict,
    ActiveWriterOrIncompleteImport(String),
    UnsupportedState(&'static str),
    SemanticConflict(String),
    SourceChanged(&'static str),
    Verification(String),
}

impl fmt::Display for RelReconciliationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DestinationExists => f.write_str("reconciliation destination already exists"),
            Self::WrongFileKind => f.write_str("divergent reconciliation supports REL owners only"),
            Self::OwnerOrScopeMismatch => f.write_str("reconciliation owner or REL scope differs"),
            Self::HistoryGenerationConflict => {
                f.write_str("reconciliation history generations differ")
            }
            Self::ActiveWriterOrIncompleteImport(error) => f.write_str(error),
            Self::UnsupportedState(domain) => {
                write!(f, "unsupported divergent owner state: {domain}")
            }
            Self::SemanticConflict(error) => write!(f, "semantic reconciliation conflict: {error}"),
            Self::SourceChanged(side) => write!(f, "{side} reconciliation source changed"),
            Self::Verification(error) => write!(f, "reconciliation verification failed: {error}"),
        }
    }
}

impl std::error::Error for RelReconciliationError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RelReconciliationReport {
    pub comparison: RelHistoryComparison,
    pub replayed_archive_records: usize,
    pub replayed_memory_revisions: usize,
    pub duplicate_memory_revisions: usize,
    pub replayed_entity_revisions: usize,
    pub replayed_relationship_revisions: usize,
    pub replayed_graph_mutations: usize,
    pub replayed_freshness_records: usize,
    pub vector_rebuild_required: bool,
    pub output_digest: [u8; 32],
}

pub fn compare_rel_histories(
    left_path: &Path,
    right_path: &Path,
) -> Result<RelHistoryComparison, RelReconciliationError> {
    let (left_plan, left) = stable_rel(left_path)?;
    let (right_plan, right) = stable_rel(right_path)?;
    if left.identity() != right.identity() || left.owner_id() != right.owner_id() {
        return Err(RelReconciliationError::OwnerOrScopeMismatch);
    }
    let left_generation =
        owner_history_generation(left_path).map_err(RelReconciliationError::Verification)?;
    let right_generation =
        owner_history_generation(right_path).map_err(RelReconciliationError::Verification)?;
    if left_generation != right_generation {
        return Err(RelReconciliationError::HistoryGenerationConflict);
    }
    left_plan
        .verify_source(&left, left_path)
        .map_err(|_| RelReconciliationError::SourceChanged("left"))?;
    right_plan
        .verify_source(&right, right_path)
        .map_err(|_| RelReconciliationError::SourceChanged("right"))?;
    let left_records = semantic_records(&left)?;
    let right_records = semantic_records(&right)?;
    let common = left_records
        .iter()
        .zip(&right_records)
        .take_while(|(left, right)| left == right)
        .count();
    let relation = if common == left_records.len() && common == right_records.len() {
        RelHistoryRelation::Identical
    } else if common == right_records.len() {
        RelHistoryRelation::LeftExtendsRight
    } else if common == left_records.len() {
        RelHistoryRelation::RightExtendsLeft
    } else {
        RelHistoryRelation::Diverged
    };
    Ok(RelHistoryComparison {
        owner_id: left.owner_id(),
        history_generation: left_generation,
        common_record_count: common,
        left_record_count: left_records.len(),
        right_record_count: right_records.len(),
        relation,
    })
}

pub fn reconcile_divergent_rel_copy(
    left_path: &Path,
    right_path: &Path,
    destination_path: &Path,
    graph_admission: &GraphAdmission,
    accepted_episodes: &[AcceptedEpisodeOrder],
    dream_admissions: &[DreamBeginAdmission],
) -> Result<RelReconciliationReport, RelReconciliationError> {
    if destination_path.exists() {
        return Err(RelReconciliationError::DestinationExists);
    }
    let comparison = compare_rel_histories(left_path, right_path)?;
    if comparison.relation != RelHistoryRelation::Diverged {
        return Err(RelReconciliationError::SemanticConflict(
            "divergent executor received a prefix-comparable history".into(),
        ));
    }
    let left_plan = VerifiedOwnerCopyPlan::prepare(left_path)
        .map_err(RelReconciliationError::ActiveWriterOrIncompleteImport)?;
    let right_plan = VerifiedOwnerCopyPlan::prepare(right_path)
        .map_err(RelReconciliationError::ActiveWriterOrIncompleteImport)?;
    let left_container = Container::open_read_only_as(left_path, FileKind::Reliquary)
        .map_err(|error| RelReconciliationError::Verification(error.to_string()))?;
    let right_container = Container::open_read_only_as(right_path, FileKind::Reliquary)
        .map_err(|error| RelReconciliationError::Verification(error.to_string()))?;
    let left_records = semantic_records(&left_container)?;
    let right_records = semantic_records(&right_container)?;
    require_supported_tail(&left_records[comparison.common_record_count..])?;
    require_supported_tail(&right_records[comparison.common_record_count..])?;

    let left = RelSession::open(
        &left_container,
        graph_admission,
        accepted_episodes.iter().cloned(),
        dream_admissions.iter().cloned(),
    )
    .map_err(|error| RelReconciliationError::Verification(error.to_string()))?;
    let right = RelSession::open(
        &right_container,
        graph_admission,
        accepted_episodes.iter().cloned(),
        dream_admissions.iter().cloned(),
    )
    .map_err(|error| RelReconciliationError::Verification(error.to_string()))?;
    replay::require_compatible_fixed_state(&left, &right)?;

    let staged =
        StagedCopy::create(destination_path).map_err(RelReconciliationError::Verification)?;
    left_plan
        .copy_to(left_path, &staged.path)
        .map_err(RelReconciliationError::Verification)?;
    let counts = replay::replay_right(&left, &right, &staged.path)?;
    let merged_admission = replay::merged_graph_admission(&left, &right);
    let reopened = RelSession::open_path(
        &staged.path,
        &merged_admission,
        accepted_episodes.iter().cloned(),
        dream_admissions.iter().cloned(),
    )
    .map_err(|error| RelReconciliationError::Verification(error.to_string()))?;
    replay::verify_merged(&left, &right, &reopened)?;
    left_plan
        .verify_source(&left_container, left_path)
        .map_err(|_| RelReconciliationError::SourceChanged("left"))?;
    right_plan
        .verify_source(&right_container, right_path)
        .map_err(|_| RelReconciliationError::SourceChanged("right"))?;
    let bytes = fs::read(&staged.path)
        .map_err(|error| RelReconciliationError::Verification(error.to_string()))?;
    let output_digest = Sha256::digest(&bytes).into();
    fs::rename(&staged.path, destination_path)
        .map_err(|error| RelReconciliationError::Verification(error.to_string()))?;
    Ok(RelReconciliationReport {
        comparison,
        replayed_archive_records: counts.archive,
        replayed_memory_revisions: counts.memory,
        duplicate_memory_revisions: counts.memory_duplicates,
        replayed_entity_revisions: counts.entities,
        replayed_relationship_revisions: counts.relationships,
        replayed_graph_mutations: counts.graph,
        replayed_freshness_records: counts.freshness,
        vector_rebuild_required: counts.semantic_change && replay::has_vectors(&left, &right),
        output_digest,
    })
}

fn stable_rel(path: &Path) -> Result<(VerifiedOwnerCopyPlan, Container), RelReconciliationError> {
    let plan = VerifiedOwnerCopyPlan::prepare(path)
        .map_err(RelReconciliationError::ActiveWriterOrIncompleteImport)?;
    if !plan.is_reliquary() {
        return Err(RelReconciliationError::WrongFileKind);
    }
    let container = Container::open_read_only_as(path, FileKind::Reliquary)
        .map_err(|error| RelReconciliationError::Verification(error.to_string()))?;
    Ok((plan, container))
}

fn semantic_records(container: &Container) -> Result<Vec<Vec<u8>>, RelReconciliationError> {
    container
        .read_frames()
        .map_err(|error| RelReconciliationError::Verification(error.to_string()))?
        .into_iter()
        .map(|frame| {
            let publication = reliquary_container::decode_publication(&frame.payload)
                .map_err(|error| RelReconciliationError::Verification(error.to_string()))?;
            Ok(publication.map_or_else(|| frame.payload.clone(), |value| value.body.to_vec()))
        })
        .collect()
}

fn require_supported_tail(records: &[Vec<u8>]) -> Result<(), RelReconciliationError> {
    const SUPPORTED: [&[u8]; 19] = [
        b"RELTURN2",
        b"RELFILE1",
        b"RELBRCH1",
        b"RELEPIS1",
        b"CVAIMPT1",
        b"CVAFRG01",
        b"RELMFMT1",
        b"RELMBOD1",
        b"RELMCRT1",
        b"RELMDLT1",
        b"RELMTMB1",
        b"RELPER01",
        b"RELETM01",
        b"RELRLP01",
        b"RELGRF01",
        b"RELFRS02",
        b"RELFRP01",
        b"RELECHO1",
        b"RELINIT1",
    ];
    for record in records {
        if !SUPPORTED.iter().any(|magic| record.starts_with(magic)) {
            return Err(RelReconciliationError::UnsupportedState(
                "record family lacks an exact replay contract",
            ));
        }
    }
    Ok(())
}
