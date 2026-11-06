use super::*;
use reliquary_container::ReliquaryScope;

fn path(tag: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(".test-tmp")
        .join(format!(
            "semantic-reconcile-acceptance-{tag}-{}.rel",
            std::process::id()
        ))
}

#[test]
fn wrong_owner_and_scope_are_typed_incompatibilities() {
    let left = path("identity-left");
    let wrong_owner = path("identity-owner");
    let wrong_scope = path("identity-scope");
    fs::create_dir_all(left.parent().unwrap()).unwrap();
    let _ = fs::remove_file(&left);
    let _ = fs::remove_file(&wrong_owner);
    let _ = fs::remove_file(&wrong_scope);
    drop(RelWriter::create_path(&left, ReliquaryScope::Connection, [71; 16]).unwrap());
    drop(RelWriter::create_path(&wrong_owner, ReliquaryScope::Connection, [72; 16]).unwrap());
    drop(RelWriter::create_path(&wrong_scope, ReliquaryScope::Project, [71; 16]).unwrap());
    assert_eq!(
        compare_rel_histories(&left, &wrong_owner).unwrap_err(),
        RelReconciliationError::OwnerOrScopeMismatch
    );
    assert_eq!(
        compare_rel_histories(&left, &wrong_scope).unwrap_err(),
        RelReconciliationError::OwnerOrScopeMismatch
    );
    for value in [left, wrong_owner, wrong_scope] {
        fs::remove_file(value).unwrap();
    }
}

#[test]
fn preserve_owner_rewrite_is_a_different_history_generation() {
    let source = path("generation-source");
    let rewritten = path("generation-rewritten");
    fs::create_dir_all(source.parent().unwrap()).unwrap();
    let _ = fs::remove_file(&source);
    let _ = fs::remove_file(&rewritten);
    let owner = [73; 16];
    drop(RelWriter::create_path(&source, ReliquaryScope::Connection, owner).unwrap());
    let source_bytes = fs::read(&source).unwrap();
    let request = FreshHistoryRequest {
        source_owner_id: owner,
        source_digest: Sha256::digest(&source_bytes).into(),
        plan_digest: [9; 32],
        identity: FreshHistoryIdentity::PreserveOwner,
        retained_memory_ids: Vec::new(),
        retain_all_revisions: true,
        retain_revisions_since_memory_version: None,
        retain_derived_state: false,
        archive: FreshHistoryArchive::All,
        freshness: FreshHistoryFreshness::Reset,
        profile_ids: Vec::new(),
    };
    let graph = GraphAdmission::for_replay(owner, [], [], [], [], []);
    let source_session = RelSession::open_path(&source, &graph, [], []).unwrap();
    let source_cut = source_session.current_historical_cut();
    let source_proof = source_session.historical_proof(owner, &source_cut).unwrap();
    drop(source_session);
    rewrite_memory_history_copy(&source, &rewritten, &request, &graph).unwrap();
    let rewritten_session = RelSession::open_path(&rewritten, &graph, [], []).unwrap();
    let rewritten_cut = rewritten_session.current_historical_cut();
    let rewritten_proof = rewritten_session
        .historical_proof(owner, &rewritten_cut)
        .unwrap();
    assert_eq!(source_cut, rewritten_cut);
    assert_eq!(source_proof.digest, rewritten_proof.digest);
    assert_ne!(
        source_proof.history_generation,
        rewritten_proof.history_generation
    );
    assert_eq!(
        compare_rel_histories(&source, &rewritten).unwrap_err(),
        RelReconciliationError::HistoryGenerationConflict
    );
    fs::remove_file(source).unwrap();
    fs::remove_file(rewritten).unwrap();
}
