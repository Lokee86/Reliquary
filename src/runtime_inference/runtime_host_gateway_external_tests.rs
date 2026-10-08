use super::*;
use crate::external_source::tests::{Endpoint, Host, SemanticEndpoint};
use crate::external_source::*;
use crate::runtime_host_test_support::test_path;

struct Fixture {
    gateway: Arc<GatewayRuntime>,
    instance: Arc<InstanceRuntime>,
    rel: String,
    phy: String,
    host: Host,
}
impl Fixture {
    fn new() -> Self {
        let gateway = Arc::new(GatewayRuntime::new(
            test_path("external-state"),
            ReliquaryRuntimeRoutes::new(None, None, None, None, None),
            InsomniaWorkerConfig {
                workers: 0,
                ..Default::default()
            },
            EpisodePolicy::default(),
        ));
        let rel = gateway
            .mount_rel(InteractionRuntime::new(
                crate::Cva::create_project(test_path("external.rel")).unwrap(),
            ))
            .unwrap();
        let phy = gateway
            .mount_phy(Phylactery::create(test_path("external.phy")).unwrap())
            .unwrap();
        let grants = BTreeMap::from([
            (
                rel.clone(),
                BTreeSet::from([OwnerGrant::ViewRel, OwnerGrant::ManageRel]),
            ),
            (phy.clone(), BTreeSet::from([OwnerGrant::UseOwnPhy])),
        ]);
        let instance = gateway
            .attach_instance("alice".into(), Some(phy.clone()), grants)
            .unwrap();
        Self {
            gateway,
            instance,
            rel,
            phy,
            host: Host::new("The project uses Rust. Alice prefers concise replies."),
        }
    }
    fn rebuild(&self) {
        self.gateway
            .rebuild_external_index_for(&self.instance, &self.rel, &self.host, &Endpoint)
            .unwrap();
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.gateway.shutdown().unwrap();
    }
}

#[test]
fn external_reference_delivery_requires_both_rel_and_host_authority() {
    let f = Fixture::new();
    f.rebuild();
    let hits = f
        .gateway
        .search_external_lexical_for(&f.instance, &f.rel, &f.host, "Rust", 10)
        .unwrap();
    assert_eq!(hits.len(), 1);
    let hydrated = f
        .gateway
        .hydrate_external_chunks_for(&f.instance, &f.rel, &f.host, &[hits[0].reference.clone()])
        .unwrap();
    assert!(hydrated[0].content.contains("Rust"));
    let mut bad = hits[0].reference.clone();
    bad.start = bad.end;
    assert!(
        f.gateway
            .hydrate_external_chunks_for(&f.instance, &f.rel, &f.host, &[bad])
            .is_err()
    );
    f.host.deny();
    assert_eq!(
        f.gateway
            .search_external_lexical_for(&f.instance, &f.rel, &f.host, "Rust", 10),
        Err(ExternalSourceError::Unauthorized)
    );
    assert_eq!(
        f.gateway
            .update_external_index_for(&f.instance, &f.rel, &f.host, &Endpoint),
        Err(ExternalSourceError::Unauthorized)
    );
}

#[test]
fn external_insomnia_routes_private_output_and_replays_without_inference() {
    let f = Fixture::new();
    f.rebuild();
    // SemanticEndpoint counts the shared ledger/ownership/wording calls.
    let count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let endpoint = || SemanticEndpoint {
        calls: count.clone(),
    };
    let extractor = crate::InsomniaExtractor::new(endpoint()).with_ownership_endpoint(endpoint());
    let refs = f.host.refs();
    let result = f
        .gateway
        .process_external_window_for(
            &f.instance,
            &f.rel,
            "project",
            &f.host,
            &refs,
            &extractor,
            100,
        )
        .unwrap();
    assert_eq!(result.project.len(), 1);
    assert_eq!(result.user.len(), 1);
    assert!(!result.replayed);
    assert_eq!(result.user[0].owner_id, f.phy);
    let before = count.load(Ordering::SeqCst);
    let replay = f
        .gateway
        .process_external_window_for(
            &f.instance,
            &f.rel,
            "project",
            &f.host,
            &refs,
            &extractor,
            200,
        )
        .unwrap();
    assert!(replay.replayed);
    assert_eq!(replay.project, result.project);
    assert_eq!(replay.user, result.user);
    assert_eq!(count.load(Ordering::SeqCst), before);
    let other = f
        .gateway
        .mount_phy(Phylactery::create(test_path("external-other.phy")).unwrap())
        .unwrap();
    let bob = f
        .gateway
        .attach_instance(
            "bob".into(),
            Some(other.clone()),
            BTreeMap::from([
                (f.rel.clone(), BTreeSet::from([OwnerGrant::ViewRel])),
                (other.clone(), BTreeSet::from([OwnerGrant::UseOwnPhy])),
            ]),
        )
        .unwrap();
    assert!(
        f.gateway
            .read_external_memory_evidence_for(&bob, &f.phy, result.user[0].memory_id)
            .is_err()
    );
    assert!(matches!(
        f.gateway
            .read_memory_source_evidence_for(&f.instance, &f.phy, result.user[0].memory_id)
            .unwrap(),
        Some(SourceEvidenceRef::External(_))
    ));
    let token = f
        .gateway
        .authorize(&f.instance, &f.rel, OwnerGrant::ViewRel)
        .unwrap();
    f.gateway
        .with_rel(&token, |r| {
            assert!(r.cva.episodes().is_empty());
            Ok(())
        })
        .unwrap();
}

#[test]
fn external_partial_phy_commit_resumes_frozen_plan_and_missing_receipt_fails_closed() {
    let f = Fixture::new();
    f.rebuild();
    let count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let endpoint = || SemanticEndpoint {
        calls: count.clone(),
    };
    let extractor = crate::InsomniaExtractor::new(endpoint()).with_ownership_endpoint(endpoint());
    let rel = f
        .gateway
        .authorize(&f.instance, &f.rel, OwnerGrant::ManageRel)
        .unwrap();
    let phy = f
        .gateway
        .authorize(&f.instance, &f.phy, OwnerGrant::UseOwnPhy)
        .unwrap();
    let state = f
        .gateway
        .with_rel(&rel, |r| {
            Ok(r.cva.external_index.sources["host:test"].clone())
        })
        .unwrap();
    let refs = f.host.refs();
    let prepared = crate::external_source::processing::prepare(
        &f.rel, "alice", "project", state, &refs, &f.host, &extractor, 100,
    )
    .unwrap();
    let plan =
        super::external_plan::ExternalWindowPlan::freeze(&prepared, "alice", Some(&f.phy)).unwrap();
    let path = super::external_plan::path(&f.gateway.state_dir, &f.rel, prepared.window);
    plan.install(&path).unwrap();
    f.gateway
        .with_phy(&phy, |p| {
            crate::external_source::publication::publish(
                &mut p.container,
                &mut p.memories,
                prepared.window,
                &prepared.user,
                None,
            )
            .map_err(operation)
        })
        .unwrap();
    let before = count.load(Ordering::SeqCst);
    let result = f
        .gateway
        .process_external_window_for(
            &f.instance,
            &f.rel,
            "project",
            &f.host,
            &refs,
            &extractor,
            200,
        )
        .unwrap();
    assert_eq!(count.load(Ordering::SeqCst), before);
    assert_eq!(result.project.len(), 1);
    assert_eq!(result.user.len(), 1);
    f.gateway
        .with_phy(&phy, |p| {
            p.memories.external_windows.clear();
            Ok(())
        })
        .unwrap();
    assert_eq!(
        f.gateway.process_external_window_for(
            &f.instance,
            &f.rel,
            "project",
            &f.host,
            &refs,
            &extractor,
            300
        ),
        Err(ExternalSourceError::Storage)
    );
}

struct RevokingSource<'a> {
    host: &'a Host,
    revoke: Mutex<Option<Box<dyn FnOnce() + Send + 'a>>>,
}
impl ExternalConversationSource for RevokingSource<'_> {
    fn source_namespace(&self) -> &str {
        self.host.source_namespace()
    }
    fn authorize(&self, p: &str) -> Result<(), ExternalSourceError> {
        self.host.authorize(p)
    }
    fn changes_after(
        &self,
        p: &str,
        c: u64,
        n: usize,
    ) -> Result<ExternalChangeBatch, ExternalSourceError> {
        self.host.changes_after(p, c, n)
    }
    fn snapshot_manifest(&self, p: &str) -> Result<ExternalSnapshotManifest, ExternalSourceError> {
        self.host.snapshot_manifest(p)
    }
    fn hydrate(
        &self,
        p: &str,
        d: &[ExternalMessageDescriptor],
    ) -> Result<Vec<HydratedExternalMessage>, ExternalSourceError> {
        let callback = self.revoke.lock().unwrap().take();
        if let Some(callback) = callback {
            callback();
        }
        self.host.hydrate(p, d)
    }
}
#[test]
fn external_host_io_holds_no_admission_lease_and_original_grant_fences_installation() {
    let f = Fixture::new();
    let source = RevokingSource {
        host: &f.host,
        revoke: Mutex::new(Some(Box::new(|| {
            let mut grants = f.instance.grants_snapshot().unwrap();
            grants
                .get_mut(&f.rel)
                .unwrap()
                .remove(&OwnerGrant::ManageRel);
            f.gateway
                .replace_instance_grants(&f.instance, grants)
                .unwrap();
        }))),
    };
    assert!(
        f.gateway
            .rebuild_external_index_for(&f.instance, &f.rel, &source, &Endpoint)
            .is_err()
    );
    let token = f
        .gateway
        .authorize(&f.instance, &f.rel, OwnerGrant::ViewRel)
        .unwrap();
    assert!(
        f.gateway
            .with_rel(&token, |r| Ok(r.cva.external_index.sources.is_empty()))
            .unwrap()
    );
}

struct ManifestSource<'a> {
    host: &'a Host,
    hidden: bool,
    stale: bool,
    callback: Mutex<Option<Box<dyn FnOnce() + Send + 'a>>>,
}
impl ExternalConversationSource for ManifestSource<'_> {
    fn source_namespace(&self) -> &str {
        self.host.source_namespace()
    }
    fn authorize(&self, p: &str) -> Result<(), ExternalSourceError> {
        self.host.authorize(p)
    }
    fn changes_after(
        &self,
        p: &str,
        c: u64,
        n: usize,
    ) -> Result<ExternalChangeBatch, ExternalSourceError> {
        self.host.changes_after(p, c, n)
    }
    fn snapshot_manifest(&self, p: &str) -> Result<ExternalSnapshotManifest, ExternalSourceError> {
        let mut manifest = self.host.snapshot_manifest(p)?;
        if self.hidden {
            manifest.messages.clear();
        }
        if self.stale {
            manifest.messages[0].reference.content_hash = [7; 32];
        }
        let callback = self.callback.lock().unwrap().take();
        if let Some(callback) = callback {
            callback();
        }
        Ok(manifest)
    }
    fn hydrate(
        &self,
        p: &str,
        d: &[ExternalMessageDescriptor],
    ) -> Result<Vec<HydratedExternalMessage>, ExternalSourceError> {
        self.host.hydrate(p, d)
    }
}
#[test]
fn external_search_requires_message_authority_and_current_hash_without_index_refresh() {
    let f = Fixture::new();
    f.rebuild();
    for (hidden, stale, expected) in [
        (true, false, ExternalSourceError::Unauthorized),
        (false, true, ExternalSourceError::StaleEvidence),
    ] {
        let source = ManifestSource {
            host: &f.host,
            hidden,
            stale,
            callback: Mutex::new(None),
        };
        assert_eq!(
            f.gateway
                .search_external_lexical_for(&f.instance, &f.rel, &source, "Rust", 10),
            Err(expected)
        );
        assert_eq!(
            f.gateway.search_external_source_for(
                &f.instance,
                &f.rel,
                &source,
                &Endpoint,
                "Rust",
                10
            ),
            Err(expected)
        );
    }
}
#[test]
fn external_search_fences_index_mutation_during_host_delivery_check() {
    for vector in [false, true] {
        let f = Fixture::new();
        f.rebuild();
        let source = ManifestSource {
            host: &f.host,
            hidden: false,
            stale: false,
            callback: Mutex::new(Some(Box::new(|| {
                f.rebuild();
            }))),
        };
        let result = if vector {
            f.gateway.search_external_source_for(
                &f.instance,
                &f.rel,
                &source,
                &Endpoint,
                "Rust",
                10,
            )
        } else {
            f.gateway
                .search_external_lexical_for(&f.instance, &f.rel, &source, "Rust", 10)
        };
        assert_eq!(result, Err(ExternalSourceError::Conflict));
    }
}
#[test]
fn external_completed_window_requires_fresh_message_authority_on_replay() {
    let f = Fixture::new();
    f.rebuild();
    let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let endpoint = || SemanticEndpoint {
        calls: calls.clone(),
    };
    let extractor = crate::InsomniaExtractor::new(endpoint()).with_ownership_endpoint(endpoint());
    let refs = f.host.refs();
    f.gateway
        .process_external_window_for(
            &f.instance,
            &f.rel,
            "project",
            &f.host,
            &refs,
            &extractor,
            100,
        )
        .unwrap();
    let before = calls.load(Ordering::SeqCst);
    let source = ManifestSource {
        host: &f.host,
        hidden: true,
        stale: false,
        callback: Mutex::new(None),
    };
    assert_eq!(
        f.gateway.process_external_window_for(
            &f.instance,
            &f.rel,
            "project",
            &source,
            &refs,
            &extractor,
            200
        ),
        Err(ExternalSourceError::Unauthorized)
    );
    assert_eq!(calls.load(Ordering::SeqCst), before);
}
