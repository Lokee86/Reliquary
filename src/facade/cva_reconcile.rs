use crate::Cva;
use crate::cva_reconcile_error::CvaReconcileError;
use crate::cva_reconcile_repack::reconcile_diverged;
use std::fs;
use std::path::Path;

pub use crate::reconciliation_values::{CvaComparison, CvaReconcileResult, CvaRelation};

impl Cva {
    pub fn compare(
        left_path: impl AsRef<Path>,
        right_path: impl AsRef<Path>,
    ) -> Result<CvaComparison, CvaReconcileError> {
        let left = Self::open_read_only(left_path)?;
        let right = Self::open_read_only(right_path)?;
        crate::maintenance::reconciliation_compare::compare(
            &left.container,
            &right.container,
            left.history_generation(),
            right.history_generation(),
        )
    }

    pub fn reconcile(
        left_path: impl AsRef<Path>,
        right_path: impl AsRef<Path>,
        output_path: impl AsRef<Path>,
    ) -> Result<CvaReconcileResult, CvaReconcileError> {
        Self::reconcile_held(left_path, right_path, output_path).map(|(result, _output)| result)
    }

    /// Reconcile and retain the validated output's writer authority for callers
    /// that publish the destination path atomically after this operation.
    pub(crate) fn reconcile_held(
        left_path: impl AsRef<Path>,
        right_path: impl AsRef<Path>,
        output_path: impl AsRef<Path>,
    ) -> Result<(CvaReconcileResult, Cva), CvaReconcileError> {
        let left_path = left_path.as_ref();
        let right_path = right_path.as_ref();
        let output_path = output_path.as_ref();

        // Keep shared OS leases on both inputs for the whole reconciliation.
        // `compare` and the owner-specific replay paths may open their own
        // short-lived readers, but these guards pin the exact source identities
        // through output validation and publication readiness.
        let left_guard = crate::Container::open_read_only_for_scan(left_path)?;
        let right_guard = crate::Container::open_read_only_for_scan(right_path)?;
        left_guard.validate_path_identity()?;
        right_guard.validate_path_identity()?;
        if output_path.exists() {
            return Err(CvaReconcileError::OutputExists);
        }

        let reconciled: Result<(CvaReconcileResult, Cva), CvaReconcileError> = (|| {
            let comparison = Self::compare(left_path, right_path)?;
            let source = match comparison.relation {
                CvaRelation::RightExtendsLeft => right_path,
                _ => left_path,
            };
            if comparison.relation != CvaRelation::Diverged {
                let output = copy_and_validate(source, output_path)?;
                return Ok((empty_result(comparison), output));
            }

            if let Some(reconciled) = crate::imports::reconcile_import_epochs(
                left_path,
                right_path,
                output_path,
                comparison.clone(),
            )? {
                let (result, (owner, due)) = reconciled;
                let output = Cva::from_reconstructed(owner, due);
                return Ok((result, output));
            }
            reconcile_diverged(left_path, right_path, output_path, comparison)
        })();

        match reconciled {
            Ok((result, output)) => {
                let source_validation = left_guard
                    .validate_path_identity()
                    .and_then(|()| right_guard.validate_path_identity());
                let output_validation = output.container.validate_path_identity();
                if source_validation.is_err() || output_validation.is_err() {
                    let cleanup_path = output_path.to_path_buf();
                    let output_is_still_owned = output_validation.is_ok();
                    if output_is_still_owned {
                        let _ = fs::remove_file(cleanup_path);
                    }
                    drop(output);
                    return Err(crate::ContainerError::UnstableSnapshot.into());
                }
                Ok((result, output))
            }
            Err(error) => Err(error),
        }
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

pub(crate) fn copy_and_validate(source: &Path, output: &Path) -> Result<Cva, CvaReconcileError> {
    let owner = crate::maintenance::reconciliation_copy::copy_and_validate(source, output)?;
    Cva::open_from_container(owner.container).map_err(Into::into)
}
