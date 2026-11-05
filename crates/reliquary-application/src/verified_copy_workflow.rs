//! Reopen exact owner copies through normal read-only owner admission.

use reliquary_owner_session::{
    AcceptedEpisodeOrder, DreamBeginAdmission, GraphAdmission, PhySession, RelSession,
    VerifiedOwnerCopyPlan, VerifiedOwnerCopyReceipt,
};
use std::path::Path;

pub struct VerifiedRelOwnerCopy {
    pub receipt: VerifiedOwnerCopyReceipt,
    pub session: RelSession,
}

pub struct VerifiedPhyOwnerCopy {
    pub receipt: VerifiedOwnerCopyReceipt,
    pub session: PhySession,
}

/// Reconcile comparable REL histories into a new independently admitted copy.
pub fn reconcile_rel_prefix(
    left: &Path,
    right: &Path,
    destination: &Path,
    graph_admission: &GraphAdmission,
    accepted_episodes: impl IntoIterator<Item = AcceptedEpisodeOrder>,
    dream_admissions: impl IntoIterator<Item = DreamBeginAdmission>,
) -> Result<VerifiedRelOwnerCopy, String> {
    let (plan, use_right) = VerifiedOwnerCopyPlan::prepare_prefix_reconciliation(left, right)?;
    RelSession::open_path(
        if use_right { left } else { right },
        graph_admission,
        [],
        [],
    )
    .map_err(|error| format!("REL prefix owner admission failed: {error}"))?;
    copy_and_reopen_rel_owner(
        &plan,
        if use_right { right } else { left },
        destination,
        graph_admission,
        accepted_episodes,
        dream_admissions,
    )
}

/// Reconcile comparable PHY histories into a new independently admitted copy.
pub fn reconcile_phy_prefix(
    left: &Path,
    right: &Path,
    destination: &Path,
    graph_admission: &GraphAdmission,
    dream_admissions: impl IntoIterator<Item = DreamBeginAdmission>,
) -> Result<VerifiedPhyOwnerCopy, String> {
    let (plan, use_right) = VerifiedOwnerCopyPlan::prepare_prefix_reconciliation(left, right)?;
    PhySession::open_path(if use_right { left } else { right }, graph_admission, [])
        .map_err(|error| format!("PHY prefix owner admission failed: {error}"))?;
    copy_and_reopen_phy_owner(
        &plan,
        if use_right { right } else { left },
        destination,
        graph_admission,
        dream_admissions,
    )
}

/// Create an exact REL copy and independently reopen it with current owner
/// admission. This returns a verified copy only; it never replaces or removes
/// the source and performs no promotion.
pub fn copy_and_reopen_rel_owner(
    plan: &VerifiedOwnerCopyPlan,
    source_path: impl AsRef<Path>,
    destination_path: impl AsRef<Path>,
    graph_admission: &GraphAdmission,
    accepted_episodes: impl IntoIterator<Item = AcceptedEpisodeOrder>,
    dream_admissions: impl IntoIterator<Item = DreamBeginAdmission>,
) -> Result<VerifiedRelOwnerCopy, String> {
    if !plan.is_reliquary() {
        return Err("REL copy workflow received a PHY maintenance plan".into());
    }
    let (receipt, session) = plan.copy_to_with_admission(
        source_path,
        destination_path.as_ref(),
        |staged_path, receipt| {
            let session = RelSession::open_path(
                staged_path,
                graph_admission,
                accepted_episodes,
                dream_admissions,
            )
            .map_err(|error| format!("independent REL owner reopen failed: {error}"))?;
            if session.owner_id() != receipt.owner_id {
                return Err("reopened REL owner does not match the verified copy receipt".into());
            }
            Ok(session)
        },
    )?;
    Ok(VerifiedRelOwnerCopy { receipt, session })
}

/// Create an exact PHY copy and independently reopen it with current owner
/// admission. This returns a verified copy only; it never replaces or removes
/// the source and performs no promotion.
pub fn copy_and_reopen_phy_owner(
    plan: &VerifiedOwnerCopyPlan,
    source_path: impl AsRef<Path>,
    destination_path: impl AsRef<Path>,
    graph_admission: &GraphAdmission,
    dream_admissions: impl IntoIterator<Item = DreamBeginAdmission>,
) -> Result<VerifiedPhyOwnerCopy, String> {
    if plan.is_reliquary() {
        return Err("PHY copy workflow received a REL maintenance plan".into());
    }
    let (receipt, session) = plan.copy_to_with_admission(
        source_path,
        destination_path.as_ref(),
        |staged_path, receipt| {
            let session = PhySession::open_path(staged_path, graph_admission, dream_admissions)
                .map_err(|error| format!("independent PHY owner reopen failed: {error}"))?;
            if session.owner_id() != receipt.owner_id {
                return Err("reopened PHY owner does not match the verified copy receipt".into());
            }
            Ok(session)
        },
    )?;
    Ok(VerifiedPhyOwnerCopy { receipt, session })
}

#[cfg(test)]
mod tests {
    use super::*;
    use reliquary_container::{Container, ContainerIdentity};
    use reliquary_owner_session::VerifiedOwnerCopyPlan;
    use reliquary_vector_store::{CompatibilityProfileStore, MemoryVectorStore, PackedVectorStore};
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
                "verified-copy-{tag}-{}-{}.phy",
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

    fn valid_phy(path: &Path, owner_id: [u8; 16]) {
        let mut container =
            Container::create(path, ContainerIdentity::phylactery(), owner_id).unwrap();
        PackedVectorStore::default()
            .initialize(&mut container)
            .unwrap();
        CompatibilityProfileStore::default()
            .initialize(&mut container)
            .unwrap();
        MemoryVectorStore::default()
            .initialize(&mut container)
            .unwrap();
        container.sync().unwrap();
    }

    #[test]
    fn verified_phy_copy_reopens_owner_and_does_not_promote_or_change_source() {
        let source = Temp::new("source");
        let destination = Temp::new("destination");
        let owner_id = [81; 16];
        valid_phy(&source.0, owner_id);
        let source_before = fs::read(&source.0).unwrap();
        let plan = VerifiedOwnerCopyPlan::prepare(&source.0).unwrap();
        let graph = GraphAdmission::for_replay(owner_id, [], [], [], [], []);
        let copied =
            copy_and_reopen_phy_owner(&plan, &source.0, &destination.0, &graph, []).unwrap();
        assert_eq!(copied.session.owner_id(), owner_id);
        assert_eq!(copied.receipt.owner_id, owner_id);
        assert_eq!(copied.receipt.source_digest, copied.receipt.copy_digest);
        assert_eq!(fs::read(&source.0).unwrap(), source_before);
        assert_eq!(fs::read(&destination.0).unwrap(), source_before);
    }

    #[test]
    fn reconciliation_reopens_prefix_extensions_and_preserves_divergent_owners() {
        use reliquary_container::ReliquaryScope;
        use reliquary_owner_session::{PhyWriter, RelWriter};

        for rel in [false, true] {
            let left = Temp::new("reconcile-left");
            let right = Temp::new("reconcile-right");
            let output = Temp::new("reconcile-output");
            let refused = Temp::new("reconcile-refused");
            let owner = [83; 16];
            let graph = GraphAdmission::for_replay(owner, [], [], [], [], []);
            if rel {
                RelWriter::create_path(&left.0, ReliquaryScope::Connection, owner).unwrap();
            } else {
                PhyWriter::create_path(&left.0, owner).unwrap();
            }
            fs::copy(&left.0, &right.0).unwrap();
            let append = |path: &Path, timestamp| {
                let mut container = Container::open_writer(path).unwrap();
                container.reserve_version(Some(timestamp)).unwrap();
                container.sync().unwrap();
            };
            append(&right.0, 20);
            let reconcile = |destination: &Path| {
                if rel {
                    reconcile_rel_prefix(&left.0, &right.0, destination, &graph, [], [])
                        .map(|verified| verified.receipt)
                } else {
                    reconcile_phy_prefix(&left.0, &right.0, destination, &graph, [])
                        .map(|verified| verified.receipt)
                }
            };
            let right_before = fs::read(&right.0).unwrap();
            assert_eq!(reconcile(&output.0).unwrap().owner_id, owner);
            assert_eq!(fs::read(&output.0).unwrap(), right_before);
            assert!(reconcile(&output.0).is_err());
            assert_eq!(fs::read(&output.0).unwrap(), right_before);

            append(&left.0, 10);
            let left_before = fs::read(&left.0).unwrap();
            assert!(
                reconcile(&refused.0)
                    .unwrap_err()
                    .contains("histories diverge")
            );
            assert!(!refused.0.exists());
            assert_eq!(fs::read(&left.0).unwrap(), left_before);
            assert_eq!(fs::read(&right.0).unwrap(), right_before);
        }
    }

    #[test]
    fn physical_copy_with_invalid_phy_semantics_is_not_reported_verified() {
        let source = Temp::new("invalid-source");
        let destination = Temp::new("invalid-copy");
        let owner_id = [82; 16];
        {
            let mut container =
                Container::create(&source.0, ContainerIdentity::phylactery(), owner_id).unwrap();
            container
                .append(b"RELTURN2invalid-rel-only-record")
                .unwrap();
            container.sync().unwrap();
        }
        let source_before = fs::read(&source.0).unwrap();
        let plan = VerifiedOwnerCopyPlan::prepare(&source.0).unwrap();
        let graph = GraphAdmission::for_replay(owner_id, [], [], [], [], []);
        assert!(copy_and_reopen_phy_owner(&plan, &source.0, &destination.0, &graph, [],).is_err());
        assert_eq!(fs::read(&source.0).unwrap(), source_before);
        assert!(!destination.0.exists());
    }
}
