use reliquary_memory::{
    Branch, Cva, GatewayAdmissionAdapter, GatewayRuntime, InsomniaWorkerConfig, OwnerGrant,
    VerifiedInstanceAdmission,
};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};
struct Admission(Mutex<Option<String>>);
impl GatewayAdmissionAdapter for Admission {
    fn verify(&self, credential: &str) -> Result<VerifiedInstanceAdmission, String> {
        if credential != "verified" {
            return Err("denied".into());
        }
        Ok(VerifiedInstanceAdmission {
            principal_id: "management-host".into(),
            phy_id: None,
            grants: BTreeMap::from([(
                self.0.lock().unwrap().clone().ok_or("owner unavailable")?,
                BTreeSet::from([OwnerGrant::ViewRel, OwnerGrant::ManageRel]),
            )]),
        })
    }
}
#[test]
fn public_management_consumer_uses_immutable_results() {
    let root = std::env::temp_dir().join(format!(
        "reliquary-management-public-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let mut cva = Cva::create_project(root.join("owner.rel")).unwrap();
    cva.append_node(
        "seed".into(),
        "conversation".into(),
        None,
        "user".into(),
        1,
        "source",
    )
    .unwrap();
    cva.append_branch(Branch {
        id: "main".into(),
        conversation_id: "conversation".into(),
        leaf_node_id: "seed".into(),
        canonical: true,
    })
    .unwrap();
    cva.sync().unwrap();
    drop(cva);
    let admission = Arc::new(Admission(Mutex::new(None)));
    let g = Arc::new(
        GatewayRuntime::new(
            root.join("state"),
            Default::default(),
            InsomniaWorkerConfig {
                workers: 0,
                ..Default::default()
            },
            Default::default(),
        )
        .with_admission_adapter(admission.clone()),
    );
    let owner = g.mount_rel_path(&root.join("owner.rel")).unwrap();
    *admission.0.lock().unwrap() = Some(owner.clone());
    let instance = g.attach_authenticated_instance("verified").unwrap();
    let api = g.workspace_management(instance);
    api.select_conversation(&owner, "conversation", "main")
        .unwrap();
    assert_eq!(
        api.conversations(&owner, 0, 1).unwrap().items[0].conversation_id,
        "conversation"
    );
    assert_eq!(
        api.transcript(&owner, "conversation", "seed", 0, 1)
            .unwrap()
            .items[0]
            .content,
        "source"
    );
    api.set_title(&owner, "conversation", "Title".into())
        .unwrap();
    let memory = api
        .create_memory(
            &owner,
            "Fact".into(),
            "Evidence".into(),
            "knowledge".into(),
            "fact".into(),
            1,
        )
        .unwrap();
    assert_eq!(api.memories(&owner, 0, 1).unwrap().items[0].id, memory.id);
    assert!(api.provenance(&owner, memory.id).is_ok());
    assert_eq!(api.statistics(&owner).unwrap().memories.memories, 1);
    assert_eq!(api.health(&owner).unwrap().conversations, 1);
    api.quiesce(&owner).unwrap();
    api.reconcile_quiesced(&owner).unwrap();
    api.reopen(&owner).unwrap();
    assert_eq!(api.memory(&owner, memory.id).unwrap().content, "Evidence");
    g.shutdown().unwrap();
    drop(api);
    drop(g);
    std::fs::remove_dir_all(root).unwrap();
}
