use super::super::super::owner_operations::{ActorClass, EpisodePlan};
use super::super::super::publication::PublicationCoordinator;
use super::*;
use crate::insomnia::{
    PreparedApplication, PreparedMemory, PreparedUserMemory, RuntimeInsomniaClaim,
};
use crate::{MemoryDraft, MemorySourceRef, Phylactery};

fn runtime(gateway: &GatewayRuntime, rel: &str) -> Arc<std::sync::Mutex<InteractionRuntime>> {
    let owner = Arc::clone(
        &gateway
            .rel_executions
            .read()
            .unwrap()
            .get(rel)
            .unwrap()
            .owner,
    );
    let runtime = Arc::clone(owner.lock().unwrap().runtime.as_ref().unwrap());
    runtime
}
fn publication_coordinator(gateway: &GatewayRuntime, rel: &str) -> PublicationCoordinator {
    PublicationCoordinator {
        rel_id: rel.to_owned(),
        journal: gateway.conversation.owner_journal(rel).unwrap(),
        phys: Arc::clone(&gateway.phy_executions),
        bindings: Arc::clone(&gateway.principal_phy_bindings),
        policy: Arc::clone(&gateway.publication_policy),
        rel_fence: Arc::clone(
            &gateway
                .rel_executions
                .read()
                .unwrap()
                .get(rel)
                .unwrap()
                .fence,
        ),
    }
}
fn permissions(rel: &str, phy: &str, personal: bool) -> BTreeMap<String, BTreeSet<OwnerGrant>> {
    let mut permissions = grants(rel, &[OwnerGrant::ViewRel, OwnerGrant::SubmitConversation]);
    if personal {
        permissions.insert(phy.to_owned(), BTreeSet::from([OwnerGrant::UseOwnPhy]));
    }
    permissions
}

struct Fixture {
    gateway: GatewayRuntime,
    rel: String,
    phys: [String; 2],
    instances: [Arc<InstanceRuntime>; 2],
    paths: [PathBuf; 3],
    state: PathBuf,
    claim: RuntimeInsomniaClaim,
}
impl Fixture {
    fn new(personal_a: bool) -> Self {
        let state = test_path("publication-state");
        let paths = [
            test_path("publication.rel"),
            test_path("a.phy"),
            test_path("b.phy"),
        ];
        let gateway = gateway_at(state.clone());
        let rel = gateway
            .mount_rel(seeded_runtime_at(paths[0].clone()))
            .unwrap();
        let phys = [
            gateway
                .mount_phy(Phylactery::create(&paths[1]).unwrap())
                .unwrap(),
            gateway
                .mount_phy(Phylactery::create(&paths[2]).unwrap())
                .unwrap(),
        ];
        let instances = [
            gateway
                .attach_instance(
                    "A".into(),
                    Some(phys[0].clone()),
                    permissions(&rel, &phys[0], personal_a),
                )
                .unwrap(),
            gateway
                .attach_instance(
                    "B".into(),
                    Some(phys[1].clone()),
                    permissions(&rel, &phys[1], true),
                )
                .unwrap(),
        ];
        for (index, node, parent) in [(0, "source-a", "root-one"), (1, "source-b", "source-a")] {
            let mut request = submission("conversation-one", node, parent);
            request.expected_conversation_revision = gateway
                .conversation
                .journal(&rel)
                .unwrap()
                .lock()
                .unwrap()
                .conversation_revision(&ConversationId::new("conversation-one").unwrap());
            gateway
                .submit_conversation(&instances[index], &rel, request)
                .unwrap();
            gateway
                .process_next_conversation_submission(
                    &instances[index],
                    &rel,
                    "conversation-one",
                    node,
                )
                .unwrap();
        }
        let claim = {
            let handle = runtime(&gateway, &rel);
            let mut runtime = handle.lock().unwrap();
            runtime
                .cva
                .finalize_explicit_path_and_queue(
                    "conversation-one",
                    "source-b",
                    EpisodePolicy::default(),
                    crate::insomnia::runtime_step::now_ns(),
                )
                .unwrap();
            runtime
                .cva
                .claim_runtime_insomnia("publication-test", &gateway.config)
                .unwrap()
                .unwrap()
        };
        Self {
            gateway,
            rel,
            phys,
            instances,
            paths,
            state,
            claim,
        }
    }

    fn restart(self) -> Self {
        let Self {
            gateway,
            rel,
            phys,
            instances,
            paths,
            state,
            claim,
        } = self;
        for instance in &instances {
            gateway.detach_instance(instance).unwrap();
        }
        for phy in &phys {
            if gateway.phy_executions.read().unwrap().contains_key(phy) {
                drop(gateway.unmount_phy(phy).unwrap());
            }
        }
        drop(gateway.unmount_rel(&rel).unwrap());
        drop(instances);
        drop(gateway);
        let gateway = gateway_at(state.clone());
        assert_eq!(
            gateway
                .mount_rel(InteractionRuntime::new(Cva::open(&paths[0]).unwrap()))
                .unwrap(),
            rel
        );
        for index in 0..2 {
            assert_eq!(
                gateway
                    .mount_phy(Phylactery::open(&paths[index + 1]).unwrap())
                    .unwrap(),
                phys[index]
            );
        }
        let instances = [
            gateway
                .attach_instance(
                    "A".into(),
                    Some(phys[0].clone()),
                    permissions(&rel, &phys[0], true),
                )
                .unwrap(),
            gateway
                .attach_instance(
                    "B".into(),
                    Some(phys[1].clone()),
                    permissions(&rel, &phys[1], true),
                )
                .unwrap(),
        ];
        let claim = runtime(&gateway, &rel)
            .lock()
            .unwrap()
            .cva
            .claim_runtime_insomnia("publication-recovery", &gateway.config)
            .unwrap()
            .unwrap_or(claim);
        Self {
            gateway,
            rel,
            phys,
            instances,
            paths,
            state,
            claim,
        }
    }

    fn freeze(&self, include_rejected: bool) -> EpisodePlan {
        let coordinator = publication_coordinator(&self.gateway, &self.rel);
        let handle = runtime(&self.gateway, &self.rel);
        coordinator
            .freeze(
                &mut handle.lock().unwrap(),
                &self.claim,
                prepared(&self.rel, &self.claim, include_rejected),
            )
            .unwrap()
    }

    fn private_count(&self, index: usize) -> usize {
        self.gateway
            .phy_executions
            .read()
            .unwrap()
            .get(&self.phys[index])
            .unwrap()
            .owner
            .lock()
            .unwrap()
            .memory_stats()
            .memories
    }
    fn commit(&self, plan: &EpisodePlan) {
        let coordinator = publication_coordinator(&self.gateway, &self.rel);
        let publication = coordinator.publish(plan, true).unwrap();
        let prepared = coordinator.thaw(plan).unwrap();
        let handle = runtime(&self.gateway, &self.rel);
        let mut runtime = handle.lock().unwrap();
        runtime
            .cva
            .commit_runtime_insomnia_verified(
                &self.claim,
                prepared,
                publication,
                &self.gateway.config,
            )
            .unwrap();
        assert!(coordinator.verify_receipt(&mut runtime, plan).unwrap());
    }
}

fn memory(claim: &RuntimeInsomniaClaim, key: &str, project: bool) -> PreparedMemory {
    let draft = MemoryDraft {
        category: "preference".into(),
        memory_type: "fact".into(),
        authority_kind: "direct".into(),
        temporal_status: "current".into(),
        title: key.into(),
        content: format!("Exact frozen {key}"),
        scope: if project {
            "project".into()
        } else {
            "user".into()
        },
        lifecycle_state: "active".into(),
        archived: false,
        superseded_by: None,
        parent_id: None,
        source_node_id: project.then(|| "source-b".into()),
        content_source_conversation_id: None,
        content_source_node_id: None,
        grounding_source_conversation_id: None,
        grounding_source_node_id: None,
        source_episode_id: project.then_some(claim.episode.id),
        source_time_ns: Some(claim.episode.source_through_ns),
        mutation_id: format!("publication:{key}"),
        created_at_ns: claim.episode.source_through_ns,
        updated_at_ns: claim.episode.source_through_ns,
    };
    let temporal = crate::insomnia::temporal::assess_draft(&draft);
    let routing_metadata = Some(crate::MemoryRoutingMetadata {
        memory_id: crate::memory_model::memory_id(&draft.mutation_id),
        body_id: crate::memory_model::memory_body_id(&draft.title, &draft.content),
        entity_mentions: vec![],
    });
    PreparedMemory {
        draft,
        temporal,
        temporal_inference: None,
        routing_metadata,
    }
}
fn personal(
    rel: &str,
    claim: &RuntimeInsomniaClaim,
    key: &str,
    node: &str,
    grounding: Option<&str>,
) -> PreparedUserMemory {
    PreparedUserMemory {
        memory: memory(claim, key, false),
        source_ref: MemorySourceRef {
            owner_id: rel.into(),
            principal_id: None,
            source_episode_id: claim.episode.id,
            source_node_id: node.into(),
            content_source_conversation_id: Some("conversation-one".into()),
            content_source_node_id: Some(node.into()),
            grounding_source_conversation_id: grounding.map(|_| "conversation-one".into()),
            grounding_source_node_id: grounding.map(str::to_owned),
        },
    }
}
fn prepared(
    rel: &str,
    claim: &RuntimeInsomniaClaim,
    include_rejected: bool,
) -> PreparedApplication {
    let mut user_drafts = vec![
        personal(rel, claim, "a", "source-a", None),
        personal(rel, claim, "b", "source-b", None),
    ];
    if include_rejected {
        user_drafts.push(personal(
            rel,
            claim,
            "ambiguous",
            "source-a",
            Some("source-b"),
        ));
        user_drafts.push(personal(rel, claim, "legacy", "root-one", None));
    }
    PreparedApplication {
        project_drafts: vec![memory(claim, "shared", true)],
        user_drafts,
        rejected: vec![],
        model: "fixture-model".into(),
    }
}

#[test]
fn mixed_principals_publish_exact_private_destinations_and_reject_ambiguous_sources() {
    let fixture = Fixture::new(true);
    let plan = fixture.freeze(true);
    assert_eq!(plan.prepared.personal_drafts.len(), 2);
    assert_eq!(plan.prepared.rejected.len(), 2);
    assert_eq!(
        plan.prepared.personal_drafts[0].destination_phy_owner_id,
        fixture.phys[0]
    );
    assert_eq!(
        plan.prepared.personal_drafts[1].destination_phy_owner_id,
        fixture.phys[1]
    );
    fixture.commit(&plan);
    let coordinator = publication_coordinator(&fixture.gateway, &fixture.rel);
    coordinator.observe(&plan).unwrap();
    assert_eq!((fixture.private_count(0), fixture.private_count(1)), (1, 1));
    let handle = runtime(&fixture.gateway, &fixture.rel);
    assert_eq!(handle.lock().unwrap().cva.memory_stats().memories, 1);
    assert!(coordinator.unobserved().unwrap().is_empty());
}

#[test]
fn phy_binding_without_use_own_phy_cannot_authorize_a_personal_write() {
    let fixture = Fixture::new(false);
    let source = fixture
        .gateway
        .conversation
        .owner_journal(&fixture.rel)
        .unwrap();
    assert!(
        source
            .lock()
            .unwrap()
            .source_attribution("conversation-one", "source-a", ActorClass::User)
            .unwrap()
            .admission
            .personal
            .is_none()
    );
    let plan = fixture.freeze(false);
    assert_eq!(plan.prepared.personal_drafts.len(), 1);
    assert_eq!(plan.prepared.rejected.len(), 1);
    fixture.commit(&plan);
    assert_eq!((fixture.private_count(0), fixture.private_count(1)), (0, 1));
}

#[test]
fn restart_replays_partial_and_complete_private_writes_and_observes_existing_receipt() {
    for cut in 0..3 {
        let fixture = Fixture::new(true);
        let plan = fixture.freeze(false);
        let coordinator = publication_coordinator(&fixture.gateway, &fixture.rel);
        match cut {
            0 => {
                drop(fixture.gateway.unmount_phy(&fixture.phys[1]).unwrap());
                assert!(coordinator.publish(&plan, true).is_err());
                assert_eq!(fixture.private_count(0), 1);
            }
            1 => {
                coordinator.publish(&plan, true).unwrap();
            }
            _ => {
                fixture.commit(&plan);
            }
        }
        drop(coordinator);
        let fixture = fixture.restart();
        let coordinator = publication_coordinator(&fixture.gateway, &fixture.rel);
        let loaded = coordinator.plan(&fixture.claim).unwrap().unwrap();
        assert_eq!(loaded, plan);
        if cut < 2 {
            assert!(
                !coordinator
                    .verify_receipt(
                        &mut runtime(&fixture.gateway, &fixture.rel).lock().unwrap(),
                        &loaded
                    )
                    .unwrap()
            );
            let owner = Arc::clone(
                &fixture
                    .gateway
                    .rel_executions
                    .read()
                    .unwrap()
                    .get(&fixture.rel)
                    .unwrap()
                    .owner,
            );
            let endpoint = (cut == 0)
                .then(|| Arc::new(ForbiddenExtraction) as Arc<dyn crate::GeneralEndpoint>);
            owner
                .lock()
                .unwrap()
                .replay_claim_for_test(&fixture.claim, endpoint)
                .unwrap();
        } else {
            let owner = Arc::clone(
                &fixture
                    .gateway
                    .rel_executions
                    .read()
                    .unwrap()
                    .get(&fixture.rel)
                    .unwrap()
                    .owner,
            );
            owner
                .lock()
                .unwrap()
                .reconcile_publication_for_test()
                .unwrap();
            assert!(
                coordinator
                    .verify_receipt(
                        &mut runtime(&fixture.gateway, &fixture.rel).lock().unwrap(),
                        &loaded
                    )
                    .unwrap()
            );
            let publication = coordinator.publish(&loaded, false).unwrap();
            assert_eq!(publication.existing.len(), 2);
            assert!(publication.created.is_empty());
        }
        coordinator.observe(&loaded).unwrap();
        assert_eq!((fixture.private_count(0), fixture.private_count(1)), (1, 1));
        assert_eq!(
            runtime(&fixture.gateway, &fixture.rel)
                .lock()
                .unwrap()
                .cva
                .memory_stats()
                .memories,
            1
        );
        assert!(coordinator.unobserved().unwrap().is_empty());
    }
}

#[test]
fn revoke_pending_phy_authority_survives_restart_and_grant_restoration() {
    let fixture = Fixture::new(true);
    let plan = fixture.freeze(false);
    fixture
        .gateway
        .replace_instance_grants(
            &fixture.instances[0],
            permissions(&fixture.rel, &fixture.phys[0], false),
        )
        .unwrap();
    let coordinator = publication_coordinator(&fixture.gateway, &fixture.rel);
    assert!(coordinator.publish(&plan, true).is_err());
    assert_eq!((fixture.private_count(0), fixture.private_count(1)), (0, 0));
    drop(coordinator);
    let fixture = fixture.restart();
    let coordinator = publication_coordinator(&fixture.gateway, &fixture.rel);
    assert!(coordinator.publish(&plan, true).is_err());
    assert_eq!((fixture.private_count(0), fixture.private_count(1)), (0, 0));
    assert!(
        !coordinator
            .verify_receipt(
                &mut runtime(&fixture.gateway, &fixture.rel).lock().unwrap(),
                &plan
            )
            .unwrap()
    );
}

#[test]
fn withdrawing_personal_permission_updates_offline_rel_journal() {
    let fixture = Fixture::new(true);
    let plan = fixture.freeze(false);
    drop(fixture.gateway.unmount_rel(&fixture.rel).unwrap());
    fixture
        .gateway
        .replace_instance_grants(&fixture.instances[0], BTreeMap::new())
        .unwrap();
    fixture
        .gateway
        .mount_rel(InteractionRuntime::new(
            Cva::open(&fixture.paths[0]).unwrap(),
        ))
        .unwrap();
    let coordinator = publication_coordinator(&fixture.gateway, &fixture.rel);
    assert!(coordinator.publish(&plan, true).is_err());
    assert_eq!((fixture.private_count(0), fixture.private_count(1)), (0, 0));
}

#[test]
fn completed_receipt_can_be_observed_read_only_after_authority_withdrawal() {
    let fixture = Fixture::new(true);
    let plan = fixture.freeze(false);
    fixture.commit(&plan);
    fixture
        .gateway
        .replace_instance_grants(
            &fixture.instances[0],
            permissions(&fixture.rel, &fixture.phys[0], false),
        )
        .unwrap();
    let coordinator = publication_coordinator(&fixture.gateway, &fixture.rel);
    assert!(coordinator.publish(&plan, true).is_err());
    let publication = coordinator.publish(&plan, false).unwrap();
    assert_eq!(publication.existing.len(), 2);
    assert!(publication.created.is_empty());
    assert!(
        coordinator
            .verify_receipt(
                &mut runtime(&fixture.gateway, &fixture.rel).lock().unwrap(),
                &plan,
            )
            .unwrap()
    );
    coordinator.observe(&plan).unwrap();
    assert!(coordinator.unobserved().unwrap().is_empty());
}

#[test]
fn frozen_binding_change_holds_without_retargeting_or_rel_completion() {
    let fixture = Fixture::new(true);
    let plan = fixture.freeze(false);
    fixture
        .gateway
        .principal_phy_bindings
        .lock()
        .unwrap()
        .insert("A".into(), fixture.phys[1].clone());
    let coordinator = publication_coordinator(&fixture.gateway, &fixture.rel);
    let error = coordinator
        .publish(&plan, true)
        .err()
        .expect("publication must be Held");
    coordinator.hold(&plan, &error).unwrap();
    assert_eq!((fixture.private_count(0), fixture.private_count(1)), (0, 0));
    assert!(
        !coordinator
            .verify_receipt(
                &mut runtime(&fixture.gateway, &fixture.rel).lock().unwrap(),
                &plan,
            )
            .unwrap()
    );
}

#[test]
fn same_id_conflicting_personal_payload_holds_before_other_owner_writes() {
    let fixture = Fixture::new(true);
    let plan = fixture.freeze(false);
    let coordinator = publication_coordinator(&fixture.gateway, &fixture.rel);
    let prepared = coordinator.thaw(&plan).unwrap();
    let mut draft = prepared.user_drafts[0].memory.draft.clone();
    draft.content = "Conflicting existing content".into();
    let owner = Arc::clone(
        &fixture
            .gateway
            .phy_executions
            .read()
            .unwrap()
            .get(&fixture.phys[0])
            .unwrap()
            .owner,
    );
    {
        let mut owner = owner.lock().unwrap();
        owner
            .publish_memory_with_source_ref_and_temporal_inference(
                None,
                0,
                draft,
                prepared.user_drafts[0].source_ref.clone(),
                prepared.user_drafts[0].memory.bound_temporal_inference(),
            )
            .unwrap();
        owner.sync().unwrap();
    }
    let error = coordinator
        .publish(&plan, true)
        .err()
        .expect("publication must be Held");
    coordinator.hold(&plan, &error).unwrap();
    assert_eq!((fixture.private_count(0), fixture.private_count(1)), (1, 0));
    assert!(
        !coordinator
            .verify_receipt(
                &mut runtime(&fixture.gateway, &fixture.rel).lock().unwrap(),
                &plan,
            )
            .unwrap()
    );
}

#[test]
fn retry_with_missing_operational_authority_requires_repair_before_extraction() {
    let mut fixture = Fixture::new(true);
    let plan = fixture.freeze(false);
    drop(fixture.gateway.unmount_phy(&fixture.phys[1]).unwrap());
    let mut coordinator = publication_coordinator(&fixture.gateway, &fixture.rel);
    assert!(coordinator.publish(&plan, true).is_err());
    assert_eq!(fixture.private_count(0), 1);
    coordinator.journal = Arc::new(std::sync::Mutex::new(
        super::super::super::owner_operations::OwnerOperationJournal::open(
            test_path("lost-operational-state"),
            fixture.rel.strip_prefix("rel-").unwrap(),
        )
        .unwrap(),
    ));
    fixture.claim.work.attempt_count = 1; // Runtime attempts reset after reopen.
    let error = coordinator.plan(&fixture.claim).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("operational-state repair required")
    );
    coordinator.hold_claim(&fixture.claim, &error).unwrap();
    assert!(coordinator.plan(&fixture.claim).is_err());
    assert_eq!(fixture.private_count(0), 1);
}

struct ForbiddenExtraction;
impl crate::GeneralEndpoint for ForbiddenExtraction {
    fn model(&self) -> &str {
        "must-not-rerun"
    }
    fn complete_json(
        &self,
        _: &str,
        _: &str,
        _: &str,
        _: &serde_json::Value,
    ) -> Result<serde_json::Value, crate::GeneralEndpointError> {
        panic!("frozen publication must not invoke extraction");
    }
}

#[test]
fn revocation_after_partial_write_holds_only_the_unwritten_destination_across_restart() {
    let fixture = Fixture::new(true);
    let plan = fixture.freeze(false);
    drop(fixture.gateway.unmount_phy(&fixture.phys[1]).unwrap());
    let coordinator = publication_coordinator(&fixture.gateway, &fixture.rel);
    assert!(coordinator.publish(&plan, true).is_err());
    fixture
        .gateway
        .replace_instance_grants(
            &fixture.instances[1],
            permissions(&fixture.rel, &fixture.phys[1], false),
        )
        .unwrap();
    drop(coordinator);
    let fixture = fixture.restart();
    let coordinator = publication_coordinator(&fixture.gateway, &fixture.rel);
    let error = coordinator
        .publish(&plan, true)
        .err()
        .expect("withdrawn destination must hold");
    coordinator.hold(&plan, &error).unwrap();
    assert_eq!((fixture.private_count(0), fixture.private_count(1)), (1, 0));
    assert!(
        !coordinator
            .verify_receipt(
                &mut runtime(&fixture.gateway, &fixture.rel).lock().unwrap(),
                &plan,
            )
            .unwrap()
    );
}

#[test]
fn restart_repairs_routing_attachment_after_exact_semantic_write_without_model() {
    let fixture = Fixture::new(true);
    let plan = fixture.freeze(false);
    let coordinator = publication_coordinator(&fixture.gateway, &fixture.rel);
    let prepared = coordinator.thaw(&plan).unwrap();
    let owner = Arc::clone(
        &fixture
            .gateway
            .phy_executions
            .read()
            .unwrap()
            .get(&fixture.phys[0])
            .unwrap()
            .owner,
    );
    {
        let mut owner = owner.lock().unwrap();
        owner
            .publish_memory_with_source_ref_and_temporal_inference(
                None,
                0,
                prepared.user_drafts[0].memory.draft.clone(),
                prepared.user_drafts[0].source_ref.clone(),
                prepared.user_drafts[0].memory.bound_temporal_inference(),
            )
            .unwrap();
        owner.sync().unwrap();
    }
    drop(owner);
    drop(coordinator);
    let fixture = fixture.restart();
    let owner = Arc::clone(
        &fixture
            .gateway
            .rel_executions
            .read()
            .unwrap()
            .get(&fixture.rel)
            .unwrap()
            .owner,
    );
    owner
        .lock()
        .unwrap()
        .replay_claim_for_test(&fixture.claim, None)
        .unwrap();
    let coordinator = publication_coordinator(&fixture.gateway, &fixture.rel);
    assert_eq!((fixture.private_count(0), fixture.private_count(1)), (1, 1));
    assert!(coordinator.unobserved().unwrap().is_empty());
    assert_eq!(coordinator.publish(&plan, false).unwrap().existing.len(), 2);
}

#[path = "runtime_host_gateway_pending_publication_tests.rs"]
mod pending;

#[test]
fn background_worker_replays_frozen_work_without_configured_extractor() {
    let fixture = Fixture::new(true);
    let plan = fixture.freeze(false);
    let handle = runtime(&fixture.gateway, &fixture.rel);
    let now = crate::insomnia::runtime_step::now_ns();
    handle
        .lock()
        .unwrap()
        .cva
        .fail_insomnia_episode(
            fixture.claim.work.episode_id,
            fixture.claim.work.lease_token.unwrap(),
            fixture.claim.started_at_ns,
            now,
            now.saturating_add(1),
            "restart boundary".into(),
        )
        .unwrap();
    let owner = Arc::clone(
        &fixture
            .gateway
            .rel_executions
            .read()
            .unwrap()
            .get(&fixture.rel)
            .unwrap()
            .owner,
    );
    owner.lock().unwrap().start_insomnia_for_test();
    let coordinator = publication_coordinator(&fixture.gateway, &fixture.rel);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while !coordinator.unobserved().unwrap().is_empty() {
        assert!(
            std::time::Instant::now() < deadline,
            "frozen worker failed to resume"
        );
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert_eq!((fixture.private_count(0), fixture.private_count(1)), (1, 1));
    assert!(
        coordinator
            .verify_receipt(&mut handle.lock().unwrap(), &plan)
            .unwrap()
    );
}
