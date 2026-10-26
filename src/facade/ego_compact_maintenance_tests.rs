use crate::ego_codec::compact;
use crate::ego_compact_contract::{EgoCreation, EgoDelta, EgoTerminal};
use crate::historical_ego_fingerprint::EgoHistory;
use crate::owner_lifecycle::LifecycleTarget;
use crate::{
    Cva, EgoAnchorId, EgoAnchorPriority, EgoIdentityId, HistoryPruneRequest, IncomingAttachment,
    IncomingTurn, MemoryDraft, Phylactery, ProjectFileRef, ProjectRepositoryKind,
    ProjectRepositoryRef, ProjectRevisionRef,
};
use reliquary_container::Container;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

struct Scratch(PathBuf);

impl Scratch {
    fn new(label: &str) -> Self {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(".test-tmp")
            .join(format!(
                "ego-compact-maintenance-{label}-{}",
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

fn memory_draft(id: &str) -> MemoryDraft {
    MemoryDraft {
        category: "fact".into(),
        memory_type: "project".into(),
        authority_kind: "direct".into(),
        temporal_status: "current".into(),
        title: "Maintenance target".into(),
        content: format!("Large body for pruning: {}", "x".repeat(96_000)),
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
        mutation_id: id.into(),
        created_at_ns: 10,
        updated_at_ns: 10,
    }
}

fn compact_rel_history() -> Vec<Vec<u8>> {
    use crate::ego_codec::EgoRecord;
    let aid = EgoAnchorId([0x71; 16]);
    let a1 = EgoRecord::Anchor {
        ego_version: 1,
        id: aid,
        revision: 1,
        deleted: false,
        priority: EgoAnchorPriority::Low,
        text: "first REL anchor".into(),
    };
    let s1 = EgoRecord::Synthesis {
        ego_version: 2,
        revision: 1,
        source_memory_version: 1,
        text: "first REL synthesis".into(),
    };
    let a2 = EgoRecord::Anchor {
        ego_version: 3,
        id: aid,
        revision: 2,
        deleted: false,
        priority: EgoAnchorPriority::High,
        text: "revised REL anchor".into(),
    };
    let s2 = EgoRecord::Synthesis {
        ego_version: 4,
        revision: 2,
        source_memory_version: 0,
        text: "revised REL synthesis".into(),
    };
    let a3 = EgoRecord::Anchor {
        ego_version: 5,
        id: aid,
        revision: 3,
        deleted: false,
        priority: EgoAnchorPriority::High,
        text: "revised REL anchor".into(),
    };
    let a4 = EgoRecord::Anchor {
        ego_version: 6,
        id: aid,
        revision: 4,
        deleted: true,
        priority: EgoAnchorPriority::Normal,
        text: String::new(),
    };
    let s3 = EgoRecord::Synthesis {
        ego_version: 7,
        revision: 3,
        source_memory_version: 0,
        text: "revised REL synthesis".into(),
    };
    vec![
        compact::encode_creation(&EgoCreation::new(a1.clone()).unwrap()).unwrap(),
        compact::encode_creation(&EgoCreation::new(s1.clone()).unwrap()).unwrap(),
        compact::encode_delta(&EgoDelta::between(&a1, &a2).unwrap()).unwrap(),
        compact::encode_delta(&EgoDelta::between(&s1, &s2).unwrap()).unwrap(),
        compact::encode_delta(&EgoDelta::between(&a2, &a3).unwrap()).unwrap(),
        compact::encode_terminal(&EgoTerminal::new(a4).unwrap()).unwrap(),
        compact::encode_delta(&EgoDelta::between(&s2, &s3).unwrap()).unwrap(),
    ]
}

fn compact_phy_history() -> Vec<Vec<u8>> {
    use crate::ego_codec::EgoRecord;
    let first_id = EgoIdentityId([0x72; 16]);
    let second_id = EgoIdentityId([0x73; 16]);
    let i1 = EgoRecord::Identity {
        ego_version: 1,
        id: first_id,
        revision: 1,
        deleted: false,
        activate: true,
        name: "Default".into(),
        text: "first identity".into(),
    };
    let i2 = EgoRecord::Identity {
        ego_version: 2,
        id: second_id,
        revision: 1,
        deleted: false,
        activate: false,
        name: "Work".into(),
        text: "work identity".into(),
    };
    let i3 = EgoRecord::Identity {
        ego_version: 3,
        id: second_id,
        revision: 2,
        deleted: false,
        activate: true,
        name: "Work".into(),
        text: "work identity".into(),
    };
    let i4 = EgoRecord::Identity {
        ego_version: 4,
        id: second_id,
        revision: 3,
        deleted: false,
        activate: true,
        name: "Work".into(),
        text: "work identity".into(),
    };
    let p1 = EgoRecord::Personality {
        ego_version: 5,
        revision: 1,
        source_memory_version: 1,
        text: "clear and concise".into(),
    };
    let p2 = EgoRecord::Personality {
        ego_version: 6,
        revision: 2,
        source_memory_version: 0,
        text: "clear and concise".into(),
    };
    let i5 = EgoRecord::Identity {
        ego_version: 7,
        id: first_id,
        revision: 2,
        deleted: false,
        activate: true,
        name: "Default".into(),
        text: "revised first identity".into(),
    };
    let i6 = EgoRecord::Identity {
        ego_version: 8,
        id: second_id,
        revision: 4,
        deleted: true,
        activate: false,
        name: String::new(),
        text: String::new(),
    };
    vec![
        compact::encode_creation(&EgoCreation::new(i1).unwrap()).unwrap(),
        compact::encode_creation(&EgoCreation::new(i2.clone()).unwrap()).unwrap(),
        compact::encode_delta(&EgoDelta::between(&i2, &i3).unwrap()).unwrap(),
        compact::encode_delta(&EgoDelta::between(&i3, &i4).unwrap()).unwrap(),
        compact::encode_creation(&EgoCreation::new(p1.clone()).unwrap()).unwrap(),
        compact::encode_delta(&EgoDelta::between(&p1, &p2).unwrap()).unwrap(),
        compact::encode_delta(
            &EgoDelta::between(
                &EgoRecord::Identity {
                    ego_version: 1,
                    id: first_id,
                    revision: 1,
                    deleted: false,
                    activate: true,
                    name: "Default".into(),
                    text: "first identity".into(),
                },
                &i5,
            )
            .unwrap(),
        )
        .unwrap(),
        compact::encode_terminal(&EgoTerminal::new(i6).unwrap()).unwrap(),
    ]
}

fn append_history(path: &Path, publications: Vec<Vec<u8>>) {
    let mut container = Container::open(path).unwrap();
    for payload in publications {
        container.append(&payload).unwrap();
    }
    container.sync().unwrap();
}

fn assert_every_ego_cut_equal(before: &EgoHistory, after: &EgoHistory, head: u64) {
    for cut in 0..=head {
        assert_eq!(
            before.at(cut).unwrap().fingerprint,
            after.at(cut).unwrap().fingerprint,
            "Ego fingerprint changed at cut {cut}"
        );
    }
}

#[test]
fn rel_prune_then_reclaim_preserves_every_compact_ego_cut() {
    let scratch = Scratch::new("rel");
    let source = scratch.file("source.rel");
    let pruned_path = scratch.file("pruned.rel");
    let reclaimed_path = scratch.file("reclaimed.rel");
    let principal_repacked_path = scratch.file("principal-repacked.rel");
    let attachment_repacked_path = scratch.file("attachment-repacked.rel");
    let mut rel = Cva::create_project(&source).unwrap();
    let attachment_bytes = b"owner-backed source attachment";
    let ingested = rel
        .ingest_turn(IncomingTurn {
            id: "maintenance-user-turn".into(),
            conversation_id: "maintenance-conversation".into(),
            parent_id: None,
            role: "user".into(),
            principal_id: None,
            timestamp_ns: 10,
            content: "Use the attached source.".into(),
            attachments: vec![IncomingAttachment {
                filename: "source.txt".into(),
                mime_type: Some("text/plain".into()),
                bytes: attachment_bytes.to_vec(),
            }],
            project_attachments: Vec::new(),
        })
        .unwrap();
    let file = ingested.attachments[0].clone();
    rel.bind_legacy_project_file(
        file.id,
        ProjectFileRef {
            revision: ProjectRevisionRef {
                repository: ProjectRepositoryRef {
                    kind: ProjectRepositoryKind::Lore,
                    repository_id: "ego-maintenance-repo".into(),
                    project_path: ".".into(),
                },
                revision: "ego-maintenance-revision".into(),
            },
            path: "uploads/source.txt".into(),
            content_hash: Some(Sha256::digest(attachment_bytes).into()),
        },
    )
    .unwrap();
    let memory = rel
        .publish_memory(None, 0, memory_draft("rel-prune-target"))
        .unwrap()
        .0;
    rel.tombstone_memory(memory.id, memory.revision).unwrap();
    rel.sync().unwrap();
    drop(rel);
    append_history(&source, compact_rel_history());

    let mut source_rel = Cva::open(&source).unwrap();
    let source_history = EgoHistory::read(&source_rel.container, &source_rel.ego).unwrap();
    let head = source_rel.ego_version();
    assert_eq!(head, 7);

    // Both REL repack paths copy the compact Ego chain as opaque owner data.
    source_rel
        .repack_missing_user_principal(
            &principal_repacked_path,
            "phy-00000000-0000-0000-0000-000000000123",
        )
        .unwrap();
    let principal_repacked = Cva::open(&principal_repacked_path).unwrap();
    let principal_history =
        EgoHistory::read(&principal_repacked.container, &principal_repacked.ego).unwrap();
    assert_every_ego_cut_equal(&source_history, &principal_history, head);
    source_rel
        .repack_project_backed_attachments(&attachment_repacked_path)
        .unwrap();
    let attachment_repacked = Cva::open(&attachment_repacked_path).unwrap();
    let attachment_history =
        EgoHistory::read(&attachment_repacked.container, &attachment_repacked.ego).unwrap();
    assert_every_ego_cut_equal(&source_history, &attachment_history, head);

    let report = source_rel
        .prune_tombstoned_history(
            &pruned_path,
            HistoryPruneRequest {
                targets: vec![LifecycleTarget::Memory(memory.id)],
            },
        )
        .unwrap();
    assert!(report.pruned_memory_bodies > 0);
    let mut pruned = Cva::open(&pruned_path).unwrap();
    let pruned_history = EgoHistory::read(&pruned.container, &pruned.ego).unwrap();
    assert_every_ego_cut_equal(&source_history, &pruned_history, head);
    pruned.reclaim_storage(&reclaimed_path).unwrap();
    let reclaimed = Cva::open(&reclaimed_path).unwrap();
    let reclaimed_history = EgoHistory::read(&reclaimed.container, &reclaimed.ego).unwrap();
    assert_every_ego_cut_equal(&source_history, &reclaimed_history, head);
}

#[test]
fn phy_prune_then_reclaim_preserves_every_compact_ego_cut() {
    let scratch = Scratch::new("phy");
    let source = scratch.file("source.phy");
    let pruned_path = scratch.file("pruned.phy");
    let reclaimed_path = scratch.file("reclaimed.phy");
    let mut phy = Phylactery::create(&source).unwrap();
    let memory = phy
        .publish_memory(None, 0, memory_draft("phy-prune-target"))
        .unwrap()
        .0;
    phy.tombstone_memory(memory.id, memory.revision).unwrap();
    phy.sync().unwrap();
    drop(phy);
    append_history(&source, compact_phy_history());

    let mut source_phy = Phylactery::open(&source).unwrap();
    let source_history = EgoHistory::read(&source_phy.container, &source_phy.ego).unwrap();
    let head = source_phy.ego_version();
    assert_eq!(head, 8);
    let report = source_phy
        .prune_tombstoned_history(
            &pruned_path,
            HistoryPruneRequest {
                targets: vec![LifecycleTarget::Memory(memory.id)],
            },
        )
        .unwrap();
    assert!(report.pruned_memory_bodies > 0);
    let mut pruned = Phylactery::open(&pruned_path).unwrap();
    let pruned_history = EgoHistory::read(&pruned.container, &pruned.ego).unwrap();
    assert_every_ego_cut_equal(&source_history, &pruned_history, head);
    pruned.reclaim_storage(&reclaimed_path).unwrap();
    let reclaimed = Phylactery::open(&reclaimed_path).unwrap();
    let reclaimed_history = EgoHistory::read(&reclaimed.container, &reclaimed.ego).unwrap();
    assert_every_ego_cut_equal(&source_history, &reclaimed_history, head);
}
