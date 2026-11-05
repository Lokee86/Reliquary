use super::*;
use reliquary_container::{Container, ContainerIdentity, ReliquaryScope};
use reliquary_memory_core::{MemoryDraft, MemoryStore};
use reliquary_owner_session::PhyWriter;
use reliquary_vector_store::{
    ArchiveVectorStore, CompatibilityProfileStore, MemoryVectorStore, PackedVectorStore,
    VectorGenerationStore,
};
use std::{fs, path::PathBuf};

struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(".test-tmp")
            .join(format!("context-{}", Uuid::new_v4()));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn initialize_rel(path: &std::path::Path, owner: [u8; 16]) {
    let mut container = Container::create(
        path,
        ContainerIdentity::reliquary(ReliquaryScope::Project),
        owner,
    )
    .unwrap();
    PackedVectorStore::default()
        .initialize(&mut container)
        .unwrap();
    CompatibilityProfileStore::default()
        .initialize(&mut container)
        .unwrap();
    ArchiveVectorStore::default()
        .initialize(&mut container)
        .unwrap();
    MemoryVectorStore::default()
        .initialize(&mut container)
        .unwrap();
    VectorGenerationStore::empty()
        .initialize(&mut container)
        .unwrap();
    container.sync().unwrap();
}

fn create_memory(path: &std::path::Path, owner: [u8; 16], phy: bool) -> [u8; 32] {
    if phy {
        drop(PhyWriter::create_path(path, owner).unwrap());
    } else {
        drop(RelWriter::create_path(path, ReliquaryScope::Project, owner).unwrap());
    }
    let mut container = Container::open_writer(path).unwrap();
    let mut memories = MemoryStore::open(&container).unwrap();
    let published = memories
        .publish_versioned_creation(
            &mut container,
            MemoryDraft {
                title: if phy { "PHY selected" } else { "REL selected" }.into(),
                content: if phy {
                    "private PHY context body"
                } else {
                    "authorized REL context body"
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
                mutation_id: if phy {
                    "context-phy-memory"
                } else {
                    "context-rel-memory"
                }
                .into(),
                created_at_ns: 1,
                updated_at_ns: 1,
            },
            1,
        )
        .unwrap();
    container.sync().unwrap();
    published.revision.record.id.0
}

struct Words;
impl ContextTokenCounter for Words {
    fn count_text(&self, text: &str) -> u64 {
        text.split_whitespace().count() as u64
    }
    fn count_message(&self, message: &reliquary_application::ContextMessage) -> u64 {
        self.count_text(&message.content).saturating_add(1)
    }
}

fn budget() -> CompactionBudget {
    CompactionBudget {
        trigger_tokens: 50,
        target_tokens: 40,
        raw_tail_tokens: 20,
        summary_tokens: 10,
        compaction_input_tokens: 160,
    }
}

#[test]
fn host_context_memory_scopes_compaction_retry_and_owner_reopen() {
    let temp = Temp::new();
    let rel_raw = [0x62; 16];
    let phy_raw = [0x63; 16];
    let rel_path = temp.0.join("owner.rel");
    let phy_path = temp.0.join("private.phy");
    let rel_memory = create_memory(&rel_path, rel_raw, false);
    let phy_memory = create_memory(&phy_path, phy_raw, true);

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
    // These are host-owned native turns; REL is empty and is never copied into the request.
    let native = ContextView {
        turns: vec![
            reliquary_application::ContextTurn {
                message_id: "host-prior".into(),
                role: reliquary_application::ContextRole::Assistant,
                content: "host owned prior response".into(),
            },
            reliquary_application::ContextTurn {
                message_id: "host-current".into(),
                role: reliquary_application::ContextRole::User,
                content: "host owned current request".into(),
            },
        ],
    };
    let memories = [
        ContextMemoryRef {
            owner: rel,
            kind: ContextMemoryOwnerKind::Rel,
            memory_id: rel_memory,
        },
        ContextMemoryRef {
            owner: phy,
            kind: ContextMemoryOwnerKind::Phy,
            memory_id: phy_memory,
        },
    ];
    let prepared = alice
        .prepare_context_workflow(
            rel,
            "conversation",
            native.clone(),
            &memories,
            &[],
            &[],
            None,
            100,
            budget(),
            &Words,
        )
        .unwrap();
    let request = alice.context_request(&prepared).unwrap();
    assert_eq!(request.messages.len(), 4);
    assert!(
        request.messages[0]
            .content
            .contains("host owned prior response")
    );
    assert!(
        request.messages[1]
            .content
            .contains("host owned current request")
    );
    assert!(
        request
            .messages
            .iter()
            .any(|message| message.content.contains("authorized REL context body"))
    );
    assert!(
        request
            .messages
            .iter()
            .any(|message| message.content.contains("private PHY context body"))
    );
    assert_eq!(request, alice.context_request(&prepared).unwrap());

    let checkpoint = alice
        .compact_context_with(
            &prepared,
            |plan| {
                // The owner can be read from the inference callback; no owner lock spans provider work.
                assert_eq!(
                    alice
                        .with_writable_reliquary(rel, |session| session.rel_turn_count())
                        .unwrap(),
                    0
                );
                Ok(format!(
                    "WORKING STATE\nsource: {}..{}",
                    plan.through_message_id, plan.through_message_id
                ))
            },
            &Words,
        )
        .unwrap()
        .unwrap();
    assert!(
        alice
            .context_checkpoint(&checkpoint)
            .unwrap()
            .summary
            .contains("source:")
    );

    let second_alice = gateway.instance("alice", [rel], [phy]).unwrap();
    assert!(
        second_alice
            .prepare_context_workflow(
                rel,
                "conversation",
                native.clone(),
                &memories,
                &[],
                &[],
                Some(&checkpoint),
                100,
                budget(),
                &Words
            )
            .is_err()
    );
    let rel_only = gateway.instance("alice", [rel], []).unwrap();
    assert!(
        rel_only
            .prepare_context_workflow(
                rel,
                "conversation",
                native.clone(),
                &memories,
                &[],
                &[],
                None,
                100,
                budget(),
                &Words
            )
            .is_err()
    );
    let bob = gateway.instance("bob", [], []).unwrap();
    assert!(
        bob.finalize_context_workflow(&prepared, "forged".into(), &Words)
            .is_err()
    );
    assert!(matches!(
        bob.read_context_memory(ContextMemoryRef {
            owner: rel,
            kind: ContextMemoryOwnerKind::Rel,
            memory_id: rel_memory
        }),
        Err(GatewayError::UnauthorizedRel { .. })
    ));
    assert!(matches!(
        alice.read_context_memory(ContextMemoryRef {
            owner: phy,
            kind: ContextMemoryOwnerKind::Phy,
            memory_id: phy_memory
        }),
        Ok(_)
    ));

    drop(second_alice);
    drop(rel_only);
    drop(bob);
    drop(alice);
    drop(gateway);
    let reopened = Gateway::new();
    reopened
        .mount_reliquary(&rel_path, &rel_admission, [], [])
        .unwrap();
    reopened
        .mount_phylactery(&phy_path, &phy_admission, [])
        .unwrap();
    let resumed = reopened.instance("alice", [rel], [phy]).unwrap();
    let after_reopen = resumed
        .prepare_context_workflow(
            rel,
            "conversation",
            native,
            &memories,
            &[],
            &[],
            None,
            100,
            budget(),
            &Words,
        )
        .unwrap();
    assert_eq!(
        resumed
            .context_request(&after_reopen)
            .unwrap()
            .messages
            .len(),
        4
    );
    assert_eq!(resumed.reliquary(rel).unwrap().rel_turn_count(), 0);
}

#[test]
fn invalid_host_context_shape_and_ungranted_phy_are_rejected() {
    let temp = Temp::new();
    let raw = [0x64; 16];
    let path = temp.0.join("owner.rel");
    initialize_rel(&path, raw);
    let gateway = Gateway::new();
    let admission = GraphAdmission::for_replay(raw, [], [], [], [], []);
    let owner = gateway
        .mount_writable_reliquary(&path, &admission, [], [])
        .unwrap();
    let alice = gateway.instance("alice", [owner], []).unwrap();
    let duplicate_ids = ContextView {
        turns: vec![
            reliquary_application::ContextTurn {
                message_id: "same".into(),
                role: reliquary_application::ContextRole::User,
                content: "one".into(),
            },
            reliquary_application::ContextTurn {
                message_id: "same".into(),
                role: reliquary_application::ContextRole::Assistant,
                content: "two".into(),
            },
        ],
    };
    assert!(
        alice
            .prepare_context_workflow(
                owner,
                "chat",
                duplicate_ids,
                &[],
                &[],
                &[],
                None,
                0,
                budget(),
                &Words
            )
            .is_err()
    );
    assert!(matches!(
        alice.read_context_memory(ContextMemoryRef {
            owner: OwnerId::from_bytes([0x65; 16]),
            kind: ContextMemoryOwnerKind::Phy,
            memory_id: [1; 32]
        }),
        Err(GatewayError::UnauthorizedPhy { .. })
    ));
}
