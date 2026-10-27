use super::test_support::{episode, test_path};
use crate::{Cva, EpisodeBoundary, EpisodeOrigin, InsomniaPriority, InsomniaWorkState};

#[test]
fn processing_claims_are_rederived_as_pending_after_restart() {
    let path = test_path("restart.cva");
    let mut cva = Cva::create(&path).unwrap();
    let episode = episode(
        &mut cva,
        "c1",
        "e",
        10,
        EpisodeOrigin::Live,
        EpisodeBoundary::Inactivity,
    );
    cva.queue_insomnia_episode(episode.id, InsomniaPriority::Live, 20)
        .unwrap();
    let first = cva.claim_insomnia_episode("w1", 30, 1000).unwrap().unwrap();
    assert_eq!(first.state, InsomniaWorkState::Processing);
    cva.sync().unwrap();
    drop(cva);

    let mut reopened = Cva::open(&path).unwrap();
    assert_eq!(reopened.insomnia.scheduler_counts(), (1, 0, 0));
    let pending = reopened.insomnia_work(episode.id).unwrap();
    assert_eq!(pending.state, InsomniaWorkState::Pending);
    assert_eq!(pending.attempt_count, 0);
    let reclaimed = reopened
        .claim_insomnia_episode("w2", 31, 1000)
        .unwrap()
        .unwrap();
    assert_eq!(reclaimed.attempt_count, 1);
    reopened
        .complete_insomnia_episode(
            episode.id,
            reclaimed.lease_token.unwrap(),
            31,
            32,
            "model".into(),
            "v1".into(),
            vec![],
            0,
        )
        .unwrap();
    reopened.sync().unwrap();
    drop(reopened);

    let final_open = Cva::open(&path).unwrap();
    assert_eq!(final_open.insomnia_attempts(episode.id).len(), 1);
    assert_eq!(
        final_open.insomnia_work(episode.id).unwrap().state,
        InsomniaWorkState::Complete
    );
}

#[test]
fn retry_delay_is_runtime_only_and_restart_requeues_immediately() {
    let path = test_path("retry.cva");
    let mut cva = Cva::create(&path).unwrap();
    let episode = episode(
        &mut cva,
        "c1",
        "e",
        10,
        EpisodeOrigin::Live,
        EpisodeBoundary::Inactivity,
    );
    cva.queue_insomnia_episode(episode.id, InsomniaPriority::Live, 20)
        .unwrap();
    let claim = cva.claim_insomnia_episode("w1", 30, 100).unwrap().unwrap();
    cva.fail_insomnia_episode(
        episode.id,
        claim.lease_token.unwrap(),
        30,
        31,
        50,
        "provider overloaded".into(),
    )
    .unwrap();
    assert_eq!(cva.insomnia.scheduler_counts(), (0, 1, 0));
    assert!(cva.claim_insomnia_episode("w2", 49, 100).unwrap().is_none());
    cva.sync().unwrap();
    drop(cva);

    let mut reopened = Cva::open(&path).unwrap();
    assert!(reopened.insomnia_attempts(episode.id).is_empty());
    let pending = reopened.insomnia_work(episode.id).unwrap();
    assert_eq!(pending.state, InsomniaWorkState::Pending);
    assert_eq!(pending.attempt_count, 0);
    assert!(
        reopened
            .claim_insomnia_episode("w2", 49, 100)
            .unwrap()
            .is_some()
    );
}

#[test]
fn transient_work_transitions_do_not_grow_the_cva() {
    let path = test_path("transient-no-growth.cva");
    let mut cva = Cva::create(&path).unwrap();
    let episode = episode(
        &mut cva,
        "c1",
        "e",
        10,
        EpisodeOrigin::Live,
        EpisodeBoundary::Inactivity,
    );
    cva.sync().unwrap();
    let before = std::fs::metadata(&path).unwrap().len();

    cva.queue_insomnia_episode(episode.id, InsomniaPriority::Live, 20)
        .unwrap();
    let claim = cva.claim_insomnia_episode("w1", 30, 100).unwrap().unwrap();
    cva.renew_insomnia_lease(episode.id, claim.lease_token.unwrap(), 31, 100)
        .unwrap();
    cva.fail_insomnia_episode(
        episode.id,
        claim.lease_token.unwrap(),
        30,
        32,
        50,
        "provider overloaded".into(),
    )
    .unwrap();
    cva.sync().unwrap();

    assert_eq!(std::fs::metadata(&path).unwrap().len(), before);
}

#[test]
fn terminal_can_be_explicitly_retried_and_reopens_pending() {
    let path = test_path("terminal-retry.cva");
    let mut cva = Cva::create(&path).unwrap();
    let episode = episode(
        &mut cva,
        "c1",
        "e",
        10,
        EpisodeOrigin::Live,
        EpisodeBoundary::Inactivity,
    );
    cva.queue_insomnia_episode(episode.id, InsomniaPriority::Live, 20)
        .unwrap();
    let claim = cva.claim_insomnia_episode("w1", 30, 100).unwrap().unwrap();
    cva.terminal_insomnia_episode(
        episode.id,
        claim.lease_token.unwrap(),
        30,
        31,
        "bad model output".into(),
    )
    .unwrap();

    let retried = cva.retry_terminal_insomnia_episode(episode.id, 40).unwrap();
    assert_eq!(retried.state, InsomniaWorkState::Pending);
    assert_eq!(retried.attempt_count, 0);
    assert!(retried.last_error.is_none());
    cva.sync().unwrap();
    drop(cva);

    let mut reopened = Cva::open(&path).unwrap();
    assert_eq!(reopened.insomnia.scheduler_counts(), (1, 0, 0));
    let pending = reopened.insomnia_work(episode.id).unwrap();
    assert_eq!(pending.state, InsomniaWorkState::Pending);
    assert_eq!(pending.attempt_count, 0);
    assert!(
        reopened
            .claim_insomnia_episode("w2", 41, 100)
            .unwrap()
            .is_some()
    );
}

#[test]
fn final_terminal_outcome_remains_durable() {
    let path = test_path("terminal-final.cva");
    let mut cva = Cva::create(&path).unwrap();
    let episode = episode(
        &mut cva,
        "c1",
        "e",
        10,
        EpisodeOrigin::Live,
        EpisodeBoundary::Inactivity,
    );
    cva.queue_insomnia_episode(episode.id, InsomniaPriority::Live, 20)
        .unwrap();
    let claim = cva.claim_insomnia_episode("w1", 30, 100).unwrap().unwrap();
    cva.terminal_insomnia_episode(
        episode.id,
        claim.lease_token.unwrap(),
        30,
        31,
        "invalid endpoint configuration".into(),
    )
    .unwrap();
    cva.sync().unwrap();
    drop(cva);

    let mut reopened = Cva::open(&path).unwrap();
    assert_eq!(reopened.insomnia.scheduler_counts(), (0, 0, 0));
    assert_eq!(
        reopened.insomnia_work(episode.id).unwrap().state,
        InsomniaWorkState::Terminal
    );
    assert!(
        reopened
            .claim_insomnia_episode("w2", 40, 100)
            .unwrap()
            .is_none()
    );
}

#[test]
fn owned_work_snapshot_survives_transitions_and_reopen() {
    let path = test_path("owned-work-snapshot.cva");
    let mut cva = Cva::create(&path).unwrap();
    let episode = episode(
        &mut cva,
        "owned",
        "episode",
        10,
        EpisodeOrigin::Live,
        EpisodeBoundary::Inactivity,
    );
    cva.queue_insomnia_episode(episode.id, InsomniaPriority::Live, 20)
        .unwrap();
    let pending = cva.insomnia_work(episode.id).unwrap();
    let claim = cva
        .claim_insomnia_episode("worker", 30, 100)
        .unwrap()
        .unwrap();
    assert_eq!(pending.episode_id, episode.id);
    assert_eq!(pending.state, InsomniaWorkState::Pending);
    assert_eq!(cva.insomnia_work(episode.id).unwrap(), claim);
    cva.complete_insomnia_episode(
        episode.id,
        claim.lease_token.unwrap(),
        30,
        31,
        "model".into(),
        "v1".into(),
        vec![],
        0,
    )
    .unwrap();
    assert_eq!(claim.state, InsomniaWorkState::Processing);
    assert_eq!(pending.state, InsomniaWorkState::Pending);
    cva.sync().unwrap();
    drop(cva);

    let reopened = Cva::open(&path).unwrap();
    let completed = reopened.insomnia_work(episode.id).unwrap();
    assert_eq!(completed.episode_id, episode.id);
    assert_eq!(completed.state, InsomniaWorkState::Complete);
    assert_eq!(pending.state, InsomniaWorkState::Pending);
}
