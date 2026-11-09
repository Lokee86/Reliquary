#[path = "operational_processing/hermes_copy.rs"]
mod hermes_copy;
#[path = "operational_processing/live.rs"]
mod live;

use reliquary_memory::test_support::mock_master_key_store;
use reliquary_memory::{
    ConfiguredRuntime, CredentialId, EmbeddingModelEndpoint, EmbeddingNormalization, EpisodeId,
    Gateway, GeneralModelEndpoint, GeneralRoute, HostCredentialBinding, ModelProvider, ModelRoute,
    ModelSlot, NativeImportFormat, NativeImportPlan, OwnerGrant, OwnerScope, Phylactery,
    ProcessOptions, ProcessingState, ProductErrorKind, Rel, ReliquaryConfig, SecretString,
    TrustedHostAdmission,
};
use serde_json::{Value, json};
use std::{
    fs,
    io::{BufRead, BufReader, Read, Write},
    net::{TcpListener, TcpStream},
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};

const PROJECT: &str = "Lantern release uses the amber checklist.";
const PRIVATE: &str = "My preferred meeting window is afternoons.";

struct Fixture {
    listener: std::net::SocketAddr,
    stop: Arc<AtomicBool>,
    changed_profile: Arc<AtomicBool>,
    server: Option<thread::JoinHandle<()>>,
}
impl Fixture {
    fn start_with(reply: impl Fn(&Value, bool) -> Value + Send + 'static) -> Self {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let address = listener.local_addr().unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let shutting_down = stop.clone();
        let changed_profile = Arc::new(AtomicBool::new(false));
        let profile = Arc::clone(&changed_profile);
        let server = thread::spawn(move || {
            for connection in listener.incoming() {
                let mut stream = connection.unwrap();
                if shutting_down.load(Ordering::Acquire) {
                    break;
                }
                stream
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let request = read_request(&stream);
                let reply = reply(&request, profile.load(Ordering::Acquire));
                let body = reply.to_string();
                write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                ).unwrap();
            }
        });
        Self {
            listener: address,
            stop,
            changed_profile,
            server: Some(server),
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        let _ = TcpStream::connect(self.listener);
        self.server.take().unwrap().join().unwrap();
    }
}

fn read_request(stream: &TcpStream) -> Value {
    let mut request = BufReader::new(stream);
    let mut length = None;
    loop {
        let mut line = String::new();
        assert_ne!(request.read_line(&mut line).unwrap(), 0);
        if line == "\r\n" {
            break;
        }
        if let Some((name, value)) = line.split_once(':') {
            if name.eq_ignore_ascii_case("content-length") {
                length = Some(value.trim().parse::<usize>().unwrap());
            }
        }
    }
    let mut body = vec![0; length.unwrap()];
    request.read_exact(&mut body).unwrap();
    serde_json::from_slice(&body).unwrap()
}

fn fixture_reply(request: &Value, changed_profile: bool) -> Value {
    if let Some(inputs) = request["input"].as_array() {
        let vector = if changed_profile {
            [0.0, 1.0]
        } else {
            [1.0, 0.0]
        };
        let data = inputs
            .iter()
            .enumerate()
            .map(|(index, _)| json!({"index":index,"embedding":vector}))
            .collect::<Vec<_>>();
        return json!({"data":data});
    }
    let schema = request["response_format"]["json_schema"]["name"]
        .as_str()
        .unwrap();
    let clause = |category, kind, text| {
        json!({
            "disposition":"retain","authority_kind":"direct","category":category,"type":kind,
            "lifecycle":"current","proposition":text,"authority_source_node_id":"",
            "grounding_source_node_id":"","reason":"direct source statement"
        })
    };
    let candidate = |category, kind, ownership, title, text| {
        json!({
            "authority_kind":"direct","category":category,"memory_type":kind,"temporal_status":"current",
            "ownership":ownership,"title":title,"content":text,"source_node_id":"accepted",
            "source_quote":text,"authority_source":null,"grounding_source":null,"routing_mentions":[]
        })
    };
    let output = match schema {
        "insomnia_authority_disposition_ledger" => json!({
            "turns":{"accepted":[clause("fact","project",PROJECT),clause("preference","communication",PRIVATE)]},
            "evidence_requests":[]
        }),
        "insomnia_memory_ownership" => json!({"ownership":{"g000":"project","g001":"user"}}),
        "insomnia_memory_wording" => json!({"candidates":[
            candidate("fact","project","project","Amber checklist",PROJECT),
            candidate("preference","communication","user","Meeting window",PRIVATE)
        ]}),
        other => panic!("unexpected provider schema {other}"),
    };
    json!({"choices":[{"message":{"content":output.to_string()}}]})
}

fn configured_fixture(
    path: &std::path::Path,
    url: &str,
    general_routes: &[(ModelSlot, &str)],
    embedding: bool,
) -> ConfiguredRuntime {
    let _keys = mock_master_key_store();
    let mut config = ReliquaryConfig::new_blank(path);
    let credential_id = CredentialId::new("KEY").unwrap();
    config.add_api_key(
        credential_id.clone(),
        SecretString::new("synthetic-operational-test-key").unwrap(),
    );
    for &(slot, model) in general_routes {
        config
            .set_model(
                slot,
                &ModelRoute::General(GeneralRoute {
                    endpoint: GeneralModelEndpoint {
                        provider: ModelProvider::OpenAiReady,
                        model: model.into(),
                        url: url.into(),
                        credential_id: credential_id.clone(),
                    },
                    reasoning_effort: None,
                }),
            )
            .unwrap();
    }
    if embedding {
        config
            .set_model(
                ModelSlot::Embedding,
                &ModelRoute::Embedding(EmbeddingModelEndpoint {
                    provider: ModelProvider::OpenAiReady,
                    model: "embed-fixture".into(),
                    url: url.into(),
                    credential_id,
                    dimensions: 2,
                    normalization: EmbeddingNormalization::L2,
                }),
            )
            .unwrap();
    }
    config.save().unwrap();
    ConfiguredRuntime::from_path(path).unwrap()
}

#[test]
fn configured_import_processes_rel_and_phy_and_reopens_without_republication() {
    let compatible_drift = Arc::new(AtomicBool::new(false));
    let fixture = Fixture::start_with({
        let compatible_drift = Arc::clone(&compatible_drift);
        move |request, changed_profile| {
            let mut reply = fixture_reply(request, changed_profile);
            if !changed_profile && compatible_drift.load(Ordering::Acquire) {
                if let Some(data) = reply["data"].as_array_mut() {
                    for row in data {
                        row["embedding"] = json!([0.99995, 0.01]);
                    }
                }
            }
            reply
        }
    });
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(".test-tmp")
        .join(format!("operational-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    let rel_path = root.join("project.rel");
    let phy_path = root.join("user.phy");
    let config = root.join("runtime.cfg");
    let input = root.join("conversations.json");
    let url = format!("http://{}", fixture.listener);
    fs::write(
        &input,
        json!([{
            "id":"chat","current_node":"accepted","mapping":{
                "accepted":{"id":"accepted","parent":null,"children":[],"message":{
                    "id":"accepted","author":{"role":"user"},"create_time":1000,
                    "content":{"content_type":"text","parts":[format!("{PROJECT} {PRIVATE}")]}
                }}
            }
        }])
        .to_string(),
    )
    .unwrap();
    let runtime = configured_fixture(
        &config,
        &url,
        &[
            (ModelSlot::General, "fixture"),
            (ModelSlot::EntityResolution, "fixture"),
        ],
        true,
    );
    let rel = Rel::create(&rel_path, OwnerScope::Project).unwrap();
    let phy = Phylactery::create(&phy_path).unwrap();
    let token = "external-fixture-operator-admission-credential";
    let mount = |rel: &Rel, phy: &Phylactery| {
        let gateway = Gateway::new();
        let owner = gateway.mount_rel(rel, true).unwrap();
        let destination = gateway.mount_phylactery(phy, true).unwrap();
        let (admission, _) = TrustedHostAdmission::new([HostCredentialBinding::new(
            "operator",
            token,
            [
                OwnerGrant::ManageRel(owner),
                OwnerGrant::OperateRel(owner),
                OwnerGrant::OperatePhylactery(destination),
            ],
        )])
        .install(&gateway)
        .unwrap();
        let instance = admission.authenticate(token).unwrap();
        (gateway, admission, instance, owner, destination)
    };
    let (gateway, admission, instance, owner, destination) = mount(&rel, &phy);
    let plan = NativeImportPlan::from_path(&input, NativeImportFormat::ChatGpt, None).unwrap();
    let stage = instance.stage_native_import(owner, plan).unwrap();
    for selection in [
        reliquary_memory::LiveProcessingSelection::Entity,
        reliquary_memory::LiveProcessingSelection::Dream,
    ] {
        assert!(
            instance
                .start_live_processing_stage(owner, &runtime, Some(destination), 1, selection)
                .is_err()
        );
    }
    let options = ProcessOptions {
        import_id: Some(stage.import_id.clone()),
        destination: Some(destination),
        max_episodes: 1,
    };
    let held = instance
        .process_and_drain(
            owner,
            &runtime,
            ProcessOptions {
                destination: None,
                ..options.clone()
            },
        )
        .unwrap();
    assert_eq!(held.state, ProcessingState::Held, "{held:?}");
    assert_eq!(held.insomnia.held, 1);
    let incomplete = &held.insomnia.incomplete_ids[0];
    assert_eq!(incomplete.len(), 64);
    let mut episode = [0; 32];
    for (index, byte) in episode.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&incomplete[index * 2..index * 2 + 2], 16).unwrap();
    }

    let management = instance.workspace_management(owner).unwrap();
    management.quiesce().unwrap();
    assert_eq!(management.verify_quiesced().unwrap().memory_count, 0);
    management.reopen().unwrap();
    assert_eq!(
        instance
            .processing_status(owner, Some(&stage.import_id))
            .unwrap()
            .held,
        1
    );

    let other_runtime = configured_fixture(
        &root.join("other.cfg"),
        &url,
        &[
            (ModelSlot::General, "fixture"),
            (ModelSlot::EntityResolution, "fixture"),
            (ModelSlot::Dream, "other-dream-model"),
        ],
        true,
    );
    assert_eq!(
        instance
            .resume_held_processing(owner, &other_runtime, options.clone(), EpisodeId(episode))
            .err()
            .expect("a different Dream route must reject before retry")
            .kind(),
        ProductErrorKind::Conflict
    );
    assert_eq!(
        instance
            .processing_status(owner, Some(&stage.import_id))
            .unwrap()
            .held,
        1
    );
    let report = instance
        .resume_held_processing(owner, &runtime, options.clone(), EpisodeId(episode))
        .unwrap()
        .join()
        .unwrap();
    assert_eq!(report.state, ProcessingState::Ready, "{report:?}");
    assert_eq!(report.insomnia.rel_memories, 1);
    assert_eq!(report.insomnia.phy_memories, 1);
    assert!(report.insomnia.import_complete);
    management.quiesce().unwrap();
    assert_eq!(management.verify_quiesced().unwrap().memory_count, 1);
    management.reopen().unwrap();
    let sealed = instance
        .native_import_status(owner, &stage.import_id)
        .unwrap()
        .unwrap();
    assert_eq!(sealed.state, reliquary_memory::NativeImportState::Completed);
    assert_eq!(sealed.receipt_digest, Some(stage.receipt_digest));
    let original_probe = runtime.probe_embedding_profile().unwrap().unwrap().id;
    drop((management, instance, admission, gateway, rel, phy));

    // Read both files after releasing their writer leases so this also works on Windows.
    let sealed_rel = fs::read(&rel_path).unwrap();
    let sealed_phy = fs::read(&phy_path).unwrap();
    compatible_drift.store(true, Ordering::Release);
    let drifted_probe = runtime.probe_embedding_profile().unwrap().unwrap().id;
    assert_ne!(
        original_probe, drifted_probe,
        "drift must change the raw probe hash"
    );

    let replay = |runtime: &ConfiguredRuntime| {
        let rel = Rel::open(&rel_path).unwrap();
        let phy = Phylactery::open(&phy_path).unwrap();
        let (gateway, admission, instance, reopened_owner, reopened_destination) =
            mount(&rel, &phy);
        assert_eq!((reopened_owner, reopened_destination), (owner, destination));
        assert_eq!(
            instance
                .process_and_drain(owner, &other_runtime, options.clone())
                .unwrap_err()
                .kind(),
            ProductErrorKind::Conflict
        );
        fixture.changed_profile.store(true, Ordering::Release);
        assert_eq!(
            instance
                .process_and_drain(owner, runtime, options.clone())
                .unwrap_err()
                .kind(),
            ProductErrorKind::Conflict
        );
        fixture.changed_profile.store(false, Ordering::Release);
        let report = instance
            .process_and_drain(owner, runtime, options.clone())
            .unwrap();
        assert_eq!(report.state, ProcessingState::Ready, "{report:?}");
        assert_eq!(
            instance
                .native_import_status(owner, &stage.import_id)
                .unwrap(),
            Some(sealed.clone())
        );
        drop((instance, admission, gateway, rel, phy));
        assert_eq!(fs::read(&rel_path).unwrap(), sealed_rel);
        assert_eq!(fs::read(&phy_path).unwrap(), sealed_phy);
        serde_json::to_value(report).unwrap()
    };
    let before_restart = replay(&runtime);
    drop(runtime);
    let restarted_runtime = ConfiguredRuntime::from_path(&config).unwrap();
    assert_eq!(replay(&restarted_runtime), before_restart);
    assert_eq!(Rel::verify(&rel_path).unwrap().memory_count, 1);
    assert_eq!(Phylactery::verify(&phy_path).unwrap().memory_count, 1);
    drop(fixture);
    fs::remove_dir_all(root).unwrap();
}
