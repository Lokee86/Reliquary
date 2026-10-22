use crate::container::publication_envelope::{PublicationClock, PublicationEnvelope, encode};
use crate::{Container, ContainerError};
use std::fs;
use std::path::{Path, PathBuf};

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(".test-tmp")
            .join(format!("publication-clock-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn body(tag: u8) -> Vec<u8> {
    [b"OWNERCLK".as_slice(), &[tag]].concat()
}

fn reservation(global: u64, time: Option<i64>) -> Vec<u8> {
    let mut payload = match time {
        Some(time) => b"CVAVERS2".to_vec(),
        None => b"CVAVERS1".to_vec(),
    };
    payload.extend_from_slice(&global.to_le_bytes());
    if let Some(time) = time {
        payload.extend_from_slice(&time.to_le_bytes());
    }
    payload
}

fn envelope(global: u64, clock: PublicationClock, body: &[u8]) -> Vec<u8> {
    encode(PublicationEnvelope {
        global_version: global,
        clock,
        body,
    })
    .unwrap()
}

fn assert_invalid_version(result: Result<Container, ContainerError>) {
    assert!(matches!(result, Err(ContainerError::InvalidVersionRecord)));
}

fn assert_unsupported_version(result: Result<Container, ContainerError>) {
    assert!(matches!(result, Err(ContainerError::UnsupportedVersion(_))));
}

fn downgrade_header_to_v1(path: &Path) {
    let mut bytes = fs::read(path).unwrap();
    bytes[8..10].copy_from_slice(&1_u16.to_le_bytes());
    bytes[10..12].copy_from_slice(&0_u16.to_le_bytes());
    fs::write(path, bytes).unwrap();
}

#[test]
fn direct_envelope_allocations_preserve_known_missing_and_nonmonotonic_times() {
    let fixture = Fixture::new();
    let path = fixture.path("direct-times.cva");
    let mut container = Container::create(&path).unwrap();
    let times = [
        Some(200),
        Some(100),
        Some(-5),
        Some(i64::MIN),
        Some(i64::MAX),
        None,
    ];
    for (index, time) in times.into_iter().enumerate() {
        container.set_next_transaction_time_override(time);
        let (global, object) = container
            .append_publication_envelope(&body(index as u8))
            .unwrap();
        assert_eq!(global, index as u64 + 1);
        let stored = container.read(object).unwrap();
        assert_eq!(&stored[..8], b"CVAPUB01");
        let decoded = crate::container::publication_envelope::decode(&stored)
            .unwrap()
            .unwrap();
        assert_eq!(decoded.global_version, global);
        assert_eq!(
            decoded.clock,
            PublicationClock::Allocate {
                transaction_time_ns: time
            }
        );
        assert_eq!(decoded.body, body(index as u8));
        assert_eq!(container.latest_version(), global);
        assert_eq!(container.transaction_time_ns(global), time);
    }
    assert_eq!(container.version_at_or_before(199), None);
    assert_eq!(container.version_at_or_before(200), Some(4));
    assert_eq!(container.version_at_or_before(i64::MAX), Some(5));
    container.sync().unwrap();
    drop(container);

    let reopened = Container::open(&path).unwrap();
    assert_eq!(reopened.latest_version(), 6);
    assert_eq!(
        (1..=6)
            .map(|version| reopened.transaction_time_ns(version))
            .collect::<Vec<_>>(),
        times
    );
    assert_eq!(reopened.version_at_or_before(200), Some(4));
    assert_eq!(reopened.version_at_or_before(i64::MAX), Some(5));
}

#[test]
fn reserved_bindings_consume_prior_standalone_tickets_without_advancing_clock() {
    let fixture = Fixture::new();
    let path = fixture.path("reserved-bindings.cva");
    let mut container = Container::create(&path).unwrap();
    container.append(&reservation(1, Some(200))).unwrap();
    container.append(&reservation(2, Some(100))).unwrap();
    container.append(&reservation(3, None)).unwrap();
    container
        .append(&envelope(2, PublicationClock::BindReserved, &body(2)))
        .unwrap();
    container
        .append(&envelope(1, PublicationClock::BindReserved, &body(1)))
        .unwrap();
    container.sync().unwrap();
    drop(container);

    let reopened = Container::open(&path).unwrap();
    assert_eq!(reopened.latest_version(), 3);
    assert_eq!(reopened.transaction_time_ns(1), Some(200));
    assert_eq!(reopened.transaction_time_ns(2), Some(100));
    assert_eq!(reopened.transaction_time_ns(3), None);
    assert_eq!(reopened.version_at_or_before(150), None);
    assert_eq!(reopened.version_at_or_before(200), Some(2));
}

#[test]
fn bounded_versioned_visitor_replays_reserved_reverse_bindings_without_mutation() {
    let fixture = Fixture::new();
    let path = fixture.path("bounded-versioned-reservations.cva");
    let mut container = Container::create(&path).unwrap();
    let mut ends = Vec::new();
    for (global, time) in [(1, Some(200)), (2, Some(100)), (3, None)] {
        container.append(&reservation(global, time)).unwrap();
        ends.push(fs::metadata(&path).unwrap().len());
    }
    container
        .append(&envelope(2, PublicationClock::BindReserved, &body(2)))
        .unwrap();
    ends.push(fs::metadata(&path).unwrap().len());
    container
        .append(&envelope(1, PublicationClock::BindReserved, &body(1)))
        .unwrap();
    ends.push(fs::metadata(&path).unwrap().len());
    container.sync().unwrap();
    drop(container);

    let mut container = Container::open(&path).unwrap();
    let before_bytes = container.physical_digest().unwrap();
    let before_len = container.physical_length().unwrap();
    let before_times = (1..=3)
        .map(|global| container.transaction_time_ns(global))
        .collect::<Vec<_>>();
    let before_head = container.latest_version();
    for (index, end) in ends.into_iter().enumerate() {
        let mut frontiers = Vec::new();
        {
            let mut ingest_frame =
                |frame: crate::container::PhysicalFrame| -> Result<(), crate::ContainerError> {
                    let frontier = frame.latest_global;
                    {
                        frontiers.push(frontier);
                        Ok::<_, ContainerError>(())
                    }
                };
            let mut cursor = container.physical_cursor_through(end).unwrap();
            (|| {
                while let Some(frame) = cursor.next_frame()? {
                    ingest_frame(frame)?;
                }
                Ok::<(), crate::ContainerError>(())
            })()
        }
        .unwrap();
        assert_eq!(frontiers, [1, 2, 3, 3, 3][..=index]);
        assert_eq!(container.latest_version(), before_head);
        assert_eq!(
            (1..=3)
                .map(|global| container.transaction_time_ns(global))
                .collect::<Vec<_>>(),
            before_times
        );
    }
    assert_eq!(container.physical_length().unwrap(), before_len);
    assert_eq!(container.physical_digest().unwrap(), before_bytes);
}

#[test]
fn bounded_versioned_visitor_handles_grouped_global_range_and_optional_time() {
    let (_path, mut cva, _) =
        crate::insomnia::completion_tests::setup("bounded-versioned-grouped-range.cva");
    crate::insomnia::completion_tests::process(&mut cva, 2, 110);
    let end = fs::metadata(cva.container.path()).unwrap().len();
    let before_head = cva.latest_global_version();
    let before_times = (1..=before_head)
        .map(|global| cva.container.transaction_time_ns(global))
        .collect::<Vec<_>>();
    let mut completion_range = None;
    let mut final_frontier = 0;
    {
        let mut ingest_frame =
            |frame: crate::container::PhysicalFrame| -> Result<(), crate::ContainerError> {
                let payload = frame.payload.as_slice();
                let frontier = frame.latest_global;
                {
                    final_frontier = frontier;
                    if let Some((start, count, time)) =
                        crate::insomnia::completion::decode_embedded_version_range(payload)
                            .map_err(|_| ContainerError::InvalidVersionRecord)?
                    {
                        completion_range = Some((start, count, time, frontier));
                    }
                    Ok::<_, ContainerError>(())
                }
            };
        let mut cursor = cva.container.physical_cursor_through(end).unwrap();
        (|| {
            while let Some(frame) = cursor.next_frame()? {
                ingest_frame(frame)?;
            }
            Ok::<(), crate::ContainerError>(())
        })()
    }
    .unwrap();
    let (start, count, time, frontier) = completion_range.expect("grouped range");
    assert!(count > 0);
    assert_eq!(frontier, start + u64::from(count) - 1);
    assert_eq!(time, before_times[(start - 1) as usize]);
    for global in start..start + u64::from(count) {
        assert_eq!(before_times[(global - 1) as usize], time);
    }
    assert_eq!(final_frontier, before_head);
    assert_eq!(cva.latest_global_version(), before_head);
    assert_eq!(
        (1..=before_head)
            .map(|global| cva.container.transaction_time_ns(global))
            .collect::<Vec<_>>(),
        before_times
    );
}

#[test]
fn bounded_versioned_visitor_rejects_midframe_malformed_clock_and_v1_envelopes_read_only() {
    let fixture = Fixture::new();
    let path = fixture.path("bounded-invalid-prefix.cva");
    let mut container = Container::create(&path).unwrap();
    let (_, object) = container.append_publication_envelope(&body(1)).unwrap();
    container.sync().unwrap();
    let object_end = object.legacy_offset() + 8 + object.legacy_len();
    drop(container);

    let mut container = Container::open_read_only_for_scan(&path).unwrap();
    let original = fs::read(&path).unwrap();
    assert!(matches!(
        {
            let mut ingest_frame =
                |frame: crate::container::PhysicalFrame| -> Result<(), crate::ContainerError> {
                    { Ok::<_, ContainerError>(()) }
                };
            let mut cursor = container.physical_cursor_through(object_end - 1).unwrap();
            (|| {
                while let Some(frame) = cursor.next_frame()? {
                    ingest_frame(frame)?;
                }
                Ok::<(), crate::ContainerError>(())
            })()
        },
        Err(ContainerError::TruncatedChunk(_))
    ));
    assert_eq!(fs::read(&path).unwrap(), original);
    drop(container);

    let truncated_path = fixture.path("bounded-later-truncated-tail.cva");
    let mut container = Container::create(&truncated_path).unwrap();
    let (_, valid_object) = container.append_publication_envelope(&body(3)).unwrap();
    container.sync().unwrap();
    let valid_end = valid_object.legacy_offset() + 8 + valid_object.legacy_len();
    drop(container);
    {
        use std::io::Write;
        let mut file = fs::OpenOptions::new()
            .append(true)
            .open(&truncated_path)
            .unwrap();
        file.write_all(&10_u64.to_le_bytes()).unwrap();
        file.write_all(b"xx").unwrap();
    }
    let original = fs::read(&truncated_path).unwrap();
    let mut container = Container::open_read_only_for_scan(&truncated_path).unwrap();
    let mut bounded_frontier = 0;
    {
        let mut ingest_frame =
            |frame: crate::container::PhysicalFrame| -> Result<(), crate::ContainerError> {
                let frontier = frame.latest_global;
                {
                    bounded_frontier = frontier;
                    Ok::<_, ContainerError>(())
                }
            };
        let mut cursor = container.physical_cursor_through(valid_end).unwrap();
        (|| {
            while let Some(frame) = cursor.next_frame()? {
                ingest_frame(frame)?;
            }
            Ok::<(), crate::ContainerError>(())
        })()
    }
    .unwrap();
    assert_eq!(bounded_frontier, 1);
    assert!(matches!(
        {
            let mut ingest_frame =
                |frame: crate::container::PhysicalFrame| -> Result<(), crate::ContainerError> {
                    { Ok::<_, ContainerError>(()) }
                };
            let mut cursor = container
                .physical_cursor_through(original.len() as u64)
                .unwrap();
            (|| {
                while let Some(frame) = cursor.next_frame()? {
                    ingest_frame(frame)?;
                }
                Ok::<(), crate::ContainerError>(())
            })()
        },
        Err(ContainerError::RepairRequired { valid_end: tail_start }) if tail_start == valid_end
    ));
    assert_eq!(fs::read(&truncated_path).unwrap(), original);
    drop(container);

    let malformed_path = fixture.path("bounded-malformed-clock.cva");
    let mut container = Container::create(&malformed_path).unwrap();
    let mut malformed = b"CVAPUB01".to_vec();
    malformed.extend_from_slice(&1_u64.to_le_bytes());
    malformed.push(0xFF);
    malformed.extend_from_slice(b"OWNERCLK");
    container.append(&malformed).unwrap();
    container.sync().unwrap();
    drop(container);
    let original = fs::read(&malformed_path).unwrap();
    let mut container = Container::open_read_only_for_scan(&malformed_path).unwrap();
    assert!(matches!(
        {
            let mut ingest_frame =
                |frame: crate::container::PhysicalFrame| -> Result<(), crate::ContainerError> {
                    { Ok::<_, ContainerError>(()) }
                };
            let mut cursor = container
                .physical_cursor_through(original.len() as u64)
                .unwrap();
            (|| {
                while let Some(frame) = cursor.next_frame()? {
                    ingest_frame(frame)?;
                }
                Ok::<(), crate::ContainerError>(())
            })()
        },
        Err(ContainerError::InvalidVersionRecord)
    ));
    assert_eq!(fs::read(&malformed_path).unwrap(), original);
    drop(container);

    let v1_path = fixture.path("bounded-v1-envelope.cva");
    let mut container = Container::create(&v1_path).unwrap();
    container.append_publication_envelope(&body(2)).unwrap();
    container.sync().unwrap();
    drop(container);
    downgrade_header_to_v1(&v1_path);
    let original = fs::read(&v1_path).unwrap();
    assert_unsupported_version(Container::open_read_only_for_scan(&v1_path));
    assert_eq!(fs::read(&v1_path).unwrap(), original);
}

#[test]
fn scan_keeps_extreme_and_missing_ticket_times_when_later_tickets_are_bound_first() {
    let fixture = Fixture::new();
    let path = fixture.path("extreme-reservations.cva");
    let mut container = Container::create(&path).unwrap();
    container.append(&reservation(1, Some(i64::MIN))).unwrap();
    container.append(&reservation(2, Some(i64::MAX))).unwrap();
    container.append(&reservation(3, None)).unwrap();
    container.sync().unwrap();
    drop(container);
    let mut container = Container::open(&path).unwrap();
    container
        .append(&envelope(2, PublicationClock::BindReserved, &body(2)))
        .unwrap();
    assert_eq!(container.latest_version(), 3);
    assert_eq!(container.version_at_or_before(i64::MIN), Some(1));
    assert_eq!(container.version_at_or_before(i64::MAX), Some(2));
    container
        .append(&envelope(1, PublicationClock::BindReserved, &body(1)))
        .unwrap();
    container.sync().unwrap();
    drop(container);

    let reopened = Container::open(&path).unwrap();
    assert_eq!(reopened.latest_version(), 3);
    assert_eq!(reopened.transaction_time_ns(1), Some(i64::MIN));
    assert_eq!(reopened.transaction_time_ns(2), Some(i64::MAX));
    assert_eq!(reopened.transaction_time_ns(3), None);
    assert_eq!(reopened.version_at_or_before(i64::MIN), Some(1));
    assert_eq!(reopened.version_at_or_before(i64::MAX), Some(2));
}

#[test]
fn rejects_bindings_without_a_unique_prior_standalone_reservation() {
    let fixture = Fixture::new();

    let future_path = fixture.path("future-binding.cva");
    let mut future = Container::create(&future_path).unwrap();
    future
        .allocate_version_with_transaction_time(Some(1))
        .unwrap();
    future
        .append(&envelope(2, PublicationClock::BindReserved, &body(0)))
        .unwrap();
    drop(future);
    assert_invalid_version(Container::open(&future_path));

    let reused_path = fixture.path("reused-binding.cva");
    let mut reused = Container::create(&reused_path).unwrap();
    reused
        .allocate_version_with_transaction_time(Some(1))
        .unwrap();
    let binding = envelope(1, PublicationClock::BindReserved, &body(1));
    reused.append(&binding).unwrap();
    reused.append(&binding).unwrap();
    drop(reused);
    assert_invalid_version(Container::open(&reused_path));

    let direct_path = fixture.path("direct-binding.cva");
    let mut direct = Container::create(&direct_path).unwrap();
    direct
        .append(&envelope(
            1,
            PublicationClock::Allocate {
                transaction_time_ns: Some(10),
            },
            &body(1),
        ))
        .unwrap();
    direct
        .append(&envelope(1, PublicationClock::BindReserved, &body(2)))
        .unwrap();
    drop(direct);
    assert_invalid_version(Container::open(&direct_path));

    let grouped_path = fixture.path("grouped-binding.cva");
    let mut grouped = Container::create(&grouped_path).unwrap();
    let mut completion = vec![0_u8; 84];
    completion[..8].copy_from_slice(b"CVAINSC5");
    completion[64..72].copy_from_slice(&1_u64.to_le_bytes());
    completion[72..76].copy_from_slice(&2_u32.to_le_bytes());
    completion[76..84].copy_from_slice(&100_i64.to_le_bytes());
    grouped.append(&completion).unwrap();
    grouped
        .append(&envelope(1, PublicationClock::BindReserved, &body(3)))
        .unwrap();
    drop(grouped);
    assert_invalid_version(Container::open(&grouped_path));
}

#[test]
fn rejects_duplicate_or_gapped_direct_allocations() {
    let fixture = Fixture::new();

    let gap_path = fixture.path("allocation-gap.cva");
    let mut gap = Container::create(&gap_path).unwrap();
    gap.append(&envelope(
        2,
        PublicationClock::Allocate {
            transaction_time_ns: None,
        },
        &body(1),
    ))
    .unwrap();
    drop(gap);
    assert_invalid_version(Container::open(&gap_path));

    let duplicate_path = fixture.path("allocation-duplicate.cva");
    let mut duplicate = Container::create(&duplicate_path).unwrap();
    let allocation = envelope(
        1,
        PublicationClock::Allocate {
            transaction_time_ns: Some(50),
        },
        &body(2),
    );
    duplicate.append(&allocation).unwrap();
    duplicate.append(&allocation).unwrap();
    drop(duplicate);
    assert_invalid_version(Container::open(&duplicate_path));
}

#[test]
fn invalid_direct_append_preserves_bytes_frontier_and_time_override() {
    let fixture = Fixture::new();
    let path = fixture.path("invalid-direct-append.cva");
    let mut container = Container::create(&path).unwrap();
    container.set_next_transaction_time_override(Some(-77));
    let before_bytes = container.physical_digest().unwrap();
    let before_len = container.physical_length().unwrap();
    assert!(matches!(
        container.append_publication_envelope(b"short"),
        Err(ContainerError::InvalidVersionRecord)
    ));
    assert_eq!(container.physical_length().unwrap(), before_len);
    assert_eq!(container.physical_digest().unwrap(), before_bytes);
    assert_eq!(container.latest_version(), 0);
    assert_eq!(container.next_version_candidate(), 1);
    assert_eq!(container.next_transaction_time_override, Some(Some(-77)));

    let (global, _) = container.append_publication_envelope(&body(5)).unwrap();
    assert_eq!(global, 1);
    assert_eq!(container.transaction_time_ns(1), Some(-77));
    assert_eq!(container.next_transaction_time_override, None);
}

#[test]
fn closed_admission_and_version_overflow_fail_before_writing() {
    let fixture = Fixture::new();
    let path = fixture.path("write-admission-closed.cva");
    let mut container = Container::create(&path).unwrap();
    let before_bytes = container.physical_digest().unwrap();
    let before_len = container.physical_length().unwrap();
    container.set_write_admission_closed(true);
    assert!(matches!(
        container.append_publication_envelope(&body(1)),
        Err(ContainerError::WriteAdmissionClosed)
    ));
    assert_eq!(container.physical_length().unwrap(), before_len);
    assert_eq!(container.physical_digest().unwrap(), before_bytes);
    assert_eq!(container.latest_version(), 0);
    drop(container);

    let overflow_path = fixture.path("version-overflow.cva");
    let mut overflow = Container::create(&overflow_path).unwrap();
    overflow.next_version = u64::MAX;
    overflow.set_next_transaction_time_override(Some(123));
    let before_bytes = overflow.physical_digest().unwrap();
    let before_len = overflow.physical_length().unwrap();
    assert!(matches!(
        overflow.append_publication_envelope(&body(2)),
        Err(ContainerError::VersionExhausted)
    ));
    assert_eq!(overflow.physical_length().unwrap(), before_len);
    assert_eq!(overflow.physical_digest().unwrap(), before_bytes);
    assert_eq!(overflow.next_transaction_time_override, Some(Some(123)));
}

#[test]
fn v1_rejects_publication_envelopes() {
    let fixture = Fixture::new();
    let path = fixture.path("v1-envelope.cva");
    let mut container = Container::create(&path).unwrap();
    container.append_publication_envelope(&body(1)).unwrap();
    container.sync().unwrap();
    drop(container);
    downgrade_header_to_v1(&path);
    assert_unsupported_version(Container::open(&path));
}

#[test]
fn every_incomplete_reserved_binding_tail_preserves_clock_and_reservation() {
    let fixture = Fixture::new();
    let binding = envelope(1, PublicationClock::BindReserved, &body(7));
    let frame_len = 8 + binding.len() as u64;
    for prefix_len in 0..frame_len {
        let path = fixture.path(&format!("partial-bind-{prefix_len}.cva"));
        let mut container = Container::create(&path).unwrap();
        container
            .allocate_version_with_transaction_time(Some(1234))
            .unwrap();
        let frame_start = fs::metadata(&path).unwrap().len();
        container.append(&binding).unwrap();
        container.sync().unwrap();
        drop(container);

        let file = fs::OpenOptions::new().write(true).open(&path).unwrap();
        file.set_len(frame_start + prefix_len).unwrap();
        drop(file);

        let mut reopened = if prefix_len == 0 {
            Container::open(&path).unwrap()
        } else {
            let before_open = fs::read(&path).unwrap();
            assert!(matches!(
                Container::open(&path),
                Err(ContainerError::RepairRequired { valid_end }) if valid_end == frame_start
            ));
            assert_eq!(fs::read(&path).unwrap(), before_open);
            let mut repair = Container::open_for_repair(&path).unwrap();
            let plan = repair.plan_tail_repair().unwrap().unwrap();
            repair.repair_incomplete_tail(plan).unwrap();
            repair
        };
        assert_eq!(reopened.physical_length().unwrap(), frame_start);
        assert_eq!(reopened.latest_version(), 1, "prefix length {prefix_len}");
        assert_eq!(reopened.transaction_time_ns(1), Some(1234));
        reopened.append(&binding).unwrap();
        reopened.sync().unwrap();
        drop(reopened);
        let reopened = Container::open(&path).unwrap();
        assert_eq!(reopened.latest_version(), 1);
        assert_eq!(reopened.transaction_time_ns(1), Some(1234));
    }
}

#[test]
fn historical_frontier_tracks_direct_allocation_cuts() {
    let fixture = Fixture::new();
    let path = fixture.path("direct-frontiers.cva");
    let mut container = Container::create(&path).unwrap();
    let mut ends = vec![fs::metadata(&path).unwrap().len()];
    for tag in 0..3 {
        container.append_publication_envelope(&body(tag)).unwrap();
        ends.push(fs::metadata(&path).unwrap().len());
    }
    container.sync().unwrap();
    for (cut, expected_end) in ends.into_iter().enumerate() {
        assert_eq!(
            crate::historical_global_frontier::frontier_end(&container, cut as u64).unwrap(),
            expected_end,
            "global cut {cut}"
        );
    }
}

#[test]
fn historical_frontier_excludes_late_binding_before_head_cut_only() {
    let fixture = Fixture::new();
    let path = fixture.path("late-bind-frontiers.cva");
    let mut container = Container::create(&path).unwrap();
    let mut ends = vec![fs::metadata(&path).unwrap().len()];
    for global in 1..=3 {
        container
            .append(&reservation(global, Some(global as i64 * 10)))
            .unwrap();
        ends.push(fs::metadata(&path).unwrap().len());
    }
    container.sync().unwrap();
    drop(container);
    let mut container = Container::open(&path).unwrap();
    container
        .append(&envelope(2, PublicationClock::BindReserved, &body(2)))
        .unwrap();
    container.sync().unwrap();
    ends[3] = fs::metadata(&path).unwrap().len();

    assert_eq!(container.latest_version(), 3);
    for (cut, expected_end) in ends.into_iter().enumerate() {
        assert_eq!(
            crate::historical_global_frontier::frontier_end(&container, cut as u64).unwrap(),
            expected_end,
            "global cut {cut}"
        );
    }
}

#[test]
fn every_incomplete_direct_allocation_tail_recovers_without_advancing_clock() {
    let fixture = Fixture::new();
    let allocation = envelope(
        1,
        PublicationClock::Allocate {
            transaction_time_ns: Some(-456),
        },
        &body(8),
    );
    let frame_len = 8 + allocation.len() as u64;
    for prefix_len in 0..frame_len {
        let path = fixture.path(&format!("partial-allocation-{prefix_len}.cva"));
        let mut container = Container::create(&path).unwrap();
        let header_end = fs::metadata(&path).unwrap().len();
        container.append(&allocation).unwrap();
        container.sync().unwrap();
        drop(container);

        let file = fs::OpenOptions::new().write(true).open(&path).unwrap();
        file.set_len(header_end + prefix_len).unwrap();
        drop(file);

        let mut reopened = if prefix_len == 0 {
            Container::open(&path).unwrap()
        } else {
            let before_open = fs::read(&path).unwrap();
            assert!(matches!(
                Container::open(&path),
                Err(ContainerError::RepairRequired { valid_end }) if valid_end == header_end
            ));
            assert_eq!(fs::read(&path).unwrap(), before_open);
            let mut repair = Container::open_for_repair(&path).unwrap();
            let plan = repair.plan_tail_repair().unwrap().unwrap();
            repair.repair_incomplete_tail(plan).unwrap();
            repair
        };
        assert_eq!(reopened.physical_length().unwrap(), header_end);
        assert_eq!(reopened.latest_version(), 0, "prefix length {prefix_len}");
        assert_eq!(reopened.transaction_time_ns(1), None);
        let (version, _) = reopened.append_publication_envelope(&body(9)).unwrap();
        assert_eq!(version, 1);
        assert_eq!(reopened.latest_version(), 1);
        assert_ne!(reopened.transaction_time_ns(1), Some(-456));
    }
}

#[test]
fn v1_rejects_complete_malformed_publication_prefix() {
    let fixture = Fixture::new();
    let path = fixture.path("v1-malformed-envelope-prefix.cva");
    let mut container = Container::create(&path).unwrap();
    container.append(b"CVAPUB").unwrap();
    container.sync().unwrap();
    drop(container);
    downgrade_header_to_v1(&path);
    assert_unsupported_version(Container::open(&path));
}

#[test]
fn v2_complete_invalid_envelopes_are_rejected_without_tail_repair() {
    let fixture = Fixture::new();
    let valid = envelope(
        1,
        PublicationClock::Allocate {
            transaction_time_ns: Some(7),
        },
        &body(1),
    );
    for (name, mutate) in [
        (
            "bad-magic.cva",
            (|payload: &mut Vec<u8>| payload[7] = b'2') as fn(&mut Vec<u8>),
        ),
        (
            "bad-tag.cva",
            (|payload: &mut Vec<u8>| payload[16] = 9) as fn(&mut Vec<u8>),
        ),
    ] {
        let path = fixture.path(name);
        let mut payload = valid.clone();
        mutate(&mut payload);
        let mut container = Container::create(&path).unwrap();
        container.append(&payload).unwrap();
        container.sync().unwrap();
        drop(container);
        let before = fs::read(&path).unwrap();
        assert_invalid_version(Container::open(&path));
        assert_eq!(fs::read(&path).unwrap(), before);
    }
}
