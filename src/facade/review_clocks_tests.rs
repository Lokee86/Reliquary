use super::*;

#[test]
fn legacy_owners_cannot_supply_an_owner_qualified_reading() {
    let temp = std::env::temp_dir().join(format!("staleness-legacy-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&temp).unwrap();
    let rel = Cva::create_legacy_cva(temp.join("clock.cva")).unwrap();
    let phy = Phylactery::create_legacy_typed(temp.join("clock.phy")).unwrap();
    let source = ClockRef::OwnerMemoryVersion {
        owner_uuid: [1; 16],
    };
    assert_eq!(rel.review_clock(source), Err(ClockError::MissingOwner));
    assert_eq!(phy.review_clock(source), Err(ClockError::MissingOwner));
    drop(rel);
    drop(phy);
    std::fs::remove_dir_all(temp).unwrap();
}

#[test]
fn owner_adapters_read_existing_sources_and_reject_foreign_bindings() {
    let temp = std::env::temp_dir().join(format!("staleness-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&temp).unwrap();
    let mut rel = Cva::create(temp.join("clock.rel")).unwrap();
    rel.append_node(
        "u".into(),
        "c".into(),
        None,
        "user".into(),
        -100,
        "clock test",
    )
    .unwrap();
    assert_eq!(rel.rel_turn_count(), 1);
    let phy = Phylactery::create(temp.join("clock.phy")).unwrap();
    let rel_id = rel.owner_uuid().unwrap();
    let phy_id = phy.owner_uuid().unwrap();
    for (source, expected) in [
        (
            ClockRef::RelActivity { owner_uuid: rel_id },
            rel.rel_turn_count(),
        ),
        (
            ClockRef::OwnerMemoryVersion { owner_uuid: rel_id },
            rel.memory_version(),
        ),
    ] {
        assert_eq!(
            rel.review_clock(source).unwrap().value(),
            ClockValue::Sequence(expected)
        );
    }
    let source = ClockRef::OwnerMemoryVersion { owner_uuid: phy_id };
    assert_eq!(
        phy.review_clock(source).unwrap().value(),
        ClockValue::Sequence(phy.memory_version())
    );
    assert_eq!(rel.review_clock(source), Err(ClockError::SourceMismatch));
    assert_eq!(
        phy.review_clock(ClockRef::RelActivity { owner_uuid: phy_id }),
        Err(ClockError::SourceMismatch)
    );
    assert_eq!(
        rel.review_clock(ClockRef::UnixTimeNs),
        Err(ClockError::SourceMismatch)
    );
    drop(rel);
    drop(phy);
    std::fs::remove_dir_all(temp).unwrap();
}
