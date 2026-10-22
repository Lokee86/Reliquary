use super::*;
use crate::owner_lifecycle::LifecycleTarget;

#[test]
fn envelope_history_digest_matches_marker_history_at_every_global_watermark() {
    let mut digests = Vec::new();
    for compact in [false, true] {
        let path = test_path("envelope-digest.cva");
        let mut container = initialize_container(&path);
        let mut first = initial_record();
        first.body_id = append_body(&mut container, "Digest title", "Digest body");
        first.global_version = 2;
        let mut next = successor(&first);
        next.global_version = 1;
        for time in [Some(-10), None, Some(50)] {
            container
                .allocate_version_with_transaction_time(time)
                .unwrap();
        }
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
            let carrier = container
                .append(&encode_creation(&MemoryCreation::new(first.clone()).unwrap()).unwrap())
                .unwrap();
            append_version_marker(&mut container, carrier, 2, 1);
            let carrier = container
                .append(&encode_delta(&MemoryDelta::between(&first, &next).unwrap()).unwrap())
                .unwrap();
            append_version_marker(&mut container, carrier, 1, 2);
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
        replay(&path).unwrap();
        let mut container = Container::open(&path).unwrap();
        let graph = crate::graph_store::GraphStore::empty();
        digests.push(
            (0..=3)
                .map(|cut| {
                    crate::owner_history_proof::owner_history_digest(&mut container, cut, &graph)
                        .unwrap()
                })
                .collect::<Vec<_>>(),
        );
    }
    assert_eq!(
        digests[0], digests[1],
        "canonical evidence preserves existing proof bytes"
    );
}

fn tamper_creation_mutation(path: &Path) {
    let mut container = Container::open(path).unwrap();
    let mut position = None;
    for object in container.chunks().unwrap() {
        let raw = container.read(object).unwrap();
        if let Some(envelope) = crate::container::publication_envelope::decode(&raw).unwrap() {
            if envelope.body.starts_with(b"CVAMC002") {
                let relative = raw
                    .windows(b"creation".len())
                    .position(|bytes| bytes == b"creation")
                    .unwrap();
                position = Some(object.offset() as usize + 8 + relative + 7);
                break;
            }
        }
    }
    drop(container);
    let mut bytes = fs::read(path).unwrap();
    bytes[position.unwrap()] = b'X';
    fs::write(path, bytes).unwrap();
}

#[test]
fn native_rel_and_phy_prune_bind_envelope_facts_and_reject_valid_mutation_tampering() {
    for kind in [FileKind::Reliquary, FileKind::Phylactery] {
        let source = test_path("envelope-prune-source.cva");
        let output = test_path("envelope-pruned.cva");
        if kind == FileKind::Reliquary {
            drop(crate::Cva::create(&source).unwrap());
        } else {
            drop(crate::Phylactery::create(&source).unwrap());
        }
        let mut container = Container::open(&source).unwrap();
        let mut first = initial_record();
        first.body_id = append_body(
            &mut container,
            "Prunable",
            &"large immutable body ".repeat(1024),
        );
        let next = successor(&first);
        for (global, publication) in [
            (
                1,
                MemoryPublication::Creation(MemoryCreation::new(first.clone()).unwrap()),
            ),
            (
                2,
                MemoryPublication::Delta(MemoryDelta::between(&first, &next).unwrap()),
            ),
            (
                3,
                MemoryPublication::Tombstone {
                    id: first.id,
                    revision: 3,
                    memory_version: 3,
                },
            ),
        ] {
            append_publication(
                &mut container,
                global,
                PublicationClock::Allocate {
                    transaction_time_ns: Some(global as i64 * 10),
                },
                publication,
            );
        }
        container.sync().unwrap();
        drop(container);
        let source_bytes = fs::read(&source).unwrap();
        let request = crate::HistoryPruneRequest {
            targets: vec![LifecycleTarget::Memory(first.id)],
        };
        if kind == FileKind::Reliquary {
            let mut owner = crate::Cva::open(&source).unwrap();
            let expected_prefix = owner
                .memories
                .first_publication_prefix(owner.memory_version())
                .unwrap();
            let report = owner.prune_tombstoned_history(&output, request).unwrap();
            assert_eq!(report.pruned_memory_bodies, 1);
            drop(owner);
            let mut pruned = crate::Cva::open(&output).unwrap();
            assert_eq!(pruned.memory_version(), 3);
            assert_eq!(
                pruned
                    .memories
                    .first_publication_prefix(pruned.memory_version())
                    .unwrap(),
                expected_prefix,
                "pruning body bytes must preserve the accepted identity prefix"
            );
            assert!(matches!(
                pruned.memory_revision(first.id, 1),
                Err(MemoryError::PayloadUnavailable(_))
            ));
        } else {
            let mut owner = crate::Phylactery::open(&source).unwrap();
            let expected_prefix = owner
                .memories
                .first_publication_prefix(owner.memory_version())
                .unwrap();
            let report = owner.prune_tombstoned_history(&output, request).unwrap();
            assert_eq!(report.pruned_memory_bodies, 1);
            drop(owner);
            let mut pruned = crate::Phylactery::open(&output).unwrap();
            assert_eq!(pruned.memory_version(), 3);
            assert_eq!(
                pruned
                    .memories
                    .first_publication_prefix(pruned.memory_version())
                    .unwrap(),
                expected_prefix,
                "pruning body bytes must preserve the accepted identity prefix"
            );
            assert!(matches!(
                pruned.memory_revision(first.id, 1),
                Err(MemoryError::PayloadUnavailable(_))
            ));
        }
        assert_eq!(fs::read(&source).unwrap(), source_bytes);
        tamper_creation_mutation(&output);
        if kind == FileKind::Reliquary {
            assert!(crate::Cva::open(&output).is_err());
        } else {
            assert!(crate::Phylactery::open(&output).is_err());
        }
    }
}
