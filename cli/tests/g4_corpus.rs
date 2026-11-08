use reliquary_memory::{
    ConversationSubmissionId, FinalizeCapture, Gateway, HostCredentialBinding, OwnerGrant,
    OwnerScope, Phylactery, QueueState, Rel, RewritePolicy, SubmissionId, TrustedHostAdmission,
};
use serde_json::Value;
use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(1);

struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(".test-tmp")
            .join(format!(
                "g4-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn cli(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_reliquary"))
        .args(args)
        .output()
        .unwrap()
}

fn json(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|_| {
        serde_json::from_slice(&output.stderr).unwrap_or_else(|error| {
            panic!(
                "not JSON ({error}): stdout={} stderr={}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            )
        })
    })
}

#[test]
fn fresh_cli_process_inspects_and_copies_populated_public_rel() {
    let temp = Temp::new();
    let source = temp.path("populated.rel");
    let cascade_path = temp.path("rel-cascade.rel");
    let phy_a_path = temp.path("phy-reader-a.phy");
    let phy_b_path = temp.path("phy-reader-b.phy");
    let peer = temp.path("identical-peer.rel");
    let copy = temp.path("copy.rel");
    let dry_run = temp.path("dry-run.rel");
    let policy_path = temp.path("policy.json");

    let manifest: Value =
        serde_json::from_str(include_str!("../../tests/fixtures/g4-native-rel21-v1.json")).unwrap();
    let rel = Rel::create(&source, OwnerScope::Project).unwrap();
    let cascade = Rel::create(&cascade_path, OwnerScope::Connection).unwrap();
    let phy_a = Phylactery::create(&phy_a_path).unwrap();
    let phy_b = Phylactery::create(&phy_b_path).unwrap();
    let gateway = Gateway::new();
    let owner = gateway.mount_rel(&rel, true).unwrap();
    let cascade_owner = gateway.mount_rel(&cascade, true).unwrap();
    let phy_a_owner = gateway.mount_phylactery(&phy_a, false).unwrap();
    let phy_b_owner = gateway.mount_phylactery(&phy_b, false).unwrap();
    let operator_token = "g4-local-token-00000000000000000000000000000000";
    let admission = TrustedHostAdmission::new([HostCredentialBinding::new(
        "g4-operator",
        operator_token,
        [
            OwnerGrant::ViewRel(owner),
            OwnerGrant::ManageRel(owner),
            OwnerGrant::OperateRel(owner),
            OwnerGrant::ViewRel(cascade_owner),
            OwnerGrant::ManageRel(cascade_owner),
            OwnerGrant::OperateRel(cascade_owner),
            OwnerGrant::ViewPhylactery(phy_a_owner),
            OwnerGrant::ViewPhylactery(phy_b_owner),
        ],
    )]);
    let (adapter, _control) = admission.install(&gateway).unwrap();
    let instance = adapter.authenticate(operator_token).unwrap();
    let management = instance.workspace_management(owner).unwrap();
    let memory = management
        .create_memory(
            "G4 release note".into(),
            "Synthetic source-backed operator evidence, line one.\nLine two stays intact: café."
                .into(),
            "project_fact".into(),
            "fact".into(),
            1_800_000_000_000_000_000,
        )
        .unwrap();
    let memory_id = memory.id;
    let cascade_management = instance.workspace_management(cascade_owner).unwrap();
    cascade_management
        .set_title("conv-c01", "Cascade operating plan")
        .ok();
    let mut lantern_queue = instance
        .open_conversation_queue(temp.path("lantern-queue"), owner)
        .unwrap();
    let mut cascade_queue = instance
        .open_conversation_queue(temp.path("cascade-queue"), cascade_owner)
        .unwrap();
    let scripts = manifest["conversation_scripts"].as_array().unwrap();
    assert_eq!(scripts.len(), 12);
    let mut observed_turns = [0usize; 2];
    for (script_index, script) in scripts.iter().enumerate() {
        let conversation = script["conversation_id"].as_str().unwrap();
        let is_lantern = script["owner"] == "rel-lantern";
        let queue = if is_lantern {
            &mut lantern_queue
        } else {
            &mut cascade_queue
        };
        let count = &mut observed_turns[usize::from(!is_lantern)];
        let mut revision = 0_u64;
        let turns = script["turns"].as_array().unwrap();
        let fork = script["branches"].as_array().unwrap().first();
        for (turn_index, turn) in turns.iter().enumerate() {
            let node = turn["id"].as_str().unwrap();
            let submission = SubmissionId::new(node).unwrap();
            let id = ConversationSubmissionId::new(conversation, submission.clone()).unwrap();
            instance
                .submit_conversation(
                    queue,
                    conversation,
                    submission,
                    turn["content"].as_str().unwrap(),
                    revision,
                )
                .unwrap();
            revision += 1;
            let parent = if let Some(fork) = fork {
                let children = fork["child_turn_ids"].as_array().unwrap();
                if children.iter().any(|child| child.as_str() == Some(node)) {
                    Some(fork["parent_turn_id"].as_str().unwrap().to_owned())
                } else {
                    turn_index
                        .checked_sub(1)
                        .map(|index| turns[index]["id"].as_str().unwrap().to_owned())
                }
            } else {
                turn_index
                    .checked_sub(1)
                    .map(|index| turns[index]["id"].as_str().unwrap().to_owned())
            };
            let branch = if let Some(fork) = fork {
                let children = fork["child_turn_ids"].as_array().unwrap();
                if children.iter().any(|child| child.as_str() == Some(node))
                    && node != fork["canonical_child_id"].as_str().unwrap()
                {
                    format!("{}-{node}", fork["id"].as_str().unwrap())
                } else {
                    "main".to_owned()
                }
            } else {
                "main".to_owned()
            };
            let status = instance
                .finalize_conversation_submission(
                    queue,
                    &id,
                    revision,
                    FinalizeCapture {
                        branch_id: branch,
                        parent_node_id: parent,
                        committed_timestamp_ns: 1_800_000_000_000_000_000_i64
                            + (script_index * 10 + turn_index) as i64,
                        verified_principal_id: "g4-operator".into(),
                    },
                )
                .unwrap();
            assert_eq!(status.state, QueueState::Committed);
            revision += 2;
            *count += 1;
        }
    }
    assert_eq!(observed_turns, [32, 16]);
    management.set_title("conv-l01", "Lantern release").unwrap();
    drop(lantern_queue);
    drop(cascade_queue);
    management.quiesce().unwrap();
    cascade_management.quiesce().unwrap();
    let before = fs::read(&source).unwrap();
    assert_eq!(management.verify_quiesced().unwrap().owner_id, owner);
    management.reopen().unwrap();
    cascade_management.reopen().unwrap();
    drop(management);
    drop(instance);
    drop(_control);
    drop(adapter);
    drop(gateway);
    drop(rel);
    drop(cascade_management);
    drop(cascade);
    drop(phy_a);
    drop(phy_b);

    // This independent public open confirms the generated REL is usable after
    // all writer/admission objects have been dropped.
    let reopened = Rel::open(&source).unwrap();
    assert_eq!(reopened.info().unwrap().owner_id, owner);
    let proof = reopened.current_historical_proof().unwrap();
    assert!(proof.retained_records > 0);
    drop(reopened);

    let reader_gateway = Gateway::new();
    let reopened = Rel::open(&source).unwrap();
    assert_eq!(reader_gateway.mount_rel(&reopened, false).unwrap(), owner);
    let reader_token = "g4-read-token-000000000000000000000000000000000";
    let reader_admission = TrustedHostAdmission::new([HostCredentialBinding::new(
        "g4-viewer",
        reader_token,
        [OwnerGrant::ViewRel(owner)],
    )]);
    let (reader_adapter, _reader_control) = reader_admission.install(&reader_gateway).unwrap();
    let reader = reader_adapter.authenticate(reader_token).unwrap();
    let reopened_memory = reader
        .workspace_management(owner)
        .unwrap()
        .memories(0, 8)
        .unwrap()
        .items
        .into_iter()
        .find(|item| item.id == memory_id)
        .expect("created Memory survives independent public reopen");
    assert!(reopened_memory.content.contains("café"));
    drop(reader);
    drop(_reader_control);
    drop(reader_adapter);
    drop(reader_gateway);
    drop(reopened);

    let path = source.to_str().unwrap();
    fs::copy(&source, &peer).unwrap();
    let peer_path = peer.to_str().unwrap();
    for (args, operation) in [
        (vec!["rel", "info", path], "info"),
        (vec!["rel", "verify", path], "verify"),
        (
            vec!["rel", "history", "archive", path, "--limit", "8"],
            "history_archive",
        ),
        (vec!["rel", "history", "proof", path], "history_proof"),
        (
            vec!["rel", "history", "memory", path, memory_id.as_str()],
            "history_memory",
        ),
    ] {
        let result = cli(&args);
        assert!(
            result.status.success(),
            "{args:?}: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        let report = json(&result);
        assert_eq!(report["operation"], operation, "{report}");
    }
    let history = json(&cli(&["rel", "history", "memory", path, &memory_id]));
    assert_eq!(history["result"]["status"], "available");
    let archive = json(&cli(&["rel", "history", "archive", path, "--limit", "32"]));
    assert_eq!(archive["page"]["total"], 8);
    assert_eq!(archive["page"]["items"].as_array().unwrap().len(), 8);
    // The history page reports the selected branch path for each conversation.
    // Lantern has two forks, so its eight selected paths contain 30 turns while
    // the owner statistics and replay witness retain all 32 accepted turns.
    assert_eq!(
        archive["page"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|item| item["turn_count"].as_u64().unwrap())
            .sum::<u64>(),
        30
    );
    let public_reopen = Rel::open(&source).unwrap();
    assert_eq!(
        public_reopen.current_historical_proof().unwrap().digest,
        json(&cli(&["rel", "history", "proof", path]))["proof"]["digest"]
            .as_str()
            .unwrap()
    );

    fs::write(
        &policy_path,
        serde_json::to_vec(&RewritePolicy::default()).unwrap(),
    )
    .unwrap();
    let policy = policy_path.to_str().unwrap();
    let plan = cli(&[
        "rel",
        "rewrite",
        path,
        dry_run.to_str().unwrap(),
        "--policy",
        policy,
        "--dry-run",
    ]);
    assert!(
        plan.status.success(),
        "{}",
        String::from_utf8_lossy(&plan.stderr)
    );
    assert_eq!(json(&plan)["operation"], "rewrite_plan");
    assert!(!dry_run.exists());

    let copied = cli(&["rel", "reconcile", path, peer_path, copy.to_str().unwrap()]);
    assert!(
        copied.status.success(),
        "{}",
        String::from_utf8_lossy(&copied.stderr)
    );
    assert_eq!(json(&copied)["operation"], "reconcile");
    assert_eq!(fs::read(&source).unwrap(), before);
    let copied_rel = Rel::open(&copy).unwrap();
    let copied_gateway = Gateway::new();
    let copied_owner = copied_gateway.mount_rel(&copied_rel, false).unwrap();
    let copied_token = "g4-copy-view-token-000000000000000000000000000000";
    let (copied_admission, _) = TrustedHostAdmission::new([HostCredentialBinding::new(
        "copy-viewer",
        copied_token,
        [OwnerGrant::ViewRel(copied_owner)],
    )])
    .install(&copied_gateway)
    .unwrap();
    let copied_management = copied_admission
        .authenticate(copied_token)
        .unwrap()
        .workspace_management(copied_owner)
        .unwrap();
    assert_eq!(
        copied_management.memories(0, 8).unwrap().items[0].id,
        memory_id
    );
    drop(copied_management);
    drop(copied_gateway);
    drop(copied_rel);
    let collision = cli(&["rel", "reconcile", path, peer_path, copy.to_str().unwrap()]);
    assert!(!collision.status.success());
    assert!(json(&collision)["error"].is_object());
    assert_eq!(fs::read(&source).unwrap(), before);

    let phy_path = temp.path("wrong-kind.phy");
    reliquary_memory::Phylactery::create(&phy_path).unwrap();
    for args in [
        vec!["cva", "info", path],
        vec!["phy", "info", path],
        vec!["rel", "info", path, "--unknown"],
        vec!["rel", "verify", phy_path.to_str().unwrap()],
    ] {
        let result = cli(&args);
        assert!(!result.status.success(), "unexpected success: {args:?}");
        assert!(json(&result)["error"].is_object());
        assert_eq!(fs::read(&source).unwrap(), before);
    }

    let legacy = temp.path("legacy-2.0.rel");
    fs::write(&legacy, b"FRS07\0\x02\x00G4 synthetic legacy reject case").unwrap();
    let rejected = cli(&["rel", "info", legacy.to_str().unwrap()]);
    assert!(!rejected.status.success());
    assert!(json(&rejected)["error"].is_object());
    assert_eq!(
        fs::read(&legacy).unwrap(),
        b"FRS07\0\x02\x00G4 synthetic legacy reject case"
    );

    let duplicate = cli(&["rel", "info", path, "--unknown", "x", "--unknown", "y"]);
    assert!(!duplicate.status.success());
    assert_eq!(fs::read(&source).unwrap(), before);
}
