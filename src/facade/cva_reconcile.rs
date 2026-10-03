use crate::Cva;
use crate::cva_reconcile_error::CvaReconcileError;
use crate::cva_reconcile_repack::reconcile_diverged;
use std::fs;
use std::path::Path;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CvaRelation {
    Identical,
    LeftExtendsRight,
    RightExtendsLeft,
    Diverged,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CvaComparison {
    pub owner_id: String,
    pub common_chunk_count: usize,
    pub left_chunk_count: usize,
    pub right_chunk_count: usize,
    pub relation: CvaRelation,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CvaReconcileResult {
    pub comparison: CvaComparison,
    pub replayed_archive_records: usize,
    pub replayed_memory_revisions: usize,
    pub duplicate_memory_revisions: usize,
    pub replayed_entity_revisions: usize,
    pub duplicate_entity_revisions: usize,
    pub replayed_relationship_revisions: usize,
    pub duplicate_relationship_revisions: usize,
    pub replayed_insomnia_completions: usize,
    pub replayed_graph_transactions: usize,
    pub replayed_graph_mutations: usize,
    pub duplicate_graph_mutations: usize,
    pub replayed_file_memory_links: usize,
    pub canonical_change_required: bool,
    pub vector_rebuild_required: bool,
}

impl Cva {
    pub fn compare(
        left_path: impl AsRef<Path>,
        right_path: impl AsRef<Path>,
    ) -> Result<CvaComparison, CvaReconcileError> {
        let mut left = Self::open(left_path)?;
        let mut right = Self::open(right_path)?;
        let left_id = owner_id(&left, "left")?;
        let right_id = owner_id(&right, "right")?;
        if left_id != right_id {
            return Err(CvaReconcileError::OwnerMismatch {
                left: left_id,
                right: right_id,
            });
        }

        let left_chunks = semantic_chunk_payloads(&mut left)?;
        let right_chunks = semantic_chunk_payloads(&mut right)?;
        let common_chunk_count = left_chunks
            .iter()
            .zip(&right_chunks)
            .take_while(|(left, right)| left == right)
            .count();
        let left_chunk_count = left_chunks.len();
        let right_chunk_count = right_chunks.len();
        let relation = classify(common_chunk_count, left_chunk_count, right_chunk_count);
        Ok(CvaComparison {
            owner_id: left_id,
            common_chunk_count,
            left_chunk_count,
            right_chunk_count,
            relation,
        })
    }

    pub fn reconcile(
        left_path: impl AsRef<Path>,
        right_path: impl AsRef<Path>,
        output_path: impl AsRef<Path>,
    ) -> Result<CvaReconcileResult, CvaReconcileError> {
        let left_path = left_path.as_ref();
        let right_path = right_path.as_ref();
        let output_path = output_path.as_ref();
        if output_path.exists() {
            return Err(CvaReconcileError::OutputExists);
        }

        let comparison = Self::compare(left_path, right_path)?;
        let source = match comparison.relation {
            CvaRelation::RightExtendsLeft => right_path,
            _ => left_path,
        };
        if comparison.relation != CvaRelation::Diverged {
            copy_and_validate(source, output_path)?;
            return Ok(empty_result(comparison));
        }

        reconcile_diverged(left_path, right_path, output_path, comparison)
    }
}

fn empty_result(comparison: CvaComparison) -> CvaReconcileResult {
    let canonical_change_required = comparison.relation == CvaRelation::RightExtendsLeft;
    CvaReconcileResult {
        comparison,
        replayed_archive_records: 0,
        replayed_memory_revisions: 0,
        duplicate_memory_revisions: 0,
        replayed_entity_revisions: 0,
        duplicate_entity_revisions: 0,
        replayed_relationship_revisions: 0,
        duplicate_relationship_revisions: 0,
        replayed_insomnia_completions: 0,
        replayed_graph_transactions: 0,
        replayed_graph_mutations: 0,
        duplicate_graph_mutations: 0,
        replayed_file_memory_links: 0,
        canonical_change_required,
        vector_rebuild_required: false,
    }
}

fn owner_id(cva: &Cva, side: &'static str) -> Result<String, CvaReconcileError> {
    cva.owner_id()
        .ok_or(CvaReconcileError::MissingOwnerId(side))
}

fn semantic_chunk_payloads(cva: &mut Cva) -> Result<Vec<Vec<u8>>, CvaReconcileError> {
    let chunks = cva.container.chunks()?;
    let mut payloads = Vec::with_capacity(chunks.len());
    for chunk in chunks {
        let payload = cva.container.read(chunk)?;
        if !crate::conversation_compaction_codec::is_compaction_payload(&payload) {
            payloads.push(payload);
        }
    }
    Ok(payloads)
}

fn classify(common: usize, left: usize, right: usize) -> CvaRelation {
    if common == left && common == right {
        CvaRelation::Identical
    } else if common == right {
        CvaRelation::LeftExtendsRight
    } else if common == left {
        CvaRelation::RightExtendsLeft
    } else {
        CvaRelation::Diverged
    }
}

pub(crate) fn copy_and_validate(source: &Path, output: &Path) -> Result<(), CvaReconcileError> {
    fs::copy(source, output)?;
    let validated = (|| {
        let source = Cva::open(source)?;
        let reopened = Cva::open(output)?;
        if !source.archive.activity_history_matches(&reopened.archive) {
            return Err(CvaReconcileError::Archive(
                crate::ArchiveError::CorruptRecord("copied REL activity history changed"),
            ));
        }
        Ok(())
    })();
    if validated.is_err() {
        let _ = fs::remove_file(output);
    }
    validated
}

pub(crate) fn with_replayed_transaction_time<T, E>(
    cva: &mut Cva,
    transaction_time_ns: Option<i64>,
    operation: impl FnOnce(&mut Cva) -> Result<T, E>,
) -> Result<T, E> {
    cva.container
        .set_next_transaction_time_override(transaction_time_ns);
    let result = operation(cva);
    cva.container.clear_next_transaction_time_override();
    result
}
