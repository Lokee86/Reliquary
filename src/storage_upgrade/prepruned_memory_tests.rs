// Preexisting pruning proof must survive conversion or fail before staging returns.
// Temporary offline migration regression; no legacy runtime-support promise.
use super::{
    StorageUpgradeError, StorageUpgradeRequest, plan_storage_upgrade, stage_memory_storage_upgrade,
};
use crate::owner_lifecycle::LifecycleTarget;
use crate::{HistoryPruneRequest, MemoryDraft, Phylactery};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(".test-tmp")
            .join(format!("luna-prepruned-memory-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn draft() -> MemoryDraft {
    MemoryDraft {
        category: "fact".into(),
        memory_type: "project".into(),
        authority_kind: "direct".into(),
        temporal_status: "current".into(),
        title: "pre-prune legacy carrier".into(),
        content: "unique removable body ".repeat(2048),
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
        mutation_id: "pre-prune-legacy-carrier".into(),
        created_at_ns: 10,
        updated_at_ns: 10,
    }
}

fn downgrade_header_to_v1(path: &Path) {
    let mut bytes = fs::read(path).unwrap();
    bytes[8..10].copy_from_slice(&1_u16.to_le_bytes());
    bytes[10..12].copy_from_slice(&0_u16.to_le_bytes());
    fs::write(path, bytes).unwrap();
}

#[test]
fn prepruned_legacy_memory_upgrade_rejects_untranslated_history_proof_without_writes() {
    let scratch = Scratch::new();
    let source = scratch.0.join("pruned-source.phy");
    let destination = scratch.0.join("staged-output.phy");

    // Build a genuine V2 owner whose Memory publication carrier is legacy V1
    // data, then create the prune manifest through the normal owner API.
    let mut phy = Phylactery::create(&source).unwrap();
    let (memory, changed) = phy
        .memories
        .publish_legacy_fixture(&mut phy.container, None, 0, draft())
        .unwrap();
    assert!(changed);
    phy.tombstone_memory(memory.id, memory.revision).unwrap();
    let retained = phy.memory_retention_descriptor(memory.id).unwrap();
    let pruned = scratch.0.join("pruned-v2.phy");
    phy.prune_tombstoned_history(
        &pruned,
        HistoryPruneRequest {
            targets: vec![LifecycleTarget::Memory(memory.id)],
        },
    )
    .unwrap();
    drop(phy);

    // The normal V2 owner reopen validates the actual manifest and descriptor.
    let reopened = Phylactery::open(&pruned).unwrap();
    assert_eq!(
        reopened.memory_retention_descriptor(memory.id).unwrap(),
        retained
    );
    drop(reopened);

    // Header-only downgrade is valid here because prune copied legacy Memory
    // carriers; it did not rewrite them as compact creations/deltas.
    downgrade_header_to_v1(&pruned);
    let source_bytes = fs::read(&pruned).unwrap();
    assert!(Phylactery::open_v1_read_only(&pruned).is_ok());
    let source_hash = Sha256::digest(&source_bytes);
    let plan = plan_storage_upgrade(StorageUpgradeRequest {
        source: pruned.clone(),
        destination: destination.clone(),
    })
    .unwrap();

    assert!(matches!(
        stage_memory_storage_upgrade(&plan),
        Err(StorageUpgradeError::UnsupportedAuthority("CVAHPR01"))
    ));
    assert_eq!(Sha256::digest(fs::read(&pruned).unwrap()), source_hash);
    assert_eq!(fs::read(&pruned).unwrap(), source_bytes);
    assert!(!destination.exists());
    assert!(fs::read_dir(&scratch.0).unwrap().all(|entry| {
        !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".storage-upgrade-")
    }));
    let unchanged = Phylactery::open_v1_read_only(&pruned).unwrap();
    assert_eq!(
        unchanged.memory_retention_descriptor(memory.id).unwrap(),
        retained
    );
}
