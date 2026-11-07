use reliquary_memory::{
    ConversationSubmissionId, FinalizeCapture, Gateway, HostCredentialBinding, OwnerGrant,
    OwnerKind, OwnerScope, Phylactery, ProductErrorKind, QueueState, Rel, SubmissionId,
    TrustedHostAdmission,
};
use serde_json::Value;
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(1);

struct TestPaths(PathBuf);

impl TestPaths {
    fn new() -> Self {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(".test-tmp")
            .join(format!(
                "g4-public-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn owner(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for TestPaths {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn public_product_creates_and_reopens_isolated_rel_and_phy_owners() {
    let paths = TestPaths::new();
    let rel_paths = [paths.owner("rel-a"), paths.owner("rel-b")];
    let phy_paths = [paths.owner("phy-a"), paths.owner("phy-b")];

    let rel_ids = [
        Rel::create(&rel_paths[0], OwnerScope::Project)
            .unwrap()
            .owner_id(),
        Rel::create(&rel_paths[1], OwnerScope::Connection)
            .unwrap()
            .owner_id(),
    ];
    let phy_ids = [
        Phylactery::create(&phy_paths[0]).unwrap().owner_id(),
        Phylactery::create(&phy_paths[1]).unwrap().owner_id(),
    ];

    assert_ne!(rel_ids[0], rel_ids[1]);
    assert_ne!(phy_ids[0], phy_ids[1]);
    for rel_id in rel_ids {
        assert!(!phy_ids.contains(&rel_id));
    }

    let rel_bytes: Vec<_> = rel_paths
        .iter()
        .map(fs::read)
        .collect::<Result<_, _>>()
        .unwrap();
    let phy_bytes: Vec<_> = phy_paths
        .iter()
        .map(fs::read)
        .collect::<Result<_, _>>()
        .unwrap();

    for (path, expected_id) in rel_paths.iter().zip(rel_ids) {
        let reopened = Rel::open(path).unwrap();
        let info = reopened.info().unwrap();
        assert_eq!(info.format, "2.1");
        assert_eq!(info.kind, OwnerKind::Rel);
        assert_eq!(info.owner_id, expected_id);
        assert!(info.archive_version.is_some());
        assert!(info.rel_turn_count.is_some());
    }
    for (path, expected_id) in phy_paths.iter().zip(phy_ids) {
        let reopened = Phylactery::open(path).unwrap();
        let info = reopened.info().unwrap();
        assert_eq!(info.format, "2.1");
        assert_eq!(info.kind, OwnerKind::Phylactery);
        assert_eq!(info.owner_id, expected_id);
        assert!(info.archive_version.is_none());
        assert!(info.rel_turn_count.is_none());
    }

    for path in &phy_paths {
        assert_eq!(
            Rel::open(path).err().unwrap().kind(),
            ProductErrorKind::WrongFormat
        );
    }
    for path in &rel_paths {
        assert_eq!(
            Phylactery::open(path).err().unwrap().kind(),
            ProductErrorKind::WrongFormat
        );
    }

    assert_eq!(
        rel_paths
            .iter()
            .map(fs::read)
            .collect::<Result<Vec<_>, _>>()
            .unwrap(),
        rel_bytes
    );
    assert_eq!(
        phy_paths
            .iter()
            .map(fs::read)
            .collect::<Result<Vec<_>, _>>()
            .unwrap(),
        phy_bytes
    );
}

#[test]
fn authored_manifest_replays_48_turns_and_three_forks_through_public_gateway() {
    let manifest: Value =
        serde_json::from_str(include_str!("fixtures/g4-native-rel21-v1.json")).unwrap();
    let paths = TestPaths::new();
    let lantern_path = paths.owner("rel-lantern.rel");
    let cascade_path = paths.owner("rel-cascade.rel");
    let phy_a_path = paths.owner("phy-reader-a.phy");
    let phy_b_path = paths.owner("phy-reader-b.phy");
    let lantern = Rel::create(&lantern_path, OwnerScope::Project).unwrap();
    let cascade = Rel::create(&cascade_path, OwnerScope::Connection).unwrap();
    let phy_a = Phylactery::create(&phy_a_path).unwrap();
    let phy_b = Phylactery::create(&phy_b_path).unwrap();
    let captured_ids = [
        lantern.owner_id(),
        cascade.owner_id(),
        phy_a.owner_id(),
        phy_b.owner_id(),
    ];

    let gateway = Gateway::new();
    let lantern_owner = gateway.mount_rel(&lantern, true).unwrap();
    let cascade_owner = gateway.mount_rel(&cascade, true).unwrap();
    let phy_a_owner = gateway.mount_phylactery(&phy_a, false).unwrap();
    let phy_b_owner = gateway.mount_phylactery(&phy_b, false).unwrap();
    let operator_token = "g4-operator-token-00000000000000000000000000000000";
    let competing_token = "g4-competing-token-000000000000000000000000000000";
    let (adapter, _) = TrustedHostAdmission::new([
        HostCredentialBinding::new(
            "operator",
            operator_token,
            [
                OwnerGrant::OperateRel(lantern_owner),
                OwnerGrant::ManageRel(lantern_owner),
                OwnerGrant::OperateRel(cascade_owner),
                OwnerGrant::ManageRel(cascade_owner),
                OwnerGrant::ViewPhylactery(phy_a_owner),
                OwnerGrant::ViewPhylactery(phy_b_owner),
            ],
        ),
        HostCredentialBinding::new(
            "outsider",
            competing_token,
            [
                OwnerGrant::OperateRel(lantern_owner),
                OwnerGrant::OperateRel(cascade_owner),
            ],
        ),
    ])
    .install(&gateway)
    .unwrap();
    let operator = adapter.authenticate(operator_token).unwrap();
    let outsider = adapter.authenticate(competing_token).unwrap();
    let mut lantern_queue = operator
        .open_conversation_queue(paths.owner("lantern-queue"), lantern_owner)
        .unwrap();
    let mut cascade_queue = operator
        .open_conversation_queue(paths.owner("cascade-queue"), cascade_owner)
        .unwrap();

    let scripts = manifest["conversation_scripts"].as_array().unwrap();
    assert_eq!(scripts.len(), 12);
    assert_eq!(manifest["cohort"]["authored_accepted_native_turns"], 48);
    let expected_forks = scripts
        .iter()
        .filter(|script| !script["branches"].as_array().unwrap().is_empty())
        .count();
    assert_eq!(expected_forks, 3);
    let mut accepted_turns = 0;
    for (script_index, script) in scripts.iter().enumerate() {
        let conversation = script["conversation_id"].as_str().unwrap();
        let owner_alias = script["owner"].as_str().unwrap();
        let (owner, queue) = if owner_alias == "rel-lantern" {
            (lantern_owner, &mut lantern_queue)
        } else {
            (cascade_owner, &mut cascade_queue)
        };
        let branch = script["branches"]
            .as_array()
            .and_then(|items| items.first());
        let turns = script["turns"].as_array().unwrap();
        accepted_turns += turns.iter().filter(|turn| turn["accepted"] == true).count();
        let mut revision = 0_u64;
        for (turn_index, turn) in turns.iter().enumerate() {
            let node = turn["id"].as_str().unwrap();
            let submission = SubmissionId::new(node).unwrap();
            let id = ConversationSubmissionId::new(conversation, submission.clone()).unwrap();
            operator
                .submit_conversation(
                    queue,
                    conversation,
                    submission,
                    turn["content"].as_str().unwrap(),
                    revision,
                )
                .unwrap();
            revision += 1;
            if script_index == 0 && turn_index == 0 {
                assert_eq!(
                    outsider
                        .finalize_conversation_submission(
                            queue,
                            &id,
                            revision,
                            FinalizeCapture {
                                branch_id: "main".into(),
                                parent_node_id: None,
                                committed_timestamp_ns: 1,
                                verified_principal_id: "outsider".into(),
                            },
                        )
                        .unwrap_err()
                        .kind(),
                    ProductErrorKind::Unauthorized
                );
            }
            let (branch_id, parent) = manifest_branch(turns, branch, turn_index, node);
            let status = operator
                .finalize_conversation_submission(
                    queue,
                    &id,
                    revision,
                    FinalizeCapture {
                        branch_id,
                        parent_node_id: parent,
                        committed_timestamp_ns: 1_000 + (script_index * 10 + turn_index) as i64,
                        verified_principal_id: "operator".into(),
                    },
                )
                .unwrap();
            assert_eq!(status.state, QueueState::Committed);
            revision += 2;
            assert_eq!(owner, queue.owner_id());
        }
    }
    assert_eq!(accepted_turns, 48);

    let provisional_submission = SubmissionId::new("provisional-only").unwrap();
    let provisional_id =
        ConversationSubmissionId::new("conv-provisional", provisional_submission.clone()).unwrap();
    operator
        .submit_conversation(
            &mut cascade_queue,
            "conv-provisional",
            provisional_submission,
            "private provisional host text",
            0,
        )
        .unwrap();
    assert_eq!(
        operator
            .cancel_own_pending_submission(&mut cascade_queue, &provisional_id, 1)
            .unwrap()
            .state,
        QueueState::Cancelled
    );

    let lantern_management = operator.workspace_management(lantern_owner).unwrap();
    let cascade_management = operator.workspace_management(cascade_owner).unwrap();
    assert_eq!(lantern_management.statistics().unwrap().conversations, 8);
    assert_eq!(lantern_management.statistics().unwrap().turns, 32);
    assert_eq!(cascade_management.statistics().unwrap().conversations, 4);
    assert_eq!(cascade_management.statistics().unwrap().turns, 16);
    assert!(
        cascade_management
            .conversations(0, 32)
            .unwrap()
            .items
            .iter()
            .all(|item| item.conversation_id != "conv-provisional")
    );

    // Read every manifest branch through the public bounded transcript API.
    // This independently checks exact accepted bodies, source node IDs,
    // branch leaves, and canonical selection after the queue writes.
    for (script_index, script) in scripts.iter().enumerate() {
        let owner_alias = script["owner"].as_str().unwrap();
        let management = if owner_alias == "rel-lantern" {
            &lantern_management
        } else {
            &cascade_management
        };
        let conversation_id = script["conversation_id"].as_str().unwrap();
        let conversation = management
            .conversations(0, 32)
            .unwrap()
            .items
            .into_iter()
            .find(|item| item.conversation_id == conversation_id)
            .unwrap_or_else(|| panic!("missing {conversation_id}"));
        let turns = script["turns"].as_array().unwrap();
        let fork = script["branches"].as_array().unwrap().first();
        if let Some(fork) = fork {
            let children = fork["child_turn_ids"].as_array().unwrap();
            let canonical_id = fork["canonical_child_id"].as_str().unwrap();
            management
                .select_conversation(conversation_id, canonical_id)
                .unwrap();
            let canonical_transcript = management.transcript(0, 32).unwrap();
            assert_eq!(
                canonical_transcript.items.last().unwrap().node_id,
                canonical_id
            );
            for child in children {
                let leaf = child.as_str().unwrap();
                management
                    .select_conversation(conversation_id, leaf)
                    .unwrap();
                let transcript = management.transcript(0, 32).unwrap();
                let leaf_turn_index = turns.iter().position(|turn| turn["id"] == leaf).unwrap();
                let parent_id = fork["parent_turn_id"].as_str().unwrap();
                let parent_index = turns
                    .iter()
                    .position(|turn| turn["id"] == parent_id)
                    .unwrap();
                let expected_indexes: Vec<usize> = if leaf_turn_index > parent_index {
                    (0..=parent_index).chain([leaf_turn_index]).collect()
                } else {
                    (0..=leaf_turn_index).collect()
                };
                assert_eq!(transcript.items.len(), expected_indexes.len());
                for (actual, expected_index) in transcript.items.iter().zip(expected_indexes) {
                    let expected = &turns[expected_index];
                    assert_eq!(actual.node_id, expected["id"].as_str().unwrap());
                    assert_eq!(actual.content, expected["content"].as_str().unwrap());
                    // This public queue smoke submits each authored text as a
                    // user message. The joined cohort and external consumer
                    // separately verify the manifest's assistant reply roles.
                    assert_eq!(actual.role, "user");
                }
            }
        } else {
            let leaf = turns.last().unwrap()["id"].as_str().unwrap();
            assert_eq!(conversation.branch.id, "main");
            assert_eq!(conversation.branch.leaf_node_id, leaf);
            assert!(conversation.branch.canonical);
            management
                .select_conversation(conversation_id, leaf)
                .unwrap();
            let transcript = management.transcript(0, 32).unwrap();
            assert_eq!(transcript.items.len(), turns.len());
            for (actual, expected) in transcript.items.iter().zip(turns) {
                assert_eq!(actual.node_id, expected["id"].as_str().unwrap());
                assert_eq!(actual.content, expected["content"].as_str().unwrap());
                assert_eq!(actual.role, "user");
            }
        }
        assert_eq!(script_index < 8, owner_alias == "rel-lantern");
    }

    drop(lantern_queue);
    drop(cascade_queue);
    lantern_management.quiesce().unwrap();
    cascade_management.quiesce().unwrap();
    let lantern_verified = lantern_management.verify_quiesced().unwrap();
    let cascade_verified = cascade_management.verify_quiesced().unwrap();
    assert_eq!(lantern_verified.rel_turn_count, Some(32));
    assert_eq!(cascade_verified.rel_turn_count, Some(16));
    assert_eq!(lantern_verified.owner_id, captured_ids[0]);
    assert_eq!(cascade_verified.owner_id, captured_ids[1]);
    lantern_management.reopen().unwrap();
    cascade_management.reopen().unwrap();
    assert_eq!(
        operator.phylactery_info(phy_a_owner).unwrap().owner_id,
        captured_ids[2]
    );
    assert_eq!(
        operator.phylactery_info(phy_b_owner).unwrap().owner_id,
        captured_ids[3]
    );
}

fn manifest_branch(
    turns: &[Value],
    branch: Option<&Value>,
    turn_index: usize,
    node: &str,
) -> (String, Option<String>) {
    let Some(branch) = branch else {
        return (
            "main".into(),
            turn_index
                .checked_sub(1)
                .map(|index| turns[index]["id"].as_str().unwrap().to_owned()),
        );
    };
    let children = branch["child_turn_ids"].as_array().unwrap();
    if children.iter().any(|child| child.as_str() == Some(node)) {
        let canonical = branch["canonical_child_id"].as_str().unwrap();
        let id = if node == canonical {
            "main".to_owned()
        } else {
            format!("{}-{node}", branch["id"].as_str().unwrap())
        };
        return (
            id,
            Some(branch["parent_turn_id"].as_str().unwrap().to_owned()),
        );
    }
    (
        "main".into(),
        turn_index
            .checked_sub(1)
            .map(|index| turns[index]["id"].as_str().unwrap().to_owned()),
    )
}
