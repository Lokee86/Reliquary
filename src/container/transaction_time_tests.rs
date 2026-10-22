use crate::{Container, Cva};
use std::fs;
use std::path::PathBuf;

fn test_dir() -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "reliquary-transaction-time-{}",
        uuid::Uuid::new_v4()
    ));
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn grouped_completion_versions_share_explicit_transaction_time() {
    let path = test_dir().join("grouped.cva");
    let mut container = Container::create(&path).unwrap();
    let mut payload = vec![0_u8; 84];
    payload[..8].copy_from_slice(b"CVAINSC7");
    payload[52..60].copy_from_slice(&777_i64.to_le_bytes());
    payload[64..72].copy_from_slice(&1_u64.to_le_bytes());
    payload[72..76].copy_from_slice(&2_u32.to_le_bytes());
    payload[76..84].copy_from_slice(&888_i64.to_le_bytes());
    container.append(&payload).unwrap();
    container.sync().unwrap();
    drop(container);

    let reopened = Container::open(path).unwrap();
    assert_eq!(reopened.latest_version(), 2);
    assert_eq!(reopened.transaction_time_ns(1), Some(888));
    assert_eq!(reopened.transaction_time_ns(2), Some(888));
    assert_eq!(reopened.version_at_or_before(887), None);
    assert_eq!(reopened.version_at_or_before(888), Some(2));
}

#[test]
fn divergent_reconciliation_preserves_known_and_unknown_transaction_times() {
    let dir = test_dir();
    let left = dir.join("left.rel");
    let right = dir.join("right.rel");
    let output = dir.join("merged.rel");

    let mut base = Cva::create_project(&left).unwrap();
    base.container.set_next_transaction_time_override(Some(100));
    base.append_node(
        "base".into(),
        "base-c".into(),
        None,
        "user".into(),
        1,
        "base",
    )
    .unwrap();
    base.sync().unwrap();
    drop(base);
    fs::copy(&left, &right).unwrap();

    let mut left_cva = Cva::open(&left).unwrap();
    left_cva.container.set_next_transaction_time_override(None);
    left_cva
        .append_node(
            "left".into(),
            "left-c".into(),
            None,
            "user".into(),
            2,
            "left",
        )
        .unwrap();
    left_cva.sync().unwrap();
    drop(left_cva);

    let mut right_cva = Cva::open(&right).unwrap();
    right_cva
        .container
        .set_next_transaction_time_override(Some(300));
    right_cva
        .append_node(
            "right".into(),
            "right-c".into(),
            None,
            "user".into(),
            3,
            "right",
        )
        .unwrap();
    right_cva.sync().unwrap();
    drop(right_cva);

    Cva::reconcile(&left, &right, &output).unwrap();
    let merged = Cva::open(output).unwrap();
    let versions = merged.record_versions();
    assert_eq!(versions.len(), 3);
    assert_eq!(
        merged.transaction_time_ns(versions[0].global_version),
        Some(100)
    );
    assert_eq!(merged.transaction_time_ns(versions[1].global_version), None);
    assert_eq!(
        merged.transaction_time_ns(versions[2].global_version),
        Some(300)
    );
    assert_eq!(merged.version_at_or_before(99), None);
    assert_eq!(
        merged.version_at_or_before(100),
        Some(versions[0].global_version)
    );
    assert_eq!(
        merged.version_at_or_before(300),
        Some(versions[0].global_version)
    );
}

#[test]
fn historical_cut_lookup_never_skips_a_later_prefix_timestamp() {
    let path = test_dir().join("out-of-order.cva");
    let mut container = Container::create(&path).unwrap();

    container.set_next_transaction_time_override(Some(100));
    assert_eq!(container.allocate_version().unwrap(), 1);
    container.set_next_transaction_time_override(Some(300));
    assert_eq!(container.allocate_version().unwrap(), 2);
    container.set_next_transaction_time_override(Some(200));
    assert_eq!(container.allocate_version().unwrap(), 3);
    container.sync().unwrap();
    drop(container);

    let reopened = Container::open(path).unwrap();
    assert_eq!(reopened.transaction_time_ns(1), Some(100));
    assert_eq!(reopened.transaction_time_ns(2), Some(300));
    assert_eq!(reopened.transaction_time_ns(3), Some(200));
    assert_eq!(reopened.version_at_or_before(99), None);
    assert_eq!(reopened.version_at_or_before(100), Some(1));
    assert_eq!(reopened.version_at_or_before(250), Some(1));
    assert_eq!(reopened.version_at_or_before(299), Some(1));
    assert_eq!(reopened.version_at_or_before(300), Some(3));
}
