use super::*;
use crate::{
    AgenticMemoryController, AgenticMemoryError, DecisionEndpoint, DecisionEndpointError,
    GraphRelationKind, MemoryRef, PreparedAgenticMemoryResult, SimulatedEmbeddingEndpoint,
    VectorNormalization,
};
use serde_json::{Value, json};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Operation {
    Query,
    Refinement,
    Traversal,
}

impl Operation {
    fn name(self) -> &'static str {
        match self {
            Self::Query => "memory_query",
            Self::Refinement => "refine_query",
            Self::Traversal => "traverse_graph",
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum SelectionAbuse {
    JsonString,
    JsonArray,
    JsonNull,
    MissingApproved,
    ApprovedWrongShape,
    UnknownField,
    MalformedIdentity,
    UnknownMemory,
    StaleOwner,
    DuplicateIdentity,
    WrongVersion,
    ProviderError,
}

impl SelectionAbuse {
    const ALL: [Self; 12] = [
        Self::JsonString,
        Self::JsonArray,
        Self::JsonNull,
        Self::MissingApproved,
        Self::ApprovedWrongShape,
        Self::UnknownField,
        Self::MalformedIdentity,
        Self::UnknownMemory,
        Self::StaleOwner,
        Self::DuplicateIdentity,
        Self::WrongVersion,
        Self::ProviderError,
    ];
}

#[derive(Clone, Copy, Debug)]
enum RewriteAbuse {
    Empty,
    Oversized,
    WrongVersion,
    MalformedShape,
    UnknownField,
    MultipleResults,
    ProviderError,
}

struct ContractEndpoint {
    selection_abuse: SelectionAbuse,
    rewrite_abuse: Option<RewriteAbuse>,
    calls: Mutex<Vec<String>>,
}

impl ContractEndpoint {
    fn malformed_identity(&self, state: &Value) -> Value {
        let mut identity = state["candidates"][0]["memory"].clone();
        identity["memory_id"] = json!(42);
        json!({"version": 1, "approved": [identity]})
    }

    fn unknown_owner(&self, state: &Value) -> Value {
        let mut identity = state["candidates"][0]["memory"].clone();
        identity["owner_id"] = json!("stale-owner-that-was-not-in-the-batch");
        json!({"version": 1, "approved": [identity]})
    }

    fn unknown_memory(&self, state: &Value) -> Value {
        let mut identity = state["candidates"][0]["memory"].clone();
        let original = identity["memory_id"].as_str().unwrap();
        let supplied = state["candidates"]
            .as_array()
            .unwrap()
            .iter()
            .map(|candidate| candidate["memory"]["memory_id"].as_str().unwrap())
            .collect::<BTreeSet<_>>();
        let hex = b"0123456789abcdef";
        let mut replacement = None;
        'positions: for position in 0..original.len() {
            for digit in hex {
                if *digit == original.as_bytes()[position] {
                    continue;
                }
                let mut candidate = original.as_bytes().to_vec();
                candidate[position] = *digit;
                let candidate = String::from_utf8(candidate).unwrap();
                if !supplied.contains(candidate.as_str()) {
                    replacement = Some(candidate);
                    break 'positions;
                }
            }
        }
        identity["memory_id"] = json!(replacement.expect("a distinct valid identity exists"));
        json!({"version": 1, "approved": [identity]})
    }

    fn duplicate_identity(&self, state: &Value) -> Value {
        let identity = state["candidates"][0]["memory"].clone();
        json!({"version": 1, "approved": [identity.clone(), identity]})
    }

    fn selection_response(&self, state: &Value) -> Result<Value, DecisionEndpointError> {
        match self.selection_abuse {
            SelectionAbuse::JsonString => Ok(json!("not an object")),
            SelectionAbuse::JsonArray => Ok(json!([{"version": 1, "approved": []}])),
            SelectionAbuse::JsonNull => Ok(Value::Null),
            SelectionAbuse::MissingApproved => Ok(json!({"version": 1})),
            SelectionAbuse::ApprovedWrongShape => {
                Ok(json!({"version": 1, "approved": "approve everything"}))
            }
            SelectionAbuse::UnknownField => Ok(json!({
                "version": 1,
                "approved": [],
                "commentary": "unsupported"
            })),
            SelectionAbuse::MalformedIdentity => Ok(self.malformed_identity(state)),
            SelectionAbuse::UnknownMemory => Ok(self.unknown_memory(state)),
            SelectionAbuse::StaleOwner => Ok(self.unknown_owner(state)),
            SelectionAbuse::DuplicateIdentity => Ok(self.duplicate_identity(state)),
            SelectionAbuse::WrongVersion => Ok(json!({"version": 99, "approved": []})),
            SelectionAbuse::ProviderError => Err(DecisionEndpointError::Failure(
                "injected provider failure".into(),
            )),
        }
    }

    fn rewrite_response(&self) -> Result<Value, DecisionEndpointError> {
        match self.rewrite_abuse {
            None => Ok(json!({"version": 1, "query": "corrected complete query"})),
            Some(RewriteAbuse::Empty) => Ok(json!({"version": 1, "query": "  "})),
            Some(RewriteAbuse::Oversized) => Ok(json!({
                "version": 1,
                "query": "x".repeat(crate::MAX_MEMORY_SEARCH_QUERY_BYTES + 1)
            })),
            Some(RewriteAbuse::WrongVersion) => {
                Ok(json!({"version": 2, "query": "corrected complete query"}))
            }
            Some(RewriteAbuse::MalformedShape) => Ok(json!("corrected complete query")),
            Some(RewriteAbuse::UnknownField) => Ok(json!({
                "version": 1,
                "query": "corrected complete query",
                "alternatives": ["another query"]
            })),
            Some(RewriteAbuse::MultipleResults) => Ok(json!({
                "version": 1,
                "query": ["first complete query", "second complete query"]
            })),
            Some(RewriteAbuse::ProviderError) => Err(DecisionEndpointError::Failure(
                "injected rewrite provider failure".into(),
            )),
        }
    }
}

impl DecisionEndpoint for ContractEndpoint {
    fn model(&self) -> &str {
        "agentic-contract-abuse-test"
    }

    fn evaluate(&self, state: &Value, _: &Value) -> Result<Value, DecisionEndpointError> {
        let operation = state["operation"]
            .as_str()
            .ok_or(DecisionEndpointError::InvalidResponse("operation missing"))?;
        self.calls.lock().unwrap().push(operation.to_owned());
        if operation == "refine_query_rewrite" {
            self.rewrite_response()
        } else {
            self.selection_response(state)
        }
    }
}

struct ContractFixture {
    gateway: GatewayRuntime,
    instance: Arc<InstanceRuntime>,
    owner: String,
    memory_ids: Vec<crate::MemoryId>,
    endpoint: Arc<ContractEndpoint>,
}

fn fixture(
    selection_abuse: SelectionAbuse,
    rewrite_abuse: Option<RewriteAbuse>,
) -> ContractFixture {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target/test-tmp")
        .join(format!("agentic-contract-abuse-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let mut rel = crate::Cva::create_project(root.join("owner.rel")).unwrap();
    let episode = crate::community_test_support::rel_episode(&mut rel);
    let memory_ids = (0..4)
        .map(|n| {
            crate::community_test_support::rel_memory(&mut rel, &episode, &format!("needle-{n}"))
        })
        .collect::<Vec<_>>();
    rel.set_memory_relation(
        memory_ids[0],
        memory_ids[1],
        GraphRelationKind::Factual,
        true,
        rel.graph_version(),
    )
    .unwrap();

    let embedding = Arc::new(SimulatedEmbeddingEndpoint::new(
        8,
        VectorNormalization::L2,
        421,
    ));
    let profile = rel
        .establish_compatibility_profile(embedding.as_ref())
        .unwrap();
    rel.build_missing_memory_vectors(profile.id, embedding.as_ref())
        .unwrap();
    let endpoint = Arc::new(ContractEndpoint {
        selection_abuse,
        rewrite_abuse,
        calls: Mutex::new(Vec::new()),
    });
    let routes = ReliquaryRuntimeRoutes::new(None, None, None, None, Some(embedding))
        .with_memory_retrieval_decision(Some(endpoint.clone()));
    let gateway = GatewayRuntime::new(
        root.join("state"),
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
            "contract-abuse-reader".into(),
            None,
            BTreeMap::from([(owner.clone(), BTreeSet::from([OwnerGrant::ViewRel]))]),
        )
        .unwrap();
    gateway.select_rel(&instance, &owner).unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        if gateway.capture_agentic_read(&instance).unwrap().owners[0]
            .profile
            .is_some()
        {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "vector worker did not publish its profile"
        );
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    ContractFixture {
        gateway,
        instance,
        owner,
        memory_ids,
        endpoint,
    }
}

fn controller_for(fixture: &ContractFixture, operation: Operation) -> AgenticMemoryController {
    let mut controller = AgenticMemoryController::new();
    controller
        .stage_query("old complete information need")
        .unwrap()
        .commit(&mut controller)
        .unwrap();
    match operation {
        Operation::Query => {
            controller.searched_ids.insert(MemoryRef {
                owner_id: fixture.owner.clone(),
                memory_id: fixture.memory_ids[0],
            });
            controller.selected_ids.insert(MemoryRef {
                owner_id: fixture.owner.clone(),
                memory_id: fixture.memory_ids[0],
            });
        }
        Operation::Refinement => {
            controller.searched_ids.insert(MemoryRef {
                owner_id: fixture.owner.clone(),
                memory_id: fixture.memory_ids[0],
            });
        }
        Operation::Traversal => {}
    }
    controller
}

fn run_operation(
    fixture: &ContractFixture,
    controller: &mut AgenticMemoryController,
    operation: Operation,
) -> Result<PreparedAgenticMemoryResult, AgenticMemoryError> {
    match operation {
        Operation::Query => {
            fixture
                .gateway
                .memory_query(&fixture.instance, controller, "needle", "abuse-query")
        }
        Operation::Refinement => fixture.gateway.refine_query(
            &fixture.instance,
            controller,
            "correct the full interpretation",
            "abuse-refinement",
        ),
        Operation::Traversal => {
            let anchor = MemoryRef {
                owner_id: fixture.owner.clone(),
                memory_id: fixture.memory_ids[0],
            };
            fixture.gateway.traverse_graph(
                &fixture.instance,
                controller,
                &[anchor],
                "abuse-traversal",
            )
        }
    }
}

#[test]
fn invalid_selection_responses_across_all_operations_leave_controller_unchanged() {
    for operation in [
        Operation::Query,
        Operation::Refinement,
        Operation::Traversal,
    ] {
        for abuse in SelectionAbuse::ALL {
            let fixture = fixture(abuse, None);
            let mut controller = controller_for(&fixture, operation);
            let before = controller.clone();
            let result = run_operation(&fixture, &mut controller, operation);
            assert!(
                result.is_err(),
                "{operation:?} accepted invalid response {abuse:?}"
            );
            assert_eq!(
                controller, before,
                "{operation:?} mutated controller for {abuse:?}"
            );
            let calls = fixture.endpoint.calls.lock().unwrap();
            if operation == Operation::Refinement {
                assert_eq!(
                    calls.iter().map(String::as_str).collect::<Vec<_>>(),
                    vec!["refine_query_rewrite", operation.name()]
                );
            } else {
                assert_eq!(
                    calls.iter().map(String::as_str).collect::<Vec<_>>(),
                    vec![operation.name()]
                );
            }
        }
    }
}

#[test]
fn malformed_refinement_rewrites_leave_controller_unchanged_and_skip_selection() {
    for abuse in [
        RewriteAbuse::Empty,
        RewriteAbuse::Oversized,
        RewriteAbuse::WrongVersion,
        RewriteAbuse::MalformedShape,
        RewriteAbuse::UnknownField,
        RewriteAbuse::MultipleResults,
        RewriteAbuse::ProviderError,
    ] {
        let fixture = fixture(SelectionAbuse::ProviderError, Some(abuse));
        let mut controller = controller_for(&fixture, Operation::Refinement);
        let before = controller.clone();
        assert!(
            fixture
                .gateway
                .refine_query(
                    &fixture.instance,
                    &mut controller,
                    "correct the full interpretation",
                    "abuse-rewrite",
                )
                .is_err(),
            "accepted malformed rewrite {abuse:?}"
        );
        assert_eq!(controller, before, "rewrite {abuse:?} changed controller");
        assert_eq!(
            *fixture.endpoint.calls.lock().unwrap(),
            vec!["refine_query_rewrite"],
            "rewrite {abuse:?} proceeded to selection"
        );
    }
}
