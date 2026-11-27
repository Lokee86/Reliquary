use super::*;
use crate::*;

const REL: [u8; 16] = [0x32; 16];
const TOPIC: EgoTopicId = EgoTopicId([1; 16]);
const POINT: EgoPointId = EgoPointId([2; 16]);

fn completion(owner_id: [u8; 16], n: u8) -> EgoCompletion {
    EgoCompletion {
        id: [n; 32],
        cut: EgoSourceCut {
            owner_id,
            history_generation: [3; 32],
            memory_version: u64::from(n),
            audit_frontier: u64::from(n),
        },
        batch_ids: vec![[n + 10; 32]],
    }
}

fn citation(owner_id: [u8; 16], revision: u64) -> EgoCitation {
    EgoCitation {
        owner_id,
        history_generation: [3; 32],
        memory_id: [4; 32],
        revision,
        body_id: [revision as u8; 32],
        field: EgoTextField::Content,
        start_byte: 1,
        end_byte: 4,
    }
}

fn point(revision: u64, statement: &str) -> EgoRelChange {
    EgoRelChange::Point {
        id: POINT,
        expected_revision: revision - 1,
        change: EgoItemChange::Put(EgoPoint {
            topic_id: TOPIC,
            statement: statement.into(),
            status: EgoPointStatus::Requested,
            time_scope: EgoTimeScope::Current,
            verified_at_ns: 5,
            citations: vec![citation(REL, revision)],
            embedding: Some(EgoEmbedding::from_components([5; 32], &[0.5, -0.125])),
        }),
    }
}

fn edit(expected_revision: u64, change: EgoItemChange<EgoPoint>) -> EgoRelPublication {
    EgoRelPublication {
        source_owner_id: REL,
        expected_revision,
        completion: None,
        changes: vec![EgoRelChange::Point {
            id: POINT,
            expected_revision,
            change,
        }],
    }
}

#[test]
fn retains_rel_points_and_applies_completion_and_suppression_once() {
    let path = TempFile::new();
    let mut container = create(FileKind::Phylactery, path.path());
    let mut store = EgoStore::open(&container).unwrap();
    let initial = EgoRelPublication {
        source_owner_id: REL,
        expected_revision: 0,
        completion: Some(completion(REL, 1)),
        changes: vec![
            EgoRelChange::Topic {
                id: TOPIC,
                expected_revision: 0,
                change: EgoItemChange::Put(EgoTopic {
                    title: "Editing preferences".into(),
                    embedding: None,
                }),
            },
            point(1, "Include an example."),
        ],
    };
    assert_eq!(
        store
            .publish_rel_summary(&mut container, initial.clone())
            .unwrap(),
        1
    );

    let replace = EgoRelPublication {
        source_owner_id: REL,
        expected_revision: 1,
        completion: Some(completion(REL, 2)),
        changes: vec![point(2, "Include two examples.")],
    };
    assert_eq!(
        store
            .publish_rel_summary(&mut container, replace.clone())
            .unwrap(),
        2
    );

    let suppress = edit(2, EgoItemChange::Tombstone { suppress: true });
    assert_eq!(
        store.publish_rel_summary(&mut container, suppress).unwrap(),
        3
    );
    assert!(
        store.rel_summary(REL).unwrap().points[&POINT]
            .tombstone
            .unwrap()
            .suppressed
    );
    let lift = edit(3, EgoItemChange::LiftSuppression);
    assert_eq!(store.publish_rel_summary(&mut container, lift).unwrap(), 4);
    drop(container);

    let mut container = Container::open_writer(path.path()).unwrap();
    let mut store = EgoStore::open(&container).unwrap();
    let current = store.rel_summary(REL).unwrap();
    assert_eq!(current.topics[&TOPIC].revision, 1);
    let terminal = &current.points[&POINT];
    assert_eq!(terminal.revision, 4);
    assert_eq!(terminal.value_revision, 2);
    assert_eq!(terminal.value.statement, "Include two examples.");
    assert!(!terminal.tombstone.unwrap().suppressed);
    let retained = store.rel_summary_revision(REL, 1).unwrap().unwrap();
    assert_eq!(
        retained.points[&POINT].value.statement,
        "Include an example."
    );
    assert_eq!(
        retained.points[&POINT].value.citations,
        vec![citation(REL, 1)]
    );
    assert_eq!(store.completion([2; 32]), replace.completion.as_ref());

    let before = fs::read(path.path()).unwrap();
    assert_eq!(
        store
            .publish_rel_summary(&mut container, replace.clone())
            .unwrap(),
        2
    );
    let mut conflicting_retry = replace;
    conflicting_retry.changes = vec![point(2, "A different statement.")];
    assert!(
        store
            .publish_rel_summary(&mut container, conflicting_retry)
            .is_err()
    );
    assert!(matches!(
        store.publish_rel_summary(
            &mut container,
            edit(1, EgoItemChange::Tombstone { suppress: false })
        ),
        Err(EgoError::RevisionConflict {
            expected: 1,
            actual: 4
        })
    ));
    assert_eq!(fs::read(path.path()).unwrap(), before);
}

#[test]
fn rejects_unverified_phy_ranges_and_replays_clear_and_empty_completion() {
    let path = TempFile::new();
    let mut container = create(FileKind::Phylactery, path.path());
    let owner = container.owner_id();
    let mut store = EgoStore::open(&container).unwrap();
    let mut publication = EgoPhyPublication {
        expected_revision: 0,
        completion: completion(owner, 1),
        replacement: Some(EgoPhyText {
            text: "Résumé\n\nUse a clear title.".into(),
            paragraphs: vec![
                EgoParagraph {
                    start_byte: 0,
                    end_byte: 8,
                    citations: vec![citation(owner, 1)],
                },
                EgoParagraph {
                    start_byte: 10,
                    end_byte: 28,
                    citations: vec![citation(owner, 2)],
                },
            ],
            token_count: 8,
        }),
    };
    let before = fs::read(path.path()).unwrap();
    publication.replacement.as_mut().unwrap().paragraphs[0].end_byte = 7;
    assert!(
        store
            .publish_phy_summary(&mut container, publication.clone())
            .is_err()
    );
    publication.replacement.as_mut().unwrap().paragraphs[0].end_byte = 8;
    publication.replacement.as_mut().unwrap().paragraphs[1].end_byte = 27;
    assert!(
        store
            .publish_phy_summary(&mut container, publication.clone())
            .is_err()
    );
    assert_eq!(fs::read(path.path()).unwrap(), before);
    publication.replacement.as_mut().unwrap().paragraphs[1].end_byte = 28;
    assert_eq!(
        store
            .publish_phy_summary(&mut container, publication.clone())
            .unwrap(),
        1
    );
    assert_eq!(
        store
            .publish_phy_summary(
                &mut container,
                EgoPhyPublication {
                    expected_revision: 1,
                    completion: completion(owner, 2),
                    replacement: None,
                }
            )
            .unwrap(),
        2
    );
    assert_eq!(store.phy_summary().unwrap().text, publication.replacement);
    assert_eq!(
        store
            .publish_phy_summary(
                &mut container,
                EgoPhyPublication {
                    expected_revision: 2,
                    completion: completion(owner, 3),
                    replacement: Some(EgoPhyText {
                        text: String::new(),
                        paragraphs: Vec::new(),
                        token_count: 0
                    }),
                }
            )
            .unwrap(),
        3
    );
    drop(container);

    let container = Container::open_read_only(path.path()).unwrap();
    let store = EgoStore::open(&container).unwrap();
    assert_eq!(store.phy_summary().unwrap().revision, 3);
    assert!(store.phy_summary().unwrap().text.is_none());
    assert_eq!(
        store.phy_summary_revision(1).unwrap().text,
        publication.replacement
    );
    assert_eq!(store.completion([2; 32]), Some(&completion(owner, 2)));
}

#[test]
fn identity_selection_checks_both_revisions_and_legacy_identity_requires_selection() {
    let path = TempFile::new();
    let mut container = create(FileKind::Phylactery, path.path());
    let mut store = EgoStore::open(&container).unwrap();
    let first = identity(1);
    let second = identity(2);
    store
        .create_identity(
            &mut container,
            first,
            "First".into(),
            "First profile".into(),
        )
        .unwrap();
    store
        .create_identity(
            &mut container,
            second,
            "Second".into(),
            "Second profile".into(),
        )
        .unwrap();
    assert!(store.active_identity().is_none());

    store
        .select_identity(&mut container, 0, Some((first, 1)))
        .unwrap();
    store
        .update_identity(&mut container, second, 1, "Second".into(), "Revised".into())
        .unwrap();
    assert!(matches!(
        store.select_identity(&mut container, 1, Some((second, 1))),
        Err(EgoError::RevisionConflict {
            expected: 1,
            actual: 2
        })
    ));
    assert!(
        store
            .select_identity(&mut container, 0, Some((second, 2)))
            .is_err()
    );
    store
        .select_identity(&mut container, 1, Some((second, 2)))
        .unwrap();
    drop(container);

    let container = Container::open_read_only(path.path()).unwrap();
    let store = EgoStore::open(&container).unwrap();
    assert_eq!(store.active_identity().unwrap().text, "Revised");
    assert_eq!(store.identity_selection().unwrap().revision, 2);
    assert_eq!(store.history_at(2).unwrap().records.len(), 2);
}

#[test]
fn unknown_ego_extension_schema_fails_closed() {
    let path = TempFile::new();
    let mut container = create(FileKind::Phylactery, path.path());
    let mut record = b"RELEGO01\x05".to_vec();
    record.extend_from_slice(&1_u64.to_le_bytes());
    record.push(99);
    container.append(&record).unwrap();
    container.sync().unwrap();
    assert!(matches!(
        EgoStore::open(&container),
        Err(EgoError::InvalidRecord(
            "unknown Ego extension schema version"
        ))
    ));
}
