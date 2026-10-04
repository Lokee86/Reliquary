use super::super::super::super::owner_operations::OwnerOperationRecord;
use super::*;

fn phy_workers(f: &Fixture, index: usize) -> usize {
    f.gateway
        .phy_executions
        .read()
        .unwrap()
        .get(&f.phys[index])
        .unwrap()
        .runtime
        .lock()
        .unwrap()
        .worker_count()
}

#[test]
fn pending_exact_plan_holds_offline_or_quiesced_then_resumes_without_retargeting() {
    for offline in [false, true] {
        let f = Fixture::new(true);
        let plan = f.freeze(false);
        let old = f
            .gateway
            .authorize(&f.instances[0], &f.phys[0], OwnerGrant::UseOwnPhy)
            .unwrap();
        let stored = if offline {
            Some(f.gateway.unmount_phy(&f.phys[0]).unwrap())
        } else {
            f.gateway
                .quiesce_phy_for(&f.instances[0], &f.phys[0])
                .unwrap();
            assert_eq!(phy_workers(&f, 0), 0);
            None
        };
        let coordinator = publication_coordinator(&f.gateway, &f.rel);
        let error = coordinator
            .publish(&plan, true)
            .err()
            .expect("target must be Held");
        coordinator.hold(&plan, &error).unwrap();
        assert_eq!(f.private_count(1), 0);
        assert!(
            !coordinator
                .verify_receipt(&mut runtime(&f.gateway, &f.rel).lock().unwrap(), &plan)
                .unwrap()
        );
        assert!(coordinator.journal.lock().unwrap().records().iter().any(
            |record| matches!(&record.record, OwnerOperationRecord::Held(held)
                if held.operation_id == plan.episode_id)
        ));
        // Unrelated personal and REL operations keep progressing while A is fenced.
        assert_eq!(phy_workers(&f, 1), 3);
        f.gateway
            .set_phylactery_profile(&f.instances[1], Some("B still live".into()))
            .unwrap();
        let other = f
            .gateway
            .mount_rel(InteractionRuntime::new(
                Cva::create_project(test_path("pending-unrelated.rel")).unwrap(),
            ))
            .unwrap();
        let instance = f
            .gateway
            .attach_instance(
                "other".into(),
                None,
                grants(&other, &[OwnerGrant::ViewRel, OwnerGrant::ManageRel]),
            )
            .unwrap();
        let memory = f
            .gateway
            .create_knowledge_memory_for(
                &instance,
                &other,
                "Progress".into(),
                "An unrelated REL remains writable".into(),
                "knowledge".into(),
                "fact".into(),
                1,
            )
            .unwrap();
        assert!(
            f.gateway
                .knowledge_memory_for_owner(&instance, &other, memory.id)
                .is_ok()
        );
        f.gateway.detach_instance(&instance).unwrap();
        drop(f.gateway.unmount_rel(&other).unwrap());
        if let Some(owner) = stored {
            assert_eq!(f.gateway.mount_phy(owner).unwrap(), f.phys[0]);
        } else {
            f.gateway
                .reopen_phy_for(&f.instances[0], &f.phys[0])
                .unwrap();
        }
        assert!(f.gateway.with_phy(&old, |_| Ok(())).is_err());
        assert_eq!(phy_workers(&f, 0), 3);
        assert_eq!(coordinator.plan(&f.claim).unwrap().unwrap(), plan);
        f.commit(&plan);
        coordinator.observe(&plan).unwrap();
        assert_eq!((f.private_count(0), f.private_count(1)), (1, 1));
        assert!(coordinator.unobserved().unwrap().is_empty());
    }
}

#[test]
fn attachment_churn_preserves_one_worker_set_and_pending_owner_authority() {
    let f = Fixture::new(true);
    let plan = f.freeze(false);
    let owner_runtime = Arc::clone(
        &f.gateway
            .phy_executions
            .read()
            .unwrap()
            .get(&f.phys[0])
            .unwrap()
            .runtime,
    );
    f.gateway.detach_instance(&f.instances[0]).unwrap();
    for _ in 0..20 {
        let first = f
            .gateway
            .attach_instance(
                "A".into(),
                Some(f.phys[0].clone()),
                permissions(&f.rel, &f.phys[0], true),
            )
            .unwrap();
        let second = f
            .gateway
            .attach_instance(
                "A".into(),
                Some(f.phys[0].clone()),
                permissions(&f.rel, &f.phys[0], true),
            )
            .unwrap();
        assert!(Arc::ptr_eq(
            &owner_runtime,
            &f.gateway
                .phy_executions
                .read()
                .unwrap()
                .get(&f.phys[0])
                .unwrap()
                .runtime
        ));
        assert_eq!(phy_workers(&f, 0), 3);
        f.gateway.detach_instance(&first).unwrap();
        f.gateway.detach_instance(&second).unwrap();
        assert_eq!(phy_workers(&f, 0), 3);
    }
    f.gateway.detach_instance(&f.instances[1]).unwrap();
    f.commit(&plan);
    let coordinator = publication_coordinator(&f.gateway, &f.rel);
    coordinator.observe(&plan).unwrap();
    assert_eq!((f.private_count(0), f.private_count(1)), (1, 1));
    let replay = coordinator.publish(&plan, false).unwrap();
    assert!(replay.created.is_empty());
    assert_eq!(replay.existing.len(), 2);
}

#[test]
fn quiesced_phy_drains_an_admitted_write_before_stopping_workers() {
    use std::sync::mpsc;
    use std::time::Duration;
    let f = Fixture::new(true);
    let token = f
        .gateway
        .authorize(&f.instances[0], &f.phys[0], OwnerGrant::UseOwnPhy)
        .unwrap();
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    std::thread::scope(|scope| {
        let gateway = &f.gateway;
        let writer_token = token.clone();
        let writer = scope.spawn(move || {
            gateway.with_phy(&writer_token, |owner| {
                owner
                    .set_profile(Some("admitted before fence".into()))
                    .unwrap();
                entered_tx.send(()).unwrap();
                release_rx.recv_timeout(Duration::from_secs(10)).unwrap();
                owner.sync().unwrap();
                Ok(())
            })
        });
        entered_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        let (done_tx, done_rx) = mpsc::channel();
        let phy = f.phys[0].clone();
        let transition = scope.spawn(move || {
            let result = gateway.quiesce_phy(&phy);
            done_tx.send(result).unwrap();
        });
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while f.gateway.owner_admission(&f.phys[0]).unwrap() != OwnerAdmission::Closed {
            assert!(
                std::time::Instant::now() < deadline,
                "quiesce did not fence"
            );
            std::thread::yield_now();
        }
        assert!(matches!(done_rx.try_recv(), Err(mpsc::TryRecvError::Empty)));
        assert!(f.gateway.with_phy(&token, |_| Ok(())).is_err());
        assert!(f.gateway.phylactery_profile(&f.instances[1]).is_ok());
        release_tx.send(()).unwrap();
        writer.join().unwrap().unwrap();
        done_rx
            .recv_timeout(Duration::from_secs(10))
            .unwrap()
            .unwrap();
        transition.join().unwrap();
    });
    assert_eq!(phy_workers(&f, 0), 0);
    f.gateway.reopen_phy(&f.phys[0]).unwrap();
    assert_eq!(
        f.gateway
            .phylactery_profile(&f.instances[0])
            .unwrap()
            .display_name
            .as_deref(),
        Some("admitted before fence")
    );
    assert!(f.gateway.with_phy(&token, |_| Ok(())).is_err());
}

#[test]
fn quiesced_pending_target_does_not_stop_unrelated_background_vector_work() {
    let f = Fixture::new(true);
    let plan = f.freeze(false);
    f.gateway
        .quiesce_phy_for(&f.instances[0], &f.phys[0])
        .unwrap();
    let coordinator = publication_coordinator(&f.gateway, &f.rel);
    assert!(coordinator.publish(&plan, true).is_err());
    let endpoint = Arc::new(crate::SimulatedEmbeddingEndpoint::new(
        8,
        crate::VectorNormalization::L2,
        88,
    ));
    *f.gateway.routes.write().unwrap() =
        crate::ReliquaryRuntimeRoutes::new(None, None, None, None, Some(endpoint));
    let token = f
        .gateway
        .authorize(&f.instances[1], &f.phys[1], OwnerGrant::UseOwnPhy)
        .unwrap();
    let mut draft = memory(&f.claim, "unrelated-background", false).draft;
    draft.lifecycle_state = "canonical".into();
    let id = f
        .gateway
        .with_phy(&token, |owner| {
            let id = owner.publish_memory(None, 0, draft).unwrap().0.id;
            owner.sync().unwrap();
            Ok(id)
        })
        .unwrap();
    f.gateway.wake_phy(&f.phys[1]).unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        let profile = f
            .gateway
            .phy_executions
            .read()
            .unwrap()
            .get(&f.phys[1])
            .unwrap()
            .runtime
            .lock()
            .unwrap()
            .memory_profile()
            .unwrap();
        let complete = match profile {
            Some(profile) => f
                .gateway
                .with_phy(&token, |owner| {
                    let body = owner.memory_body_id(id).unwrap();
                    Ok(owner.memory_vector_location(profile, body).is_some())
                })
                .unwrap(),
            None => false,
        };
        if complete {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "unrelated PHY vector worker stopped"
        );
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert_eq!(phy_workers(&f, 0), 0);
    assert!(
        !coordinator
            .verify_receipt(&mut runtime(&f.gateway, &f.rel).lock().unwrap(), &plan)
            .unwrap()
    );
    f.gateway
        .reopen_phy_for(&f.instances[0], &f.phys[0])
        .unwrap();
    f.commit(&plan);
    coordinator.observe(&plan).unwrap();
    assert_eq!((f.private_count(0), f.private_count(1)), (1, 2));
}
