use super::TurnAcceptance;
use crate::{ArchiveError, ContainerError, Cva, IncomingAttachment, IncomingTurn, Node};
use std::path::PathBuf;

struct Fixture {
    path: PathBuf,
    cva: Option<Cva>,
}
impl Fixture {
    fn new() -> Self {
        let path =
            std::env::temp_dir().join(format!("rel-live-activity-{}.rel", uuid::Uuid::new_v4()));
        let cva = Some(Cva::create(&path).unwrap());
        Self { path, cva }
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
        self.cva = Some(Cva::open(&self.path).unwrap());
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        drop(self.cva.take());
        std::fs::remove_file(&self.path).unwrap();
    }
}
fn turn(id: &str, role: &str) -> IncomingTurn {
    IncomingTurn {
        id: id.into(),
        conversation_id: "c".into(),
        parent_id: None,
        role: role.into(),
        principal_id: None,
        timestamp_ns: -10,
        content: "same content".into(),
        attachments: Vec::new(),
        project_attachments: Vec::new(),
    }
}
fn append(cva: &mut Cva, incoming: IncomingTurn) -> Result<TurnAcceptance<Node>, ArchiveError> {
    cva.archive.append_node(
        &mut cva.container,
        incoming.id,
        incoming.conversation_id,
        incoming.parent_id,
        incoming.role,
        incoming.principal_id,
        incoming.timestamp_ns,
        &incoming.content,
    )
}
fn accept(
    cva: &mut Cva,
    incoming: IncomingTurn,
    ingest: bool,
) -> Result<(bool, Option<u64>, u64), ArchiveError> {
    if ingest {
        let result = cva
            .archive
            .ingest_turn(&mut cva.container, &cva.project_files, incoming)?;
        Ok((
            result.inserted,
            result.activity_position,
            result.rel_turn_count,
        ))
    } else {
        let result = append(cva, incoming)?;
        Ok((
            result.inserted,
            result.activity_position,
            result.rel_turn_count,
        ))
    }
}

#[test]
fn live_paths_share_exact_role_eligibility_and_rel_wide_positions() {
    let mut fixture = Fixture::new();
    assert_eq!(fixture.cva().rel_turn_count(), 0);
    assert_eq!(
        fixture.cva().activity_position_for_turn("c", "missing"),
        None
    );
    let rows = [
        ("user", false, Some(1), 1),
        ("system", true, None, 1),
        ("assistant", true, Some(2), 2),
        ("tool", false, None, 2),
        ("agent", true, None, 2),
        ("USER", false, None, 2),
        ("unknown", true, None, 2),
        ("user", true, Some(3), 3),
    ];
    for (i, (role, ingest, position, count)) in rows.into_iter().enumerate() {
        let id = format!("n{i}");
        assert_eq!(
            accept(fixture.cva_mut(), turn(&id, role), ingest).unwrap(),
            (true, position, count)
        );
        assert_eq!(fixture.cva().activity_position_for_turn("c", &id), position);
        assert_eq!(fixture.cva().rel_turn_count(), count);
        assert_eq!(
            accept(fixture.cva_mut(), turn(&id, role), ingest).unwrap(),
            (false, position, count)
        );
    }
}

#[test]
fn cross_format_replays_return_first_position_and_current_count() {
    for initial_ingest in [false, true] {
        let mut fixture = Fixture::new();
        assert_eq!(
            accept(fixture.cva_mut(), turn("first", "user"), initial_ingest).unwrap(),
            (true, Some(1), 1)
        );
        assert_eq!(
            accept(
                fixture.cva_mut(),
                turn("later", "assistant"),
                !initial_ingest
            )
            .unwrap(),
            (true, Some(2), 2)
        );
        for ingest in [false, true] {
            assert_eq!(
                accept(fixture.cva_mut(), turn("first", "user"), ingest).unwrap(),
                (false, Some(1), 2)
            );
        }
        assert_eq!(fixture.cva().archive_version(), 2);
    }
}

#[test]
fn conflicts_and_attachment_drift_do_not_advance_the_projection() {
    let mut fixture = Fixture::new();
    let mut incoming = turn("attached", "user");
    incoming.attachments.push(IncomingAttachment {
        filename: "a.txt".into(),
        mime_type: None,
        bytes: b"a".to_vec(),
    });
    assert_eq!(
        accept(fixture.cva_mut(), incoming.clone(), true).unwrap(),
        (true, Some(1), 1)
    );
    let version = fixture.cva().archive_version();
    let mut changed = incoming.clone();
    changed.content = "different".into();
    assert!(matches!(
        accept(fixture.cva_mut(), changed, false),
        Err(ArchiveError::ConflictingNode)
    ));
    let mut changed = incoming.clone();
    changed.attachments.clear();
    assert!(matches!(
        accept(fixture.cva_mut(), changed, true),
        Err(ArchiveError::ConflictingTurnIngest)
    ));
    let mut changed = incoming.clone();
    changed.attachments[0].filename = "drift.txt".into();
    assert!(matches!(
        accept(fixture.cva_mut(), changed, true),
        Err(ArchiveError::ConflictingTurnIngest)
    ));
    assert_eq!(
        accept(fixture.cva_mut(), incoming, true).unwrap(),
        (false, Some(1), 1)
    );
    assert_eq!(
        accept(fixture.cva_mut(), turn("attached", "user"), false).unwrap(),
        (false, Some(1), 1)
    );
    assert_eq!(fixture.cva().archive_version(), version);
    assert_eq!(fixture.cva().rel_turn_count(), 1);
}

#[test]
fn activity_overflow_is_rejected_before_any_bytes_or_version_are_published() {
    for ingest in [false, true] {
        let mut fixture = Fixture::new();
        accept(fixture.cva_mut(), turn("first", "user"), ingest).unwrap();
        // Reach the checked boundary without allocating u64::MAX actual turns.
        fixture.cva_mut().archive.activity.current_count = u64::MAX;
        let before = std::fs::metadata(&fixture.path).unwrap().len();
        let version = fixture.cva().archive_version();
        assert!(matches!(
            accept(fixture.cva_mut(), turn("overflow", "assistant"), ingest),
            Err(ArchiveError::ActivityPositionExhausted)
        ));
        assert_eq!(std::fs::metadata(&fixture.path).unwrap().len(), before);
        assert_eq!(fixture.cva().archive_version(), version);
        assert_eq!(
            fixture.cva().activity_position_for_turn("c", "overflow"),
            None
        );
        assert_eq!(
            accept(fixture.cva_mut(), turn("first", "user"), !ingest).unwrap(),
            (false, Some(1), u64::MAX)
        );
        assert_eq!(
            accept(fixture.cva_mut(), turn("system", "system"), ingest).unwrap(),
            (true, None, u64::MAX)
        );
    }
}

#[test]
fn failed_version_publication_installs_neither_node_nor_activity_tick() {
    for ingest in [false, true] {
        let mut fixture = Fixture::new();
        accept(fixture.cva_mut(), turn("first", "user"), ingest).unwrap();
        fixture.cva_mut().container.next_version = u64::MAX;
        assert!(matches!(
            accept(fixture.cva_mut(), turn("unpublished", "assistant"), ingest),
            Err(ArchiveError::Container(ContainerError::VersionExhausted))
        ));
        assert_eq!(fixture.cva().archive_version(), 1);
        assert_eq!(fixture.cva().rel_turn_count(), 1);
        assert_eq!(
            fixture.cva().activity_position_for_turn("c", "unpublished"),
            None
        );
        assert_eq!(fixture.cva().stats().nodes, 1);
        fixture.reopen();
        assert_eq!(fixture.cva().rel_turn_count(), 1);
        assert_eq!(
            fixture.cva().activity_position_for_turn("c", "unpublished"),
            None
        );
    }
}

#[test]
fn ordinal_growth_and_live_acceptance_after_reopen_preserve_positions() {
    let mut fixture = Fixture::new();
    for i in 0..128 {
        let id = format!("n{i}");
        let mut incoming = turn(&id, if i % 2 == 0 { "user" } else { "assistant" });
        incoming.conversation_id = format!("c{}", i % 3);
        incoming.timestamp_ns = -(i as i64);
        assert_eq!(
            accept(fixture.cva_mut(), incoming, i % 2 == 0).unwrap(),
            (true, Some(i + 1), i + 1)
        );
    }
    fixture.reopen();
    assert_eq!(fixture.cva().rel_turn_count(), 128);
    for i in 0..128 {
        assert_eq!(
            fixture
                .cva()
                .activity_position_for_turn(&format!("c{}", i % 3), &format!("n{i}")),
            Some(i + 1)
        );
    }
    assert_eq!(
        accept(fixture.cva_mut(), turn("next", "assistant"), true).unwrap(),
        (true, Some(129), 129)
    );
}
