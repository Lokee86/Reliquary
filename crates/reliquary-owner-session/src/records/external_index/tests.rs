use super::*;
use reliquary_archive::{Archive, TurnInput};
use reliquary_container::{Container, ContainerIdentity, ReliquaryScope};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(1);
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".owner-evidence-test-tmp");
        fs::create_dir_all(&root).unwrap();
        Self(root.join(format!(
            "external-index-{}-{}.rel",
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
fn container(path: &PathBuf, owner: u8) -> Container {
    let mut id = [0; 16];
    id[0] = owner;
    Container::create(
        path,
        ContainerIdentity::reliquary(ReliquaryScope::Project),
        id,
    )
    .unwrap()
}
fn state(owner_archive: &mut Archive, container: &mut Container) -> SourceState {
    let accepted = owner_archive
        .accept_turn(
            container,
            TurnInput {
                id: "message-1".into(),
                conversation_id: "conversation-1".into(),
                parent_id: None,
                role: "user".into(),
                principal_id: Some("principal-1".into()),
                timestamp_ns: 42,
                content: "canonical external message".into(),
                attachments: vec![],
                project_attachments: vec![],
            },
        )
        .unwrap();
    let node = accepted.value;
    SourceState {
        namespace: "connector/account".into(),
        contract: 1,
        profile: [1; 32],
        dimensions: 2,
        generation: 1,
        revision: 1,
        cursor: 1,
        messages: vec![IndexedMessage {
            descriptor: ExternalMessageDescriptor {
                reference: ExternalMessageRef {
                    source_namespace: "connector/account".into(),
                    conversation_id: node.conversation_id,
                    message_id: node.id,
                    content_hash: node.content_id.0,
                },
                role: node.role,
                principal_id: node.principal_id,
                timestamp_ns: node.timestamp_ns,
                active: true,
            },
            chunks: vec![],
        }],
        last_base: 0,
        last_digest: [2; 32],
        rebuild: false,
    }
}

#[test]
fn admitted_index_is_owner_bound_replayable_and_only_matches_archive_sources() {
    let path = Temp::new();
    let snapshot;
    {
        let mut c = container(&path.0, 1);
        let mut archive = Archive::empty();
        let valid = state(&mut archive, &mut c);
        snapshot = valid.clone();
        let mut index = ExternalIndexStore::open(&c).unwrap();
        let mut forged = valid.clone();
        forged.messages[0].descriptor.reference.content_hash = [9; 32];
        assert_eq!(
            index.install_admitted(&mut c, &archive, forged, None),
            Err(ExternalSourceError::Unauthorized)
        );
        assert!(c.read_frames().unwrap().len() == 1); // only Archive's source turn exists
        index
            .install_admitted(&mut c, &archive, valid, None)
            .unwrap();
        assert_eq!(
            c.read_frames().unwrap().last().unwrap().payload.get(..8),
            Some(b"RELXIDX2".as_slice())
        );

        let mut out_of_bounds = snapshot.clone();
        let out_of_bounds_ref = out_of_bounds.messages[0].descriptor.reference.clone();
        out_of_bounds.messages[0].chunks.push(IndexedChunk {
            reference: ExternalChunkRef {
                message: out_of_bounds_ref,
                start: 0,
                end: 65,
            },
            matrix: [3; 32],
            row: 0,
            terms: vec![],
        });
        assert_eq!(
            index.install_admitted(&mut c, &archive, out_of_bounds, Some(1)),
            Err(ExternalSourceError::InvalidContract)
        );

        let foreign_path = Temp::new();
        let mut foreign_container = container(&foreign_path.0, 9);
        let mut foreign_archive = Archive::empty();
        let foreign_state = state(&mut foreign_archive, &mut foreign_container);
        assert_eq!(
            index.install_admitted(&mut c, &foreign_archive, foreign_state, Some(1)),
            Err(ExternalSourceError::Unauthorized)
        );
    }
    let c = Container::open_read_only(&path.0).unwrap();
    let index = ExternalIndexStore::open(&c).unwrap();
    assert_eq!(index.sources.get("connector/account"), Some(&snapshot));

    let owner_one = Temp::new();
    let owner_two = Temp::new();
    let owner_one_container = container(&owner_one.0, 3);
    let mut owner_two_container = container(&owner_two.0, 4);
    let mut owner_one_index = ExternalIndexStore::open(&owner_one_container).unwrap();
    let mut owner_two_archive = Archive::empty();
    let owner_two_state = state(&mut owner_two_archive, &mut owner_two_container);
    assert_eq!(
        owner_one_index.install_admitted(
            &mut owner_two_container,
            &owner_two_archive,
            owner_two_state,
            None
        ),
        Err(ExternalSourceError::Unauthorized)
    );
}

#[test]
fn legacy_marker_is_ignored_and_does_not_establish_index_authority() {
    let mut store = ExternalIndexStore::default();
    store.ingest(&[
        0x43, 0x56, 0x41, 0x58, 0x49, 0x44, 0x58, 0x32, b'o', b'p', b'a', b'q', b'u', b'e',
    ]);
    assert!(!store.damaged);
    assert!(store.sources.is_empty());
}

#[test]
fn discard_is_persisted_as_derived_state_invalidation() {
    let path = Temp::new();
    {
        let mut c = container(&path.0, 7);
        let mut archive = Archive::empty();
        let source_state = state(&mut archive, &mut c);
        let mut store = ExternalIndexStore::open(&c).unwrap();
        store
            .install_admitted(&mut c, &archive, source_state, None)
            .unwrap();
        assert_eq!(store.sources.len(), 1);
        store.discard_all(&mut c).unwrap();
        assert!(store.sources.is_empty());
    }
    let c = Container::open_read_only(&path.0).unwrap();
    let store = ExternalIndexStore::open(&c).unwrap();
    assert!(store.sources.is_empty());
    assert!(store.damaged);
}
