use reliquary_memory::{
    ConversationSubmissionId, FinalizeCapture, Gateway, HostCredentialBinding, OwnerGrant,
    OwnerScope, Phylactery, Rel, SubmissionId, TrustedHostAdmission,
};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(1);

struct Cohort(PathBuf);

impl Cohort {
    fn new() -> Self {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(".test-tmp")
            .join(format!(
                "g4-process-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}

impl Drop for Cohort {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn create_completed_cohort(root: &Path) -> [PathBuf; 4] {
    let paths = [
        root.join("rel-lantern.rel"),
        root.join("rel-cascade.rel"),
        root.join("phy-reader-a.phy"),
        root.join("phy-reader-b.phy"),
    ];
    let lantern = Rel::create(&paths[0], OwnerScope::Project).unwrap();
    let cascade = Rel::create(&paths[1], OwnerScope::Connection).unwrap();
    let phy_a = Phylactery::create(&paths[2]).unwrap();
    let phy_b = Phylactery::create(&paths[3]).unwrap();
    let gateway = Gateway::new();
    let lantern_id = gateway.mount_rel(&lantern, true).unwrap();
    let cascade_id = gateway.mount_rel(&cascade, true).unwrap();
    let phy_a_id = gateway.mount_phylactery(&phy_a, false).unwrap();
    let phy_b_id = gateway.mount_phylactery(&phy_b, false).unwrap();
    let token = "g4-process-seed-token-000000000000000000000000000";
    let (adapter, _) = TrustedHostAdmission::new([HostCredentialBinding::new(
        "seed",
        token,
        [
            OwnerGrant::OperateRel(lantern_id),
            OwnerGrant::ManageRel(lantern_id),
            OwnerGrant::OperateRel(cascade_id),
            OwnerGrant::ManageRel(cascade_id),
            OwnerGrant::ViewPhylactery(phy_a_id),
            OwnerGrant::ViewPhylactery(phy_b_id),
        ],
    )])
    .install(&gateway)
    .unwrap();
    let instance = adapter.authenticate(token).unwrap();
    let mut lantern_queue = instance
        .open_conversation_queue(&root.join("lantern.queue"), lantern_id)
        .unwrap();
    let mut cascade_queue = instance
        .open_conversation_queue(&root.join("cascade.queue"), cascade_id)
        .unwrap();

    for conversation_index in 0..12 {
        let lantern_owner = conversation_index < 8;
        let owner = if lantern_owner {
            lantern_id
        } else {
            cascade_id
        };
        let queue = if lantern_owner {
            &mut lantern_queue
        } else {
            &mut cascade_queue
        };
        let conversation = format!("conv-g4-{:02}", conversation_index + 1);
        let mut parent = None;
        let mut revision = 0_u64;
        for turn_index in 0..4 {
            let node = format!("g4-{:02}-t{}", conversation_index + 1, turn_index + 1);
            let submission = SubmissionId::new(&node).unwrap();
            let id = ConversationSubmissionId::new(&conversation, submission.clone()).unwrap();
            instance
                .submit_conversation(
                    queue,
                    &conversation,
                    submission,
                    format!("Authored G4 process witness {node} — café λ."),
                    revision,
                )
                .unwrap();
            revision += 1;
            instance
                .finalize_conversation_submission(
                    queue,
                    &id,
                    revision,
                    FinalizeCapture {
                        branch_id: "main".into(),
                        parent_node_id: parent.clone(),
                        committed_timestamp_ns: 1000 + conversation_index * 10 + turn_index,
                        verified_principal_id: "seed".into(),
                    },
                )
                .unwrap();
            revision += 2;
            parent = Some(node);
        }
        assert_eq!(owner, queue.owner_id());
    }

    let lantern_management = instance.workspace_management(lantern_id).unwrap();
    let cascade_management = instance.workspace_management(cascade_id).unwrap();
    lantern_management
        .create_memory(
            "Lantern release".into(),
            "Lantern release uses the amber checklist.".into(),
            "fact".into(),
            "project".into(),
            2000,
        )
        .unwrap();
    cascade_management
        .create_memory(
            "Cascade plan".into(),
            "Cascade project operating plan version 3.".into(),
            "fact".into(),
            "project".into(),
            2001,
        )
        .unwrap();
    drop(lantern_queue);
    drop(cascade_queue);
    lantern_management.quiesce().unwrap();
    cascade_management.quiesce().unwrap();
    lantern_management.verify_quiesced().unwrap();
    cascade_management.verify_quiesced().unwrap();
    drop(lantern_management);
    drop(cascade_management);
    drop(instance);
    drop(adapter);
    drop(gateway);
    drop(lantern);
    drop(cascade);
    drop(phy_a);
    drop(phy_b);
    paths
}

#[test]
fn external_root_crate_consumer_reopens_completed_cohort_in_fresh_process() {
    let cohort = Cohort::new();
    let paths = create_completed_cohort(&cohort.0);
    let binary = env!("CARGO_BIN_EXE_reliquary-external-consumer-check");
    let output = Command::new(binary).args(&paths).output().unwrap();
    assert!(
        output.status.success(),
        "external consumer failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report = String::from_utf8(output.stdout).unwrap();
    assert!(report.contains("\"conversations\":8"));
    assert!(report.contains("\"turns\":32"));
    assert!(report.contains("\"conversations\":4"));
    assert!(report.contains("\"turns\":16"));
    assert!(report.contains("\"memories\":1"));
    assert!(report.contains("\"memory_count\":0"));

    let wrong_kind = Command::new(binary)
        .args([&paths[2], &paths[1], &paths[0], &paths[3]])
        .output()
        .unwrap();
    assert!(!wrong_kind.status.success());
    assert!(
        String::from_utf8(wrong_kind.stderr)
            .unwrap()
            .contains("\"error\"")
    );
}
