use crate::entity_principal::{PRINCIPAL_ENTITY_KIND, principal_entity_id};
use crate::runtime_host_perception_test_support::{
    wait_phylactery_entities, wait_phylactery_revisions,
};
use crate::runtime_host_test_support::{
    BlockingEndpoint, OmitEndpoint, UserMemoryEndpoint, one_worker, queue_memory_episode,
    test_path, wait_complete, wait_phylactery_memory, wait_phylactery_vectors,
};
use crate::{
    Cva, EchoEvent, EchoEventKind, EntityDraft, EpisodeBoundary, EpisodeConfig, EpisodePolicy,
    InteractionRole, InteractionRuntime, Phylactery, ReliquaryRuntimeHost, ReliquaryRuntimeRoutes,
    SimulatedEmbeddingEndpoint, VectorNormalization,
};
use std::sync::{Arc, Barrier};

#[test]
fn inference_does_not_hold_the_interaction_runtime_lock() {
    let mut cva = Cva::create(test_path("concurrent.cva")).unwrap();
    cva.append_node(
        "u0".into(),
        "import".into(),
        None,
        "user".into(),
        1,
        "No durable memory here.",
    )
    .unwrap();
    cva.finalize_import_path_and_queue("import", "u0", EpisodeConfig::default(), 2)
        .unwrap();

    let entered = Arc::new(Barrier::new(2));
    let release = Arc::new(Barrier::new(2));
    let endpoint = Arc::new(BlockingEndpoint {
        entered: Arc::clone(&entered),
        release: Arc::clone(&release),
    });
    let host = ReliquaryRuntimeHost::start(
        InteractionRuntime::new(cva),
        ReliquaryRuntimeRoutes::new(Some(endpoint), None, None, None, None),
        one_worker(),
        EpisodePolicy {
            episode: EpisodeConfig::default(),
            inactivity_ns: i64::MAX,
        },
    )
    .unwrap();

    entered.wait();
    host.open_session("live".into(), None).unwrap();
    host.begin_message("live", "u1".into(), InteractionRole::User, 3)
        .unwrap();
    host.append_text("live", "u1", "The interface remains usable.")
        .unwrap();
    host.complete_message("live", "u1").unwrap();
    release.wait();

    wait_complete(&host, 1);
    assert_eq!(host.into_cva().unwrap().insomnia_stats().complete, 1);
}

#[test]
fn live_completion_wakes_idle_insomnia_workers() {
    let cva = Cva::create(test_path("wake.cva")).unwrap();
    let policy = EpisodePolicy {
        episode: EpisodeConfig { max_input_bytes: 1 },
        inactivity_ns: 60_000_000_000,
    };
    let host = ReliquaryRuntimeHost::start(
        InteractionRuntime::new(cva),
        ReliquaryRuntimeRoutes::new(Some(Arc::new(OmitEndpoint)), None, None, None, None),
        one_worker(),
        policy,
    )
    .unwrap();

    host.open_session("live".into(), None).unwrap();
    host.begin_message("live", "u1".into(), InteractionRole::User, 1)
        .unwrap();
    host.append_text("live", "u1", "Question").unwrap();
    host.complete_message("live", "u1").unwrap();
    host.begin_message("live", "a1".into(), InteractionRole::Agent, 2)
        .unwrap();
    host.append_text("live", "a1", "Answer").unwrap();
    host.complete_live_message("live", "a1", policy, 3).unwrap();

    wait_complete(&host, 1);
    assert!(host.insomnia_stats().unwrap().complete >= 1);
    host.into_cva().unwrap();
}

#[test]
fn runtime_host_close_session_finalizes_nonempty_tail_explicitly() {
    let cva = Cva::create(test_path("close-explicit.cva")).unwrap();
    let host = ReliquaryRuntimeHost::start_inactive(
        InteractionRuntime::new(cva),
        ReliquaryRuntimeRoutes::default(),
        one_worker(),
        EpisodePolicy::default(),
    )
    .unwrap();

    host.open_session("live".into(), None).unwrap();
    host.begin_message("live", "u1".into(), InteractionRole::User, 1)
        .unwrap();
    host.append_text("live", "u1", "Question").unwrap();
    host.complete_message("live", "u1").unwrap();
    host.begin_message("live", "a1".into(), InteractionRole::Agent, 2)
        .unwrap();
    host.append_text("live", "a1", "Answer").unwrap();
    host.complete_message("live", "a1").unwrap();

    host.close_session("live").unwrap();
    assert_eq!(host.insomnia_stats().unwrap().pending, 1);
    let status = host.background_status().unwrap();
    assert!(!status.quiescent());
    assert_eq!(status.insomnia.pending, 1);
    let cva = host.into_cva().unwrap();
    let episodes = cva.episodes_for_conversation("live");
    assert_eq!(episodes.len(), 1);
    assert_eq!(episodes[0].boundary, EpisodeBoundary::Explicit);
}

#[test]
fn runtime_host_persists_echo_inside_rel() {
    let path = test_path("echo-runtime.prj.rel");
    let cva = Cva::create_project(&path).unwrap();
    let host = ReliquaryRuntimeHost::start_inactive(
        InteractionRuntime::new(cva),
        ReliquaryRuntimeRoutes::default(),
        one_worker(),
        EpisodePolicy::default(),
    )
    .unwrap();
    let event = EchoEvent {
        conversation_id: "conversation".into(),
        message_id: "assistant".into(),
        sequence: 0,
        timestamp_ns: 1,
        model_round: Some(1),
        kind: EchoEventKind::ReasoningTrace,
        correlation_id: None,
        name: None,
        content: "raw trace".into(),
    };

    assert!(host.put_echo_event(event).unwrap());
    assert_eq!(
        host.echo_events("conversation", "assistant").unwrap()[0].content,
        "raw trace"
    );
    drop(host);

    let reopened = Cva::open(path).unwrap();
    assert_eq!(
        reopened.echo_events("conversation", "assistant")[0].content,
        "raw trace"
    );
}

#[test]
fn runtime_host_profile_updates_matching_rel_principal_on_attach_and_rename() {
    let mut cva = Cva::create_project(test_path("principal-profile.prj.rel")).unwrap();
    let mut phylactery = Phylactery::create(test_path("principal-profile.phy")).unwrap();
    let principal_id = phylactery.owner_id().unwrap();
    phylactery.set_profile(Some("Example User".into())).unwrap();
    phylactery.sync().unwrap();

    let entity_id = principal_entity_id(&principal_id);
    cva.publish_entity(
        Some(entity_id),
        0,
        EntityDraft {
            canonical_name: principal_id.clone(),
            aliases: Vec::new(),
            kind: PRINCIPAL_ENTITY_KIND.into(),
            summary: "Durable Phylactery-backed user principal.".into(),
            mutation_id: "runtime-principal-profile".into(),
            created_at_ns: 0,
            updated_at_ns: 0,
        },
    )
    .unwrap();

    let host = ReliquaryRuntimeHost::start_inactive(
        InteractionRuntime::new(cva),
        ReliquaryRuntimeRoutes::default(),
        one_worker(),
        EpisodePolicy::default(),
    )
    .unwrap();
    host.attach_phylactery(phylactery).unwrap();

    let attached = host.phylactery_profile().unwrap().unwrap();
    assert_eq!(attached.display_name.as_deref(), Some("Example User"));

    assert!(
        host.set_phylactery_profile(Some("Example Renamed".into()))
            .unwrap()
    );

    let (cva, phylactery) = host.into_cva_and_phylactery().unwrap();
    let entity = cva.entity(entity_id).unwrap();
    assert_eq!(entity.id, entity_id);
    assert_eq!(entity.canonical_name, principal_id);
    assert_eq!(entity.aliases, vec!["Example Renamed"]);
    let profile = phylactery.unwrap().profile();
    assert_eq!(profile.display_name.as_deref(), Some("Example Renamed"));
}

#[test]
fn runtime_host_captures_phy_principal_and_agent_inherits_parent() {
    let rel_path = test_path("principal-capture.prj.rel");
    let cva = Cva::create_project(&rel_path).unwrap();
    let phylactery_a = Phylactery::create(test_path("principal-a.phy")).unwrap();
    let principal_a = phylactery_a.owner_id().unwrap();
    let host = ReliquaryRuntimeHost::start_with_phylactery(
        InteractionRuntime::new(cva),
        phylactery_a,
        ReliquaryRuntimeRoutes::default(),
        one_worker(),
        EpisodePolicy::default(),
    )
    .unwrap();

    host.open_session("live".into(), None).unwrap();
    host.begin_message("live", "u1".into(), InteractionRole::User, 1)
        .unwrap();
    host.append_text("live", "u1", "Question").unwrap();
    host.complete_message("live", "u1").unwrap();

    let _phylactery_a = host.detach_phylactery().unwrap().unwrap();
    let phylactery_b = Phylactery::create(test_path("principal-b.phy")).unwrap();
    let principal_b = phylactery_b.owner_id().unwrap();
    assert_ne!(principal_a, principal_b);
    host.attach_phylactery(phylactery_b).unwrap();

    host.begin_message("live", "a1".into(), InteractionRole::Agent, 2)
        .unwrap();
    host.append_text("live", "a1", "Answer").unwrap();
    host.complete_message("live", "a1").unwrap();
    host.begin_message("live", "u2".into(), InteractionRole::User, 3)
        .unwrap();
    host.append_text("live", "u2", "Second user turn").unwrap();
    host.complete_message("live", "u2").unwrap();

    let transcript = host.conversation_transcript("live", "u2").unwrap();
    assert_eq!(transcript.len(), 3);
    assert_eq!(
        transcript[0].principal_id.as_deref(),
        Some(principal_a.as_str())
    );
    assert_eq!(
        transcript[1].principal_id.as_deref(),
        Some(principal_a.as_str())
    );
    assert_eq!(
        transcript[2].principal_id.as_deref(),
        Some(principal_b.as_str())
    );

    let (cva, _) = host.into_cva_and_phylactery().unwrap();
    drop(cva);
    let mut reopened = Cva::open_project(rel_path).unwrap();
    let turns = reopened.conversation_turns("live", "u2").unwrap();
    assert_eq!(turns[0].principal_id.as_deref(), Some(principal_a.as_str()));
    assert_eq!(turns[1].principal_id.as_deref(), Some(principal_a.as_str()));
    assert_eq!(turns[2].principal_id.as_deref(), Some(principal_b.as_str()));
}

#[test]
fn runtime_host_routes_user_memory_and_vectors_to_attached_phylactery() {
    let mut cva = Cva::create_project(test_path("routed.prj.rel")).unwrap();
    queue_memory_episode(&mut cva);
    let phylactery = Phylactery::create(test_path("routed.phy")).unwrap();
    let phy_owner = phylactery.owner_id().unwrap();
    let host = ReliquaryRuntimeHost::start_with_phylactery(
        InteractionRuntime::new(cva),
        phylactery,
        ReliquaryRuntimeRoutes::new(
            Some(Arc::new(UserMemoryEndpoint)),
            None,
            None,
            None,
            Some(Arc::new(SimulatedEmbeddingEndpoint::new(
                8,
                VectorNormalization::L2,
                7,
            ))),
        )
        .with_entity_routes(
            Some(Arc::new(UserMemoryEndpoint)),
            Some(Arc::new(UserMemoryEndpoint)),
        ),
        one_worker(),
        EpisodePolicy {
            episode: EpisodeConfig::default(),
            inactivity_ns: i64::MAX,
        },
    )
    .unwrap();

    wait_complete(&host, 1);
    wait_phylactery_memory(&host, 1);
    wait_phylactery_vectors(&host, 1);
    wait_phylactery_revisions(&host, 2);
    wait_phylactery_entities(&host, 1);
    assert_eq!(host.memory_stats().unwrap().memories, 0);
    assert_eq!(
        host.phylactery_owner_id().unwrap().as_deref(),
        Some(phy_owner.as_str())
    );

    let (cva, phylactery) = host.into_cva_and_phylactery().unwrap();
    let mut phylactery = phylactery.unwrap();
    assert_eq!(phylactery.memory_stats().memories, 1);
    assert_eq!(phylactery.memory_vector_stats().bindings, 1);
    let episode = cva.episodes().into_iter().next().unwrap();
    let attempt = &cva.insomnia_attempts(episode.id)[0];
    assert!(attempt.memory_ids.is_empty());
    assert_eq!(attempt.external_memory_refs.len(), 1);
    assert_eq!(attempt.external_memory_refs[0].owner_id, phy_owner);
    let memory = phylactery
        .memory(attempt.external_memory_refs[0].memory_id)
        .unwrap();
    assert_eq!(memory.source_episode_id, None);
    assert!(memory.source_time_ns.is_some());
    assert_eq!(memory.lifecycle_state, "canonical");
    let metadata = phylactery.memory_routing_metadata(memory.id).unwrap();
    let key = crate::MemoryEntityMentionKey::new(memory.id, &metadata.entity_mentions[0]);
    assert!(matches!(
        phylactery.entity_resolution(key).unwrap().status,
        crate::MemoryEntityResolutionStatus::Resolved { .. }
    ));
}
