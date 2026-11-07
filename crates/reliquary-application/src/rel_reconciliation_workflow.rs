use crate::verified_copy_workflow::copy_and_reopen_rel_owner;
use reliquary_owner_session::{
    AcceptedEpisodeOrder, DreamBeginAdmission, GraphAdmission, RelHistoryComparison,
    RelHistoryRelation, RelReconciliationReport, RelSession, VerifiedOwnerCopyPlan,
    VerifiedOwnerCopyReceipt, compare_rel_histories, reconcile_divergent_rel_copy,
};
use std::path::Path;

pub enum RelReconciliationEvidence {
    ExactCopy(VerifiedOwnerCopyReceipt),
    SemanticReplay(RelReconciliationReport),
}

pub struct ReconciledRelOwner {
    pub comparison: RelHistoryComparison,
    pub evidence: RelReconciliationEvidence,
    pub session: RelSession,
}

/// Reconcile any supported same-owner REL pair. Identical and strict-extension
/// histories retain the exact-copy path; only true divergence is replayed.
pub fn reconcile_rel_owner(
    left: &Path,
    right: &Path,
    destination: &Path,
    graph_admission: &GraphAdmission,
    accepted_episodes: impl IntoIterator<Item = AcceptedEpisodeOrder>,
    dream_admissions: impl IntoIterator<Item = DreamBeginAdmission>,
) -> Result<ReconciledRelOwner, String> {
    let accepted_episodes = accepted_episodes.into_iter().collect::<Vec<_>>();
    let dream_admissions = dream_admissions.into_iter().collect::<Vec<_>>();
    let comparison = compare_rel_histories(left, right).map_err(|error| error.to_string())?;
    for path in [left, right] {
        RelSession::open_path(
            path,
            graph_admission,
            accepted_episodes.iter().cloned(),
            dream_admissions.iter().cloned(),
        )
        .map_err(|error| format!("REL reconciliation owner admission failed: {error}"))?;
    }
    if comparison.relation == RelHistoryRelation::Diverged {
        let report = reconcile_divergent_rel_copy(
            left,
            right,
            destination,
            graph_admission,
            &accepted_episodes,
            &dream_admissions,
        )
        .map_err(|error| error.to_string())?;
        let destination_dream_admissions = report.destination_dream_admissions.clone();
        let session = RelSession::open_path(
            destination,
            graph_admission,
            accepted_episodes,
            dream_admissions
                .into_iter()
                .chain(destination_dream_admissions),
        )
        .map_err(|error| format!("independent reconciled REL reopen failed: {error}"))?;
        return Ok(ReconciledRelOwner {
            comparison,
            evidence: RelReconciliationEvidence::SemanticReplay(report),
            session,
        });
    }
    let source = match comparison.relation {
        RelHistoryRelation::RightExtendsLeft => right,
        RelHistoryRelation::Identical | RelHistoryRelation::LeftExtendsRight => left,
        RelHistoryRelation::Diverged => unreachable!(),
    };
    let plan = VerifiedOwnerCopyPlan::prepare(source)?;
    let copied = copy_and_reopen_rel_owner(
        &plan,
        source,
        destination,
        graph_admission,
        accepted_episodes,
        dream_admissions,
    )?;
    Ok(ReconciledRelOwner {
        comparison,
        evidence: RelReconciliationEvidence::ExactCopy(copied.receipt),
        session: copied.session,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use reliquary_container::ReliquaryScope;
    use reliquary_owner_session::RelWriter;
    use std::{
        fs,
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
    };

    static NEXT: AtomicU64 = AtomicU64::new(1);

    struct Temp(PathBuf);
    impl Temp {
        fn new(tag: &str) -> Self {
            let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".test-tmp");
            fs::create_dir_all(&directory).unwrap();
            Self(directory.join(format!(
                "rel-reconcile-{tag}-{}-{}.rel",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            )))
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = fs::remove_file(&self.0);
        }
    }

    #[test]
    fn identical_rel_uses_exact_copy_evidence_and_reopens() {
        let left = Temp::new("left");
        let right = Temp::new("right");
        let output = Temp::new("output");
        let owner = [92; 16];
        drop(RelWriter::create_path(&left.0, ReliquaryScope::Connection, owner).unwrap());
        fs::copy(&left.0, &right.0).unwrap();
        let graph = GraphAdmission::for_replay(owner, [], [], [], [], []);
        let reconciled = reconcile_rel_owner(&left.0, &right.0, &output.0, &graph, [], []).unwrap();
        assert_eq!(
            reconciled.comparison.relation,
            RelHistoryRelation::Identical
        );
        assert!(matches!(
            reconciled.evidence,
            RelReconciliationEvidence::ExactCopy(_)
        ));
        assert_eq!(reconciled.session.owner_id(), owner);
        assert_eq!(fs::read(&output.0).unwrap(), fs::read(&left.0).unwrap());
    }
}
