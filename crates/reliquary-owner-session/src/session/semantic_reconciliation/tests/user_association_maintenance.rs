use super::*;
use crate::MemoryAssociationCut;

fn seed_link(path: &Path) -> [MemoryId; 2] {
    let mutations = ["association-source", "association-target"];
    append_branch(path, "shared-turn", "shared content", &mutations);
    mutations.map(memory_id)
}

fn unlink(path: &Path, [source, target]: [MemoryId; 2]) {
    let mut writer = RelWriter::open_path_current(path).unwrap();
    let receipt = writer
        .publish_memory_association(MemoryAssociationCut {
            request_key: [12; 32],
            fingerprint: [13; 32],
            source,
            target,
            source_revision: 1,
            target_revision: 1,
            graph_version: 1,
            expected_edge_revision: Some(1),
            active: false,
        })
        .unwrap();
    assert!(!receipt.active);
}

#[test]
fn fresh_history_refuses_to_drop_the_user_unlink_fence() {
    let (source, _unused, owner) = owner_pair();
    let mut ids = seed_link(&source.0);
    unlink(&source.0, ids);
    ids.sort_unstable();
    let source_bytes = fs::read(&source.0).unwrap();
    let graph = GraphAdmission::for_replay(owner, ids.map(|id| id.0), [], [], [], []);
    let request = FreshHistoryRequest {
        source_owner_id: owner,
        source_digest: Sha256::digest(&source_bytes).into(),
        plan_digest: [9; 32],
        identity: FreshHistoryIdentity::PreserveOwner,
        retained_memory_ids: ids.to_vec(),
        retain_all_revisions: true,
        retain_revisions_since_memory_version: None,
        retain_derived_state: true,
        archive: FreshHistoryArchive::All,
        freshness: FreshHistoryFreshness::Reset,
        profile_ids: Vec::new(),
    };
    let output = Temp::new("rewrite-association");
    let error = rewrite_memory_history_copy(&source.0, &output.0, &request, &graph).unwrap_err();
    assert!(error.contains("operational evidence that cannot be safely replayed"));
    assert!(!output.0.exists());
    assert_eq!(fs::read(&source.0).unwrap(), source_bytes);
}

#[test]
fn divergent_reconciliation_refuses_to_drop_an_association_receipt() {
    let (left, right, owner) = owner_pair();
    let ids = seed_link(&left.0);
    fs::copy(&left.0, &right.0).unwrap();
    append_branch(&left.0, "left-turn", "independent content", &[]);
    unlink(&right.0, ids);
    assert_eq!(
        compare_rel_histories(&left.0, &right.0).unwrap().relation,
        RelHistoryRelation::Diverged,
    );
    let left_bytes = fs::read(&left.0).unwrap();
    let right_bytes = fs::read(&right.0).unwrap();
    let graph = GraphAdmission::for_replay(owner, ids.map(|id| id.0), [], [], [], []);
    let output = Temp::new("merge-association");
    for (base, incoming) in [(&left.0, &right.0), (&right.0, &left.0)] {
        assert_eq!(
            reconcile_divergent_rel_copy(base, incoming, &output.0, &graph, &[], &[]).unwrap_err(),
            RelReconciliationError::UnsupportedState(
                "record family lacks an exact replay contract"
            ),
        );
        assert!(!output.0.exists());
    }
    assert_eq!(fs::read(&left.0).unwrap(), left_bytes);
    assert_eq!(fs::read(&right.0).unwrap(), right_bytes);
}
