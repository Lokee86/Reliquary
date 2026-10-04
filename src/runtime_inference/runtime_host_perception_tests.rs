use crate::entity_candidate_test_support::{
    entity_draft, install_phy_mention, install_rel_mention, publish_phy_memory, publish_rel_memory,
};
use crate::entity_resolution_processor_test_support::BootstrapEndpoint;
use crate::runtime_host_perception_test_support::{BlockingEntityEndpoint, wait_entities};
use crate::runtime_host_test_support::{
    one_worker, test_path, wait_memory_revisions, wait_vectors,
};
use crate::{
    Cva, EpisodePolicy, InteractionRole, InteractionRuntime, Phylactery, ReliquaryRuntimeHost,
    ReliquaryRuntimeRoutes, SimulatedEmbeddingEndpoint, SimulatedGeneralEndpoint,
    VectorNormalization,
};
use serde_json::json;
use std::sync::{Arc, Barrier};

#[test]
fn perception_inference_does_not_hold_the_interaction_runtime_lock() {
    let mut cva = Cva::create_project(test_path("perception-concurrent.prj.rel")).unwrap();
    let memory = publish_rel_memory(
        &mut cva,
        "perception-concurrent",
        "Editor",
        "Helix is the local code editor.",
    );
    let key = install_rel_mention(&mut cva, memory, "Helix");
    cva.mark_dream_processed(memory, 0, 2).unwrap();

    let entered = Arc::new(Barrier::new(2));
    let release = Arc::new(Barrier::new(2));
    let endpoint: Arc<dyn crate::GeneralEndpoint> = Arc::new(BlockingEntityEndpoint {
        entered: Arc::clone(&entered),
        release: Arc::clone(&release),
    });
    let host = ReliquaryRuntimeHost::start(
        InteractionRuntime::new(cva),
        ReliquaryRuntimeRoutes::new(
            Some(Arc::clone(&endpoint)),
            None,
            Some(Arc::clone(&endpoint)),
            None,
            None,
        )
        .with_entity_routes(None, Some(endpoint)),
        one_worker(),
        EpisodePolicy::default(),
    )
    .unwrap();

    entered.wait();
    let status = host.background_status().unwrap();
    assert_eq!(status.reliquary_perception_pending, 1);
    assert!(!status.quiescent());
    host.open_session("live".into(), None).unwrap();
    host.begin_message("live", "u1".into(), InteractionRole::User, 3)
        .unwrap();
    host.append_text("live", "u1", "Perception must not block interaction.")
        .unwrap();
    host.complete_message("live", "u1").unwrap();
    release.wait();

    wait_entities(&host, 1);
    let cva = host.into_cva().unwrap();
    assert!(matches!(
        cva.entity_resolution(key).unwrap().status,
        crate::MemoryEntityResolutionStatus::Resolved { .. }
    ));
}

#[test]
fn detached_phylactery_inference_is_not_requeued_into_replacement_owner() {
    let cva = Cva::create_project(test_path("perception-owner-swap.prj.rel")).unwrap();
    let mut old_phy = Phylactery::create(test_path("perception-owner-swap-old.phy")).unwrap();
    let memory = publish_phy_memory(&mut old_phy, "Helix is the local code editor.");
    install_phy_mention(&mut old_phy, memory, "Helix");
    old_phy.mark_dream_processed(memory, 0, 2).unwrap();

    let entered = Arc::new(Barrier::new(2));
    let release = Arc::new(Barrier::new(2));
    let endpoint: Arc<dyn crate::GeneralEndpoint> = Arc::new(BlockingEntityEndpoint {
        entered: Arc::clone(&entered),
        release: Arc::clone(&release),
    });
    let host = ReliquaryRuntimeHost::start_with_phylactery(
        InteractionRuntime::new(cva),
        old_phy,
        ReliquaryRuntimeRoutes::new(
            Some(Arc::clone(&endpoint)),
            None,
            Some(Arc::clone(&endpoint)),
            None,
            None,
        )
        .with_entity_routes(None, Some(endpoint)),
        one_worker(),
        EpisodePolicy::default(),
    )
    .unwrap();

    entered.wait();
    let detached = host.detach_phylactery().unwrap().unwrap();
    let old_owner = detached.owner_id().unwrap();
    let replacement = Phylactery::create(test_path("perception-owner-swap-new.phy")).unwrap();
    let new_owner = replacement.owner_id().unwrap();
    assert_ne!(old_owner, new_owner);
    host.attach_phylactery(replacement).unwrap();
    release.wait();

    std::thread::sleep(std::time::Duration::from_millis(50));
    assert_eq!(
        host.phylactery_owner_id().unwrap().as_deref(),
        Some(new_owner.as_str())
    );
    let (_, replacement) = host.into_cva_and_phylactery().unwrap();
    assert!(replacement.unwrap().entities().is_empty());
}

#[test]
fn runtime_host_reconciles_entities_when_perception_queue_drains() {
    let mut cva = Cva::create_project(test_path("perception-reconcile.prj.rel")).unwrap();
    let left_memory = publish_rel_memory(
        &mut cva,
        "garden-left",
        "Garden",
        "The family garden has tomatoes.",
    );
    let right_memory = publish_rel_memory(
        &mut cva,
        "garden-right",
        "Garden",
        "The backyard garden has raised beds.",
    );
    let left = cva
        .publish_entity(
            None,
            0,
            entity_draft(
                "family garden",
                &[],
                "The family's continuing backyard vegetable garden.",
                1,
            ),
        )
        .unwrap()
        .0;
    let right = cva
        .publish_entity(
            None,
            0,
            entity_draft(
                "backyard garden",
                &[],
                "The continuing family garden with raised beds.",
                2,
            ),
        )
        .unwrap()
        .0;
    cva.set_entity_association(left_memory, left.id, true, cva.graph_version())
        .unwrap();
    cva.set_entity_association(right_memory, right.id, true, cva.graph_version())
        .unwrap();

    let endpoint: Arc<dyn crate::GeneralEndpoint> = Arc::new(SimulatedGeneralEndpoint::new(
        "runtime-reconciler",
        vec![json!({"relation":"same_identity"})],
    ));
    let host = ReliquaryRuntimeHost::start(
        InteractionRuntime::new(cva),
        ReliquaryRuntimeRoutes::new(None, None, None, None, None)
            .with_entity_routes(None, Some(endpoint)),
        one_worker(),
        EpisodePolicy::default(),
    )
    .unwrap();

    for _ in 0..1_000 {
        if host.entity_stats().unwrap().entities == 1 {
            let cva = host.into_cva().unwrap();
            assert_eq!(cva.entities().len(), 1);
            assert_eq!(cva.memories_for_entity(cva.entities()[0].id).len(), 2);
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    panic!("Entity reconciliation did not complete");
}

#[test]
fn runtime_host_resolves_project_entities_after_dream() {
    let mut cva = Cva::create_project(test_path("perception-project.prj.rel")).unwrap();
    let memory = publish_rel_memory(
        &mut cva,
        "perception-project",
        "Editor",
        "Helix is the code editor installed for Rust work.",
    );
    let key = install_rel_mention(&mut cva, memory, "Helix");
    let (endpoint, calls) = BootstrapEndpoint::new();
    let endpoint: Arc<dyn crate::GeneralEndpoint> = Arc::new(endpoint);
    let host = ReliquaryRuntimeHost::start(
        InteractionRuntime::new(cva),
        ReliquaryRuntimeRoutes::new(
            Some(Arc::clone(&endpoint)),
            None,
            Some(Arc::clone(&endpoint)),
            None,
            Some(Arc::new(SimulatedEmbeddingEndpoint::new(
                8,
                VectorNormalization::L2,
                19,
            ))),
        )
        .with_entity_routes(None, Some(endpoint)),
        one_worker(),
        EpisodePolicy::default(),
    )
    .unwrap();

    wait_vectors(&host, 1);
    wait_memory_revisions(&host, 2);
    wait_entities(&host, 1);
    assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 1);

    let cva = host.into_cva().unwrap();
    assert!(matches!(
        cva.entity_resolution(key).unwrap().status,
        crate::MemoryEntityResolutionStatus::Resolved { .. }
    ));
}
