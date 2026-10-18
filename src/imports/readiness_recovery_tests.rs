use super::{
    Fixture, ImportCompletionOutcome, ImportCompletionPending, ImportReadinessOwner,
    ImportReadinessStatus, nonempty_import, watermarks,
};
use crate::Cva;
use std::fs;

#[test]
fn reopen_preserves_pending_import_readiness_and_write_fence() {
    let fixture = Fixture::new();
    let path = fixture.rel();
    let mut cva = Cva::create(&path).unwrap();
    cva.import_native(&nonempty_import(), 100).unwrap();
    let start = cva.open_import_epoch().unwrap();
    let readiness = cva.native_import_readiness(&start.import_id).unwrap();
    let blocker = readiness.blocker.expect("import Episode is still pending");
    assert_eq!(blocker.owner, ImportReadinessOwner::Insomnia);
    assert_eq!(blocker.status, ImportReadinessStatus::Pending);
    let bytes = fs::read(&path).unwrap();
    let clocks = watermarks(&cva);
    drop(cva);

    let mut reopened = Cva::open(&path).unwrap();
    assert_eq!(
        reopened
            .native_import_readiness(&start.import_id)
            .unwrap()
            .blocker,
        Some(blocker.clone())
    );
    assert_eq!(
        reopened
            .complete_native_import(&start.import_id, 200)
            .unwrap(),
        ImportCompletionOutcome::Pending(ImportCompletionPending::Readiness(blocker))
    );
    assert_eq!(fs::read(&path).unwrap(), bytes);
    assert_eq!(watermarks(&reopened), clocks);
    assert_eq!(reopened.open_import_epoch(), Some(start));
    assert!(
        reopened
            .append_node(
                "blocked-after-reopen".into(),
                "live".into(),
                None,
                "user".into(),
                201,
                "the pending import keeps ordinary writes fenced",
            )
            .is_err()
    );
    assert_eq!(fs::read(&path).unwrap(), bytes);
}

#[test]
fn reopen_preserves_terminal_failed_import_readiness_and_write_fence() {
    let fixture = Fixture::new();
    let path = fixture.rel();
    let mut cva = Cva::create(&path).unwrap();
    cva.import_native(&nonempty_import(), 100).unwrap();
    let start = cva.open_import_epoch().unwrap();
    let scope = cva.import_epoch_scope().unwrap();
    let episode = crate::EpisodeId(*scope.episodes.iter().next().unwrap());
    cva.container.set_owner_write_scope(true);
    let claim = cva
        .claim_insomnia_episode("recovery-terminal-worker", 110, 1_000)
        .unwrap()
        .unwrap();
    assert_eq!(claim.episode_id, episode);
    cva.terminal_insomnia_episode(
        episode,
        claim.lease_token.unwrap(),
        110,
        111,
        "recovery test terminal failure".into(),
    )
    .unwrap();
    cva.container.set_owner_write_scope(false);

    let readiness = cva.native_import_readiness(&start.import_id).unwrap();
    let blocker = readiness.blocker.expect("terminal imported Episode");
    assert_eq!(blocker.owner, ImportReadinessOwner::Insomnia);
    assert_eq!(blocker.status, ImportReadinessStatus::Failed);
    let bytes = fs::read(&path).unwrap();
    let clocks = watermarks(&cva);
    drop(cva);

    let mut reopened = Cva::open(&path).unwrap();
    assert_eq!(
        reopened
            .native_import_readiness(&start.import_id)
            .unwrap()
            .blocker,
        Some(blocker.clone())
    );
    assert_eq!(
        reopened
            .complete_native_import(&start.import_id, 200)
            .unwrap(),
        ImportCompletionOutcome::Pending(ImportCompletionPending::Readiness(blocker))
    );
    assert_eq!(fs::read(&path).unwrap(), bytes);
    assert_eq!(watermarks(&reopened), clocks);
    assert_eq!(reopened.open_import_epoch(), Some(start));
    assert!(
        reopened
            .append_node(
                "blocked-after-failure".into(),
                "live".into(),
                None,
                "user".into(),
                201,
                "the failed import keeps ordinary writes fenced",
            )
            .is_err()
    );
    assert_eq!(fs::read(&path).unwrap(), bytes);
}
