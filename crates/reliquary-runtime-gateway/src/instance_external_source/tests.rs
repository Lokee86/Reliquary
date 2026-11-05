use super::*;
use reliquary_container::ReliquaryScope;
use reliquary_owner_session::records::external_index::ExternalMessageRef;
use sha2::{Digest, Sha256};
use std::sync::atomic::AtomicUsize;

struct FixtureHost {
    calls: AtomicUsize,
    text: &'static str,
    descriptor: ExternalMessageDescriptor,
}

impl ExternalHostSource for FixtureHost {
    fn namespace(&self) -> &str {
        "fixture-host"
    }
    fn contract_version(&self) -> u32 {
        EXTERNAL_SOURCE_CONTRACT
    }
    fn authorize(&self, principal: &str) -> Result<(), ExternalSourceError> {
        if principal == "alice" {
            Ok(())
        } else {
            Err(ExternalSourceError::Unauthorized)
        }
    }
    fn snapshot_manifest(
        &self,
        principal: &str,
    ) -> Result<ExternalReferenceSnapshotManifest, ExternalSourceError> {
        self.authorize(principal)?;
        Ok(ExternalReferenceSnapshotManifest {
            namespace: self.namespace().into(),
            contract_version: EXTERNAL_SOURCE_CONTRACT,
            watermark: 10,
            descriptors: vec![self.descriptor.clone()],
        })
    }
    fn changes_after(
        &self,
        principal: &str,
        cursor: u64,
        limit: usize,
    ) -> Result<ExternalChangeBatch, ExternalSourceError> {
        self.authorize(principal)?;
        assert_eq!(cursor, 10);
        assert_eq!(limit, MAX_EXTERNAL_BATCH);
        Ok(ExternalChangeBatch {
            feed_floor: 1,
            feed_watermark: 10,
            changes: vec![],
        })
    }

    fn hydrate(
        &self,
        principal: &str,
        expected: &[ExternalMessageDescriptor],
    ) -> Result<Vec<HydratedExternalMessage>, ExternalSourceError> {
        self.authorize(principal)?;
        self.calls.fetch_add(1, Ordering::Relaxed);
        Ok(expected
            .iter()
            .cloned()
            .map(|descriptor| HydratedExternalMessage {
                descriptor,
                content: self.text.to_owned(),
            })
            .collect())
    }
}

#[test]
fn host_hydration_requires_both_grants_and_preserves_native_archive() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join(".test-tmp");
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join(format!("external-{}.rel", Uuid::new_v4()));
    let owner_id = [97; 16];
    drop(RelWriter::create_path(&path, ReliquaryScope::Project, owner_id).unwrap());
    let admission = GraphAdmission::for_replay(owner_id, [], [], [], [], []);
    let gateway = Gateway::new();
    let owner = gateway
        .mount_writable_reliquary(&path, &admission, [], [])
        .unwrap();
    let alice = gateway.instance("alice", [owner], []).unwrap();
    let bob = gateway.instance("bob", [owner], []).unwrap();
    let ungranted = gateway.instance("alice", [], []).unwrap();
    let text = "Host-only evidence 🦀.";
    let descriptor = ExternalMessageDescriptor {
        reference: ExternalMessageRef {
            source_namespace: "fixture-host".into(),
            conversation_id: "external-conversation".into(),
            message_id: "external-message".into(),
            content_hash: Sha256::digest(text.as_bytes()).into(),
        },
        role: "user".into(),
        principal_id: Some("alice".into()),
        timestamp_ns: 10,
        active: true,
    };
    let host = FixtureHost {
        calls: AtomicUsize::new(0),
        text,
        descriptor: descriptor.clone(),
    };
    let expected = std::slice::from_ref(&descriptor);
    assert!(matches!(
        gateway.hydrate_external_source(&ungranted, owner, &host, expected),
        Err(GatewayError::UnauthorizedRel { .. })
    ));
    assert!(
        gateway
            .hydrate_external_source(&bob, owner, &host, expected)
            .is_err()
    );
    assert_eq!(host.calls.load(Ordering::Relaxed), 0);
    assert_eq!(
        gateway
            .hydrate_external_source(&alice, owner, &host, expected)
            .unwrap()[0]
            .content,
        text
    );
    assert_eq!(
        gateway
            .external_source_manifest(&alice, owner, &host)
            .unwrap()
            .watermark,
        10
    );
    assert!(
        gateway
            .external_source_manifest(&bob, owner, &host)
            .is_err()
    );
    let receipt = gateway
        .record_external_source_manifest(&alice, owner, &host)
        .unwrap();
    let retry = gateway
        .record_external_source_manifest(&alice, owner, &host)
        .unwrap();
    assert_eq!(receipt, retry);
    assert_eq!(
        gateway
            .advance_external_source_manifest(&alice, owner, &host)
            .unwrap(),
        receipt
    );
    let index = gateway
        .rebuild_external_lexical_index(&alice, owner, &host)
        .unwrap();
    assert!(
        gateway
            .search_external_lexical_index(&bob, &host, &index, "evidence", 1)
            .is_err()
    );
    let hits = gateway
        .search_external_lexical_index(&alice, &host, &index, "evidence", 1)
        .unwrap();
    assert_eq!(hits[0].message, descriptor.reference);
    assert_eq!(hits[0].end, text.len() as u64);
    let chunk = gateway
        .hydrate_external_chunk(&alice, owner, &host, &descriptor, hits[0].clone())
        .unwrap();
    assert_eq!(chunk.content, text);
    let changed = FixtureHost {
        calls: AtomicUsize::new(0),
        text: "changed body",
        descriptor: descriptor.clone(),
    };
    assert!(
        gateway
            .hydrate_external_source(&alice, owner, &changed, expected)
            .is_err()
    );
    drop((alice, bob, ungranted, gateway));
    let rel = RelSession::open_path(&path, &admission, [], []).unwrap();
    assert!(
        rel.archive_node("external-conversation", "external-message")
            .is_none()
    );
    assert_eq!(
        rel.external_reference_snapshot("fixture-host", "alice"),
        Some(&receipt)
    );
    assert!(
        rel.external_reference_snapshot("fixture-host", "bob")
            .is_none()
    );
    drop(rel);
    let bytes = std::fs::read(&path).unwrap();
    assert!(
        !bytes
            .windows(text.len())
            .any(|candidate| candidate == text.as_bytes())
    );
    let reopened = Gateway::new();
    let owner = reopened.mount_reliquary(&path, &admission, [], []).unwrap();
    let alice = reopened.instance("alice", [owner], []).unwrap();
    let index = reopened
        .rebuild_external_lexical_index(&alice, owner, &host)
        .unwrap();
    assert_eq!(
        reopened
            .search_external_lexical_index(&alice, &host, &index, "evidence", 1)
            .unwrap(),
        hits,
    );
    drop((alice, reopened));
    std::fs::remove_file(path).unwrap();
}
