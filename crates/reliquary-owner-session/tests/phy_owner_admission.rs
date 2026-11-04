use reliquary_container::{Container, ContainerIdentity};
use reliquary_owner_session::{GraphAdmission, OwnerSessionError, PhySession};
use reliquary_vector_store::{CompatibilityProfileStore, MemoryVectorStore, PackedVectorStore};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(1);

struct Temp(PathBuf);

impl Temp {
    fn new() -> Self {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".owner-session-test-tmp");
        fs::create_dir_all(&root).unwrap();
        Self(root.join(format!(
            "phy-rel-record-{}-{}.phy",
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

fn external_memory_record() -> Vec<u8> {
    let mut payload = b"RELXMEM2".to_vec();
    payload.extend_from_slice(&0_u64.to_le_bytes()); // body length
    payload.extend_from_slice(&0_u64.to_le_bytes()); // first global version
    payload.extend_from_slice(&0_u32.to_le_bytes()); // zero routed Memories
    payload.extend_from_slice(&0_i64.to_le_bytes()); // transaction time
    let digest = Sha256::digest(&payload);
    payload.extend_from_slice(&digest);
    payload
}

#[test]
fn phy_owner_rejects_rel_external_memory_record_without_mutation() {
    let path = Temp::new();
    let owner = [41; 16];
    let mut container = Container::create(&path.0, ContainerIdentity::phylactery(), owner).unwrap();
    PackedVectorStore::default()
        .initialize(&mut container)
        .unwrap();
    CompatibilityProfileStore::default()
        .initialize(&mut container)
        .unwrap();
    MemoryVectorStore::default()
        .initialize(&mut container)
        .unwrap();
    container.append(&external_memory_record()).unwrap();
    container.sync().unwrap();
    drop(container);

    let before = fs::read(&path.0).unwrap();
    let admission = GraphAdmission::for_replay(owner, [], [], [], [], []);
    let error = match PhySession::open_path(&path.0, &admission, []) {
        Err(error) => error,
        Ok(_) => panic!("PhySession accepted a REL-only external Memory record"),
    };
    assert!(
        matches!(
            error,
            OwnerSessionError::RelOnlyRecordInPhylactery("external Memory reference")
        ),
        "unexpected Phy open error: {error:?}"
    );
    assert_eq!(fs::read(&path.0).unwrap(), before);
}

#[test]
fn phy_owner_rejects_future_rel_grouped_completion_without_mutation() {
    let path = Temp::new();
    let owner = [42; 16];
    let mut container = Container::create(&path.0, ContainerIdentity::phylactery(), owner).unwrap();
    PackedVectorStore::default()
        .initialize(&mut container)
        .unwrap();
    CompatibilityProfileStore::default()
        .initialize(&mut container)
        .unwrap();
    MemoryVectorStore::default()
        .initialize(&mut container)
        .unwrap();
    // Shape 1 is a valid RELINSC8 grouped-clock record without an embedded range.
    container.append(b"RELINSC8\x01").unwrap();
    container.sync().unwrap();
    drop(container);

    let before = fs::read(&path.0).unwrap();
    let admission = GraphAdmission::for_replay(owner, [], [], [], [], []);
    let error = match PhySession::open_path(&path.0, &admission, []) {
        Err(error) => error,
        Ok(_) => panic!("PhySession accepted a REL-only grouped completion"),
    };
    assert!(
        matches!(
            error,
            OwnerSessionError::RelOnlyRecordInPhylactery("grouped Insomnia completion v8")
        ),
        "unexpected Phy open error: {error:?}"
    );
    assert_eq!(fs::read(&path.0).unwrap(), before);
}

#[test]
fn phy_owner_rejects_unsupported_rel_freshness_versions_without_mutation() {
    for marker in [b"RELFRS07", b"RELFRS99"] {
        let path = Temp::new();
        let owner = [43; 16];
        let mut container =
            Container::create(&path.0, ContainerIdentity::phylactery(), owner).unwrap();
        PackedVectorStore::default()
            .initialize(&mut container)
            .unwrap();
        CompatibilityProfileStore::default()
            .initialize(&mut container)
            .unwrap();
        MemoryVectorStore::default()
            .initialize(&mut container)
            .unwrap();
        container.append(marker).unwrap();
        container.sync().unwrap();
        drop(container);

        let before = fs::read(&path.0).unwrap();
        let admission = GraphAdmission::for_replay(owner, [], [], [], [], []);
        let error = match PhySession::open_path(&path.0, &admission, []) {
            Err(error) => error,
            Ok(_) => panic!("PhySession accepted unsupported REL Freshness record {marker:?}"),
        };
        assert!(
            matches!(
                error,
                OwnerSessionError::RelOnlyRecordInPhylactery("Freshness record")
            ),
            "unexpected Phy open error: {error:?}"
        );
        assert_eq!(fs::read(&path.0).unwrap(), before);
    }
}
