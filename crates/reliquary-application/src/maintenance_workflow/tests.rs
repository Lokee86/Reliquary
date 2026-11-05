use super::*;
use reliquary_container::{Container, ContainerIdentity};
use reliquary_memory_core::{MemoryDraft, MemoryStore};
use reliquary_owner_session::{GraphAdmission, PhySession};
use reliquary_vector_store::{CompatibilityProfileStore, MemoryVectorStore, PackedVectorStore};
use std::{
    fs,
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
            "maintenance-workflow-{}-{}.phy",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        )))
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

fn memory(mutation: &str) -> MemoryDraft {
    MemoryDraft {
        title: "old title".into(),
        content: "old content".into(),
        category: "fact".into(),
        memory_type: "project".into(),
        authority_kind: "direct".into(),
        temporal_status: "past".into(),
        scope: "project".into(),
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
fn terminal_prune_candidate_is_read_only_and_requires_reference_audit_and_copy_reopen() {
    let path = Temp::new();
    let owner_id = [74; 16];
    let mut container =
        Container::create(&path.0, ContainerIdentity::phylactery(), owner_id).unwrap();
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
    let live = memories
        .publish(&mut container, None, 0, memory("maint-live"))
        .unwrap()
        .revision
        .record
        .id;
    let deleted = memories
        .publish(&mut container, None, 0, memory("maint-deleted"))
        .unwrap()
        .revision
        .record
        .id;
    memories.tombstone(&mut container, deleted, 1).unwrap();
    container.sync().unwrap();
    drop(memories);
    drop(container);

    let admission = GraphAdmission::for_replay(owner_id, [], [], [], [], []);
    let session = PhySession::open_path(&path.0, &admission, []).unwrap();
    let owner = EntityOwnerSession::Phy(&session);
    assert!(matches!(
        prepare_tombstoned_memory_maintenance(&owner, owner_id, live).unwrap(),
        MemoryMaintenancePreparation::Blocked {
            reason: MaintenanceBlocker::NotTerminallyTombstoned,
            ..
        }
    ));
    let MemoryMaintenancePreparation::Candidate(candidate) =
        prepare_tombstoned_memory_maintenance(&owner, owner_id, deleted).unwrap()
    else {
        panic!("expected candidate")
    };
    assert_eq!(candidate.candidate_payloads.len(), 1);
    assert!(candidate.requires_cross_domain_reference_audit);
    assert!(candidate.requires_verified_copy_and_reopen);
    assert!(recheck_tombstoned_memory_maintenance(&owner, owner_id, &candidate).unwrap());
    assert!(!recheck_tombstoned_memory_maintenance(&owner, [75; 16], &candidate).unwrap());
    assert_eq!(
        session
            .memory_history(deleted)
            .unwrap()
            .unwrap()
            .revisions
            .len(),
        1
    );
    assert!(matches!(
        session.memory_history(deleted).unwrap().unwrap().revisions[0].payload,
        PayloadState::Available(_)
    ));
    let backup = Temp::new();
    let original = fs::read(&path.0).unwrap();
    let plan = VerifiedOwnerCopyPlan::prepare(&path.0).unwrap();
    assert!(matches!(
        promote_verified_memory_copy(&plan, &path.0, &backup.0, [75; 16], &[deleted], &admission),
        Err(MaintenanceWorkflowError::OwnerRead(message))
            if message == "maintenance source is outside the authorized owner"
    ));
    assert!(!backup.0.exists());
    assert_eq!(fs::read(&path.0).unwrap(), original);
}
