//! Entity reconciliation of owner publication envelopes.
use super::{read_entity_range, replay_entity_tail};
use reliquary_perception_store::entity_compact_codec::publication::{self, EntityPublication};
use reliquary_perception_store::entity_compact_contract::{EntityCreation, EntityDelta};
use reliquary_perception_store::entity_model::EntityRecord;
use crate::owner_lifecycle::{LifecycleState, TombstoneCoordinates};
use crate::{Cva, EntityGlobalId, EntityId};
use reliquary_container::Container;
use reliquary_container::publication_envelope::{self, PublicationClock, PublicationEnvelope};
use std::fs;
use std::path::{Path, PathBuf};

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(".test-tmp")
            .join(format!(
                "cva-reconcile-entity-envelope-{}",
                uuid::Uuid::new_v4()
            ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn file(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn record(
    id: u8,
    name: &str,
    mutation_id: &str,
    revision: u64,
    global: u64,
    local: u64,
) -> EntityRecord {
    EntityRecord {
        id: EntityId([id; 32]),
        global_id: Some(EntityGlobalId([id.wrapping_add(70); 16])),
        revision,
        canonical_name: name.into(),
        aliases: vec![format!("{name} alias")],
        kind: "project".into(),
        summary: format!("Summary for {name}"),
        mutation_id: mutation_id.into(),
        created_at_ns: 10,
        updated_at_ns: 20 + revision as i64,
        global_version: global,
        entity_version: local,
    }
}

fn append_allocated(
    container: &mut Container,
    publication: &EntityPublication,
    time_ns: Option<i64>,
) -> u64 {
    let body = publication::encode(publication).unwrap();
    container.set_next_transaction_time_override(time_ns);
    container.append_publication_envelope(&body).unwrap().0
}

fn append_reserved(
    container: &mut Container,
    global: u64,
    publication: &EntityPublication,
) -> usize {
    let body = publication::encode(publication).unwrap();
    let frame = publication_envelope::encode(PublicationEnvelope {
        global_version: global,
        clock: PublicationClock::BindReserved,
        body: &body,
    })
    .unwrap();
    let index = container.chunks().unwrap().len();
    container.append(&frame).unwrap();
    index
}

fn append_reverse_tail(path: &Path, id: EntityId) -> (usize, usize) {
    let owner = Cva::open(path).unwrap();
    let previous = owner
        .entities
        .records()
        .iter()
        .find(|record| record.id == id)
        .unwrap()
        .clone();
    drop(owner);

    let mut container = Container::open(path).unwrap();
    assert_eq!(
        container
            .allocate_version_with_transaction_time(Some(111))
            .unwrap(),
        2
    );
    assert_eq!(
        container
            .allocate_version_with_transaction_time(Some(222))
            .unwrap(),
        3
    );

    let mut second = previous.clone();
    second.revision = 2;
    second.global_version = 3;
    second.entity_version = 2;
    second.mutation_id = "envelope-revision-two".into();
    second.summary = "Second revision bound to the later reserved ticket".into();
    second.updated_at_ns += 1;
    let first_slot = append_reserved(
        &mut container,
        3,
        &EntityPublication::Delta(EntityDelta::between(&previous, &second).unwrap()),
    );

    let mut third = second.clone();
    third.revision = 3;
    third.global_version = 2;
    third.entity_version = 3;
    third.mutation_id = "envelope-revision-three".into();
    third.summary = "Third revision bound to the earlier reserved ticket".into();
    third.updated_at_ns += 1;
    let second_slot = append_reserved(
        &mut container,
        2,
        &EntityPublication::Delta(EntityDelta::between(&second, &third).unwrap()),
    );
    container.sync().unwrap();
    (first_slot, second_slot)
}

fn entity_slot(container: &mut Container, global: u64, local: u64) -> usize {
    container
        .chunks()
        .unwrap()
        .iter()
        .position(|chunk| {
            let payload = container.read(*chunk).unwrap();
            publication::coordinates(&payload).unwrap() == Some((global, local))
        })
        .unwrap()
}

fn base_owner(path: &Path, id: u8) -> EntityRecord {
    let mut owner = Cva::create(path).unwrap();
    let first = record(
        id,
        &format!("Entity {id}"),
        &format!("entity-{id}-create"),
        1,
        1,
        1,
    );
    append_allocated(
        &mut owner.container,
        &EntityPublication::Creation(EntityCreation::new(first.clone()).unwrap()),
        Some(100),
    );
    owner.sync().unwrap();
    first
}

#[test]
fn finite_entity_range_replays_one_slot_and_duplicate_replay_is_a_noop() {
    let scratch = Scratch::new();
    let source_path = scratch.file("source.rel");
    let base = base_owner(&source_path, 1);
    let (second_slot, third_slot) = append_reverse_tail(&source_path, base.id);

    let mut source = Cva::open(&source_path).unwrap();
    assert_eq!(source.transaction_time_ns(2), Some(111));
    assert_eq!(source.transaction_time_ns(3), Some(222));
    let creation_slot = entity_slot(&mut source.container, 1, 1);
    let mut destination = Cva::create(scratch.file("destination.rel")).unwrap();

    let creation = read_entity_range(&mut source, creation_slot, creation_slot + 1).unwrap();
    assert_eq!(
        replay_entity_tail(&mut destination, creation)
            .unwrap()
            .revisions,
        1
    );

    // The exclusive end cuts immediately before local revision three.
    let revision_two = read_entity_range(&mut source, second_slot, third_slot).unwrap();
    let replayed = replay_entity_tail(&mut destination, revision_two).unwrap();
    assert_eq!((replayed.revisions, replayed.duplicate_revisions), (1, 0));
    assert_eq!(destination.entity(base.id).unwrap().revision, 2);
    assert!(destination.entity_revision(base.id, 3).is_err());
    let revision_two = destination.entity_revision(base.id, 2).unwrap();
    let source_revision_two = source.entity_revision(base.id, 2).unwrap();
    assert_eq!(
        destination.transaction_time_ns(revision_two.global_version),
        source.transaction_time_ns(source_revision_two.global_version)
    );

    let final_slot = read_entity_range(&mut source, third_slot, third_slot + 1).unwrap();
    let replayed = replay_entity_tail(&mut destination, final_slot).unwrap();
    assert_eq!((replayed.revisions, replayed.duplicate_revisions), (1, 0));
    let latest_global = destination.latest_global_version();
    let duplicate = read_entity_range(&mut source, third_slot, third_slot + 1).unwrap();
    let duplicate = replay_entity_tail(&mut destination, duplicate).unwrap();
    assert_eq!((duplicate.revisions, duplicate.duplicate_revisions), (0, 1));
    assert_eq!(destination.latest_global_version(), latest_global);
    let destination_revision_three = destination.entity_revision(base.id, 3).unwrap();
    let source_revision_three = source.entity_revision(base.id, 3).unwrap();
    assert_eq!(
        destination.transaction_time_ns(destination_revision_three.global_version),
        source.transaction_time_ns(source_revision_three.global_version)
    );
}

#[test]
fn divergent_reconcile_replays_entity_envelopes_with_reserved_source_times() {
    let scratch = Scratch::new();
    let base_path = scratch.file("base.rel");
    let base = base_owner(&base_path, 2);
    let left_path = scratch.file("left.rel");
    let right_path = scratch.file("right.rel");
    fs::copy(&base_path, &left_path).unwrap();
    fs::copy(&base_path, &right_path).unwrap();
    append_reverse_tail(&right_path, base.id);

    let mut left = Cva::open(&left_path).unwrap();
    left.append_node(
        "left-only-user".into(),
        "left-only-conversation".into(),
        None,
        "user".into(),
        50,
        "An unrelated branch event.".into(),
    )
    .unwrap();
    left.sync().unwrap();
    drop(left);

    let output_path = scratch.file("reconciled.rel");
    let result = Cva::reconcile(&left_path, &right_path, &output_path).unwrap();
    assert_eq!(result.replayed_entity_revisions, 2);
    assert_eq!(result.duplicate_entity_revisions, 0);
    let mut output = Cva::open(&output_path).unwrap();
    assert_eq!(output.entity(base.id).unwrap().revision, 3);
    assert_eq!(output.archive.source_node_count(), 1);
    for revision in [2, 3] {
        let mut source = Cva::open(&right_path).unwrap();
        let source_revision = source.entity_revision(base.id, revision).unwrap();
        let output_revision = output.entity_revision(base.id, revision).unwrap();
        assert_eq!(
            output.transaction_time_ns(output_revision.global_version),
            source.transaction_time_ns(source_revision.global_version),
            "source time for Entity revision {revision}"
        );
    }
}

#[test]
fn entity_envelope_direct_and_merge_terminal_evidence_replays() {
    let scratch = Scratch::new();
    let source_path = scratch.file("terminals.rel");
    let mut source = Cva::create(&source_path).unwrap();
    let direct = record(3, "Direct terminal", "direct-create", 1, 1, 1);
    let merged = record(4, "Merged terminal", "merge-create", 1, 2, 2);
    let survivor = record(5, "Merge survivor", "survivor-create", 1, 3, 3);
    for (record, time) in [(&direct, 101), (&merged, 102), (&survivor, 103)] {
        append_allocated(
            &mut source.container,
            &EntityPublication::Creation(EntityCreation::new(record.clone()).unwrap()),
            Some(time),
        );
    }
    append_allocated(
        &mut source.container,
        &EntityPublication::DirectTombstone {
            id: direct.id,
            revision: 2,
            entity_version: 4,
        },
        Some(104),
    );
    append_allocated(
        &mut source.container,
        &EntityPublication::MergeTombstone {
            id: merged.id,
            revision: 2,
            entity_version: 5,
            replacement_id: survivor.id,
        },
        Some(105),
    );
    source.sync().unwrap();
    drop(source);

    let mut source = Cva::open(&source_path).unwrap();
    let direct_slot = entity_slot(&mut source.container, 4, 4);
    let merge_slot = entity_slot(&mut source.container, 5, 5);
    let mut destination = Cva::create(scratch.file("terminal-destination.rel")).unwrap();
    let initial = read_entity_range(&mut source, 0, direct_slot).unwrap();
    assert_eq!(
        replay_entity_tail(&mut destination, initial)
            .unwrap()
            .revisions,
        3
    );

    let direct_tail = read_entity_range(&mut source, direct_slot, direct_slot + 1).unwrap();
    let direct_result = replay_entity_tail(&mut destination, direct_tail).unwrap();
    assert_eq!(
        (direct_result.revisions, direct_result.duplicate_revisions),
        (1, 0)
    );
    assert!(matches!(
        destination.entity_lifecycle_state(direct.id).unwrap(),
        LifecycleState::Tombstoned(TombstoneCoordinates {
            global_version: Some(4),
            domain_version: Some(4),
        })
    ));

    let merge_tail = read_entity_range(&mut source, merge_slot, merge_slot + 1).unwrap();
    let merge_result = replay_entity_tail(&mut destination, merge_tail).unwrap();
    assert_eq!(
        (merge_result.revisions, merge_result.duplicate_revisions),
        (1, 0)
    );
    assert_eq!(
        destination.retired_entity_replacement(merged.id),
        Some(survivor.id)
    );
    let duplicate_merge = read_entity_range(&mut source, merge_slot, merge_slot + 1).unwrap();
    let duplicate_merge = replay_entity_tail(&mut destination, duplicate_merge).unwrap();
    assert_eq!(
        (
            duplicate_merge.revisions,
            duplicate_merge.duplicate_revisions
        ),
        (0, 1)
    );
    assert_eq!(
        destination.retired_entity_replacement(merged.id),
        Some(survivor.id)
    );
}

#[test]
fn reconcile_interval_rejects_unknown_and_malformed_entity_envelopes() {
    let scratch = Scratch::new();
    for (name, body) in [
        ("unknown", b"UNKNOWN1opaque owner body".as_slice()),
        ("malformed", b"CVAENC02".as_slice()),
    ] {
        let base_path = scratch.file(&format!("{name}-base.rel"));
        base_owner(&base_path, if name == "unknown" { 6 } else { 7 });
        let left_path = scratch.file(&format!("{name}-left.rel"));
        let right_path = scratch.file(&format!("{name}-right.rel"));
        fs::copy(&base_path, &left_path).unwrap();
        fs::copy(&base_path, &right_path).unwrap();
        let mut right = Container::open(&right_path).unwrap();
        if name == "unknown" {
            let reserved = right
                .allocate_version_with_transaction_time(Some(700))
                .unwrap();
            let frame = publication_envelope::encode(PublicationEnvelope {
                global_version: reserved,
                clock: PublicationClock::BindReserved,
                body,
            })
            .unwrap();
            right.append(&frame).unwrap();
        } else {
            right.append_publication_envelope(body).unwrap();
        }
        right.sync().unwrap();
        drop(right);
        let mut left = Cva::open(&left_path).unwrap();
        left.append_node(
            format!("{name}-user"),
            format!("{name}-conversation"),
            None,
            "user".into(),
            70,
            "A divergent owner event.".into(),
        )
        .unwrap();
        left.sync().unwrap();
        drop(left);

        let output = scratch.file(&format!("{name}-output.rel"));
        assert!(Cva::reconcile(&left_path, &right_path, &output).is_err());
    }
}
