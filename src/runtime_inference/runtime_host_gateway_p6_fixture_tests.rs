//! Physical files and deterministic logical IDs; no second queue or runtime.
use super::super::*;
use crate::{Branch, Cva, EpisodePolicy, InsomniaWorkerConfig, InteractionRuntime, Phylactery};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub(super) fn seed() -> u64 {
    std::env::var("RELIQUARY_P6_SEED")
        .ok()
        .map(|s| s.parse().expect("P6 seed"))
        .unwrap_or(38)
}
pub(super) fn order(seed: u64, count: usize) -> Vec<usize> {
    let mut state = seed;
    let mut entries: Vec<_> = (0..count).collect();
    for i in (1..count).rev() {
        state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        entries.swap(i, (state % (i as u64 + 1)) as usize);
    }
    entries
}
pub(super) fn id(index: usize) -> String {
    format!("p6-{:016x}-{index:04}", seed())
}
pub(super) fn gateway(root: &Path) -> GatewayRuntime {
    GatewayRuntime::new(
        root.join("state"),
        Default::default(),
        InsomniaWorkerConfig {
            workers: 0,
            ..Default::default()
        },
        EpisodePolicy::default(),
    )
}
pub(super) fn seeded(path: &Path) -> InteractionRuntime {
    let mut cva = Cva::create_project(path).unwrap();
    for c in ["a", "b"] {
        let node = format!("root-{c}");
        cva.append_node(
            node.clone(),
            c.into(),
            None,
            "user".into(),
            1,
            "P6_SOURCE_SECRET",
        )
        .unwrap();
        cva.append_branch(Branch {
            id: "main".into(),
            conversation_id: c.into(),
            leaf_node_id: node,
            canonical: true,
        })
        .unwrap();
    }
    cva.sync().unwrap();
    InteractionRuntime::new(cva)
}
pub(super) fn grants(
    rels: &[String],
    phy: Option<&String>,
    writer: bool,
) -> BTreeMap<String, BTreeSet<OwnerGrant>> {
    let mut grants = BTreeMap::new();
    for rel in rels {
        let mut caps = BTreeSet::from([OwnerGrant::ViewRel]);
        if writer {
            caps.extend([
                OwnerGrant::ManageRel,
                OwnerGrant::SubmitConversation,
                OwnerGrant::CancelOwnPendingSubmission,
                OwnerGrant::CancelSharedGeneration,
            ]);
        }
        grants.insert(rel.clone(), caps);
    }
    if let Some(phy) = phy {
        grants.insert(phy.clone(), BTreeSet::from([OwnerGrant::UseOwnPhy]));
    }
    grants
}
pub(super) fn attach(
    g: &GatewayRuntime,
    rels: &[String],
    phy: Option<&String>,
    principal: &str,
    writer: bool,
) -> Arc<InstanceRuntime> {
    g.attach_instance(principal.into(), phy.cloned(), grants(rels, phy, writer))
        .unwrap()
}
pub(super) fn request(
    g: &GatewayRuntime,
    owner: &str,
    conversation: &str,
    index: usize,
    parent: &str,
) -> crate::GatewayConversationSubmission {
    let cid = crate::multiplexing_journal::ConversationId::new(conversation).unwrap();
    let revision = g
        .conversation
        .journal(owner)
        .unwrap()
        .lock()
        .unwrap()
        .conversation_revision(&cid);
    crate::GatewayConversationSubmission {
        conversation_id: conversation.into(),
        submission_id: id(index),
        expected_conversation_revision: revision,
        expected_parent_node_id: Some(parent.into()),
        branch_id: "main".into(),
        content: format!("P6_QUEUE_SECRET-{index}"),
        attachments: vec![],
        client_timestamp_ns: Some(-(index as i64)),
    }
}
pub(super) fn metric(workload: &str, values: Value) {
    println!(
        "P6_METRIC {}",
        json!({"workload":workload,"seed":seed(),"values":values})
    );
}
pub(super) struct Fixture {
    pub root: PathBuf,
    pub g: Arc<GatewayRuntime>,
    pub rels: Vec<String>,
    pub phys: Vec<String>,
}
impl Fixture {
    pub fn new(label: &str, rel_count: usize, phy_count: usize) -> Self {
        let parent = std::env::var_os("RELIQUARY_P6_FIXTURE_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        let root = parent.join(format!("p6-{label}-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let g = Arc::new(gateway(&root));
        let rels = (0..rel_count)
            .map(|n| g.mount_rel(seeded(&root.join(format!("{n}.rel")))).unwrap())
            .collect();
        let phys = (0..phy_count)
            .map(|n| {
                g.mount_phy(Phylactery::create(root.join(format!("{n}.phy"))).unwrap())
                    .unwrap()
            })
            .collect();
        Self {
            root,
            g,
            rels,
            phys,
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.g.shutdown().unwrap();
        // File handles belong to the runtime until Fixture itself is dropped.
        // Runner preserves its explicitly isolated fixture root as evidence.
    }
}
