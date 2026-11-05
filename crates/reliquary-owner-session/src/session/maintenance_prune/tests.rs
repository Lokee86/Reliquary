use super::*;
use reliquary_container::ContainerIdentity;
use reliquary_memory_core::{MemoryDraft, MemoryStore};
use reliquary_vector_store::{CompatibilityProfileStore, MemoryVectorStore, PackedVectorStore};
use std::{
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(1);
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".test-tmp");
        fs::create_dir_all(&root).unwrap();
        Self(root.join(format!(
            "pruned-owner-{}-{}.phy",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed),
        )))
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

fn draft(mutation: &str, content: &str) -> MemoryDraft {
    MemoryDraft {
        title: "maintenance-content".into(),
        content: content.into(),
        category: "fact".into(),
        memory_type: "user".into(),
        authority_kind: "direct".into(),
        temporal_status: "past".into(),
        scope: "user".into(),
        lifecycle_state: "knowledge".into(),
        archived: false,
        parent_id: None,
        superseded_by: None,
        source_node_id: None,
        content_source_conversation_id: None,
        content_source_node_id: None,
        grounding_source_conversation_id: None,
        grounding_source_node_id: None,
        source_episode_id: None,
        source_ref: None,
        source_time_ns: None,
        temporal_inference: None,
        mutation_id: mutation.into(),
        created_at_ns: 1,
        updated_at_ns: 1,
    }
}

#[test]
fn terminal_memory_prune_removes_payload_reopens_and_rejects_live_or_tampered_history() {
    let source = Temp::new();
    let output = Temp::new();
    let forged = Temp::new();
    let owner = [0xa1; 16];
    let private = "sensitive terminal content ".repeat(350);
    let (tombstoned, live);
    {
        let mut container =
            Container::create(&source.0, ContainerIdentity::phylactery(), owner).unwrap();
        PackedVectorStore::default()
            .initialize(&mut container)
            .unwrap();
        CompatibilityProfileStore::default()
            .initialize(&mut container)
            .unwrap();
        MemoryVectorStore::default()
            .initialize(&mut container)
            .unwrap();
        let mut memories = MemoryStore::create(&mut container).unwrap();
        live = memories
            .publish(
                &mut container,
                None,
                0,
                draft("keep-live", "important active fact"),
            )
            .unwrap()
            .revision
            .record
            .id;
        tombstoned = memories
            .publish(&mut container, None, 0, draft("discard-later", &private))
            .unwrap()
            .revision
            .record
            .id;
        memories.tombstone(&mut container, tombstoned, 1).unwrap();
        container.sync().unwrap();
    }
    let snapshot = fs::read(&source.0).unwrap();
    let grant = GraphAdmission::for_replay(owner, [], [], [], [], []);
    let plan = VerifiedOwnerCopyPlan::prepare(&source.0).unwrap();
    assert!(prune_terminal_memory_copy(&plan, &source.0, &output.0, &[live], &grant).is_err());
    assert!(!output.0.exists());
    let report =
        prune_terminal_memory_copy(&plan, &source.0, &output.0, &[tombstoned], &grant).unwrap();
    assert_eq!(report.pruned_memories, 1);
    assert_eq!(fs::read(&source.0).unwrap(), snapshot);
    assert!(report.transformed_bytes < report.source_bytes);
    let transformed = fs::read(&output.0).unwrap();
    assert!(
        !transformed
            .windows(private.len())
            .any(|part| part == private.as_bytes())
    );
    assert!(matches!(
        PhyWriter::open_path(&output.0, &grant, []),
        Err(OwnerSessionError::Record(message))
            if message == "transformed maintenance snapshots are read-only"
    ));
    let session = PhySession::open_path(&output.0, &grant, []).unwrap();
    let removed = session.memory_history(tombstoned).unwrap().unwrap();
    assert!(removed.tombstone.is_some());
    assert!(matches!(removed.revisions[0].payload, PayloadState::Absent));
    let retained = session.memory_history(live).unwrap().unwrap();
    assert!(matches!(
        retained.revisions[0].payload,
        PayloadState::Available(_)
    ));
    drop(session);
    fs::write(&forged.0, &transformed).unwrap();
    let mut bytes = fs::read(&forged.0).unwrap();
    let at = bytes
        .windows(8)
        .position(|window| window == b"RELHPR01")
        .unwrap();
    bytes[at + 30] ^= 1;
    fs::write(&forged.0, &bytes).unwrap();
    assert!(PhySession::open_path(&forged.0, &grant, []).is_err());
    let appended = Temp::new();
    fs::write(&appended.0, &transformed).unwrap();
    let mut appended_writer = Container::open_writer(&appended.0).unwrap();
    appended_writer.append(b"RELMFMT1").unwrap();
    appended_writer.sync().unwrap();
    drop(appended_writer);
    assert!(PhySession::open_path(&appended.0, &grant, []).is_err());
    assert!(
        prune_terminal_memory_copy(&plan, &source.0, &output.0, &[tombstoned], &grant).is_err()
    );
    assert_eq!(fs::read(&output.0).unwrap(), transformed);
}

#[test]
fn unsafe_shared_memory_body_and_unknown_frame_fail_without_creating_destination() {
    let source = Temp::new();
    let target = Temp::new();
    let owner = [0xa2; 16];
    let mut container =
        Container::create(&source.0, ContainerIdentity::phylactery(), owner).unwrap();
    PackedVectorStore::default()
        .initialize(&mut container)
        .unwrap();
    CompatibilityProfileStore::default()
        .initialize(&mut container)
        .unwrap();
    MemoryVectorStore::default()
        .initialize(&mut container)
        .unwrap();
    let mut memories = MemoryStore::create(&mut container).unwrap();
    let body = "identical shared body ".repeat(80);
    let first = memories
        .publish(&mut container, None, 0, draft("shared-a", &body))
        .unwrap()
        .revision
        .record
        .id;
    let second = memories
        .publish(&mut container, None, 0, draft("shared-b", &body))
        .unwrap()
        .revision
        .record
        .id;
    memories.tombstone(&mut container, first, 1).unwrap();
    memories.tombstone(&mut container, second, 1).unwrap();
    container.sync().unwrap();
    drop(memories);
    drop(container);
    let plan = VerifiedOwnerCopyPlan::prepare(&source.0).unwrap();
    let grant = GraphAdmission::for_replay(owner, [], [], [], [], []);
    assert!(prune_terminal_memory_copy(&plan, &source.0, &target.0, &[first], &grant).is_err());
    assert!(!target.0.exists());
    let mut writer = Container::open_writer(&source.0).unwrap();
    writer.append(b"UNKNWN01-opaque-reference").unwrap();
    writer.sync().unwrap();
    drop(writer);
    let plan = VerifiedOwnerCopyPlan::prepare(&source.0).unwrap();
    assert!(
        prune_terminal_memory_copy(&plan, &source.0, &target.0, &[first, second], &grant).is_err()
    );
    assert!(!target.0.exists());
}
