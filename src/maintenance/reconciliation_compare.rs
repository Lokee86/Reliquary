//! Generic owner comparison policy over validated immutable physical sources.
use crate::cva_reconcile_error::CvaReconcileError;
use crate::reconciliation_values::{CvaComparison, CvaRelation};
pub(crate) fn compare(
    left: &crate::Container,
    right: &crate::Container,
    left_generation: crate::HistoryGenerationId,
    right_generation: crate::HistoryGenerationId,
) -> Result<CvaComparison, CvaReconcileError> {
    let left_id = owner_id(left, "left")?;
    let right_id = owner_id(right, "right")?;
    if left_id != right_id {
        return Err(CvaReconcileError::OwnerMismatch {
            left: left_id,
            right: right_id,
        });
    }
    if left_generation != right_generation {
        return Err(CvaReconcileError::HistoryGenerationConflict {
            left: left_generation,
            right: right_generation,
        });
    }

    let left_chunks = semantic_chunk_payloads(left)?;
    let right_chunks = semantic_chunk_payloads(right)?;
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
fn owner_id(cva: &crate::Container, side: &'static str) -> Result<String, CvaReconcileError> {
    cva.owner_id()
        .ok_or(CvaReconcileError::MissingOwnerId(side))
}

fn semantic_chunk_payloads(cva: &crate::Container) -> Result<Vec<Vec<u8>>, CvaReconcileError> {
    let chunks = cva.chunk_refs()?;
    let mut payloads = Vec::with_capacity(chunks.len());
    for chunk in chunks {
        let payload = cva.read(chunk)?;
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
