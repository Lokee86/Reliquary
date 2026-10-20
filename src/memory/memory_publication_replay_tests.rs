#[path = "memory_publication_copy_tests.rs"]
mod copy;
#[path = "memory_publication_prune_tests.rs"]
mod prune;

use super::*;
use crate::container::publication_envelope::{PublicationClock, PublicationEnvelope};
use crate::memory_compact_codec::publication::{self, MemoryPublication};

fn append_publication(
    container: &mut Container,
    global: u64,
    clock: PublicationClock,
    publication: MemoryPublication,
) {
    let body = publication::encode(&publication).unwrap();
    let payload = crate::container::publication_envelope::encode(PublicationEnvelope {
        global_version: global,
        clock,
        body: &body,
    })
    .unwrap();
    container.append(&payload).unwrap();
}

#[test]
fn envelope_creation_delta_and_terminal_replay_with_earlier_reserved_globals() {
    let path = test_path("envelope-reversed-and-terminal.cva");
    let mut container = initialize_container(&path);
    let mut first = initial_record();
    first.body_id = append_body(&mut container, "Envelope Memory", "Immutable body.");
    for time in [Some(10), None, Some(-4)] {
        container
            .allocate_version_with_transaction_time(time)
            .unwrap();
    }
    first.global_version = 2;
    append_publication(
        &mut container,
        2,
        PublicationClock::BindReserved,
        MemoryPublication::Creation(MemoryCreation::new(first.clone()).unwrap()),
    );
    let mut second = successor(&first);
    second.global_version = 1;
    append_publication(
        &mut container,
        1,
        PublicationClock::BindReserved,
        MemoryPublication::Delta(MemoryDelta::between(&first, &second).unwrap()),
    );
    append_publication(
        &mut container,
        3,
        PublicationClock::BindReserved,
        MemoryPublication::Tombstone {
            id: first.id,
            revision: 3,
            memory_version: 3,
        },
    );
    container.sync().unwrap();
    drop(container);
    let store = replay(&path).unwrap();
    assert_eq!(store.records(), &[first.clone(), second]);
    assert_eq!(store.memory_version(), 3);
    assert!(matches!(
        store.lifecycle_state(first.id).unwrap(),
        LifecycleState::Tombstoned(_)
    ));
    let reopened = Container::open(&path).unwrap();
    assert_eq!(reopened.latest_version(), 3);
    assert_eq!(reopened.transaction_time_ns(1), Some(10));
    assert_eq!(reopened.transaction_time_ns(2), None);
    assert_eq!(reopened.transaction_time_ns(3), Some(-4));
}

#[test]
fn envelope_rejects_missing_body_invalid_local_and_unsupported_owner() {
    for (name, missing_body, bad_local, unsupported) in [
        ("missing", true, false, false),
        ("local", false, true, false),
        ("unknown", false, false, true),
    ] {
        let path = test_path(&format!("envelope-invalid-{name}.cva"));
        let mut container = initialize_container(&path);
        let mut first = initial_record();
        if !missing_body {
            first.body_id = append_body(&mut container, "Title", "Body");
        }
        if bad_local {
            first.memory_version = 2;
        }
        let body = if unsupported {
            b"UNKNOWN1opaque".to_vec()
        } else {
            publication::encode(&MemoryPublication::Creation(
                MemoryCreation::new(first).unwrap(),
            ))
            .unwrap()
        };
        container.append_publication_envelope(&body).unwrap();
        container.sync().unwrap();
        drop(container);
        assert!(replay(&path).is_err(), "{name}");
    }
}

#[test]
fn envelope_rel_and_phy_facades_reopen_creation_and_reject_unknown_bodies() {
    for kind in [FileKind::Reliquary, FileKind::Phylactery] {
        let path = test_path(if kind == FileKind::Reliquary {
            "envelope.rel"
        } else {
            "envelope.phy"
        });
        if kind == FileKind::Reliquary {
            drop(crate::Cva::create(&path).unwrap());
        } else {
            drop(crate::Phylactery::create(&path).unwrap());
        }
        let mut container = Container::open(&path).unwrap();
        let mut first = initial_record();
        first.body_id = append_body(&mut container, "Facade title", "Facade body");
        first.global_version = container.latest_version() + 1;
        let body = publication::encode(&MemoryPublication::Creation(
            MemoryCreation::new(first.clone()).unwrap(),
        ))
        .unwrap();
        container.set_next_transaction_time_override(Some(-123));
        let (global, _) = container.append_publication_envelope(&body).unwrap();
        assert_eq!(global, first.global_version);
        container.sync().unwrap();
        drop(container);
        if kind == FileKind::Reliquary {
            let owner = crate::Cva::open(&path).unwrap();
            assert_eq!(owner.memories.records(), &[first]);
        } else {
            let owner = crate::Phylactery::open(&path).unwrap();
            assert_eq!(owner.memories.records(), &[first]);
        }
        let mut container = Container::open(&path).unwrap();
        container
            .append_publication_envelope(b"UNKNOWN1opaque")
            .unwrap();
        container.sync().unwrap();
        drop(container);
        if kind == FileKind::Reliquary {
            assert!(crate::Cva::open(&path).is_err());
        } else {
            assert!(crate::Phylactery::open(&path).is_err());
        }
    }
}

#[test]
fn invalid_envelope_successor_does_not_mutate_accepted_memory() {
    for bad_base in [false, true] {
        let path = test_path("envelope-invalid-successor.cva");
        let mut container = initialize_container(&path);
        let mut first = initial_record();
        first.body_id = append_body(&mut container, "Title", "Body");
        let creation = publication::encode(&MemoryPublication::Creation(
            MemoryCreation::new(first.clone()).unwrap(),
        ))
        .unwrap();
        container.append_publication_envelope(&creation).unwrap();
        let mut next = successor(&first);
        if !bad_base {
            next.memory_version = 3;
        }
        let mut delta = MemoryDelta::between(&first, &next).unwrap();
        if bad_base {
            delta.previous_revision = 2;
            delta.revision = 3;
        }
        let body = publication::encode(&MemoryPublication::Delta(delta)).unwrap();
        container.append_publication_envelope(&body).unwrap();
        container.sync().unwrap();
        drop(container);
        let (store, errors) = replay_while_capturing_ingest_errors(&path);
        assert_eq!(errors.len(), 1);
        assert_eq!(store.records(), &[first]);
        assert_eq!(store.memory_version(), 1);
    }
}

#[test]
fn envelope_body_outside_physical_envelope_and_v1_wrapper_reject() {
    for wrapped in [false, true] {
        let path = test_path("envelope-body-boundary.cva");
        let mut container = initialize_container(&path);
        let mut first = initial_record();
        first.body_id = append_body(&mut container, "Title", "Body");
        let body = publication::encode(&MemoryPublication::Creation(
            MemoryCreation::new(first).unwrap(),
        ))
        .unwrap();
        if wrapped {
            container.append_publication_envelope(&body).unwrap();
        } else {
            container.append(&body).unwrap();
        }
        container.sync().unwrap();
        drop(container);
        if wrapped {
            let mut bytes = fs::read(&path).unwrap();
            bytes[8..10].copy_from_slice(&1_u16.to_le_bytes());
            fs::write(&path, bytes).unwrap();
        }
        assert!(replay(&path).is_err());
    }
}

#[test]
fn envelope_memory_history_matches_marker_history_at_every_local_cut() {
    use crate::historical_memory_fingerprint::MemoryHistory;
    for kind in [FileKind::Reliquary, FileKind::Phylactery] {
        let mut histories = Vec::new();
        for compact in [false, true] {
            let path = test_path("envelope-history-cuts.cva");
            let mut container = Container::create_with_identity_and_uuid(
                &path,
                crate::ContainerIdentity {
                    file_kind: kind,
                    scope: None,
                },
                [93; 16],
            )
            .unwrap();
            MemoryStore::empty().initialize(&mut container).unwrap();
            let mut first = initial_record();
            first.body_id = append_body(&mut container, "Cut title", "Cut immutable body");
            for time in [Some(-10), Some(7), None] {
                container
                    .allocate_version_with_transaction_time(time)
                    .unwrap();
            }
            first.global_version = 2;
            let mut next = successor(&first);
            next.global_version = 1;
            if compact {
                append_publication(
                    &mut container,
                    2,
                    PublicationClock::BindReserved,
                    MemoryPublication::Creation(MemoryCreation::new(first.clone()).unwrap()),
                );
                append_publication(
                    &mut container,
                    1,
                    PublicationClock::BindReserved,
                    MemoryPublication::Delta(MemoryDelta::between(&first, &next).unwrap()),
                );
                append_publication(
                    &mut container,
                    3,
                    PublicationClock::BindReserved,
                    MemoryPublication::Tombstone {
                        id: first.id,
                        revision: 3,
                        memory_version: 3,
                    },
                );
            } else {
                let creation = container
                    .append(&encode_creation(&MemoryCreation::new(first.clone()).unwrap()).unwrap())
                    .unwrap();
                append_version_marker(&mut container, creation, 2, 1);
                let delta = container
                    .append(&encode_delta(&MemoryDelta::between(&first, &next).unwrap()).unwrap())
                    .unwrap();
                append_version_marker(&mut container, delta, 1, 2);
                let terminal = container
                    .append(&encode_tombstone(MemoryTombstone {
                        id: first.id,
                        revision: 3,
                    }))
                    .unwrap();
                append_version_marker(&mut container, terminal, 3, 3);
            }
            container.sync().unwrap();
            drop(container);
            let store = replay(&path).unwrap();
            let mut container = Container::open(&path).unwrap();
            let history = MemoryHistory::read(&mut container, &store).unwrap();
            histories.push(
                (0..=3)
                    .map(|cut| history.at(cut).unwrap().fingerprint)
                    .collect::<Vec<_>>(),
            );
        }
        assert_eq!(
            histories[0], histories[1],
            "every retained local cut, kind {kind:?}"
        );
    }
}

#[test]
fn envelope_coexists_with_grouped_completion_at_all_memory_and_global_cuts() {
    use crate::historical_memory_fingerprint::MemoryHistory;
    let (path, mut owner, _) = crate::insomnia::completion_tests::setup("envelope-grouped-source");
    let completed = crate::insomnia::completion_tests::process(&mut owner, 1, 200);
    assert_eq!(completed.created.len(), 1);
    owner.sync().unwrap();
    drop(owner);
    let second_path = test_path("envelope-grouped-copy.rel");
    fs::copy(&path, &second_path).unwrap();
    for (target, compact) in [(&path, false), (&second_path, true)] {
        let source = crate::Cva::open(target).unwrap();
        let mut first = initial_record();
        first.memory_version = source.memory_version() + 1;
        first.global_version = source.latest_global_version() + 1;
        drop(source);
        let mut container = Container::open(target).unwrap();
        first.body_id = append_body(&mut container, "After grouped", "Same new body");
        if compact {
            let body = publication::encode(&MemoryPublication::Creation(
                MemoryCreation::new(first).unwrap(),
            ))
            .unwrap();
            container.set_next_transaction_time_override(Some(-700));
            container.append_publication_envelope(&body).unwrap();
        } else {
            let carrier = container
                .append(&encode_creation(&MemoryCreation::new(first.clone()).unwrap()).unwrap())
                .unwrap();
            let global = container
                .allocate_version_with_transaction_time(Some(-700))
                .unwrap();
            append_version_marker(&mut container, carrier, global, first.memory_version);
        }
        container.sync().unwrap();
    }
    let mut left = crate::Cva::open(&path).unwrap();
    let mut right = crate::Cva::open(&second_path).unwrap();
    assert_eq!(left.memories.records(), right.memories.records());
    assert_eq!(left.latest_global_version(), right.latest_global_version());
    let left_history = MemoryHistory::read(&mut left.container, &left.memories).unwrap();
    let right_history = MemoryHistory::read(&mut right.container, &right.memories).unwrap();
    for cut in 0..=left.memory_version() {
        assert_eq!(
            left_history.at(cut).unwrap().fingerprint,
            right_history.at(cut).unwrap().fingerprint
        );
    }
    for cut in 0..=left.latest_global_version() {
        assert_eq!(
            crate::historical_global_cuts::reliquary_global_cuts(&left, cut).unwrap(),
            crate::historical_global_cuts::reliquary_global_cuts(&right, cut).unwrap()
        );
        assert_eq!(
            left.container.transaction_time_ns(cut),
            right.container.transaction_time_ns(cut)
        );
    }
}
