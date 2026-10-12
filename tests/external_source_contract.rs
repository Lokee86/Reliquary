use reliquary_memory::external_source::{
    EXTERNAL_SOURCE_CONTRACT, ExternalChangeBatch, ExternalConversationSource, ExternalIndexStatus,
    ExternalMessageDescriptor, ExternalMessageRef, ExternalSearchHit, ExternalSnapshotManifest,
    ExternalSourceError, HydratedExternalMessage, external_content_hash,
};
use reliquary_memory::{
    Cva, EmbeddingEndpoint, EmbeddingEndpointError, EmbeddingMode, EpisodePolicy,
    GatewayAdmissionAdapter, GatewayRuntime, InsomniaWorkerConfig, OwnerGrant,
    ReliquaryRuntimeRoutes, VectorNormalization, VerifiedInstanceAdmission,
};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

struct Source {
    hidden: AtomicBool,
    body: String,
}
impl Source {
    fn descriptor(&self) -> ExternalMessageDescriptor {
        ExternalMessageDescriptor {
            reference: ExternalMessageRef {
                source_namespace: "public-host".into(),
                conversation_id: "conversation".into(),
                message_id: "message".into(),
                content_hash: external_content_hash(&self.body),
            },
            role: "user".into(),
            principal_id: Some("alice".into()),
            timestamp_ns: 42,
            active: true,
        }
    }
}
impl ExternalConversationSource for Source {
    fn source_namespace(&self) -> &str {
        "public-host"
    }
    fn authorize(&self, principal: &str) -> Result<(), ExternalSourceError> {
        if principal == "alice" {
            Ok(())
        } else {
            Err(ExternalSourceError::Unauthorized)
        }
    }
    fn changes_after(
        &self,
        p: &str,
        cursor: u64,
        _: usize,
    ) -> Result<ExternalChangeBatch, ExternalSourceError> {
        self.authorize(p)?;
        if cursor != 1 {
            return Err(ExternalSourceError::InvalidContract);
        }
        Ok(ExternalChangeBatch {
            feed_floor: 1,
            feed_watermark: 1,
            changes: Vec::new(),
        })
    }
    fn snapshot_manifest(&self, p: &str) -> Result<ExternalSnapshotManifest, ExternalSourceError> {
        self.authorize(p)?;
        Ok(ExternalSnapshotManifest {
            source_namespace: self.source_namespace().into(),
            contract_version: EXTERNAL_SOURCE_CONTRACT,
            watermark: 1,
            messages: if self.hidden.load(Ordering::SeqCst) {
                Vec::new()
            } else {
                vec![self.descriptor()]
            },
        })
    }
    fn hydrate(
        &self,
        p: &str,
        refs: &[ExternalMessageDescriptor],
    ) -> Result<Vec<HydratedExternalMessage>, ExternalSourceError> {
        self.authorize(p)?;
        if self.hidden.load(Ordering::SeqCst) {
            return Err(ExternalSourceError::Unauthorized);
        }
        refs.iter()
            .map(|r| {
                if *r != self.descriptor() {
                    return Err(ExternalSourceError::StaleEvidence);
                }
                Ok(HydratedExternalMessage {
                    descriptor: r.clone(),
                    content: self.body.clone(),
                })
            })
            .collect()
    }
}
struct Embed;
impl EmbeddingEndpoint for Embed {
    fn dimensions(&self) -> u32 {
        2
    }
    fn normalization(&self) -> VectorNormalization {
        VectorNormalization::L2
    }
    fn embed(
        &self,
        _: EmbeddingMode,
        inputs: &[String],
    ) -> Result<Vec<Vec<f32>>, EmbeddingEndpointError> {
        Ok(inputs.iter().map(|_| vec![0.6, 0.8]).collect())
    }
}
struct Admission(String);
impl GatewayAdmissionAdapter for Admission {
    fn verify(&self, credential: &str) -> Result<VerifiedInstanceAdmission, String> {
        if credential != "credential" {
            return Err("denied".into());
        }
        Ok(VerifiedInstanceAdmission {
            principal_id: "alice".into(),
            phy_id: None,
            grants: BTreeMap::from([(
                self.0.clone(),
                BTreeSet::from([OwnerGrant::ViewRel, OwnerGrant::ManageRel]),
            )]),
        })
    }
}
#[test]
fn downstream_source_uses_verified_gateway_and_reference_only_reopen() {
    let dir = std::env::temp_dir().join(format!("external-public-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("owner.rel");
    drop(Cva::create_project(&path).unwrap());
    let gateway = GatewayRuntime::new(
        dir.join("state"),
        ReliquaryRuntimeRoutes::default(),
        InsomniaWorkerConfig {
            workers: 0,
            ..Default::default()
        },
        EpisodePolicy::default(),
    );
    let owner = gateway.mount_rel_path(&path).unwrap();
    let gateway = gateway.with_admission_adapter(Arc::new(Admission(owner.clone())));
    assert!(gateway.attach_authenticated_instance("forged").is_err());
    let instance = gateway.attach_authenticated_instance("credential").unwrap();
    let source = Source {
        hidden: AtomicBool::new(false),
        body: "secret café 🦀 canonical public fixture 56b38f".into(),
    };
    let status: ExternalIndexStatus = gateway
        .rebuild_external_index_for(&instance, &owner, &source, &Embed)
        .unwrap();
    assert_eq!(status.cursor, 1);
    let hits: Vec<ExternalSearchHit> = gateway
        .search_external_source_for(&instance, &owner, &source, &Embed, "secret", 10)
        .unwrap();
    assert_eq!(hits.len(), 1);
    let chunks = gateway
        .hydrate_external_chunks_for(&instance, &owner, &source, &[hits[0].reference.clone()])
        .unwrap();
    assert_eq!(chunks[0].content, source.body);
    source.hidden.store(true, Ordering::SeqCst);
    assert_eq!(
        gateway.search_external_lexical_for(&instance, &owner, &source, "secret", 10),
        Err(ExternalSourceError::Unauthorized)
    );
    source.hidden.store(false, Ordering::SeqCst);
    assert_eq!(
        gateway
            .update_external_index_for(&instance, &owner, &source, &Embed)
            .unwrap(),
        status
    );
    gateway.detach_instance(&instance).unwrap();
    drop(gateway.unmount_rel(&owner).unwrap());
    gateway.shutdown().unwrap();
    drop(gateway);
    let bytes = std::fs::read(&path).unwrap();
    assert!(
        !bytes
            .windows(source.body.len())
            .any(|w| w == source.body.as_bytes())
    );
    let mut reopened = Cva::open(&path).unwrap();
    assert!(reopened.episodes().is_empty());
    drop(reopened);
    std::fs::remove_dir_all(dir).unwrap();
}
