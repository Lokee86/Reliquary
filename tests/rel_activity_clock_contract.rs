//! REL activity reference-ledger contracts for live acceptance and validated reopen.
use reliquary_memory::{Branch, Container, Cva, IncomingAttachment, IncomingTurn};
use std::collections::BTreeMap;

struct Fixture {
    dir: std::path::PathBuf,
    cva: Option<Cva>,
}
impl Fixture {
    fn new() -> Self {
        let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(".test-tmp")
            .join(format!("rel-activity-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let cva = Some(Cva::create(dir.join("clock.rel")).unwrap());
        Self { dir, cva }
    }
    fn cva(&self) -> &Cva {
        self.cva.as_ref().unwrap()
    }
    fn cva_mut(&mut self) -> &mut Cva {
        self.cva.as_mut().unwrap()
    }
    fn reopen(&mut self) {
        self.cva().sync().unwrap();
        drop(self.cva.take());
        self.cva = Some(Cva::open(self.dir.join("clock.rel")).unwrap());
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        drop(self.cva.take());
        std::fs::remove_dir_all(&self.dir).unwrap();
    }
}

// Explicit expected counts are the oracle, not production eligibility logic.
// Record every observed version boundary, including no-op operations.
#[derive(Default)]
struct ReferenceLedger {
    count: u64,
    positions: BTreeMap<(String, String), Option<u64>>,
    cuts: BTreeMap<u64, u64>,
}
impl ReferenceLedger {
    fn observe(&mut self, version: u64, turn: Option<(&str, &str, Option<u64>)>, count: u64) {
        self.count = count;
        self.cuts.insert(version, count);
        if let Some((conversation, id, position)) = turn {
            let old = self
                .positions
                .entry((conversation.into(), id.into()))
                .or_insert(position);
            assert_eq!(*old, position, "a replay must retain its first position");
        }
    }
    fn assert_matches(&self, cva: &Cva) {
        assert_eq!(cva.rel_turn_count(), self.count);
        assert_eq!(cva.activity_cut_at_archive_version(0).unwrap(), 0);
        for (version, count) in &self.cuts {
            assert_eq!(
                cva.activity_cut_at_archive_version(*version).unwrap(),
                *count
            );
        }
        for ((conversation, id), position) in &self.positions {
            assert_eq!(cva.activity_position_for_turn(conversation, id), *position);
        }
        assert_eq!(cva.activity_position_for_turn("absent", "absent"), None);
        assert!(
            cva.activity_cut_at_archive_version(cva.archive_version() + 1)
                .is_err()
        );
    }
}
fn turn(conversation: &str, id: &str, role: &str, parent: Option<&str>) -> IncomingTurn {
    IncomingTurn {
        id: id.into(),
        conversation_id: conversation.into(),
        parent_id: parent.map(str::to_owned),
        role: role.into(),
        principal_id: None,
        timestamp_ns: -100,
        content: "identical historical content".into(),
        attachments: Vec::new(),
        project_attachments: Vec::new(),
    }
}
fn append(cva: &mut Cva, incoming: &IncomingTurn) {
    cva.append_node(
        incoming.id.clone(),
        incoming.conversation_id.clone(),
        incoming.parent_id.clone(),
        incoming.role.clone(),
        incoming.timestamp_ns,
        &incoming.content,
    )
    .unwrap();
}

#[test]
fn empty_rel_and_independent_owner_have_zero_activity() {
    let mut first = Fixture::new();
    let other = Fixture::new();
    ReferenceLedger::default().assert_matches(first.cva());
    first
        .cva_mut()
        .ingest_turn(turn("c", "u", "user", None))
        .unwrap();
    assert_eq!(first.cva().rel_turn_count(), 1);
    ReferenceLedger::default().assert_matches(other.cva());
    first.reopen();
    assert_eq!(first.cva().rel_turn_count(), 1);
}

#[test]
fn mixed_acceptance_paths_follow_rel_wide_publication_order() {
    let mut fixture = Fixture::new();
    let mut ledger = ReferenceLedger::default();
    // Same timestamp/content, different identity; sibling branches and conversations
    // share one clock. Noneligible roles are explicit negative cases.
    let rows = [
        ("a", "root", "user", None, false, Some(1), 1),
        ("b", "root", "assistant", None, true, Some(2), 2),
        ("a", "system", "system", Some("root"), false, None, 2),
        ("a", "left", "assistant", Some("root"), true, Some(3), 3),
        ("a", "right", "user", Some("root"), false, Some(4), 4),
        ("a", "tool", "tool", Some("right"), true, None, 4),
        ("a", "agent", "agent", Some("right"), false, None, 4),
    ];
    for (conversation, id, role, parent, ingest, position, count) in rows {
        let incoming = turn(conversation, id, role, parent);
        if ingest {
            fixture.cva_mut().ingest_turn(incoming).unwrap();
        } else {
            append(fixture.cva_mut(), &incoming);
        }
        ledger.observe(
            fixture.cva().archive_version(),
            Some((conversation, id, position)),
            count,
        );
        ledger.assert_matches(fixture.cva());
    }
    fixture
        .cva_mut()
        .append_branch(Branch {
            id: "main".into(),
            conversation_id: "a".into(),
            leaf_node_id: "left".into(),
            canonical: true,
        })
        .unwrap();
    ledger.observe(fixture.cva().archive_version(), None, 4);
    ledger.assert_matches(fixture.cva());
    fixture.reopen();
    ledger.assert_matches(fixture.cva());
}

#[test]
fn compatible_replays_in_both_formats_retain_first_position() {
    for first_is_ingest in [false, true] {
        let mut fixture = Fixture::new();
        let mut ledger = ReferenceLedger::default();
        let incoming = turn("c", "u", "user", None);
        if first_is_ingest {
            fixture.cva_mut().ingest_turn(incoming.clone()).unwrap();
        } else {
            append(fixture.cva_mut(), &incoming);
        }
        ledger.observe(
            fixture.cva().archive_version(),
            Some(("c", "u", Some(1))),
            1,
        );
        let version = fixture.cva().archive_version();
        append(fixture.cva_mut(), &incoming);
        fixture.cva_mut().ingest_turn(incoming.clone()).unwrap();
        assert_eq!(fixture.cva().archive_version(), version);
        fixture
            .cva_mut()
            .ingest_turn(turn("c", "next", "assistant", Some("u")))
            .unwrap();
        ledger.observe(
            fixture.cva().archive_version(),
            Some(("c", "next", Some(2))),
            2,
        );
        append(fixture.cva_mut(), &incoming);
        fixture.cva_mut().ingest_turn(incoming).unwrap();
        ledger.assert_matches(fixture.cva());
        fixture.reopen();
        ledger.assert_matches(fixture.cva());
    }
}

#[test]
fn rejected_node_and_attachment_writes_leave_activity_and_history_unchanged() {
    let mut fixture = Fixture::new();
    let mut incoming = turn("c", "u", "user", None);
    incoming.attachments.push(IncomingAttachment {
        filename: "evidence.txt".into(),
        mime_type: None,
        bytes: b"evidence".to_vec(),
    });
    fixture.cva_mut().ingest_turn(incoming.clone()).unwrap();
    let version = fixture.cva().archive_version();
    let mut ledger = ReferenceLedger::default();
    ledger.observe(version, Some(("c", "u", Some(1))), 1);
    let mut changed = incoming.clone();
    changed.content = "conflict".into();
    assert!(fixture.cva_mut().ingest_turn(changed).is_err());
    let mut changed = incoming.clone();
    changed.attachments[0].filename = "drift.txt".into();
    assert!(fixture.cva_mut().ingest_turn(changed).is_err());
    let mut invalid = turn("c", "invalid", "user", Some("missing"));
    assert!(fixture.cva_mut().ingest_turn(invalid.clone()).is_err());
    invalid.parent_id = None;
    invalid.attachments = vec![incoming.attachments[0].clone(); 2];
    assert!(fixture.cva_mut().ingest_turn(invalid).is_err());
    assert!(
        fixture
            .cva_mut()
            .append_node(
                "u".into(),
                "c".into(),
                None,
                "user".into(),
                -100,
                "conflict"
            )
            .is_err()
    );
    assert_eq!(fixture.cva().archive_version(), version);
    assert_eq!(
        fixture.cva().activity_position_for_turn("c", "invalid"),
        None
    );
    ledger.assert_matches(fixture.cva());
    fixture.reopen();
    ledger.assert_matches(fixture.cva());
}

#[test]
fn attachments_and_old_imported_timestamps_count_one_logical_turn() {
    let mut fixture = Fixture::new();
    let mut incoming = turn("import", "old", "user", None);
    incoming.attachments = (0..3)
        .map(|i| IncomingAttachment {
            filename: format!("attachment-{i}.txt"),
            mime_type: None,
            bytes: b"same bytes".to_vec(),
        })
        .collect();
    fixture.cva_mut().ingest_turn(incoming.clone()).unwrap();
    fixture.cva_mut().ingest_turn(incoming).unwrap();
    let mut ledger = ReferenceLedger::default();
    ledger.observe(
        fixture.cva().archive_version(),
        Some(("import", "old", Some(1))),
        1,
    );
    ledger.assert_matches(fixture.cva());
    fixture.reopen();
    ledger.assert_matches(fixture.cva());
}

fn append_fixture_publication(
    container: &mut Container,
    _path: &std::path::Path,
    body: &[u8],
    global: u64,
    archive: u64,
) {
    let mut body = body.to_vec();
    body[8..16].copy_from_slice(&archive.to_le_bytes());
    append_fixture_global_version(container, global);
    let mut envelope = b"CVAPUB01".to_vec();
    envelope.extend_from_slice(&global.to_le_bytes());
    envelope.push(2); // Bind the exact existing global reservation.
    envelope.extend_from_slice(&body);
    container.append(&envelope).unwrap();
}

#[test]
fn distinct_backing_records_deduplicate_identity_and_conflicts_fail_closed() {
    for conflict in [false, true] {
        let mut fixture = Fixture::new();
        append(fixture.cva_mut(), &turn("c", "u", "user", None));
        fixture
            .cva_mut()
            .ingest_turn(turn("c", "a", "assistant", None))
            .unwrap();
        fixture.cva().sync().unwrap();
        drop(fixture.cva.take());
        let path = fixture.dir.join("clock.rel");
        let mut container = Container::open(&path).unwrap();
        let payloads = container
            .chunks()
            .unwrap()
            .into_iter()
            .map(|record| container.read(record).unwrap())
            .collect::<Vec<_>>();
        // Current Node -> current IngestedTurn of the same immutable identity, no attachments.
        let mut duplicate = current_publication_body(&payloads, b"CVANOD02");
        duplicate[..8].copy_from_slice(b"CVATUR03");
        duplicate.extend_from_slice(&0_u32.to_le_bytes());
        if conflict {
            duplicate[16..24].copy_from_slice(&999_i64.to_le_bytes());
        }
        // Another owner reserves global version 3 without an Archive publication.
        append_fixture_global_version(&mut container, 3);
        append_fixture_publication(&mut container, &path, &duplicate, 4, 3);
        container.sync().unwrap();
        drop(container);
        let before = std::fs::read(&path).unwrap();
        let reopened = Cva::open(&path);
        if conflict {
            assert!(matches!(
                reopened,
                Err(reliquary_memory::CvaError::Archive(
                    reliquary_memory::ArchiveError::ConflictingNode
                ))
            ));
        } else {
            fixture.cva = Some(reopened.unwrap());
            let mut ledger = ReferenceLedger::default();
            ledger.observe(1, Some(("c", "u", Some(1))), 1);
            ledger.observe(2, Some(("c", "a", Some(2))), 2);
            ledger.observe(3, None, 2);
            ledger.assert_matches(fixture.cva());
        }
        assert_eq!(
            std::fs::read(&path).unwrap(),
            before,
            "open and activity reads must not rewrite history"
        );
    }
}

#[test]
fn invalid_publication_cut_fails_closed_instead_of_counting_payload() {
    let mut fixture = Fixture::new();
    append(fixture.cva_mut(), &turn("c", "u", "user", None));
    fixture.cva().sync().unwrap();
    drop(fixture.cva.take());
    let path = fixture.dir.join("clock.rel");
    let mut container = Container::open(&path).unwrap();
    let payloads = container
        .chunks()
        .unwrap()
        .into_iter()
        .map(|record| container.read(record).unwrap())
        .collect::<Vec<_>>();
    let legacy = current_publication_body(&payloads, b"CVANOD02");
    // Archive version 3 skips the required version 2.
    append_fixture_publication(&mut container, &path, &legacy, 2, 3);
    container.sync().unwrap();
    drop(container);
    assert!(matches!(
        Cva::open(&path),
        Err(reliquary_memory::CvaError::Archive(
            reliquary_memory::ArchiveError::InvalidArchiveRecordVersion
        ))
    ));
}

#[test]
fn nonturn_prefix_has_zero_cut_before_first_eligible_turn() {
    let mut fixture = Fixture::new();
    fixture
        .cva_mut()
        .store_file("prefix.txt".into(), None, b"prefix")
        .unwrap();
    let prefix = fixture.cva().archive_version();
    let mut ledger = ReferenceLedger::default();
    for version in 1..=prefix {
        ledger.observe(version, None, 0);
    }
    append(fixture.cva_mut(), &turn("c", "u", "user", None));
    ledger.observe(
        fixture.cva().archive_version(),
        Some(("c", "u", Some(1))),
        1,
    );
    ledger.assert_matches(fixture.cva());
    fixture.reopen();
    ledger.assert_matches(fixture.cva());
}

// Independent literal framing of current Archive bodies; activity expectations
// remain explicit in ReferenceLedger rather than derived from production rules.
fn current_publication_body(payloads: &[Vec<u8>], magic: &[u8; 8]) -> Vec<u8> {
    payloads
        .iter()
        .filter_map(|payload| {
            if !payload.starts_with(b"CVAPUB01") {
                return None;
            }
            let at = match payload.get(16).copied()? {
                0 | 2 => 17,
                1 => 25,
                _ => return None,
            };
            payload.get(at..)
        })
        .find(|body| body.starts_with(magic))
        .expect("native Archive body")
        .to_vec()
}

fn append_fixture_global_version(container: &mut Container, version: u64) {
    let mut bytes = b"CVAVERS1".to_vec();
    bytes.extend_from_slice(&version.to_le_bytes());
    container.append(&bytes).unwrap();
}

#[test]
fn validated_history_counts_current_records_but_not_orphans_or_repeats() {
    let mut fixture = Fixture::new();
    append(fixture.cva_mut(), &turn("c", "u", "user", None));
    fixture
        .cva_mut()
        .ingest_turn(turn("c", "a", "assistant", None))
        .unwrap();
    assert_eq!(fixture.cva().archive_version(), 2);
    fixture.cva().sync().unwrap();
    drop(fixture.cva.take());
    let path = fixture.dir.join("clock.rel");
    let mut container = Container::open(&path).unwrap();
    let payloads = container
        .chunks()
        .unwrap()
        .into_iter()
        .map(|record| container.read(record).unwrap())
        .collect::<Vec<_>>();
    let mut node = current_publication_body(&payloads, b"CVANOD02");
    let mut ingested = current_publication_body(&payloads, b"CVATUR03");
    node[60] = b'x';
    ingested[60] = b'y';
    let mut version = 2_u64;
    for bytes in [node.as_slice(), ingested.as_slice()] {
        version += 1;
        append_fixture_publication(&mut container, &path, bytes, version, version);
    }
    let mut repeated = b"CVAARP02".to_vec();
    repeated.extend_from_slice(&5_u64.to_le_bytes());
    repeated.extend_from_slice(&3_u64.to_le_bytes());
    append_fixture_publication(&mut container, &path, &repeated, 5, 5);
    for original in [node.as_slice(), ingested.as_slice()] {
        let mut orphan = original.to_vec();
        orphan[60] = b'z';
        container.append(&orphan).unwrap();
    }
    append_fixture_global_version(&mut container, 6);
    container.sync().unwrap();
    drop(container);
    fixture.cva = Some(Cva::open(&path).unwrap());
    let mut ledger = ReferenceLedger::default();
    for (version, id, position) in [(1, "u", 1), (2, "a", 2), (3, "x", 3), (4, "y", 4)] {
        ledger.observe(version, Some(("c", id, Some(position))), position);
    }
    ledger.observe(5, None, 4);
    assert_eq!(fixture.cva().archive_version(), 5);
    assert_eq!(fixture.cva().activity_position_for_turn("c", "z"), None);
    ledger.assert_matches(fixture.cva());
    fixture.reopen();
    ledger.assert_matches(fixture.cva());
}

#[test]
fn file_fragment_and_episode_publications_use_the_preceding_activity_cut() {
    let mut fixture = Fixture::new();
    let mut ledger = ReferenceLedger::default();
    append(fixture.cva_mut(), &turn("c", "u", "user", None));
    ledger.observe(
        fixture.cva().archive_version(),
        Some(("c", "u", Some(1))),
        1,
    );
    fixture
        .cva_mut()
        .ingest_turn(turn("c", "a", "assistant", Some("u")))
        .unwrap();
    ledger.observe(
        fixture.cva().archive_version(),
        Some(("c", "a", Some(2))),
        2,
    );
    let before = fixture.cva().archive_version();
    fixture
        .cva_mut()
        .store_file("separate.txt".into(), None, b"file")
        .unwrap();
    for version in before + 1..=fixture.cva().archive_version() {
        ledger.observe(version, None, 2);
    }
    ledger.assert_matches(fixture.cva());
    let before = fixture.cva().archive_version();
    let result = fixture
        .cva_mut()
        .materialize_path_episodes(
            "c",
            "a",
            reliquary_memory::EpisodeConfig::default(),
            reliquary_memory::EpisodeOrigin::Import,
            Some((reliquary_memory::EpisodeBoundary::ImportEnd, 1000)),
        )
        .unwrap();
    assert_eq!(result.created.len(), 1);
    assert!(fixture.cva().archive_version() > before);
    for version in before + 1..=fixture.cva().archive_version() {
        ledger.observe(version, None, 2);
    }
    ledger.assert_matches(fixture.cva());
    fixture.reopen();
    ledger.assert_matches(fixture.cva());
}
