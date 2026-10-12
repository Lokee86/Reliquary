use crate::{Cva, freshness::historical_planner::PlannerError};

struct Fixture(std::path::PathBuf);
impl Fixture {
    fn new() -> Self {
        let dir = std::env::current_dir()
            .unwrap()
            .join(".historical-clock-tests")
            .join(uuid::Uuid::new_v4().to_string());
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }
    fn path(&self) -> std::path::PathBuf {
        self.0.join("owner.rel")
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn append(rel: &mut Cva, c: &str, n: &str, parent: Option<&str>, role: &str, t: i64) {
    rel.archive
        .append_node(
            &mut rel.container,
            n.into(),
            c.into(),
            parent.map(str::to_owned),
            role.into(),
            None,
            t,
            "source",
        )
        .unwrap();
}

#[test]
fn historical_clock_matching_retry_non_activity_and_reopen() {
    let f = Fixture::new();
    let mut rel = Cva::create(f.path()).unwrap();
    assert!(
        rel.historical_freshness_timeline()
            .unwrap()
            .activity_order
            .is_empty()
    );
    append(&mut rel, "a", "system", None, "system", 0);
    append(&mut rel, "a", "z", Some("system"), "user", 1);
    append(&mut rel, "a", "a", Some("z"), "assistant", 1);
    append(&mut rel, "b", "u", None, "user", 1);
    append(&mut rel, "a", "z", Some("system"), "user", 1);
    let before = rel.latest_global_version();
    let timeline = rel.historical_freshness_timeline().unwrap();
    assert_eq!(rel.latest_global_version(), before);
    assert_eq!(rel.rel_turn_count(), 3);
    for (i, key) in timeline.activity_order.iter().enumerate() {
        assert_eq!(
            rel.activity_position_for_turn(&key.conversation_id, &key.node_id),
            Some(i as u64 + 1)
        );
    }
    rel.sync().unwrap();
    drop(rel);
    let reopened = Cva::open(f.path()).unwrap();
    assert_eq!(reopened.historical_freshness_timeline().unwrap(), timeline);
}

#[test]
fn historical_clock_rejects_reordered_conversations_and_ties_without_mutation() {
    for tied in [false, true] {
        let f = Fixture::new();
        let mut rel = Cva::create(f.path()).unwrap();
        append(&mut rel, "b", "u", None, "user", if tied { 1 } else { 2 });
        append(&mut rel, "a", "u", None, "user", 1);
        let before = rel.latest_global_version();
        assert!(matches!(
            rel.historical_freshness_timeline(),
            Err(PlannerError::ArchiveClockMismatch {
                expected: 1,
                actual: Some(2),
                ..
            })
        ));
        assert_eq!(rel.latest_global_version(), before);
        assert_eq!(rel.activity_position_for_turn("b", "u"), Some(1));
        rel.sync().unwrap();
        drop(rel);
        let reopened = Cva::open(f.path()).unwrap();
        assert!(matches!(
            reopened.historical_freshness_timeline(),
            Err(PlannerError::ArchiveClockMismatch { .. })
        ));
    }
}
