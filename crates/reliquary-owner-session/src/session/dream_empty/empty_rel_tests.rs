use super::tests::*;
use super::*;
use reliquary_memory_core::MemoryStore;
use std::fs;
#[test]
fn rel_empty_first_cycle_reopens_and_identical_retry_is_byte_stable() {
    let path = Temp::new();
    let owner = [194; 16];
    drop(
        RelWriter::create_path(&path.0, reliquary_container::ReliquaryScope::Project, owner)
            .unwrap(),
    );
    let mut container = Container::open_writer(&path.0).unwrap();
    let mut memories = MemoryStore::open(&container).unwrap();
    let source = memories
        .publish(&mut container, None, 0, memory())
        .unwrap()
        .revision
        .record;
    container.sync().unwrap();
    drop(container);
    let graph = GraphAdmission::for_replay(owner, [], [], [], [], []);
    let mut writer = RelWriter::open_path(&path.0, graph.clone(), [], []).unwrap();
    let observation = DreamMemoryCut {
        id: source.id,
        revision: 1,
        body_id: source.body_id,
    };
    let admission = writer
        .begin_first_empty_dream_pass("first-empty".into(), observation, 4)
        .unwrap();
    let mut wrong = observation;
    wrong.body_id = reliquary_memory_core::MemoryBodyId([255; 32]);
    assert!(
        writer
            .commit_empty_dream_pass("first-empty", wrong, 0, 4)
            .is_err()
    );
    wrong = observation;
    wrong.revision = 2;
    assert!(
        writer
            .commit_empty_dream_pass("first-empty", wrong, 0, 4)
            .is_err()
    );
    assert!(
        writer
            .commit_empty_dream_pass("first-empty", observation, 1, 4)
            .is_err()
    );
    assert!(
        writer
            .commit_empty_dream_pass("first-empty", observation, 0, 3)
            .is_err()
    );
    assert!(
        writer
            .commit_empty_dream_pass("first-empty", observation, 0, 4)
            .unwrap()
    );
    drop(writer);

    let reopened = RelSession::open_path(&path.0, &graph, [], [admission.clone()]).unwrap();
    assert!(reopened.common.dreams.journal().is_settled("first-empty"));
    assert_eq!(
        reopened.common.dreams.initial_settlement("first-empty"),
        Some((4, 0))
    );
    assert_eq!(reopened.graph_version(), 0);
    assert!(
        reopened
            .freshness
            .has_durable_birth(FreshnessMemoryId(source.id.0))
    );
    assert_eq!(
        reopened
            .memory_history(source.id)
            .unwrap()
            .unwrap()
            .revisions
            .len(),
        1
    );
    drop(reopened);
    let before = fs::read(&path.0).unwrap();
    let mut retry = RelWriter::open_path(&path.0, graph.clone(), [], [admission]).unwrap();
    assert!(
        !retry
            .commit_empty_dream_pass("first-empty", observation, 0, 4)
            .unwrap()
    );
    assert!(
        retry
            .commit_empty_dream_pass("first-empty", observation, 0, 5)
            .is_err()
    );
    drop(retry);
    assert_eq!(fs::read(&path.0).unwrap(), before);
}

#[test]
fn rel_synced_settlement_recovers_before_any_projection_is_applied() {
    let path = Temp::new();
    let owner = [194; 16];
    drop(
        RelWriter::create_path(&path.0, reliquary_container::ReliquaryScope::Project, owner)
            .unwrap(),
    );
    let mut container = Container::open_writer(&path.0).unwrap();
    let mut memories = MemoryStore::open(&container).unwrap();
    let source = memories
        .publish(&mut container, None, 0, memory())
        .unwrap()
        .revision
        .record;
    container.sync().unwrap();
    drop(container);
    let graph = GraphAdmission::for_replay(owner, [], [], [], [], []);
    let mut writer = RelWriter::open_path(&path.0, graph.clone(), [], []).unwrap();
    let observation = DreamMemoryCut {
        id: source.id,
        revision: 1,
        body_id: source.body_id,
    };
    let admission = writer
        .begin_first_empty_dream_pass("first-empty".into(), observation, 4)
        .unwrap();
    let record = InitialDreamSettlement {
        owner,
        pass_id: "first-empty".into(),
        source: source.id,
        revision: 1,
        body: source.body_id,
        graph_version: 0,
        accepted_turn: 4,
        community_generation: None,
        memory_delta: None,
        roots: Default::default(),
        credit_roots: Default::default(),
        admit: true,
        freshness_effects: writer
            .session
            .freshness
            .prepare_initial_admission(FreshnessMemoryId(source.id.0), 4, 4)
            .unwrap(),
    };
    writer.container.append(&record.encode().unwrap()).unwrap();
    writer.container.sync().unwrap();
    assert!(
        !writer
            .session
            .common
            .dreams
            .journal()
            .is_settled("first-empty")
    );
    drop(writer);

    let reopened = RelSession::open_path(&path.0, &graph, [], [admission.clone()]).unwrap();
    assert!(reopened.common.dreams.journal().is_settled("first-empty"));
    assert_eq!(
        reopened.common.dreams.initial_settlement("first-empty"),
        Some((4, 0))
    );
    assert_eq!(reopened.graph_version(), 0);
    assert!(
        reopened
            .freshness
            .has_durable_birth(FreshnessMemoryId(source.id.0))
    );
    assert_eq!(
        reopened
            .memory_history(source.id)
            .unwrap()
            .unwrap()
            .revisions
            .len(),
        1
    );
    drop(reopened);
    let before = fs::read(&path.0).unwrap();
    let mut retry = RelWriter::open_path(&path.0, graph.clone(), [], [admission]).unwrap();
    assert!(
        !retry
            .commit_empty_dream_pass("first-empty", observation, 0, 4)
            .unwrap()
    );
    assert!(
        retry
            .commit_empty_dream_pass("first-empty", observation, 0, 5)
            .is_err()
    );
    drop(retry);
    assert_eq!(fs::read(&path.0).unwrap(), before);
}
