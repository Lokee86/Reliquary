use super::*;
use reliquary_memory::{
    ConversationSubmissionId, EpisodeBoundary, FinalizeCapture, ImportStage,
    LiveProcessingSelection, StageState, SubmissionId,
};
use std::sync::atomic::AtomicUsize;

#[test]
fn live_processing_recovers_exact_rel_and_phy_work_after_reopen() {
    let fail_entity = Arc::new(AtomicBool::new(true));
    let entity_calls = Arc::new(AtomicUsize::new(0));
    let fixture = Fixture::start_with({
        let fail_entity = fail_entity.clone();
        let entity_calls = entity_calls.clone();
        move |request, changed_profile| {
            let schema = request["response_format"]["json_schema"]["name"].as_str();
            let message =
                |value: Value| json!({"choices":[{"message":{"content":value.to_string()}}]});
            if schema == Some("entity_resolution") {
                assert_eq!(request["model"], "entity-live");
                entity_calls.fetch_add(1, Ordering::Relaxed);
                if fail_entity.load(Ordering::Acquire) {
                    return message(json!({"decision":"invalid"}));
                }
                return message(json!({
                    "decision":"unresolved","reason":"ambiguous","target_candidate_index":0,
                    "identity_relation":"uncertain","mention_kind":"unknown"
                }));
            }
            let mut reply = fixture_reply(request, changed_profile);
            if schema == Some("insomnia_memory_wording") {
                let mut result: Value = serde_json::from_str(
                    reply["choices"][0]["message"]["content"].as_str().unwrap(),
                )
                .unwrap();
                let candidates = result["candidates"].as_array_mut().unwrap();
                candidates[0]["routing_mentions"] = json!([{
                    "field":"content","start_byte":0,"end_byte":7,"text":"Lantern"
                }]);
                candidates[1]["title"] = json!("Meetup window");
                candidates[1]["routing_mentions"] = json!([{
                    "field":"title","start_byte":0,"end_byte":6,"text":"Meetup"
                }]);
                reply = message(result);
            }
            reply
        }
    });
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(".test-tmp")
        .join(format!(
            "operational-live-{}-{}",
            std::process::id(),
            fixture.listener.port()
        ));
    fs::create_dir_all(&root).unwrap();
    let rel_path = root.join("project.rel");
    let phy_path = root.join("user.phy");
    let url = format!("http://{}", fixture.listener);
    let full = configured_fixture(
        &root.join("full.cfg"),
        &url,
        &[
            (ModelSlot::General, "fixture"),
            (ModelSlot::EntityResolution, "entity-live"),
        ],
        true,
    );
    let entity_only = configured_fixture(
        &root.join("entity.cfg"),
        &url,
        &[(ModelSlot::EntityResolution, "entity-live")],
        false,
    );
    let dream_only = configured_fixture(
        &root.join("dream.cfg"),
        &url,
        &[(ModelSlot::Dream, "dream-live")],
        true,
    );

    let rel = Rel::create(&rel_path, OwnerScope::Project).unwrap();
    let phy = Phylactery::create(&phy_path).unwrap();
    let gateway = Gateway::new();
    let owner = gateway.mount_rel(&rel, true).unwrap();
    let destination = gateway.mount_phylactery(&phy, true).unwrap();
    let token = "live-fixture-operator-credential";
    let viewer_token = "live-fixture-viewer-admission-credential";
    let (admission, _) = TrustedHostAdmission::new([
        HostCredentialBinding::new(
            "operator",
            token,
            [
                OwnerGrant::ManageRel(owner),
                OwnerGrant::OperateRel(owner),
                OwnerGrant::OperatePhylactery(destination),
            ],
        ),
        HostCredentialBinding::new("viewer", viewer_token, [OwnerGrant::ViewRel(owner)]),
    ])
    .install(&gateway)
    .unwrap();
    let instance = admission.authenticate(token).unwrap();
    let viewer = admission.authenticate(viewer_token).unwrap();
    let mut queue = instance
        .open_conversation_queue(root.join("queue"), owner)
        .unwrap();
    let submission = SubmissionId::new("accepted").unwrap();
    let text = format!("{PROJECT} {PRIVATE}");
    instance
        .submit_conversation(&mut queue, "chat", submission.clone(), &text, 0)
        .unwrap();
    let revision = instance.conversation_revision(&queue, "chat").unwrap();
    instance
        .finalize_conversation_submission(
            &mut queue,
            &ConversationSubmissionId::new("chat", submission).unwrap(),
            revision,
            FinalizeCapture {
                branch_id: "main".into(),
                parent_node_id: None,
                committed_timestamp_ns: 10,
                verified_principal_id: "operator".into(),
            },
        )
        .unwrap();
    instance
        .finalize_conversation_episode(
            owner,
            "chat",
            "accepted",
            "accepted",
            EpisodeBoundary::Explicit,
            11,
        )
        .unwrap();
    drop(queue);
    let options = ProcessOptions {
        import_id: None,
        destination: Some(destination),
        max_episodes: 1,
    };

    assert!(
        instance
            .process_and_drain(owner, &full, options.clone())
            .is_err()
    );
    assert_eq!(entity_calls.load(Ordering::Relaxed), 1);
    assert_eq!(
        viewer
            .live_stage_status(owner, None, ImportStage::Entity)
            .unwrap()
            .pending,
        1
    );
    assert_eq!(
        viewer
            .live_stage_status(owner, Some(destination), ImportStage::Entity)
            .unwrap_err()
            .kind(),
        ProductErrorKind::Unauthorized,
    );
    let management = instance.workspace_management(owner).unwrap();
    management.quiesce().unwrap();
    assert_eq!(management.verify_quiesced().unwrap().memory_count, 1);
    management.reopen().unwrap();

    fail_entity.store(false, Ordering::Release);
    let run = |runtime, stage| {
        instance
            .start_live_processing_stage(owner, runtime, Some(destination), 1, stage)
            .unwrap()
            .join()
            .unwrap()
    };
    let entity = run(&entity_only, LiveProcessingSelection::Entity);
    assert_eq!(entity.state, ProcessingState::Incomplete, "{entity:?}");
    let entity = run(&entity_only, LiveProcessingSelection::Entity);
    assert_eq!(entity.state, ProcessingState::Ready, "{entity:?}");
    assert_eq!(entity.live_stages[0].completed, 2);
    assert_eq!(entity_calls.load(Ordering::Relaxed), 3);

    let dream = run(&dream_only, LiveProcessingSelection::Dream);
    assert_eq!(dream.state, ProcessingState::Incomplete, "{dream:?}");
    let dream = run(&dream_only, LiveProcessingSelection::Dream);
    assert_eq!(dream.state, ProcessingState::Ready, "{dream:?}");
    assert_eq!(dream.insomnia.rel_memories, 1);
    assert_eq!(dream.insomnia.phy_memories, 1);
    assert_eq!(dream.live_stages.last().unwrap().completed, 2);
    assert_eq!(
        dream.live_stages.last().unwrap().state,
        StageState::Complete
    );

    let before = instance
        .phylactery_info(destination)
        .unwrap()
        .memory_version;
    management.quiesce().unwrap();
    let rel_bytes = fs::read(&rel_path).unwrap();
    management.reopen().unwrap();
    let replay = instance.process_and_drain(owner, &full, options).unwrap();
    assert_eq!(replay.state, ProcessingState::Ready, "{replay:?}");
    assert_eq!(
        before,
        instance
            .phylactery_info(destination)
            .unwrap()
            .memory_version
    );
    management.quiesce().unwrap();
    assert_eq!(rel_bytes, fs::read(&rel_path).unwrap());
    assert_eq!(entity_calls.load(Ordering::Relaxed), 3);
    drop((management, viewer, instance, admission, gateway, rel, phy));
    let rel_info = Rel::verify(&rel_path).unwrap();
    let phy_info = Phylactery::verify(&phy_path).unwrap();
    assert_eq!((rel_info.memory_count, phy_info.memory_count), (1, 1));
    assert_eq!((rel_info.graph_version, phy_info.graph_version), (0, 0));
    fs::remove_dir_all(root).unwrap();
}
