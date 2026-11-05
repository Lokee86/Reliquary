use super::*;
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
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".test-tmp");
        fs::create_dir_all(&root).unwrap();
        Self(root.join(format!(
            "external-reference-{}-{}.rel",
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

fn manifest(message: &str, principal: &str, watermark: u64) -> ExternalReferenceSnapshotManifest {
    ExternalReferenceSnapshotManifest {
        namespace: "fixture-host".into(),
        contract_version: EXTERNAL_SOURCE_CONTRACT,
        watermark,
        descriptors: vec![ExternalMessageDescriptor {
            reference: super::super::external_index::ExternalMessageRef {
                source_namespace: "fixture-host".into(),
                conversation_id: "host-conversation".into(),
                message_id: message.into(),
                content_hash: [message.as_bytes()[0]; 32],
            },
            role: "user".into(),
            principal_id: Some(principal.into()),
            timestamp_ns: 23,
            active: true,
        }],
    }
}

#[test]
fn external_reference_receipts_are_durable_per_principal_without_archive_authority() {
    let path = Temp::new();
    let owner_id = [31; 16];
    let mut writer =
        crate::RelWriter::create_path(&path.0, ReliquaryScope::Project, owner_id).unwrap();
    let alice_manifest = manifest("alice-message", "alice", 1);
    let alice = writer
        .record_external_reference_snapshot("alice", alice_manifest.clone())
        .unwrap();
    assert_eq!(alice.owner_id(), owner_id);
    assert_eq!(alice.authorizing_principal(), "alice");
    assert_eq!(alice.manifest(), &alice_manifest);
    assert_eq!(
        writer
            .read_session()
            .external_reference_snapshot("fixture-host", "alice"),
        Some(&alice)
    );
    assert_eq!(
        writer
            .record_external_reference_snapshot("alice", alice_manifest.clone())
            .unwrap(),
        alice
    );

    let bob_manifest = manifest("bob-message", "bob", 1);
    let bob = writer
        .record_external_reference_snapshot("bob", bob_manifest.clone())
        .unwrap();
    assert_eq!(
        writer
            .read_session()
            .external_reference_snapshot("fixture-host", "bob"),
        Some(&bob)
    );
    assert_ne!(alice.digest(), bob.digest());
    assert!(
        writer
            .record_external_reference_snapshot("alice", manifest("changed", "alice", 1))
            .is_err()
    );
    assert!(
        writer
            .record_external_reference_snapshot("alice", manifest("older", "alice", 0))
            .is_err()
    );

    // A host descriptor is reference-only evidence; it creates no native turn.
    assert_eq!(writer.read_session().rel_turn_count(), 0);
    assert!(
        writer
            .read_session()
            .archive_node("host-conversation", "alice-message")
            .is_none()
    );
    drop(writer);

    let reopened = crate::RelSession::open_path(
        &path.0,
        &crate::GraphAdmission::for_replay(owner_id, [], [], [], [], []),
        [],
        [],
    )
    .unwrap();
    assert_eq!(
        reopened.external_reference_snapshot("fixture-host", "alice"),
        Some(&alice)
    );
    assert_eq!(
        reopened.external_reference_snapshot("fixture-host", "bob"),
        Some(&bob)
    );
    assert!(
        reopened
            .external_reference_snapshot("fixture-host", "mallory")
            .is_none()
    );
    assert_eq!(reopened.rel_turn_count(), 0);
}

#[test]
fn external_reference_receipt_replay_rejects_wrong_owner_and_corrupt_digest() {
    let receipt = make_receipt([41; 16], "alice", manifest("message", "alice", 4)).unwrap();
    let mut bytes = encode(&receipt).unwrap();
    let wrong_owner_path = Temp::new();
    let mut wrong_owner = Container::create(
        &wrong_owner_path.0,
        ContainerIdentity::reliquary(ReliquaryScope::Project),
        [42; 16],
    )
    .unwrap();
    wrong_owner.append(&bytes).unwrap();
    wrong_owner.sync().unwrap();
    drop(wrong_owner);
    let wrong_owner = Container::open_read_only(&wrong_owner_path.0).unwrap();
    assert!(ExternalReferenceStore::open(&wrong_owner).is_err());

    let last = bytes.len() - 1;
    bytes[last] ^= 1;
    let corrupt_path = Temp::new();
    let mut corrupt = Container::create(
        &corrupt_path.0,
        ContainerIdentity::reliquary(ReliquaryScope::Project),
        [41; 16],
    )
    .unwrap();
    corrupt.append(&bytes).unwrap();
    corrupt.sync().unwrap();
    drop(corrupt);
    let corrupt = Container::open_read_only(&corrupt_path.0).unwrap();
    assert!(ExternalReferenceStore::open(&corrupt).is_err());
}
