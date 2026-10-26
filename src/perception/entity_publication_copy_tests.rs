//! Entity envelope history behavior across native copies and semantic rewrites.
use reliquary_perception_store::entity_compact_codec::publication::{self, EntityPublication};
use reliquary_perception_store::entity_compact_contract::{EntityCreation, EntityDelta};
use reliquary_perception_store::entity_model::EntityRecord;
use crate::rewrite::MemorySelection;
use crate::{
    Cva, EntityGlobalId, EntityId, MemoryDraft, MemoryId, Phylactery, ProjectFileRef,
    ProjectRepositoryKind, ProjectRepositoryRef, ProjectRevisionRef, RewriteRequest,
};
use reliquary_container::publication_envelope::{PublicationClock, PublicationEnvelope};
use reliquary_container::{Container, FileKind};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(".test-tmp")
            .join(format!("entity-publication-copy-{}", uuid::Uuid::new_v4()));
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

fn record(id: u8, revision: u64, global: u64, local: u64) -> EntityRecord {
    EntityRecord {
        id: EntityId([id; 32]),
        global_id: Some(EntityGlobalId([id.wrapping_add(70); 16])),
        revision,
        canonical_name: format!("Entity {id} revision {revision}"),
        aliases: vec![format!("Alias {id}")],
        kind: "project".into(),
        summary: format!("Summary {id} revision {revision}"),
        mutation_id: format!("entity-{id}-mutation-{revision}"),
        created_at_ns: -10,
        updated_at_ns: -5 + revision as i64,
        global_version: global,
        entity_version: local,
    }
}

fn append_allocate(container: &mut Container, value: EntityPublication, time: Option<i64>) -> u64 {
    let expected = container.latest_version() + 1;
    let body = publication::encode(&value).unwrap();
    container.set_next_transaction_time_override(time);
    assert_eq!(
        container.append_publication_envelope(&body).unwrap().0,
        expected
    );
    expected
}

fn append_reserved(container: &mut Container, value: EntityPublication, time: Option<i64>) -> u64 {
    let expected = container
        .allocate_version_with_transaction_time(time)
        .unwrap();
    let body = publication::encode(&value).unwrap();
    let envelope = reliquary_container::publication_envelope::encode(PublicationEnvelope {
        global_version: expected,
        clock: PublicationClock::BindReserved,
        body: &body,
    })
    .unwrap();
    container.append(&envelope).unwrap();
    expected
}

fn append_history_item(
    container: &mut Container,
    reserved: bool,
    value: EntityPublication,
    time: Option<i64>,
) -> u64 {
    if reserved {
        append_reserved(container, value, time)
    } else {
        append_allocate(container, value, time)
    }
}

fn write_copy_history(
    container: &mut Container,
    reserved: bool,
    terminals: bool,
) -> (EntityId, EntityId) {
    let a1 = record(0x41, 1, container.latest_version() + 1, 1);
    append_history_item(
        container,
        reserved,
        EntityPublication::Creation(EntityCreation::new(a1.clone()).unwrap()),
        Some(-111),
    );
    let mut a2 = a1.clone();
    a2.revision = 2;
    a2.global_version = container.latest_version() + 1;
    a2.entity_version = 2;
    a2.mutation_id = "entity-65-mutation-2".into();
    a2.canonical_name = "Entity 65 revised".into();
    append_history_item(
        container,
        reserved,
        EntityPublication::Delta(EntityDelta::between(&a1, &a2).unwrap()),
        None,
    );
    let b1 = record(0x42, 1, container.latest_version() + 1, 3);
    append_history_item(
        container,
        reserved,
        EntityPublication::Creation(EntityCreation::new(b1.clone()).unwrap()),
        Some(0),
    );
    if terminals {
        append_history_item(
            container,
            reserved,
            EntityPublication::MergeTombstone {
                id: a1.id,
                revision: 3,
                entity_version: 4,
                replacement_id: b1.id,
            },
            Some(-444),
        );
        append_history_item(
            container,
            reserved,
            EntityPublication::DirectTombstone {
                id: b1.id,
                revision: 2,
                entity_version: 5,
            },
            None,
        );
    }
    (a1.id, b1.id)
}

fn envelope_payloads(container: &Container) -> Vec<Vec<u8>> {
    let source = container.read_source().unwrap();
    let mut cursor = source.physical_cursor().unwrap();
    let mut payloads = Vec::new();
    while let Some(frame) = cursor.next_frame().unwrap() {
        if frame.payload.starts_with(b"CVAPUB01") {
            payloads.push(frame.payload);
        }
    }
    payloads
}

fn assert_entity_history_equal(
    left_container: &Container,
    left: &reliquary_perception_store::EntityStore,
    right_container: &Container,
    right: &reliquary_perception_store::EntityStore,
) {
    assert_eq!(left.entity_version(), right.entity_version());
    for cut in 0..=left.entity_version() {
        let a =
            crate::historical_entity_fingerprint::entity_cut(left_container, left, cut).unwrap();
        let b =
            crate::historical_entity_fingerprint::entity_cut(right_container, right, cut).unwrap();
        assert_eq!(
            a.fingerprint, b.fingerprint,
            "Entity fingerprint at cut {cut}"
        );
        assert_eq!(a.current, b.current, "current entities at cut {cut}");
        assert_eq!(
            a.replacements, b.replacements,
            "replacement map at cut {cut}"
        );
    }
    assert_eq!(
        left_container.latest_version(),
        right_container.latest_version()
    );
    for global in 0..=left_container.latest_version() {
        assert_eq!(
            left_container.transaction_time_ns(global),
            right_container.transaction_time_ns(global),
            "transaction time at global cut {global}"
        );
    }
}

fn project_ref(bytes: &[u8]) -> ProjectFileRef {
    ProjectFileRef {
        revision: ProjectRevisionRef {
            repository: ProjectRepositoryRef {
                kind: ProjectRepositoryKind::Lore,
                repository_id: "entity-publication-copy".into(),
                project_path: ".".into(),
            },
            revision: "immutable-revision".into(),
        },
        path: "uploads/backed.bin".into(),
        content_hash: Some(Sha256::digest(bytes).into()),
    }
}

#[test]
fn rel_native_attachment_repack_preserves_all_entity_envelope_cuts() {
    let scratch = Scratch::new();
    let source_path = scratch.file("source.rel");
    let output_path = scratch.file("repacked.rel");
    drop(Cva::create_project(&source_path).unwrap());
    let bytes = vec![0x7a; 32 * 1024];
    let mut owner = Cva::open_project(&source_path).unwrap();
    let file = owner.store_file("backed.bin".into(), None, &bytes).unwrap();
    owner
        .bind_legacy_project_file(file.id, project_ref(&bytes))
        .unwrap();
    owner.sync().unwrap();
    drop(owner);

    let mut container = Container::open(&source_path).unwrap();
    let ids = write_copy_history(&mut container, false, true);
    container.sync().unwrap();
    drop(container);
    let mut source = Cva::open_project(&source_path).unwrap();
    let before_envelopes = envelope_payloads(&source.container);
    let report = source
        .repack_project_backed_attachments(&output_path)
        .unwrap();
    assert_eq!(report.dropped_content_objects, 1);
    let mut output = Cva::open_project(&output_path).unwrap();
    assert_eq!(envelope_payloads(&output.container), before_envelopes);
    assert_eq!(source.entities.records(), output.entities.records());
    assert_entity_history_equal(
        &source.container,
        &source.entities,
        &output.container,
        &output.entities,
    );
    assert_eq!(
        source.entity_retention_descriptor(ids.0).unwrap(),
        output.entity_retention_descriptor(ids.0).unwrap()
    );
    assert_eq!(
        source.entity_retention_descriptor(ids.1).unwrap(),
        output.entity_retention_descriptor(ids.1).unwrap()
    );
}

fn memory_draft(name: &str) -> MemoryDraft {
    MemoryDraft {
        category: "fact".into(),
        memory_type: "project".into(),
        authority_kind: "direct".into(),
        temporal_status: "current".into(),
        title: name.into(),
        content: format!("Body for {name}"),
        scope: "project".into(),
        lifecycle_state: "knowledge".into(),
        archived: false,
        superseded_by: None,
        parent_id: None,
        source_node_id: None,
        content_source_conversation_id: None,
        content_source_node_id: None,
        grounding_source_conversation_id: None,
        grounding_source_node_id: None,
        source_episode_id: None,
        source_time_ns: None,
        mutation_id: format!("create-{name}"),
        created_at_ns: 10,
        updated_at_ns: 10,
    }
}

fn add_selection_anchor_rel(owner: &mut Cva, entity_id: EntityId) -> (MemoryId, MemoryId) {
    let (selected, _) = owner
        .publish_memory(None, 0, memory_draft("selected-anchor"))
        .unwrap();
    let (unselected, _) = owner
        .publish_memory(None, 0, memory_draft("unselected-anchor"))
        .unwrap();
    let expected_graph_version = owner.graph_version();
    owner
        .set_entity_association(selected.id, entity_id, true, expected_graph_version)
        .unwrap();
    (selected.id, unselected.id)
}

fn add_selection_anchor_phy(owner: &mut Phylactery, entity_id: EntityId) -> (MemoryId, MemoryId) {
    let (selected, _) = owner
        .publish_memory(None, 0, memory_draft("selected-anchor"))
        .unwrap();
    let (unselected, _) = owner
        .publish_memory(None, 0, memory_draft("unselected-anchor"))
        .unwrap();
    let expected_graph_version = owner.graph_version();
    owner
        .set_entity_association(selected.id, entity_id, true, expected_graph_version)
        .unwrap();
    (selected.id, unselected.id)
}

fn assert_rewrite_selected_entity_rel(path: &Path, selected: EntityId, excluded: EntityId) {
    let owner = Cva::open(path).unwrap();
    assert_eq!(owner.entities.records().len(), 1);
    let copied = owner.entities.records()[0].clone();
    assert_eq!(copied.id, selected);
    assert_eq!(copied.revision, 1);
    assert_eq!(copied.canonical_name, "Entity 65 revised");
    assert!(owner.entity_revision(selected, 2).is_err());
    assert!(owner.entity_revision(excluded, 1).is_err());
    assert_eq!(owner.entity_version(), 1);
}

fn assert_rewrite_selected_entity_phy(path: &Path, selected: EntityId, excluded: EntityId) {
    let owner = Phylactery::open(path).unwrap();
    assert_eq!(owner.entities.records().len(), 1);
    let copied = owner.entities.records()[0].clone();
    assert_eq!(copied.id, selected);
    assert_eq!(copied.revision, 1);
    assert_eq!(copied.canonical_name, "Entity 65 revised");
    assert!(owner.entity_revision(selected, 2).is_err());
    assert!(owner.entity_revision(excluded, 1).is_err());
    assert_eq!(owner.entity_version(), 1);
}

#[test]
fn semantic_rewrite_selects_current_entity_closure_and_resets_entity_history() {
    for kind in [FileKind::Reliquary, FileKind::Phylactery] {
        let scratch = Scratch::new();
        let source_path = scratch.file(if kind == FileKind::Reliquary {
            "rewrite-source.rel"
        } else {
            "rewrite-source.phy"
        });
        let output_path = scratch.file(if kind == FileKind::Reliquary {
            "rewrite-output.rel"
        } else {
            "rewrite-output.phy"
        });
        match kind {
            FileKind::Reliquary => drop(Cva::create(&source_path).unwrap()),
            FileKind::Phylactery => drop(Phylactery::create(&source_path).unwrap()),
        }
        let mut container = Container::open(&source_path).unwrap();
        let selected_id = EntityId([0x41; 32]);
        let excluded_id = EntityId([0x42; 32]);
        let a1 = record(0x41, 1, container.latest_version() + 1, 1);
        append_allocate(
            &mut container,
            EntityPublication::Creation(EntityCreation::new(a1.clone()).unwrap()),
            Some(1),
        );
        let mut a2 = a1.clone();
        a2.revision = 2;
        a2.global_version = container.latest_version() + 1;
        a2.entity_version = 2;
        a2.mutation_id = "entity-65-mutation-2".into();
        a2.canonical_name = "Entity 65 revised".into();
        append_allocate(
            &mut container,
            EntityPublication::Delta(EntityDelta::between(&a1, &a2).unwrap()),
            Some(2),
        );
        let b1 = record(0x42, 1, container.latest_version() + 1, 3);
        append_allocate(
            &mut container,
            EntityPublication::Creation(EntityCreation::new(b1).unwrap()),
            Some(3),
        );
        container.sync().unwrap();
        drop(container);

        let (selected_memory, excluded_memory) = match kind {
            FileKind::Reliquary => {
                let mut owner = Cva::open(&source_path).unwrap();
                add_selection_anchor_rel(&mut owner, selected_id)
            }
            FileKind::Phylactery => {
                let mut owner = Phylactery::open(&source_path).unwrap();
                add_selection_anchor_phy(&mut owner, selected_id)
            }
        };
        let selected_hex = selected_memory
            .0
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        let excluded_hex = excluded_memory
            .0
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        let request = RewriteRequest {
            memories: MemorySelection {
                include_ids: vec![selected_hex],
                exclude_ids: vec![excluded_hex],
                ..Default::default()
            },
            ..Default::default()
        };
        crate::rewrite::rewrite_file(&source_path, &output_path, &request).unwrap();
        match kind {
            FileKind::Reliquary => {
                assert_rewrite_selected_entity_rel(&output_path, selected_id, excluded_id)
            }
            FileKind::Phylactery => {
                assert_rewrite_selected_entity_phy(&output_path, selected_id, excluded_id)
            }
        }
    }
}
