use super::*;
use reliquary_application::{EmbeddingEndpointError, VectorNormalization};
use reliquary_container::{Container, ReliquaryScope};
use reliquary_memory_core::{MemoryDraft, MemoryStore};
use std::{cell::Cell, fs, path::PathBuf};

struct Endpoint<'a> {
    calls: Cell<usize>,
    callback: Option<Box<dyn Fn() + 'a>>,
    dimensions: u32,
    saw_private: Cell<bool>,
}
impl Endpoint<'_> {
    fn new() -> Self {
        Self {
            calls: Cell::new(0),
            callback: None,
            dimensions: 2,
            saw_private: Cell::new(false),
        }
    }
}
impl EmbeddingEndpoint for Endpoint<'_> {
    fn dimensions(&self) -> u32 {
        self.dimensions
    }
    fn normalization(&self) -> VectorNormalization {
        VectorNormalization::L2
    }
    fn supports_bounded_embedding(&self) -> bool {
        true
    }
    fn embed(
        &self,
        _: reliquary_owner_session::model::EmbeddingMode,
        _: &[String],
    ) -> Result<Vec<Vec<f32>>, EmbeddingEndpointError> {
        panic!("unbounded inference must not run")
    }
    fn embed_bounded(
        &self,
        _: reliquary_owner_session::model::EmbeddingMode,
        inputs: &[String],
        bound: usize,
    ) -> Result<Vec<Vec<f32>>, EmbeddingEndpointError> {
        self.calls.set(self.calls.get() + 1);
        if inputs.iter().any(|text| text.contains("private")) {
            self.saw_private.set(true);
        }
        if let Some(callback) = &self.callback {
            callback();
        }
        assert!(inputs.len() * self.dimensions as usize * 4 <= bound);
        Ok(inputs
            .iter()
            .map(|_| {
                let mut row = vec![0.0; self.dimensions as usize];
                row[0] = 1.0;
                row
            })
            .collect())
    }
}

fn create(path: &Path, owner: [u8; 16], phy: bool) -> MemoryId {
    if phy {
        drop(PhyWriter::create_path(path, owner).unwrap());
    } else {
        drop(RelWriter::create_path(path, ReliquaryScope::Project, owner).unwrap());
    }
    let mut container = Container::open_writer(path).unwrap();
    let mut memory = MemoryStore::open(&container).unwrap();
    let draft = MemoryDraft {
        title: "source".into(),
        content: if phy {
            "private vector source 🦀"
        } else {
            "project vector source 🦀"
        }
        .into(),
        category: "fact".into(),
        memory_type: "semantic".into(),
        authority_kind: "user".into(),
        temporal_status: "current".into(),
        scope: if phy { "user" } else { "project" }.into(),
        lifecycle_state: "extracted".into(),
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
        mutation_id: format!("source-{phy}"),
        created_at_ns: 1,
        updated_at_ns: 1,
    };
    memory
        .publish_versioned_creation(&mut container, draft, 1)
        .unwrap()
        .revision
        .record
        .id
}

#[test]
fn archive_vector_generation_reuses_accepted_rows_after_reopen() {
    use reliquary_owner_session::model::{Branch, TurnInput};
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".test-tmp");
    fs::create_dir_all(&root).unwrap();
    let path = root.join(format!("archive-vectors-{}.rel", Uuid::new_v4()));
    let raw = [163; 16];
    let mut writer = RelWriter::create_path(&path, ReliquaryScope::Project, raw).unwrap();
    writer
        .accept_turn_and_advance_branch(
            TurnInput {
                id: "source".into(),
                conversation_id: "conversation".into(),
                parent_id: None,
                role: "user".into(),
                principal_id: Some("alice".into()),
                timestamp_ns: 1,
                content: "Exact source".into(),
                attachments: vec![],
                project_attachments: vec![],
            },
            Branch {
                id: "main".into(),
                conversation_id: "conversation".into(),
                leaf_node_id: "source".into(),
                canonical: true,
            },
        )
        .unwrap();
    drop(writer);
    let admission = GraphAdmission::for_replay(raw, [], [], [], [], []);
    let gateway = Gateway::new();
    let owner = gateway
        .mount_writable_reliquary(&path, &admission, [], [])
        .unwrap();
    let alice = gateway.instance("alice", [owner], []).unwrap();
    let endpoint = Endpoint::new();
    let profile = alice.publish_embedding_profile(owner, &endpoint).unwrap();
    let generation = alice
        .publish_archive_vectors(owner, profile, &endpoint)
        .unwrap()
        .unwrap();
    drop(alice);
    drop(gateway);
    let gateway = Gateway::new();
    gateway
        .mount_writable_reliquary(&path, &admission, [], [])
        .unwrap();
    let alice = gateway.instance("alice", [owner], []).unwrap();
    let retry = Endpoint::new();
    assert_eq!(
        alice
            .publish_archive_vectors(owner, profile, &retry)
            .unwrap(),
        Some(generation)
    );
    assert_eq!(retry.calls.get(), 0);
    drop(alice);
    drop(gateway);
    {
        use reliquary_vector_store::{
            ArchiveVectorStore, CompatibilityProfileStore, PackedVectorStore, VectorGenerationStore,
        };
        let mut container = Container::open_writer(&path).unwrap();
        let packed = PackedVectorStore::open(&container).unwrap();
        let profiles = CompatibilityProfileStore::open(&container).unwrap();
        let mappings = ArchiveVectorStore::open(&container, &packed).unwrap();
        let mut generations =
            VectorGenerationStore::open(&container, &profiles, &mappings).unwrap();
        generations
            .publish(
                &mut container,
                &profiles,
                &mappings,
                profile,
                generation.archive_vector_id,
                generation.source_archive_version + 1,
            )
            .unwrap();
        container.sync().unwrap();
    }
    assert!(matches!(
        RelSession::open_path(&path, &admission, [], []),
        Err(OwnerSessionError::Record(message)) if message.contains("generation source cut")
    ));
    fs::remove_file(path).unwrap();
}

#[test]
fn rel_phy_vectors_are_source_bound_private_durable_and_retry_without_inference() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(".test-tmp")
        .join(format!("vp-{}", Uuid::new_v4()));
    fs::create_dir_all(&root).unwrap();
    let rel_path = root.join("owner.rel");
    let phy_path = root.join("private.phy");
    let rel_raw = [161; 16];
    let phy_raw = [162; 16];
    let rel_memory = create(&rel_path, rel_raw, false);
    let phy_memory = create(&phy_path, phy_raw, true);
    let gateway = Gateway::new();
    let rel_admission = GraphAdmission::for_replay(rel_raw, [], [], [], [], []);
    let phy_admission = GraphAdmission::for_replay(phy_raw, [], [], [], [], []);
    let rel = gateway
        .mount_writable_reliquary(&rel_path, &rel_admission, [], [])
        .unwrap();
    let phy = gateway
        .mount_writable_phylactery(&phy_path, &phy_admission, [])
        .unwrap();
    let alice = gateway.instance("alice", [rel], [phy]).unwrap();
    let denied = gateway.instance("bob", [rel], []).unwrap();
    let endpoint = Endpoint::new();
    assert!(denied.publish_embedding_profile(phy, &endpoint).is_err());
    assert_eq!(endpoint.calls.get(), 0);
    let profile = alice.publish_embedding_profile(rel, &endpoint).unwrap();
    assert_eq!(
        profile,
        alice.publish_embedding_profile(phy, &endpoint).unwrap()
    );
    let before = fs::metadata(&rel_path).unwrap().len();
    let incompatible = Endpoint {
        dimensions: 3,
        ..Endpoint::new()
    };
    assert!(
        alice
            .prepare_owner_memory_vectors(rel, profile, &[rel_memory], &incompatible)
            .is_err()
    );
    assert_eq!(fs::metadata(&rel_path).unwrap().len(), before);
    let callback_endpoint = Endpoint {
        callback: Some(Box::new(|| {
            alice
                .with_rel_writer(rel, |w| assert_eq!(w.owner_id(), rel_raw))
                .unwrap();
            alice
                .with_entity_phy_writer(phy, |w| assert_eq!(w.owner_id(), phy_raw))
                .unwrap();
        })),
        ..Endpoint::new()
    };
    let rel_proposal = alice
        .prepare_owner_memory_vectors(rel, profile, &[rel_memory], &callback_endpoint)
        .unwrap();
    assert!(!callback_endpoint.saw_private.get());
    let phy_proposal = alice
        .prepare_owner_memory_vectors(phy, profile, &[phy_memory], &callback_endpoint)
        .unwrap();
    assert!(callback_endpoint.saw_private.get());
    assert!(denied.publish_owner_memory_vectors(&phy_proposal).is_err());
    let foreign_instance = gateway.instance("alice", [rel], [phy]).unwrap();
    assert!(
        foreign_instance
            .publish_owner_memory_vectors(&rel_proposal)
            .is_err()
    );
    assert!(alice.publish_owner_memory_vectors(&rel_proposal).unwrap());
    assert!(alice.publish_owner_memory_vectors(&phy_proposal).unwrap());
    assert!(!alice.publish_owner_memory_vectors(&rel_proposal).unwrap());
    let cached = Endpoint::new();
    let retry = alice
        .prepare_owner_memory_vectors(rel, profile, &[rel_memory], &cached)
        .unwrap();
    assert_eq!(cached.calls.get(), 0);
    assert!(!alice.publish_owner_memory_vectors(&retry).unwrap());
    drop(callback_endpoint);
    drop(foreign_instance);
    drop(denied);
    drop(alice);
    drop(gateway);
    let gateway = Gateway::new();
    gateway
        .mount_writable_reliquary(&rel_path, &rel_admission, [], [])
        .unwrap();
    gateway
        .mount_writable_phylactery(&phy_path, &phy_admission, [])
        .unwrap();
    let alice = gateway.instance("alice", [rel], [phy]).unwrap();
    let endpoint = Endpoint::new();
    for (owner, memory) in [(rel, rel_memory), (phy, phy_memory)] {
        let prepared = alice
            .prepare_owner_memory_vectors(owner, profile, &[memory], &endpoint)
            .unwrap();
        assert!(!alice.publish_owner_memory_vectors(&prepared).unwrap());
    }
    assert_eq!(endpoint.calls.get(), 0);
    let private = alice
        .with_entity_phy_writer(phy, |writer| writer.memory_history(phy_memory))
        .unwrap()
        .unwrap()
        .unwrap();
    assert!(private.revisions[0].record.source_node_id.is_none());
    drop(alice);
    drop(gateway);
    let gateway = Gateway::new();
    gateway
        .mount_reliquary(&rel_path, &rel_admission, [], [])
        .unwrap();
    gateway
        .mount_phylactery(&phy_path, &phy_admission, [])
        .unwrap();
    let alice = gateway.instance("alice", [rel], [phy]).unwrap();
    let endpoint = Endpoint::new();
    for (owner, memory) in [(rel, rel_memory), (phy, phy_memory)] {
        assert!(matches!(
            alice.publish_embedding_profile(owner, &endpoint),
            Err(GatewayError::WriterUnavailable)
        ));
        assert!(matches!(
            alice.prepare_owner_memory_vectors(owner, profile, &[memory], &endpoint),
            Err(GatewayError::WriterUnavailable)
        ));
    }
    assert_eq!(endpoint.calls.get(), 0);
    drop(alice);
    drop(gateway);
    fs::remove_dir_all(root).unwrap();
}
