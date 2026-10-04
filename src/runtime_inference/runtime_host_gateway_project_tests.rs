use super::*;
use crate::runtime_host_test_support::test_path;
use crate::{Cva, EpisodePolicy, InsomniaWorkerConfig, InteractionRuntime, ReliquaryRuntimeRoutes};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug)]
enum ProjectOp {
    Root,
    Validate,
    Checkpoint,
    Ingest,
    Bytes,
}
const PROJECT: &[ProjectOp] = &[
    ProjectOp::Root,
    ProjectOp::Validate,
    ProjectOp::Checkpoint,
    ProjectOp::Ingest,
    ProjectOp::Bytes,
];
fn grants(owner: &str, caps: &[OwnerGrant]) -> BTreeMap<String, BTreeSet<OwnerGrant>> {
    BTreeMap::from([(owner.to_owned(), caps.iter().copied().collect())])
}
fn fixture() -> (
    Arc<GatewayRuntime>,
    String,
    Arc<InstanceRuntime>,
    PathBuf,
    PathBuf,
    crate::FileId,
) {
    let project = test_path("project-authorization");
    std::fs::create_dir_all(&project).unwrap();
    let rel_path = test_path("project-authorization.rel");
    let cva = crate::project_environment::create_project_rel(
        &rel_path,
        &project,
        crate::ProjectRepositoryMode::ManualLore,
    )
    .unwrap();
    let gateway = Arc::new(GatewayRuntime::new(
        test_path("project-authorization-state"),
        ReliquaryRuntimeRoutes::default(),
        InsomniaWorkerConfig {
            workers: 0,
            ..InsomniaWorkerConfig::default()
        },
        EpisodePolicy::default(),
    ));
    let owner = gateway.mount_rel(InteractionRuntime::new(cva)).unwrap();
    let manager = gateway
        .attach_instance(
            "project-manager".into(),
            None,
            grants(&owner, &[OwnerGrant::ViewRel, OwnerGrant::ManageProject]),
        )
        .unwrap();
    let source = project.join("upload.txt");
    std::fs::write(&source, b"authorized project bytes").unwrap();
    let file = gateway
        .ingest_project_attachment_for(&manager, &owner, &project, &source)
        .unwrap();
    (gateway, owner, manager, project, source, file.id)
}
impl ProjectOp {
    fn call(
        self,
        g: &GatewayRuntime,
        i: &InstanceRuntime,
        owner: &str,
        project: &std::path::Path,
        source: &std::path::Path,
        file: crate::FileId,
    ) -> Result<(), ReliquaryRuntimeHostError> {
        match self {
            Self::Root => g
                .project_repository_root_for(i, owner, project)
                .map(|root| assert!(root.is_dir())),
            Self::Validate => g
                .validate_project_environment_for(i, owner, project)
                .map(|_| ()),
            Self::Checkpoint => g
                .checkpoint_manual_project_for(i, owner, project)
                .map(|_| ()),
            Self::Ingest => g
                .ingest_project_attachment_for(i, owner, project, source)
                .map(|_| ()),
            Self::Bytes => g
                .resolved_file_bytes_for(i, owner, Some(project), file)
                .map(|bytes| assert_eq!(bytes, b"authorized project bytes")),
        }
    }
}

#[test]
fn project_families_require_the_exact_owner_and_applicable_grant() {
    let (g, owner, manager, project, source, file) = fixture();
    let other = g
        .mount_rel(InteractionRuntime::new(
            Cva::create_project(test_path("foreign-project.rel")).unwrap(),
        ))
        .unwrap();
    let viewer = g
        .attach_instance(
            "project-viewer".into(),
            None,
            grants(&owner, &[OwnerGrant::ViewRel]),
        )
        .unwrap();
    let mutate_only = g
        .attach_instance(
            "project-mutate-only".into(),
            None,
            grants(&owner, &[OwnerGrant::ManageProject]),
        )
        .unwrap();
    for op in PROJECT {
        op.call(&g, &manager, &owner, &project, &source, file)
            .unwrap();
        assert!(
            op.call(&g, &manager, &other, &project, &source, file)
                .is_err(),
            "{op:?}: wrong owner"
        );
        let wrong = if matches!(op, ProjectOp::Bytes) {
            &*mutate_only
        } else {
            &*viewer
        };
        let error = op
            .call(&g, wrong, &owner, &project, &source, file)
            .unwrap_err()
            .to_string();
        assert!(
            error.contains(if matches!(op, ProjectOp::Bytes) {
                "ViewRel"
            } else {
                "ManageProject"
            }),
            "{op:?}: {error}"
        );
    }
    g.replace_instance_grants(&manager, BTreeMap::new())
        .unwrap();
    for op in PROJECT {
        assert!(
            op.call(&g, &manager, &owner, &project, &source, file)
                .is_err(),
            "{op:?}: revoked"
        );
    }
    g.replace_instance_grants(
        &manager,
        grants(&owner, &[OwnerGrant::ViewRel, OwnerGrant::ManageProject]),
    )
    .unwrap();
    g.detach_instance(&manager).unwrap();
    for op in PROJECT {
        assert!(
            op.call(&g, &manager, &owner, &project, &source, file)
                .is_err(),
            "{op:?}: detached"
        );
    }
    for i in [&*viewer, &*mutate_only] {
        g.detach_instance(i).unwrap();
    }
    drop(g.unmount_rel(&owner).unwrap());
    drop(g.unmount_rel(&other).unwrap());
}

#[test]
fn project_io_races_reauthorize_before_commit_or_delivery() {
    for op in PROJECT {
        for detach in [false, true] {
            let (g, owner, manager, project, source, file) = fixture();
            if matches!(op, ProjectOp::Ingest) {
                std::fs::write(&source, b"new uncommitted race content").unwrap();
            }
            if matches!(op, ProjectOp::Checkpoint) {
                std::fs::write(
                    project.join("checkpoint-change.txt"),
                    b"new checkpoint content",
                )
                .unwrap();
            }
            let execution =
                Arc::clone(&g.rel_executions.read().unwrap().get(&owner).unwrap().owner);
            let runtime = Arc::clone(execution.lock().unwrap().runtime.as_ref().unwrap());
            let journal = g.conversation.journal_for_test(&owner).unwrap();
            let before = {
                let runtime = runtime.lock().unwrap();
                (
                    runtime.cva().files().len(),
                    runtime.cva().project_revision_correlations().len(),
                )
            };
            let (entered_tx, entered_rx) = std::sync::mpsc::channel();
            let (release_tx, release_rx) = std::sync::mpsc::channel();
            let release_rx = Mutex::new(release_rx);
            let hook_execution = Arc::clone(&execution);
            let hook_runtime = Arc::clone(&runtime);
            let hook_journal = Arc::clone(&journal);
            *g.project_io_test_hook.lock().unwrap() = Some(Arc::new(move || {
                assert!(
                    hook_execution.try_lock().is_ok(),
                    "project I/O held owner execution"
                );
                let deadline = Instant::now() + Duration::from_secs(5);
                loop {
                    if hook_runtime.try_lock().is_ok() {
                        break;
                    }
                    assert!(
                        Instant::now() < deadline,
                        "project I/O held semantic runtime"
                    );
                    std::thread::yield_now();
                }
                assert!(
                    hook_journal.try_lock().is_ok(),
                    "project I/O held journal writer"
                );
                entered_tx.send(()).unwrap();
                release_rx
                    .lock()
                    .unwrap()
                    .recv_timeout(Duration::from_secs(10))
                    .unwrap();
            }));
            let worker_gateway = Arc::clone(&g);
            let worker_manager = Arc::clone(&manager);
            let worker_owner = owner.clone();
            let operation = *op;
            let worker = std::thread::spawn(move || {
                operation.call(
                    &worker_gateway,
                    &worker_manager,
                    &worker_owner,
                    &project,
                    &source,
                    file,
                )
            });
            entered_rx.recv_timeout(Duration::from_secs(10)).unwrap();
            if detach {
                g.detach_instance(&manager).unwrap();
            } else {
                g.replace_instance_grants(&manager, BTreeMap::new())
                    .unwrap();
            }
            release_tx.send(()).unwrap();
            assert!(
                worker.join().unwrap().is_err(),
                "{op:?}: stale commit/delivery"
            );
            *g.project_io_test_hook.lock().unwrap() = None;
            let after = {
                let runtime = runtime.lock().unwrap();
                (
                    runtime.cva().files().len(),
                    runtime.cva().project_revision_correlations().len(),
                )
            };
            assert_eq!(
                after, before,
                "{op:?}: authorization loss published REL state"
            );
            if !detach {
                g.detach_instance(&manager).unwrap();
            }
            drop(runtime);
            drop(execution);
            drop(journal);
            drop(g.unmount_rel(&owner).unwrap());
        }
    }
}
