use super::super::{InstanceRuntime, snapshots::RelSubscription, subscriptions::RelDelivery};
use super::*;
use crate::runtime_host_test_support::test_path;
use crate::{Cva, InteractionRuntime};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, mpsc};
use std::time::Duration;

fn setup(label: &str) -> (Arc<GatewayRuntime>, String, Arc<InstanceRuntime>) {
    let g = Arc::new(GatewayRuntime::new(
        test_path(&format!("{label}-state")),
        super::super::super::ReliquaryRuntimeRoutes::default(),
        crate::InsomniaWorkerConfig {
            workers: 0,
            ..Default::default()
        },
        crate::EpisodePolicy::default(),
    ));
    let rel = g
        .mount_rel(InteractionRuntime::new(
            Cva::create_project(test_path(&format!("{label}.rel"))).unwrap(),
        ))
        .unwrap();
    let instance = g
        .attach_instance(
            label.into(),
            None,
            BTreeMap::from([(
                rel.clone(),
                BTreeSet::from([
                    OwnerGrant::ViewRel,
                    OwnerGrant::ManageRel,
                    OwnerGrant::ManageProject,
                    OwnerGrant::SubmitConversation,
                ]),
            )]),
        )
        .unwrap();
    (g, rel, instance)
}
fn drain(
    g: &GatewayRuntime,
    i: &InstanceRuntime,
    s: &RelSubscription,
) -> Vec<super::super::events::RelEventEnvelope> {
    let mut events = Vec::new();
    while let Some(delivery) = g.next_rel_event(i, s).unwrap() {
        let RelDelivery::Event(event) = delivery else {
            panic!("unexpected overflow")
        };
        events.push((*event).clone());
    }
    events
}
#[test]
fn community_name_without_source_clock_change_still_invalidates_semantic_retrieval() {
    let (g, rel, i) = setup("p44-community");
    let a = create(&g, &i, &rel);
    let b = create(&g, &i, &rel);
    g.mutate_knowledge_relation_for(
        &i,
        &rel,
        a.id,
        b.id,
        None,
        Some(crate::GraphRelationKind::Topical),
    )
    .unwrap();
    let token = g.authorize(&i, &rel, OwnerGrant::ManageRel).unwrap();
    let community = g
        .with_rel_mutation(&token, RelMutationFamily::Semantic, |runtime| {
            let snapshot = runtime
                .cva
                .refresh_communities_leiden()
                .map_err(operation)?;
            Ok((snapshot.communities[0].id, false, vec![]))
        })
        .unwrap();
    let (before, s) = g.subscribe_rel(&i, &rel, None, 32).unwrap();
    g.rename_knowledge_community_for(&i, &rel, community, "NAME_SECRET".into())
        .unwrap();
    let events = drain(&g, &i, &s);
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].class, RelEventClass::SemanticOwnerChanged);
    assert_eq!(events[1].class, RelEventClass::RetrievalInvalidated);
    assert!(
        events
            .iter()
            .all(|e| e.watermarks == before.state.watermarks
                && e.invalidations == vec![RelInvalidation::Community(community)])
    );
    assert!(
        events
            .iter()
            .all(|e| !format!("{e:?}").contains("NAME_SECRET"))
    );
    g.rename_knowledge_community_for(&i, &rel, community, "NAME_SECRET".into())
        .unwrap();
    assert!(drain(&g, &i, &s).is_empty());
}
#[test]
fn project_upload_publishes_after_sync_and_failed_read_publishes_nothing() {
    let (g, _, _) = setup("p44-project");
    let project = test_path("p44-project-folder");
    std::fs::create_dir_all(&project).unwrap();
    let rel_path = test_path("p44-project-rel.rel");
    let cva = crate::project_environment::create_project_rel(
        &rel_path,
        &project,
        crate::ProjectRepositoryMode::ManualLore,
    )
    .unwrap();
    let rel = g.mount_rel(InteractionRuntime::new(cva)).unwrap();
    let i = g
        .attach_instance(
            "project".into(),
            None,
            BTreeMap::from([(
                rel.clone(),
                BTreeSet::from([OwnerGrant::ViewRel, OwnerGrant::ManageProject]),
            )]),
        )
        .unwrap();
    let (before, s) = g.subscribe_rel(&i, &rel, None, 32).unwrap();
    let source = project.join("PRIVATE_PATH_SECRET.txt");
    std::fs::write(&source, "FILE_BODY_SECRET").unwrap();
    let file = g
        .ingest_project_attachment_for(&i, &rel, &project, &source)
        .unwrap();
    let events = drain(&g, &i, &s);
    assert!(
        events
            .iter()
            .any(|e| e.class == RelEventClass::ArchiveChanged)
    );
    assert!(
        events
            .iter()
            .all(|e| e.watermarks.archive_version > before.state.watermarks.archive_version)
    );
    assert!(events.iter().all(
        |e| e.invalidations.contains(&RelInvalidation::File(file.id))
            && !format!("{e:?}").contains("SECRET")
    ));
    g.checkpoint_manual_project_for(&i, &rel, &project).unwrap();
    let events = drain(&g, &i, &s);
    let project_event = events
        .iter()
        .find(|e| e.class == RelEventClass::ProjectStateChanged)
        .expect("checkpoint correlation summary");
    assert!(
        project_event
            .invalidations
            .contains(&RelInvalidation::ProjectCorrelation(
                project_event
                    .watermarks
                    .project_correlation_sequence
                    .unwrap()
            ))
    );
    assert!(
        g.ingest_project_attachment_for(&i, &rel, &project, &project.join("missing"))
            .is_err()
    );
    assert!(drain(&g, &i, &s).is_empty());
}
struct BlockingEmbedding {
    simulated: crate::SimulatedEmbeddingEndpoint,
    state: (std::sync::Mutex<(bool, bool)>, std::sync::Condvar),
}
impl crate::EmbeddingEndpoint for BlockingEmbedding {
    fn dimensions(&self) -> u32 {
        8
    }
    fn normalization(&self) -> crate::VectorNormalization {
        crate::VectorNormalization::L2
    }
    fn embed(
        &self,
        mode: crate::EmbeddingMode,
        inputs: &[String],
    ) -> Result<Vec<Vec<f32>>, crate::EmbeddingEndpointError> {
        if mode == crate::EmbeddingMode::Document
            && inputs.iter().any(|s| s.contains("BODY_SECRET"))
        {
            let mut state = self.state.0.lock().unwrap();
            state.0 = true;
            self.state.1.notify_all();
            let (state, timeout) = self
                .state
                .1
                .wait_timeout_while(state, Duration::from_secs(20), |s| !s.1)
                .unwrap();
            assert!(!timeout.timed_out() && state.1);
        }
        self.simulated.embed(mode, inputs)
    }
}
#[test]
fn real_vector_worker_publishes_after_embedding_without_holding_subscription_gate() {
    let endpoint = Arc::new(BlockingEmbedding {
        simulated: crate::SimulatedEmbeddingEndpoint::new(8, crate::VectorNormalization::L2, 44),
        state: Default::default(),
    });
    let g = GatewayRuntime::new(
        test_path("p44-vectors-state"),
        super::super::super::ReliquaryRuntimeRoutes::new(
            None,
            None,
            None,
            None,
            Some(endpoint.clone()),
        ),
        crate::InsomniaWorkerConfig {
            workers: 0,
            ..Default::default()
        },
        crate::EpisodePolicy::default(),
    );
    let rel = g
        .mount_rel(InteractionRuntime::new(
            Cva::create_project(test_path("p44-vectors.rel")).unwrap(),
        ))
        .unwrap();
    let i = g
        .attach_instance(
            "vectors".into(),
            None,
            BTreeMap::from([(
                rel.clone(),
                BTreeSet::from([OwnerGrant::ViewRel, OwnerGrant::ManageRel]),
            )]),
        )
        .unwrap();
    create(&g, &i, &rel);
    let state = endpoint.state.0.lock().unwrap();
    let (state, timeout) = endpoint
        .state
        .1
        .wait_timeout_while(state, Duration::from_secs(10), |s| !s.0)
        .unwrap();
    assert!(!timeout.timed_out() && state.0);
    drop(state);
    let (_, s) = g.subscribe_rel(&i, &rel, None, 32).unwrap();
    assert!(drain(&g, &i, &s).is_empty());
    endpoint.state.0.lock().unwrap().1 = true;
    endpoint.state.1.notify_all();
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    let event = loop {
        if let Some(event) = drain(&g, &i, &s)
            .into_iter()
            .find(|e| e.class == RelEventClass::RetrievalInvalidated)
        {
            break event;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "vector commit did not publish"
        );
        std::thread::yield_now();
    };
    assert!(matches!(
        event.invalidations.as_slice(),
        [RelInvalidation::RetrievalProfile(_)]
    ));
    assert!(!format!("{event:?}").contains("SECRET"));
    g.shutdown().unwrap();
}
#[test]
fn host_capability_invalidation_is_safe_and_only_targets_live_rel_recipients() {
    let (g, rel, i) = setup("p44-host");
    let (before, s) = g.subscribe_rel(&i, &rel, None, 32).unwrap();
    g.publish_host_capability_change();
    let events = drain(&g, &i, &s);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].class, RelEventClass::HostWorkChanged);
    assert_eq!(events[0].watermarks, before.state.watermarks);
    assert_eq!(
        events[0].invalidations,
        super::super::RuntimeCapability::ALL
            .into_iter()
            .map(RelInvalidation::HostCapability)
            .collect::<Vec<_>>()
    );
    g.quiesce_rel(&rel).unwrap();
    g.publish_host_capability_change();
    assert!(g.next_rel_event(&i, &s).is_err());
}
fn create(g: &GatewayRuntime, i: &InstanceRuntime, rel: &str) -> crate::Memory {
    g.create_knowledge_memory_for(
        i,
        rel,
        "TITLE_SECRET".into(),
        "BODY_SECRET".into(),
        "knowledge".into(),
        "fact".into(),
        1,
    )
    .unwrap()
}
#[test]
fn memory_publication_is_one_summary_per_class_with_durable_clocks_and_safe_ids() {
    let (g, rel, i) = setup("p44-memory");
    let (before, s) = g.subscribe_rel(&i, &rel, None, 32).unwrap();
    let memory = create(&g, &i, &rel);
    let events = drain(&g, &i, &s);
    assert!(
        events
            .iter()
            .any(|e| e.class == RelEventClass::SemanticOwnerChanged)
    );
    assert!(
        events
            .iter()
            .any(|e| e.class == RelEventClass::RetrievalInvalidated)
    );
    let mut classes = BTreeSet::new();
    for (index, event) in events.iter().enumerate() {
        assert!(classes.insert(format!("{:?}", event.class)));
        assert_eq!(event.cursor, before.watermark.cursor + index as u64 + 1);
        assert!(event.watermarks.memory_version > before.state.watermarks.memory_version);
        assert_eq!(
            event.invalidations,
            vec![RelInvalidation::Memory(memory.id)]
        );
        assert!(!format!("{event:?}").contains("SECRET"));
    }
    let (after, _) = g.subscribe_rel(&i, &rel, None, 32).unwrap();
    assert_eq!(after.state.watermarks, events.last().unwrap().watermarks);
}
#[test]
fn rejected_revision_and_unchanged_metadata_publish_nothing() {
    let (g, rel, i) = setup("p44-noop");
    let memory = create(&g, &i, &rel);
    g.set_rel_metadata_for(&i, &rel, Some("project".into()), vec![])
        .unwrap();
    let (before, s) = g.subscribe_rel(&i, &rel, None, 32).unwrap();
    assert!(
        !g.set_rel_metadata_for(&i, &rel, Some("project".into()), vec![])
            .unwrap()
    );
    assert!(
        g.replace_knowledge_memory_for(
            &i,
            &rel,
            memory.id,
            u64::MAX,
            "rejected".into(),
            "REJECTED_SECRET".into(),
            2
        )
        .is_err()
    );
    assert!(
        g.set_rel_metadata_for(&i, &rel, None, vec![rel.clone()])
            .is_err()
    );
    assert!(drain(&g, &i, &s).is_empty());
    let (after, _) = g.subscribe_rel(&i, &rel, None, 32).unwrap();
    assert_eq!(after, before);
}
#[test]
fn graph_commit_is_classified_once_and_failed_target_has_no_success_event() {
    let (g, rel, i) = setup("p44-graph");
    let a = create(&g, &i, &rel);
    let b = create(&g, &i, &rel);
    let (before, s) = g.subscribe_rel(&i, &rel, None, 32).unwrap();
    g.mutate_knowledge_relation_for(
        &i,
        &rel,
        a.id,
        b.id,
        None,
        Some(crate::GraphRelationKind::Topical),
    )
    .unwrap();
    let events = drain(&g, &i, &s);
    assert_eq!(
        events
            .iter()
            .filter(|e| e.class == RelEventClass::SemanticOwnerChanged)
            .count(),
        1
    );
    assert!(
        events
            .iter()
            .all(|e| e.watermarks.graph_version > before.state.watermarks.graph_version)
    );
    assert!(
        events.iter().all(|e| e.invalidations
            == vec![RelInvalidation::Memory(a.id), RelInvalidation::Memory(b.id)])
    );
    assert!(
        g.mutate_knowledge_relation_for(
            &i,
            &rel,
            a.id,
            crate::MemoryId([255; 32]),
            None,
            Some(crate::GraphRelationKind::Topical)
        )
        .is_err()
    );
    assert!(drain(&g, &i, &s).is_empty());
}
#[test]
fn processing_switch_is_explicitly_ephemeral_and_does_not_fabricate_source_versions() {
    let (g, rel, i) = setup("p44-switch");
    let (before, s) = g.subscribe_rel(&i, &rel, None, 32).unwrap();
    g.set_rel_processing_enabled_for(&i, &rel, false).unwrap();
    let events = drain(&g, &i, &s);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].class, RelEventClass::ProcessingStatusChanged);
    assert_eq!(
        events[0].invalidations,
        vec![RelInvalidation::EphemeralProcessingEnabled(false)]
    );
    assert_eq!(events[0].watermarks, before.state.watermarks);
    let (after, _) = g.subscribe_rel(&i, &rel, None, 32).unwrap();
    assert!(!after.state.processing_enabled);
    g.set_rel_processing_enabled_for(&i, &rel, false).unwrap();
    assert!(drain(&g, &i, &s).is_empty());
}
#[test]
fn publication_is_isolated_from_an_unrelated_rel() {
    let (g, rel, i) = setup("p44-isolated");
    let other = g
        .mount_rel(InteractionRuntime::new(
            Cva::create_project(test_path("p44-other.rel")).unwrap(),
        ))
        .unwrap();
    let second = g
        .attach_instance(
            "second".into(),
            None,
            BTreeMap::from([(other.clone(), BTreeSet::from([OwnerGrant::ViewRel]))]),
        )
        .unwrap();
    let (before, s) = g.subscribe_rel(&second, &other, None, 32).unwrap();
    create(&g, &i, &rel);
    assert!(drain(&g, &second, &s).is_empty());
    let (after, _) = g.subscribe_rel(&second, &other, None, 32).unwrap();
    assert_eq!(before, after);
}
#[test]
fn real_memory_mutation_racing_snapshot_is_observed_after_its_cut() {
    let (g, rel, i) = setup("p44-race");
    let token = g.authorize(&i, &rel, OwnerGrant::ViewRel).unwrap();
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let subscriber = {
        let g = Arc::clone(&g);
        std::thread::spawn(move || {
            g.subscribe_rel_from_reader(&token, None, 32, |_| {
                let state = g.read_rel_snapshot_state(&token)?;
                entered_tx.send(()).unwrap();
                release_rx.recv_timeout(Duration::from_secs(10)).unwrap();
                Ok(state)
            })
        })
    };
    entered_rx.recv_timeout(Duration::from_secs(10)).unwrap();
    let mutator = {
        let g = Arc::clone(&g);
        let i = Arc::clone(&i);
        let rel = rel.clone();
        std::thread::spawn(move || create(&g, &i, &rel))
    };
    release_tx.send(()).unwrap();
    let (before, s) = subscriber.join().unwrap().unwrap();
    let memory = mutator.join().unwrap();
    let events = drain(&g, &i, &s);
    assert!(
        events
            .iter()
            .any(|e| e.class == RelEventClass::SemanticOwnerChanged)
    );
    assert!(events.iter().all(|e| {
        e.cursor > before.watermark.cursor
            && e.invalidations
                .contains(&RelInvalidation::Memory(memory.id))
    }));
}
#[test]
fn failed_partial_commit_invalidates_continuity_without_publishing_success() {
    let (g, rel, i) = setup("p44-partial");
    let (_, s) = g.subscribe_rel(&i, &rel, None, 32).unwrap();
    let token = g.authorize(&i, &rel, OwnerGrant::ManageRel).unwrap();
    let result: Result<(), _> =
        g.with_rel_mutation(&token, RelMutationFamily::Metadata, |runtime| {
            runtime
                .cva
                .set_rel_metadata(Some("partial".into()), vec![])
                .map_err(operation)?;
            Err(operation("injected after owner change"))
        });
    assert!(result.is_err());
    assert!(g.next_rel_event(&i, &s).is_err());
    assert!(g.subscribe_rel(&i, &rel, None, 32).is_err());
}
#[test]
fn private_profile_change_has_no_rel_event_but_projection_has_safe_entity_id() {
    let (g, rel, manager) = setup("p44-private");
    let mut phylactery = crate::Phylactery::create(test_path("p44-private.phy")).unwrap();
    phylactery
        .publish_memory(
            None,
            0,
            crate::MemoryDraft {
                category: "knowledge".into(),
                memory_type: "fact".into(),
                authority_kind: "direct".into(),
                title: "PRIVATE_PHY_TITLE_SECRET".into(),
                content: "PRIVATE_PHY_BODY_SECRET".into(),
                scope: "user".into(),
                lifecycle_state: "canonical".into(),
                temporal_status: "current".into(),
                archived: false,
                superseded_by: None,
                parent_id: None,
                source_node_id: None,
                content_source_conversation_id: None,
                content_source_node_id: None,
                grounding_source_conversation_id: None,
                grounding_source_node_id: None,
                source_episode_id: None,
                source_time_ns: Some(1),
                mutation_id: "private-phy-seed".into(),
                created_at_ns: 1,
                updated_at_ns: 1,
            },
        )
        .unwrap();
    phylactery.sync().unwrap();
    let phy = g.mount_phy(phylactery).unwrap();
    let token = g.authorize(&manager, &rel, OwnerGrant::ManageRel).unwrap();
    g.with_rel(&token, |runtime| {
        runtime
            .cva
            .publish_entity(
                Some(crate::entity_principal::principal_entity_id(&phy)),
                0,
                crate::EntityDraft {
                    canonical_name: phy.clone(),
                    aliases: vec![],
                    kind: "principal".into(),
                    summary: "principal".into(),
                    mutation_id: "seed".into(),
                    created_at_ns: 0,
                    updated_at_ns: 0,
                },
            )
            .map_err(operation)?;
        runtime.cva.sync().map_err(operation)
    })
    .unwrap();
    let i = g
        .attach_instance(
            "private".into(),
            Some(phy.clone()),
            BTreeMap::from([
                (phy.clone(), BTreeSet::from([OwnerGrant::UseOwnPhy])),
                (
                    rel.clone(),
                    BTreeSet::from([OwnerGrant::ViewRel, OwnerGrant::SubmitConversation]),
                ),
            ]),
        )
        .unwrap();
    let (_, s) = g.subscribe_rel(&i, &rel, None, 32).unwrap();
    assert!(
        g.set_phylactery_profile(&i, Some("PRIVATE_NAME_SECRET".into()))
            .unwrap()
    );
    assert!(drain(&g, &i, &s).is_empty());
    assert!(g.refresh_principal_projection_for(&i, &rel).unwrap());
    let events = drain(&g, &i, &s);
    assert!(
        events
            .iter()
            .any(|e| e.class == RelEventClass::SemanticOwnerChanged)
    );
    assert!(events.iter().all(|e| e.invalidations
        == vec![RelInvalidation::PrincipalEntity(
            crate::entity_principal::principal_entity_id(&phy)
        )]));
    assert!(
        events
            .iter()
            .all(|e| !format!("{e:?}").contains("PRIVATE_NAME_SECRET"))
    );

    // The user's active PHY profile is never composed into a REL snapshot,
    // even for an attachment authorized to read that PHY.
    let (snapshot, _fresh_subscription) = g.subscribe_rel(&i, &rel, None, 32).unwrap();
    let snapshot_debug = format!("{snapshot:?}");
    for secret in [
        "PRIVATE_NAME_SECRET",
        "PRIVATE_PHY_TITLE_SECRET",
        "PRIVATE_PHY_BODY_SECRET",
        phy.as_str(),
    ] {
        assert!(!snapshot_debug.contains(secret));
    }

    assert!(!g.refresh_principal_projection_for(&i, &rel).unwrap());
    assert!(drain(&g, &i, &s).is_empty());
}
