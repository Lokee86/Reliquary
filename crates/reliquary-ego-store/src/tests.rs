use crate::{
    EgoAnchorId, EgoAnchorPriority, EgoError, EgoIdentityId, EgoLifecycleState, EgoOwnerKind,
    EgoStore,
};
use reliquary_container::{Container, ContainerIdentity, FileKind, ReliquaryScope};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_FILE: AtomicU64 = AtomicU64::new(1);
static NEXT_OWNER: AtomicU64 = AtomicU64::new(1);

struct TempFile(PathBuf);

impl TempFile {
    fn new() -> Self {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".ego-test-tmp");
        fs::create_dir_all(&dir).unwrap();
        Self(dir.join(format!(
            "reliquary-ego-{}-{}.rel",
            std::process::id(),
            NEXT_FILE.fetch_add(1, Ordering::Relaxed)
        )))
    }

    fn path(&self) -> &PathBuf {
        &self.0
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
        if let Some(parent) = self.0.parent() {
            let _ = fs::remove_dir(parent);
        }
    }
}

fn identity(byte: u8) -> EgoIdentityId {
    EgoIdentityId([byte; 16])
}

fn anchor(byte: u8) -> EgoAnchorId {
    EgoAnchorId([byte; 16])
}

fn create(kind: FileKind, path: &PathBuf) -> Container {
    let identity = match kind {
        FileKind::Reliquary => ContainerIdentity::reliquary(ReliquaryScope::Project),
        FileKind::Phylactery => ContainerIdentity::phylactery(),
    };
    let mut owner_id = [0; 16];
    owner_id[..8].copy_from_slice(&NEXT_OWNER.fetch_add(1, Ordering::Relaxed).to_le_bytes());
    owner_id[8] = kind as u8 + 1;
    Container::create(path, identity, owner_id).unwrap()
}

#[test]
fn phy_private_state_and_revision_history_survive_reopen() {
    let path = TempFile::new();
    let id = identity(7);
    let anchor_id = anchor(8);
    {
        let mut container = create(FileKind::Phylactery, path.path());
        let mut store = EgoStore::open(&container).unwrap();
        store
            .create_identity(
                &mut container,
                id,
                "Default".into(),
                "first identity".into(),
            )
            .unwrap();
        store
            .update_identity(
                &mut container,
                id,
                1,
                "Default".into(),
                "revised identity".into(),
            )
            .unwrap();
        store
            .put_personality(&mut container, 0, "careful and direct".into(), 3)
            .unwrap();
        store
            .put_personality(&mut container, 1, "careful, direct, and patient".into(), 4)
            .unwrap();
        store
            .put_synthesis(&mut container, 0, 4, "supplied synthesis text".into())
            .unwrap();
        store
            .create_anchor(
                &mut container,
                anchor_id,
                EgoAnchorPriority::High,
                "Preserve privacy".into(),
            )
            .unwrap();
    }
    {
        let mut container = Container::open_writer(path.path()).unwrap();
        let mut store = EgoStore::open(&container).unwrap();
        assert_eq!(store.owner_kind(), EgoOwnerKind::Phylactery);
        assert_eq!(store.version(), 6);
        assert_eq!(store.identity(id).unwrap().revision, 2);
        assert_eq!(store.identity(id).unwrap().text, "revised identity");
        assert_eq!(
            store.identity_revision(id, 1).unwrap().text,
            "first identity"
        );
        assert_eq!(
            store.identity_revision(id, 2).unwrap().text,
            "revised identity"
        );
        assert_eq!(store.personality().unwrap().revision, 2);
        assert_eq!(store.personality().unwrap().source_memory_version, 4);
        assert_eq!(
            store.personality_revision(1).unwrap().text,
            "careful and direct"
        );
        assert_eq!(store.synthesis().unwrap().source_memory_version, 4);
        assert_eq!(store.synthesis().unwrap().text, "supplied synthesis text");
        assert_eq!(
            store.synthesis_revision(1).unwrap().text,
            "supplied synthesis text"
        );
        assert_eq!(
            store.anchor(anchor_id).unwrap().priority,
            EgoAnchorPriority::High
        );
        assert_eq!(
            store.anchor_revision(anchor_id, 1).unwrap().text,
            "Preserve privacy"
        );
        store.delete_identity(&mut container, id, 2).unwrap();
        assert!(store.identity(id).is_none());
        assert!(matches!(
            store.identity_lifecycle(id),
            Some(EgoLifecycleState::Tombstoned { .. })
        ));
    }
}

#[test]
fn ego_history_projects_pre_update_and_post_tombstone_cuts() {
    let path = TempFile::new();
    let id = identity(0x31);
    let mut container = create(FileKind::Phylactery, path.path());
    let mut store = EgoStore::open(&container).unwrap();
    store
        .create_identity(&mut container, id, "private".into(), "first".into())
        .unwrap();
    store
        .update_identity(&mut container, id, 1, "private".into(), "second".into())
        .unwrap();
    store.delete_identity(&mut container, id, 2).unwrap();

    assert_eq!(store.history_at(1).unwrap().identities[0].text, "first");
    assert_eq!(store.history_at(2).unwrap().identities[0].text, "second");
    let deleted = store.history_at(3).unwrap();
    assert!(deleted.identities.is_empty());
    assert_eq!(deleted.tombstoned_identities, vec![id]);
    assert!(store.history_at(4).is_err());
}

#[test]
fn rel_rejects_private_identity_and_personality_writes() {
    let path = TempFile::new();
    let mut container = create(FileKind::Reliquary, path.path());
    let mut store = EgoStore::open(&container).unwrap();
    assert!(matches!(
        store.create_identity(&mut container, identity(2), "private".into(), "fact".into()),
        Err(EgoError::PrivateRecordInReliquary("Identity"))
    ));
    assert!(matches!(
        store.put_personality(&mut container, 0, "private facts".into(), 0),
        Err(EgoError::PrivateRecordInReliquary("Personality"))
    ));
    assert!(container.read_frames().unwrap().is_empty());
}

#[test]
fn identical_anchor_update_does_not_append_a_new_revision() {
    let path = TempFile::new();
    let mut container = create(FileKind::Reliquary, path.path());
    let mut store = EgoStore::open(&container).unwrap();
    let id = anchor(21);
    let original = store
        .create_anchor(
            &mut container,
            id,
            EgoAnchorPriority::Normal,
            "Keep the source grounded".into(),
        )
        .unwrap();
    let frame_count = container.read_frames().unwrap().len();

    let retry = store
        .update_anchor(
            &mut container,
            id,
            original.revision,
            original.priority,
            original.text.clone(),
        )
        .unwrap();

    assert_eq!(retry, original);
    assert_eq!(store.anchor_revision(id, 2), None);
    assert_eq!(container.read_frames().unwrap().len(), frame_count);
    let reopened = EgoStore::open(&container).unwrap();
    assert_eq!(reopened.anchor(id), Some(&original));
    assert_eq!(reopened.anchor_revision(id, 2), None);
}

#[test]
fn rel_replay_rejects_private_phy_records() {
    let phy_path = TempFile::new();
    let mut phy = create(FileKind::Phylactery, phy_path.path());
    let mut phy_store = EgoStore::open(&phy).unwrap();
    phy_store
        .create_identity(
            &mut phy,
            identity(4),
            "private identity".into(),
            "sensitive text".into(),
        )
        .unwrap();
    let private_record = phy.read_frames().unwrap().remove(0).payload;

    let rel_path = TempFile::new();
    let mut rel = create(FileKind::Reliquary, rel_path.path());
    rel.append(&private_record).unwrap();
    rel.sync().unwrap();
    drop(rel);
    let rel = Container::open_read_only(rel_path.path()).unwrap();
    assert!(matches!(
        EgoStore::open(&rel),
        Err(EgoError::PrivateRecordInReliquary("Identity"))
    ));
}

#[test]
fn owner_kind_and_identity_mismatches_fail_before_append() {
    let first_path = TempFile::new();
    let mut first = create(FileKind::Phylactery, first_path.path());
    let mut store = EgoStore::open(&first).unwrap();
    store
        .create_identity(&mut first, identity(1), "name".into(), "body".into())
        .unwrap();

    let rel_path = TempFile::new();
    let mut rel = create(FileKind::Reliquary, rel_path.path());
    assert!(matches!(
        store.create_anchor(
            &mut rel,
            anchor(1),
            EgoAnchorPriority::Normal,
            "anchor".into()
        ),
        Err(EgoError::WrongOwnerKind)
    ));
    let other_phy_path = TempFile::new();
    let mut other_phy = create(FileKind::Phylactery, other_phy_path.path());
    assert!(matches!(
        store.create_anchor(
            &mut other_phy,
            anchor(1),
            EgoAnchorPriority::Normal,
            "anchor".into()
        ),
        Err(EgoError::OwnerIdentityMismatch)
    ));
}

#[test]
fn fresh_ego_store_replays_before_claiming_existing_identity() {
    let path = TempFile::new();
    let mut container = create(FileKind::Phylactery, path.path());
    let mut first = EgoStore::open(&container).unwrap();
    first
        .create_identity(
            &mut container,
            identity(9),
            "name".into(),
            "original".into(),
        )
        .unwrap();
    let before = container.read_frames().unwrap().len();
    let mut fresh = EgoStore::empty_for(FileKind::Phylactery);
    assert!(matches!(
        fresh.create_identity(
            &mut container,
            identity(9),
            "name".into(),
            "conflict".into()
        ),
        Err(EgoError::InvalidRecord(_))
    ));
    assert_eq!(container.read_frames().unwrap().len(), before);
    assert_eq!(
        EgoStore::open(&container)
            .unwrap()
            .identity(identity(9))
            .unwrap()
            .text,
        "original"
    );
}

#[test]
fn ego_rejects_same_owner_id_with_different_rel_scope() {
    let first_path = TempFile::new();
    let second_path = TempFile::new();
    let first = create(FileKind::Reliquary, first_path.path());
    let mut other = Container::create(
        second_path.path(),
        ContainerIdentity::reliquary(ReliquaryScope::Organization),
        first.owner_id(),
    )
    .unwrap();
    let mut store = EgoStore::open(&first).unwrap();
    assert!(matches!(
        store.create_anchor(
            &mut other,
            anchor(1),
            EgoAnchorPriority::Normal,
            "anchor".into()
        ),
        Err(EgoError::OwnerIdentityMismatch)
    ));
    assert!(other.read_frames().unwrap().is_empty());
}
