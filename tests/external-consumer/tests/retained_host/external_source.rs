use super::{FixtureDir, VIEWER, admission};
use reliquary_memory::{
    AuthorizedExternalLexicalIndex, EXTERNAL_SOURCE_CONTRACT, ExternalChangeBatch,
    ExternalChangeKind, ExternalChunkRef, ExternalConversationChange, ExternalHostSource,
    ExternalMemoryProposal, ExternalMemoryProvenance, ExternalMemoryReceipt,
    ExternalMessageDescriptor, ExternalMessageRef, ExternalReferenceSnapshotManifest,
    ExternalReferenceSnapshotReceipt, ExternalSourceError, Gateway, GeneralEndpoint,
    GeneralEndpointError, HydratedExternalChunk, HydratedExternalMessage, InsomniaCandidate,
    InsomniaOwnership, MAX_EXTERNAL_BATCH, ProductErrorKind,
};
use serde_json::{Value, json};
use std::sync::atomic::{AtomicBool, Ordering};

const TEXT: &str = "I prefer concise answers.";
const NAMESPACE: &str = "fixture-host";

struct Source {
    descriptor: ExternalMessageDescriptor,
    revoked: AtomicBool,
}

impl Source {
    fn new() -> Self {
        Self {
            descriptor: ExternalMessageDescriptor {
                reference: ExternalMessageRef {
                    source_namespace: NAMESPACE.into(),
                    conversation_id: "external-chat".into(),
                    message_id: "external-message".into(),
                    content_hash: [
                        0x0c, 0xd6, 0x05, 0x71, 0x36, 0x94, 0x20, 0xa1, 0x68, 0x2a, 0xaf, 0xe1,
                        0xe6, 0x1a, 0xce, 0x17, 0xdb, 0xc9, 0xb7, 0xa0, 0x8a, 0x5d, 0xe7, 0x39,
                        0x4c, 0x1e, 0x63, 0xd5, 0x9b, 0xa5, 0x24, 0x74,
                    ],
                },
                role: "user".into(),
                principal_id: Some("alice".into()),
                timestamp_ns: 10,
                active: true,
            },
            revoked: AtomicBool::new(false),
        }
    }
}

impl ExternalHostSource for Source {
    fn namespace(&self) -> &str {
        NAMESPACE
    }

    fn contract_version(&self) -> u32 {
        EXTERNAL_SOURCE_CONTRACT
    }

    fn authorize(&self, principal: &str) -> Result<(), ExternalSourceError> {
        if principal != "alice" || self.revoked.load(Ordering::Acquire) {
            return Err(ExternalSourceError::Unauthorized);
        }
        Ok(())
    }

    fn snapshot_manifest(
        &self,
        principal: &str,
    ) -> Result<ExternalReferenceSnapshotManifest, ExternalSourceError> {
        self.authorize(principal)?;
        Ok(ExternalReferenceSnapshotManifest {
            namespace: NAMESPACE.into(),
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
        assert_eq!((cursor, limit), (10, MAX_EXTERNAL_BATCH));
        Ok(ExternalChangeBatch {
            feed_floor: 11,
            feed_watermark: 11,
            changes: vec![ExternalConversationChange {
                sequence: 11,
                kind: ExternalChangeKind::DeleteConversation {
                    conversation_id: self.descriptor.reference.conversation_id.clone(),
                },
            }],
        })
    }

    fn hydrate(
        &self,
        principal: &str,
        expected: &[ExternalMessageDescriptor],
    ) -> Result<Vec<HydratedExternalMessage>, ExternalSourceError> {
        self.authorize(principal)?;
        if expected != std::slice::from_ref(&self.descriptor) {
            return Err(ExternalSourceError::StaleEvidence);
        }
        Ok(vec![HydratedExternalMessage {
            descriptor: self.descriptor.clone(),
            content: TEXT.into(),
        }])
    }
}

struct Endpoint(bool);

impl GeneralEndpoint for Endpoint {
    fn model(&self) -> &str {
        assert!(self.0, "publication retry invoked the endpoint");
        "external-fixture"
    }

    fn complete_json(
        &self,
        _: &str,
        _: &str,
        schema: &str,
        _: &Value,
    ) -> Result<Value, GeneralEndpointError> {
        assert!(self.0, "publication retry invoked the endpoint");
        Ok(match schema {
            "insomnia_authority_disposition_ledger" => json!({
                "turns": {"external-message": [{
                    "disposition": "retain", "authority_kind": "direct", "category": "preference",
                    "type": "communication", "lifecycle": "current",
                    "proposition": "The user prefers concise answers.",
                    "authority_source_node_id": "", "grounding_source_node_id": "", "reason": "direct assertion"
                }]},
                "evidence_requests": []
            }),
            "insomnia_memory_wording" => json!({
                "candidates": [{
                    "authority_kind": "direct", "category": "preference", "memory_type": "communication",
                    "temporal_status": "current", "ownership": "project", "title": "Concise answers",
                    "content": "The user prefers concise answers.", "source_node_id": "external-message",
                    "source_quote": TEXT, "authority_source": null, "grounding_source": null, "routing_mentions": []
                }]
            }),
            other => panic!("unexpected model call {other}"),
        })
    }
}

#[test]
fn source_operations_require_dual_authorization_and_publish_one_exact_window() {
    let dir = FixtureDir::new();
    let rel = dir.create_rel();
    let gateway = Gateway::new();
    let owner = gateway.mount_rel(&rel, true).unwrap();
    let (operator, viewer, control) = admission(&gateway, owner);
    let source = Source::new();
    let manifest = gateway
        .external_source_manifest(&viewer, owner, &source)
        .unwrap();
    assert_eq!(manifest.watermark, 10);
    let expected = &manifest.descriptors;
    assert_eq!(
        gateway
            .record_external_source_manifest(&viewer, owner, &source)
            .unwrap_err()
            .kind(),
        ProductErrorKind::Unauthorized
    );
    let recorded: ExternalReferenceSnapshotReceipt = gateway
        .record_external_source_manifest(&operator, owner, &source)
        .unwrap();
    assert_eq!(
        gateway
            .recorded_external_source_manifest(&viewer, owner, &source)
            .unwrap(),
        Some(recorded)
    );

    let index: AuthorizedExternalLexicalIndex = gateway
        .rebuild_external_lexical_index(&viewer, owner, &source)
        .unwrap();
    let hits: Vec<ExternalChunkRef> = gateway
        .search_external_lexical_index(&viewer, &source, &index, "concise", 1)
        .unwrap();
    assert_eq!(hits.len(), 1);
    let chunk: HydratedExternalChunk = gateway
        .hydrate_external_chunk(&viewer, owner, &source, &expected[0], hits[0].clone())
        .unwrap();
    assert_eq!(chunk.content, TEXT);
    assert_eq!(
        gateway
            .search_external_lexical_index(&operator, &source, &index, "concise", 1)
            .unwrap_err()
            .kind(),
        ProductErrorKind::Unauthorized
    );
    let proposals: Vec<ExternalMemoryProposal> = gateway
        .prepare_external_memory(&viewer, owner, &source, expected, &Endpoint(true))
        .unwrap();
    let candidate: &InsomniaCandidate = &proposals[0].candidate;
    assert_eq!(candidate.ownership, InsomniaOwnership::Project);
    assert_eq!(proposals[0].authority.message, expected[0].reference);
    let receipt: ExternalMemoryReceipt = gateway
        .publish_external_memory(&operator, owner, &source, expected, &Endpoint(true))
        .unwrap();
    assert_eq!(receipt.memory_ids().len(), 1);
    let provenance: &ExternalMemoryProvenance = &receipt.sources()[0];
    assert_eq!(provenance.authority_source.message, expected[0].reference);
    assert_eq!(
        gateway
            .publish_external_memory(&operator, owner, &source, expected, &Endpoint(false))
            .unwrap(),
        receipt
    );
    let advanced = gateway
        .advance_external_source_manifest(&operator, owner, &source)
        .unwrap();
    assert_eq!(advanced.manifest().watermark, 11);
    assert!(advanced.manifest().descriptors.is_empty());
    source.revoked.store(true, Ordering::Release);
    assert_eq!(
        gateway
            .hydrate_external_source(&operator, owner, &source, expected)
            .unwrap_err()
            .kind(),
        ProductErrorKind::Unauthorized
    );
    control.revoke(VIEWER).unwrap();
    assert_eq!(
        gateway
            .external_source_manifest(&viewer, owner, &source)
            .unwrap_err()
            .kind(),
        ProductErrorKind::Revoked
    );
}
