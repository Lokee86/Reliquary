//! Relationship envelope history across exact physical copies and semantic rewrites.
use crate::relationship_compact_codec::publication::{self, RelationshipPublication};
use crate::relationship_compact_contract::{RelationshipCreation, RelationshipDelta};
use crate::relationship_model::RelationshipRecord;
use crate::rewrite::MemorySelection;
use crate::{
    Container, Cva, EntityId, EntityRef, MemoryDraft, MemoryId, MemoryRef, Phylactery,
    ProjectFileRef, ProjectRepositoryKind, ProjectRepositoryRef, ProjectRevisionRef,
    RelationshipDraft, RelationshipId, RelationshipParticipant, RewriteRequest,
};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(".test-tmp")
            .join(format!(
                "relationship-publication-copy-{}",
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

fn record(id: u8, revision: u64, global: u64, local: u64, mutation: &str) -> RelationshipRecord {
    RelationshipRecord {
        id: RelationshipId([id; 32]),
        revision,
        kind: "supports".into(),
        participants: vec![RelationshipParticipant {
            entity: EntityRef {
                owner_id: "remote-entity-owner".into(),
                entity_id: EntityId([id.wrapping_add(10); 32]),
            },
            role: Some("source".into()),
        }],
        evidence: vec![MemoryRef {
            owner_id: "remote-memory-owner".into(),
            memory_id: MemoryId([id.wrapping_add(20); 32]),
        }],
        summary: format!("Relationship {id} revision {revision}"),
        mutation_id: mutation.into(),
        created_at_ns: -10,
        updated_at_ns: -5 + revision as i64,
        global_version: global,
        relationship_version: local,
    }
}

fn append(
    container: &mut Container,
    value: &RelationshipPublication,
    time: Option<i64>,
) -> Vec<u8> {
    let body = publication::encode(value).unwrap();
    container.set_next_transaction_time_override(time);
    let (_, object) = container.append_publication_envelope(&body).unwrap();
    container.read(object).unwrap()
}

fn envelope_payloads(path: &Path) -> Vec<Vec<u8>> {
    let mut container = Container::open(path).unwrap();
    container
        .chunks()
        .unwrap()
        .into_iter()
        .filter_map(|object| {
            let bytes = container.read(object).unwrap();
            bytes.starts_with(b"CVAPUB01").then_some(bytes)
        })
        .collect()
}

fn assert_cuts_equal(left: &Cva, right: &Cva) {
    assert_eq!(left.relationship_version(), right.relationship_version());
    assert_eq!(left.latest_global_version(), right.latest_global_version());
    for cut in 0..=left.relationship_version() {
        let a = crate::historical_relationship_fingerprint::relationship_cut(
            &left.container,
            &left.relationships,
            cut,
        )
        .unwrap();
        let b = crate::historical_relationship_fingerprint::relationship_cut(
            &right.container,
            &right.relationships,
            cut,
        )
        .unwrap();
        assert_eq!(a.fingerprint, b.fingerprint, "Relationship cut {cut}");
        assert_eq!(a.current, b.current, "current Relationships at cut {cut}");
    }
    for global in 0..=left.latest_global_version() {
        assert_eq!(
            left.transaction_time_ns(global),
            right.transaction_time_ns(global),
            "transaction time at global {global}"
        );
    }
}

fn project_ref(bytes: &[u8]) -> ProjectFileRef {
    ProjectFileRef {
        revision: ProjectRevisionRef {
            repository: ProjectRepositoryRef {
                kind: ProjectRepositoryKind::Lore,
                repository_id: "relationship-publication-copy".into(),
                project_path: ".".into(),
            },
            revision: "immutable-revision".into(),
        },
        path: "uploads/backed.bin".into(),
        content_hash: Some(Sha256::digest(bytes).into()),
    }
}

#[test]
fn rel_attachment_repack_preserves_exact_relationship_envelopes_cuts_and_terminals() {
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

    let mut owner = Cva::open_project(&source_path).unwrap();
    let first = record(0x31, 1, owner.latest_global_version() + 1, 1, "create-one");
    let first_payload = append(
        &mut owner.container,
        &RelationshipPublication::Creation(RelationshipCreation::new(first.clone()).unwrap()),
        Some(-111),
    );
    let mut second = first.clone();
    second.revision = 2;
    second.global_version = owner.latest_global_version() + 1;
    second.relationship_version = 2;
    second.mutation_id = "update-one".into();
    second.summary = "updated Relationship".into();
    let second_payload = append(
        &mut owner.container,
        &RelationshipPublication::Delta(RelationshipDelta::between(&first, &second).unwrap()),
        None,
    );
    let independent = record(0x32, 1, owner.latest_global_version() + 1, 3, "create-two");
    let independent_payload = append(
        &mut owner.container,
        &RelationshipPublication::Creation(RelationshipCreation::new(independent.clone()).unwrap()),
        Some(0),
    );
    let terminal_payload = append(
        &mut owner.container,
        &RelationshipPublication::Tombstone {
            id: first.id,
            revision: 3,
            relationship_version: 4,
        },
        Some(-444),
    );
    owner.sync().unwrap();
    drop(owner);
    let mut owner = Cva::open_project(&source_path).unwrap();
    let expected_envelopes = vec![
        first_payload,
        second_payload,
        independent_payload,
        terminal_payload,
    ];
    assert_eq!(envelope_payloads(&source_path), expected_envelopes);
    let report = owner
        .repack_project_backed_attachments(&output_path)
        .unwrap();
    assert_eq!(report.dropped_content_objects, 1);
    let mut output = Cva::open_project(&output_path).unwrap();
    assert_eq!(envelope_payloads(&output_path), expected_envelopes);
    assert_cuts_equal(&owner, &output);
    assert_eq!(
        owner.relationship_retention_descriptor(first.id).unwrap(),
        output.relationship_retention_descriptor(first.id).unwrap()
    );
    assert_eq!(
        output.relationship_lifecycle_state(first.id).unwrap(),
        crate::owner_lifecycle::LifecycleState::Tombstoned(
            crate::owner_lifecycle::TombstoneCoordinates {
                global_version: Some(owner.latest_global_version()),
                domain_version: Some(4),
            }
        )
    );
}

#[test]
fn native_phy_reclaim_preserves_relationship_envelopes_and_every_retained_cut() {
    let scratch = Scratch::new();
    let source_path = scratch.file("source.phy");
    let output_path = scratch.file("reclaimed.phy");
    let mut source = Phylactery::create(&source_path).unwrap();
    let first = record(0x41, 1, 1, 1, "phy-create");
    let first_bytes = append(
        &mut source.container,
        &RelationshipPublication::Creation(RelationshipCreation::new(first.clone()).unwrap()),
        Some(401),
    );
    let mut second = first.clone();
    second.revision = 2;
    second.global_version = 2;
    second.relationship_version = 2;
    second.mutation_id = "phy-update".into();
    second.summary = "revised PHY Relationship".into();
    let second_bytes = append(
        &mut source.container,
        &RelationshipPublication::Delta(RelationshipDelta::between(&first, &second).unwrap()),
        Some(402),
    );
    let terminal_bytes = append(
        &mut source.container,
        &RelationshipPublication::Tombstone {
            id: first.id,
            revision: 3,
            relationship_version: 3,
        },
        Some(403),
    );
    source.sync().unwrap();
    drop(source);
    let expected = vec![first_bytes, second_bytes, terminal_bytes];
    let mut source = Phylactery::open(&source_path).unwrap();
    let report = source.reclaim_storage(&output_path).unwrap();
    let output = Phylactery::open(&output_path).unwrap();
    assert_eq!(report.reclaimed_chunks, 0);
    assert_eq!(envelope_payloads(&output_path), expected);
    assert_eq!(
        source.relationships.records(),
        output.relationships.records()
    );
    for cut in 0..=source.relationship_version() {
        let a = crate::historical_relationship_fingerprint::relationship_cut(
            &source.container,
            &source.relationships,
            cut,
        )
        .unwrap();
        let b = crate::historical_relationship_fingerprint::relationship_cut(
            &output.container,
            &output.relationships,
            cut,
        )
        .unwrap();
        assert_eq!(a.fingerprint, b.fingerprint, "PHY Relationship cut {cut}");
        assert_eq!(a.current, b.current);
    }
    for global in 0..=source.latest_global_version() {
        assert_eq!(
            source.transaction_time_ns(global),
            output.transaction_time_ns(global)
        );
    }
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

fn relationship_draft(owner_id: String, memory: MemoryId, mutation: &str) -> RelationshipDraft {
    RelationshipDraft {
        kind: "supports".into(),
        participants: vec![RelationshipParticipant {
            entity: EntityRef {
                owner_id: "remote-entity-owner".into(),
                entity_id: EntityId([0x51; 32]),
            },
            role: Some("subject".into()),
        }],
        evidence: vec![MemoryRef {
            owner_id,
            memory_id: memory,
        }],
        summary: format!("summary for {mutation}"),
        mutation_id: mutation.into(),
        created_at_ns: 10,
        updated_at_ns: 10,
    }
}

#[test]
fn semantic_rewrite_filters_relationships_by_selected_closure_and_resets_history() {
    let scratch = Scratch::new();
    let source_path = scratch.file("rewrite-source.rel");
    let output_path = scratch.file("rewrite-output.rel");
    let mut source = Cva::create(&source_path).unwrap();
    let (selected_memory, _) = source
        .publish_memory(None, 0, memory_draft("selected"))
        .unwrap();
    let (excluded_memory, _) = source
        .publish_memory(None, 0, memory_draft("excluded"))
        .unwrap();
    let owner_id = source.owner_id().unwrap();
    let (selected, _) = source
        .publish_relationship(
            None,
            0,
            relationship_draft(
                owner_id.clone(),
                selected_memory.id,
                "selected-relationship",
            ),
        )
        .unwrap();
    let (excluded, _) = source
        .publish_relationship(
            None,
            0,
            relationship_draft(owner_id, excluded_memory.id, "excluded-relationship"),
        )
        .unwrap();
    let selected_hex = selected_memory
        .id
        .0
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    let request = RewriteRequest {
        memories: MemorySelection {
            include_ids: vec![selected_hex],
            exclude_ids: vec![
                excluded_memory
                    .id
                    .0
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect(),
            ],
            ..Default::default()
        },
        ..Default::default()
    };
    source.sync().unwrap();
    drop(source);
    crate::rewrite::rewrite_file(&source_path, &output_path, &request).unwrap();
    let output = Cva::open(&output_path).unwrap();
    let copied = output.relationships();
    assert_eq!(copied.len(), 1);
    assert_eq!(copied[0].id, selected.id);
    assert_eq!(copied[0].revision, 1);
    assert_eq!(copied[0].summary, selected.summary);
    assert_eq!(copied[0].relationship_version, 1);
    assert!(output.relationship(excluded.id).is_err());
    assert_eq!(output.relationship_version(), 1);
}
