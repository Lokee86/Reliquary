use super::*;
use crate::container::publication_envelope::PublicationClock;
use crate::memory_compact_codec::publication::MemoryPublication;
use crate::{
    ProjectFileRef, ProjectRepositoryKind, ProjectRepositoryRef, ProjectRevisionRef, RewriteRequest,
};
use sha2::{Digest, Sha256};

fn project_ref(bytes: &[u8]) -> ProjectFileRef {
    ProjectFileRef {
        revision: ProjectRevisionRef {
            repository: ProjectRepositoryRef {
                kind: ProjectRepositoryKind::Lore,
                repository_id: "memory-copy-tests".into(),
                project_path: ".".into(),
            },
            revision: "immutable-revision".into(),
        },
        path: "uploads/backed.bin".into(),
        content_hash: Some(Sha256::digest(bytes).into()),
    }
}

fn make_envelope_history(path: &Path, kind: FileKind, tombstone: bool, backed_file: bool) {
    match kind {
        FileKind::Reliquary => drop(crate::Cva::create_project(path).unwrap()),
        FileKind::Phylactery => drop(crate::Phylactery::create(path).unwrap()),
    }
    if backed_file {
        let bytes = vec![0x7a; 32 * 1024];
        let mut rel = crate::Cva::open_project(path).unwrap();
        let file = rel.store_file("backed.bin".into(), None, &bytes).unwrap();
        rel.bind_legacy_project_file(file.id, project_ref(&bytes))
            .unwrap();
        rel.sync().unwrap();
    }

    let mut container = Container::open(path).unwrap();
    let mut first = initial_record();
    first.body_id = append_body(&mut container, "Stable title", "Revision one body");
    let first_global = container
        .allocate_version_with_transaction_time(Some(101))
        .unwrap();
    first.global_version = first_global;
    append_publication(
        &mut container,
        first_global,
        PublicationClock::BindReserved,
        MemoryPublication::Creation(MemoryCreation::new(first.clone()).unwrap()),
    );

    let mut second = successor(&first);
    second.global_version = container
        .allocate_version_with_transaction_time(Some(202))
        .unwrap();
    second.memory_version = 2;
    second.scope = "updated scope".into();
    second.updated_at_ns = 202;
    append_publication(
        &mut container,
        second.global_version,
        PublicationClock::BindReserved,
        MemoryPublication::Delta(MemoryDelta::between(&first, &second).unwrap()),
    );

    if tombstone {
        let terminal_global = container
            .allocate_version_with_transaction_time(Some(303))
            .unwrap();
        append_publication(
            &mut container,
            terminal_global,
            PublicationClock::BindReserved,
            MemoryPublication::Tombstone {
                id: first.id,
                revision: 3,
                memory_version: 3,
            },
        );
    }
    container.sync().unwrap();
}

fn assert_memory_history_equal(left: &mut crate::Cva, right: &mut crate::Cva) {
    use crate::historical_memory_fingerprint::MemoryHistory;
    assert_eq!(left.memories.records(), right.memories.records());
    assert_eq!(left.memory_version(), right.memory_version());
    assert_eq!(left.latest_global_version(), right.latest_global_version());
    let left_history = MemoryHistory::read(&mut left.container, &left.memories).unwrap();
    let right_history = MemoryHistory::read(&mut right.container, &right.memories).unwrap();
    for cut in 0..=left.memory_version() {
        assert_eq!(
            left_history.at(cut).unwrap().fingerprint,
            right_history.at(cut).unwrap().fingerprint,
            "Memory semantic projection at local cut {cut}"
        );
        assert_eq!(
            left_history.at(cut).unwrap().current,
            right_history.at(cut).unwrap().current,
            "current membership at local cut {cut}"
        );
    }
    for global in 0..=left.latest_global_version() {
        assert_eq!(
            left.container.transaction_time_ns(global),
            right.container.transaction_time_ns(global),
            "global transaction time at {global}"
        );
    }
    for record in left.memories.records().to_vec() {
        let a = left.memory_revision(record.id, record.revision).unwrap();
        let b = right.memory_revision(record.id, record.revision).unwrap();
        assert_eq!((a.title, a.content), (b.title, b.content));
    }
}

#[test]
fn rel_native_attachment_repack_preserves_memory_envelope_history_and_hydration() {
    let source_path = test_path("envelope-native-repack-source.rel");
    let output_path = test_path("envelope-native-repack-output.rel");
    make_envelope_history(&source_path, FileKind::Reliquary, true, true);
    let mut source = crate::Cva::open_project(&source_path).unwrap();
    assert_eq!(source.memories.records().len(), 2);
    assert_eq!(source.memory_version(), 3);
    let report = source
        .repack_project_backed_attachments(&output_path)
        .unwrap();
    assert_eq!(report.dropped_content_objects, 1);
    let mut reopened = crate::Cva::open_project(&output_path).unwrap();
    assert_memory_history_equal(&mut source, &mut reopened);
    assert!(matches!(
        reopened
            .memories
            .lifecycle_state(initial_record().id)
            .unwrap(),
        LifecycleState::Tombstoned(_)
    ));
    let old = reopened.memory_revision(initial_record().id, 1).unwrap();
    let revised = reopened.memory_revision(initial_record().id, 2).unwrap();
    assert_eq!(old.content, "Revision one body");
    assert_eq!(revised.content, "Revision one body");
    assert_eq!(revised.scope, "updated scope");
}

#[test]
fn rel_and_phy_semantic_rewrites_rehydrate_current_envelope_memory() {
    for kind in [FileKind::Reliquary, FileKind::Phylactery] {
        let source_path = test_path(if kind == FileKind::Reliquary {
            "envelope-semantic-source.rel"
        } else {
            "envelope-semantic-source.phy"
        });
        let output_path = test_path(if kind == FileKind::Reliquary {
            "envelope-semantic-output.rel"
        } else {
            "envelope-semantic-output.phy"
        });
        make_envelope_history(&source_path, kind, false, false);
        let source_record = match kind {
            FileKind::Reliquary => {
                let owner = crate::Cva::open(&source_path).unwrap();
                owner.memories.records().last().unwrap().clone()
            }
            FileKind::Phylactery => {
                let owner = crate::Phylactery::open(&source_path).unwrap();
                owner.memories.records().last().unwrap().clone()
            }
        };

        crate::rewrite::rewrite_file(&source_path, &output_path, &RewriteRequest::default())
            .unwrap();
        let (records, memory) = match kind {
            FileKind::Reliquary => {
                let mut owner = crate::Cva::open(&output_path).unwrap();
                let record = owner.memories.records().last().unwrap().clone();
                let memory = owner.memory_revision(record.id, record.revision).unwrap();
                (owner.memories.records().len(), memory)
            }
            FileKind::Phylactery => {
                let mut owner = crate::Phylactery::open(&output_path).unwrap();
                let record = owner.memories.records().last().unwrap().clone();
                let memory = owner.memory_revision(record.id, record.revision).unwrap();
                (owner.memories.records().len(), memory)
            }
        };
        // Default semantic rewrite policy selects only the current revision and
        // assigns fresh owner-local/global coordinates. Assert its actual
        // contract: stable identity/current semantic content and body hydration.
        assert_eq!(records, 1);
        assert_eq!(memory.id, source_record.id);
        assert_eq!(memory.revision, 1);
        assert_eq!(
            crate::memory_model::memory_body_id(&memory.title, &memory.content),
            source_record.body_id
        );
        assert_eq!(memory.scope, "updated scope");
        assert_eq!(memory.title, "Stable title");
        assert_eq!(memory.content, "Revision one body");
        assert_eq!(memory.mutation_id, "delta-one");
    }
}

#[test]
fn legacy_phy_semantic_migration_preserves_each_live_revision_and_its_time() {
    let source_path = test_path("envelope-migration-source.phy");
    let output_path = test_path("envelope-migration-output.phy");
    drop(crate::Phylactery::create_legacy_typed(&source_path).unwrap());
    let mut container = Container::open(&source_path).unwrap();
    let mut first = initial_record();
    first.body_id = append_body(
        &mut container,
        "Migrated immutable title",
        "Body remains hydrated through semantic migration",
    );
    first.global_version = container
        .allocate_version_with_transaction_time(Some(10))
        .unwrap();
    first.mutation_id = "migration-create".into();
    first.created_at_ns = 10;
    first.updated_at_ns = 10;
    append_publication(
        &mut container,
        first.global_version,
        PublicationClock::BindReserved,
        MemoryPublication::Creation(MemoryCreation::new(first.clone()).unwrap()),
    );
    let mut second = successor(&first);
    second.global_version = container
        .allocate_version_with_transaction_time(Some(20))
        .unwrap();
    second.memory_version = 2;
    second.scope = "second scope".into();
    second.updated_at_ns = 20;
    second.mutation_id = "migration-revise".into();
    append_publication(
        &mut container,
        second.global_version,
        PublicationClock::BindReserved,
        MemoryPublication::Delta(MemoryDelta::between(&first, &second).unwrap()),
    );
    container.sync().unwrap();
    drop(container);

    let mut source = crate::Phylactery::open(&source_path).unwrap();
    assert_eq!(source.owner_uuid(), None);
    assert_eq!(source.memories.records(), &[first.clone(), second.clone()]);
    let source_rows = source.memories.records().to_vec();
    let source_times = source_rows
        .iter()
        .map(|record| {
            (
                (record.id, record.revision),
                source.transaction_time_ns(record.global_version),
            )
        })
        .collect::<std::collections::BTreeMap<_, _>>();
    let source_memory_head = source.memory_version();
    assert_eq!(source_rows.len(), 2);
    assert_eq!(source_rows[0].revision, 1);
    assert_eq!(source_rows[1].revision, 2);
    drop(source);

    crate::migrate_file(&source_path, &output_path).unwrap();
    let mut migrated = crate::Phylactery::open(&output_path).unwrap();
    assert_eq!(migrated.memory_version(), source_memory_head);
    assert_eq!(migrated.memories.records().len(), 2);
    for source_record in source_rows {
        let migrated_record = migrated
            .memories
            .records()
            .iter()
            .find(|record| {
                record.id == source_record.id && record.revision == source_record.revision
            })
            .expect("accepted revision retained")
            .clone();
        let mut expected = source_record.clone();
        expected.global_version = migrated_record.global_version;
        assert_eq!(migrated_record, expected);
        assert_eq!(
            migrated.transaction_time_ns(migrated_record.global_version),
            source_times[&(source_record.id, source_record.revision)],
            "transaction time for revision {}",
            source_record.revision
        );
        let memory = migrated
            .memory_revision(source_record.id, source_record.revision)
            .unwrap();
        assert_eq!(
            crate::memory_model::memory_body_id(&memory.title, &memory.content),
            source_record.body_id
        );
        assert_eq!(memory.title, "Migrated immutable title");
        assert_eq!(
            memory.content,
            "Body remains hydrated through semantic migration"
        );
        assert_eq!(memory.scope, source_record.scope);
    }
    assert_eq!(migrated.memory(first.id).unwrap().revision, second.revision);
}
