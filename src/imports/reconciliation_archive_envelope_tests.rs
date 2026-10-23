use super::*;
use crate::archive_codec::{
    ArchiveRecord,
    publication::{self, ArchivePublication},
};

#[test]
fn archive_envelope_interval_replays_reserved_time_and_normalized_evidence() {
    let dir =
        std::env::temp_dir().join(format!("archive-import-envelope-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let source_path = dir.join("source.rel");
    let target_path = dir.join("target.rel");
    let mut source = Cva::create(&source_path).unwrap();
    source
        .append_node("n".into(), "c".into(), None, "user".into(), 1, "text")
        .unwrap();
    source.sync().unwrap();
    drop(source);
    std::fs::copy(&source_path, &target_path).unwrap();
    let branch = crate::Branch {
        id: "b".into(),
        conversation_id: "c".into(),
        leaf_node_id: "n".into(),
        canonical: true,
    };
    let body = publication::encode(&ArchivePublication::Record {
        archive_version: 2,
        record: ArchiveRecord::Branch(branch.clone()),
    })
    .unwrap();
    let mut container = crate::Container::open(&source_path).unwrap();
    let reserved = container
        .allocate_version_with_transaction_time(Some(-123))
        .unwrap();
    container
        .allocate_version_with_transaction_time(Some(500))
        .unwrap();
    let index = container.chunks().unwrap().len();
    let payload = crate::container::publication_envelope::encode(
        crate::container::publication_envelope::PublicationEnvelope {
            global_version: reserved,
            clock: crate::container::publication_envelope::PublicationClock::BindReserved,
            body: &body,
        },
    )
    .unwrap();
    let object = container.append(&payload).unwrap();
    container.sync().unwrap();
    drop(container);
    let mut source = Cva::open(&source_path).unwrap();
    let mut target = Cva::open(&target_path).unwrap();
    super::validate_replay_interval(
        &crate::owner_lifecycle::HistoryReadAccess::new(&source.container)
            .snapshot()
            .unwrap(),
        &source.archive,
        &source.insomnia,
        index,
    )
    .unwrap();
    let mut coordinates =
        crate::imports::reconciliation_interval::Coordinates::shared(&mut source, index).unwrap();
    crate::imports::reconciliation_interval::replay_frame(
        &mut source,
        &mut target,
        index,
        &payload,
        &mut coordinates,
    )
    .unwrap();
    let version = *target.archive.record_version(2).unwrap();
    assert_eq!(
        target.transaction_time_ns(version.global_version),
        Some(-123)
    );
    assert_eq!(
        target
            .archive
            .branch_at(&mut target.container, "c", "b", 2)
            .unwrap(),
        Some(branch)
    );
    let native_version = publication::version_at(object, &payload).unwrap().unwrap();
    let (native_record, _) = source
        .archive
        .read_publication(&mut source.container, native_version)
        .unwrap();
    let (target_record, _) = target
        .archive
        .read_publication(&mut target.container, version)
        .unwrap();
    assert_eq!(
        publication::import_evidence(native_record).unwrap(),
        publication::import_evidence(target_record).unwrap()
    );
    drop(source);
    drop(target);
    std::fs::remove_dir_all(dir).unwrap();
}
