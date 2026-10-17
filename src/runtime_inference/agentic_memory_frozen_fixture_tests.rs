//! Ignored, local-only acceptance runner. Only disposable copies may be supplied.
use super::*;
use crate::{
    AgenticMemoryController, AgenticMemoryHandoffConfirmation, AgenticMemoryOutcome, Memory,
    MemoryId,
};
use serde_json::{Value, json};
use std::sync::atomic::{AtomicUsize, Ordering};
#[path = "agentic_memory_frozen_fixture_endpoints.rs"]
mod endpoints;
#[path = "agentic_memory_frozen_fixture_fidelity.rs"]
mod fidelity;
#[path = "agentic_memory_frozen_fixture_scenarios.rs"]
mod scenarios;
use endpoints::{Oracle, Replay};
struct Fixture {
    gateway: GatewayRuntime,
    instance: Arc<InstanceRuntime>,
    owner: String,
    memories: BTreeMap<MemoryId, Memory>,
    vectors: BTreeMap<MemoryId, Vec<f32>>,
    embedding: Arc<Replay>,
    oracle: Arc<Oracle>,
    metadata: Value,
}
impl Fixture {
    fn open(path: &std::path::Path) -> Result<Self, crate::CvaError> {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .canonicalize()
            .unwrap();
        assert!(
            path.canonicalize().unwrap().starts_with(&root),
            "fixture must be a worktree-local disposable copy"
        );
        let mut rel = crate::Cva::open(path)?;
        let mut memories = BTreeMap::new();
        for id in rel.memories.current_ids() {
            let body = rel.memory(id).unwrap();
            if !body.archived {
                memories.insert(id, body);
            }
        }
        let mut profiles = rel.compatibility_profiles();
        profiles.sort_by_key(|profile| profile.id.0);
        let (profile, vectors) = profiles
            .into_iter()
            .map(|profile| {
                let (_, vectors) = crate::memory_retrieval_vectors::load_searchable_memory_vectors(
                    &mut rel.container,
                    &rel.memories,
                    &rel.memory_vectors,
                    &rel.packed_vectors,
                    profile.id,
                )
                .unwrap();
                (profile, vectors)
            })
            .max_by_key(|(_, vectors)| vectors.len())
            .expect("retained profile required");
        let vectors = vectors.into_iter().collect::<BTreeMap<_, _>>();
        let vector = vectors
            .values()
            .next()
            .expect("retained current Memory vector required")
            .clone();
        let metadata = json!({"active_memories":memories.len(),"current_vectors":vectors.len(),
            "profile":crate::agentic_memory_retrieval_manifest::hex_id(MemoryId(profile.id.0)),
            "dimensions":profile.dimensions,"activity_cut":rel.rel_turn_count(),
            "graph_relations":rel.graph_relations().len(),
            "inference":"stored compatibility probes and target document vector replay; no live embedding or general inference"});
        let embedding = Arc::new(Replay {
            profile,
            query: Mutex::new(vector),
            calls: AtomicUsize::new(0),
        });
        let oracle = Arc::new(Oracle {
            mode: AtomicUsize::new(0),
            calls: Mutex::new(Vec::new()),
        });
        let routes = ReliquaryRuntimeRoutes::new(None, None, None, None, Some(embedding.clone()))
            .with_memory_retrieval_decision(Some(oracle.clone()));
        let gateway = GatewayRuntime::new(
            path.parent().unwrap().join("state"),
            routes,
            InsomniaWorkerConfig {
                workers: 0,
                ..Default::default()
            },
            EpisodePolicy::default(),
        );
        let owner = gateway.mount_rel(InteractionRuntime::new(rel)).unwrap();
        let instance = gateway
            .attach_instance(
                "frozen-fixture-evaluator".into(),
                None,
                BTreeMap::from([(owner.clone(), BTreeSet::from([OwnerGrant::ViewRel]))]),
            )
            .unwrap();
        gateway.select_rel(&instance, &owner).unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
        loop {
            if gateway.capture_agentic_read(&instance).unwrap().owners[0].profile
                == Some(embedding.profile.id)
            {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "retained replay profile was not published"
            );
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        Ok(Self {
            gateway,
            instance,
            owner,
            memories,
            vectors,
            embedding,
            oracle,
            metadata,
        })
    }
    fn reference(&self, id: MemoryId) -> MemoryRef {
        MemoryRef {
            owner_id: self.owner.clone(),
            memory_id: id,
        }
    }
    fn controller() -> AgenticMemoryController {
        let mut controller = AgenticMemoryController::new();
        controller
            .stage_query("retained fixture evidence")
            .unwrap()
            .commit(&mut controller)
            .unwrap();
        controller
    }
    fn complete(
        &self,
        controller: &mut AgenticMemoryController,
        prepared: &crate::PreparedAgenticMemoryResult,
    ) {
        self.gateway
            .complete_agentic_memory_delivery(
                &self.instance,
                controller,
                prepared,
                &AgenticMemoryHandoffConfirmation::Succeeded {
                    delivery_id: prepared.delivery_id().into(),
                    manifest_digest: prepared.manifest_digest(),
                },
            )
            .unwrap();
    }
    fn snapshot(&self) -> (u64, u64, BTreeMap<MemoryId, crate::FreshnessRecord>) {
        let envelope = self.gateway.capture_agentic_read(&self.instance).unwrap();
        self.gateway
            .with_rel(&envelope.owners[0].token, |runtime| {
                Ok((
                    std::fs::metadata(runtime.cva.container.path())
                        .unwrap()
                        .len(),
                    runtime.cva.rel_turn_count(),
                    runtime.cva.freshness.records().clone(),
                ))
            })
            .unwrap()
    }
}
#[test]
#[ignore = "requires worktree-local copies of authoritative REL fixtures; use scripts/agentic_memory_fixture_acceptance.py"]
fn agentic_frozen_fixture_acceptance() {
    let path = PathBuf::from(
        std::env::var_os("RELIQUARY_AGENTIC_FIXTURE").expect("fixture copy required"),
    );
    let phase = std::env::var("RELIQUARY_AGENTIC_PHASE").unwrap();
    let result = match Fixture::open(&path) {
        Ok(fixture) => match phase.as_str() {
            "inventory" => json!({"status":"passed","inventory":fixture.metadata}),
            "functional" => scenarios::run(&fixture),
            "fidelity" => fidelity::run(&fixture),
            _ => panic!("unknown fixture phase"),
        },
        Err(crate::CvaError::Freshness(message))
            if message.contains("unsupported Freshness prototype format") =>
        {
            corpus_format_limitation(&path, &phase, &message)
        }
        Err(error) => panic!("unexpected fixture open failure: {error}"),
    };
    std::fs::write(
        std::env::var_os("RELIQUARY_AGENTIC_RESULT").unwrap(),
        serde_json::to_vec_pretty(&result).unwrap(),
    )
    .unwrap();
}

fn corpus_format_limitation(path: &std::path::Path, phase: &str, message: &str) -> Value {
    let mut container = crate::Container::open(path).unwrap();
    let chunks = container.chunks().unwrap();
    let count = chunks.len();
    let mut formats = BTreeMap::<String, usize>::new();
    for chunk in chunks {
        let payload = container.read(chunk).unwrap();
        if payload.starts_with(b"CVAFRS") {
            *formats
                .entry(String::from_utf8_lossy(&payload[..8.min(payload.len())]).into_owned())
                .or_default() += 1;
        }
    }
    assert!(
        container.owner_uuid().is_some(),
        "this limitation is for current-owner prototype inputs"
    );
    drop(container);
    let migration = crate::migrate_file(
        path,
        path.parent().unwrap().join("legacy-migration-output.rel"),
    )
    .unwrap_err();
    assert!(matches!(migration, crate::MigrationError::AlreadyCurrent));
    json!({"status":"corpus_limitation","phase":phase,"reason":message,"physical_chunks":count,
        "freshness_formats":formats,"required_freshness_format":"CVAFRS02","current_owner_uuid":true,
        "existing_migration":"migrate_file rejects AlreadyCurrent; it is not a prototype Freshness converter",
        "functional_scenarios_executed":0,"retrieval_calls":0,"candidate_counts":null,
        "evidence_recall":null,"answer_recall":null,"latency_us":null,
        "blocked_scenarios":["Freshness ordering","more-than-ten one-hop neighbors","repeat traversal","refinement",
            "shared-neighbor concurrency","already_provided","failed handoff","success and retry","broader-context fidelity"],
        "policy":"No bypass, reconstruction, prototype decoder, synthetic replacement corpus or new runtime migration path was added"})
}

#[test]
fn agentic_fixture_runner_current_format_protocol_smoke() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target/test-tmp")
        .join(format!("agentic-fixture-smoke-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("owner.rel");
    let mut rel = crate::Cva::create_project(&path).unwrap();
    let episode = crate::community_test_support::rel_episode(&mut rel);
    let names = [
        "albatross",
        "blueberry",
        "carpenter",
        "dandelion",
        "elephant",
        "fireplace",
        "grapefruit",
        "honeycomb",
        "infinity",
        "jellyfish",
        "kangaroo",
        "lavender",
        "mountain",
        "nectarine",
        "orchestra",
        "porcelain",
        "quicksand",
        "radiation",
        "strawberry",
        "telescope",
        "umbrella",
        "velocity",
        "waterfall",
        "xylophone",
    ];
    let ids = names
        .iter()
        .map(|name| crate::community_test_support::rel_memory(&mut rel, &episode, name))
        .collect::<Vec<_>>();
    for anchor in &ids[..2] {
        for neighbor in &ids[2..] {
            rel.set_memory_relation(
                *anchor,
                *neighbor,
                crate::GraphRelationKind::Topical,
                true,
                rel.graph_version(),
            )
            .unwrap();
        }
    }
    let endpoint = crate::SimulatedEmbeddingEndpoint::new(8, crate::VectorNormalization::L2, 711);
    let profile = rel.establish_compatibility_profile(&endpoint).unwrap();
    rel.build_missing_memory_vectors(profile.id, &endpoint)
        .unwrap();
    rel.sync().unwrap();
    drop(rel);
    let fixture = Fixture::open(&path).unwrap();
    let functional = scenarios::run(&fixture);
    assert_eq!(functional["repeated_one_hop"]["status"], "passed");
    assert_eq!(functional["concurrent_shared_neighbor"]["status"], "passed");
    assert!(functional["refinement"]["reconsidered"].as_u64().unwrap() > 10);
    let fidelity = fidelity::run(&fixture);
    assert_eq!(fidelity["tasks"].as_array().unwrap().len(), 12);
}
