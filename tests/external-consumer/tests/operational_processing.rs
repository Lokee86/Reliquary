use reliquary_memory::{
    ConfiguredRuntime, CredentialId, CredentialMaterial, CredentialResolver, EpisodeId, Gateway,
    HostCredentialBinding, NativeImportFormat, NativeImportPlan, OwnerGrant, OwnerScope,
    Phylactery, ProcessOptions, ProcessingState, ProductErrorKind, Rel, TrustedHostAdmission,
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
    fn start() -> Self {
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
                let reply = fixture_reply(&request, profile.load(Ordering::Acquire));
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

struct TestCredential;
impl CredentialResolver for TestCredential {
    fn resolve(&self, _: &CredentialId) -> Result<CredentialMaterial, String> {
        Ok(CredentialMaterial::ApiKey(
            "synthetic-operational-test-key".into(),
        ))
    }
}

#[test]
fn configured_import_processes_rel_and_phy_and_reopens_without_republication() {
    let fixture = Fixture::start();
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(".test-tmp")
        .join(format!("operational-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    let rel_path = root.join("project.rel");
    let phy_path = root.join("user.phy");
    let config = root.join("runtime.json");
    let input = root.join("conversations.json");
    let url = format!("http://{}", fixture.listener);
    fs::write(&config, json!({
        "general":{"model":"fixture","url":url,"credential_id":"KEY"},
        "embedding":{"model":"embed-fixture","url":url,"credential_id":"KEY","dimensions":2,"normalization":"l2"}
    }).to_string()).unwrap();
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
    let runtime = ConfiguredRuntime::from_path(&config, &TestCredential).unwrap();
    let rel = Rel::create(&rel_path, OwnerScope::Project).unwrap();
    let phy = Phylactery::create(&phy_path).unwrap();
    let gateway = Gateway::new();
    let owner = gateway.mount_rel(&rel, true).unwrap();
    let destination = gateway.mount_phylactery(&phy, true).unwrap();
    let token = "external-fixture-operator-admission-credential";
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
    let plan = NativeImportPlan::from_path(&input, NativeImportFormat::ChatGpt, None).unwrap();
    let stage = instance.stage_native_import(owner, plan).unwrap();
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

    let mut other_config: Value = serde_json::from_slice(&fs::read(&config).unwrap()).unwrap();
    other_config["dream"] = other_config["general"].clone();
    other_config["dream"]["model"] = json!("other-dream-model");
    let other_path = root.join("other.json");
    fs::write(&other_path, other_config.to_string()).unwrap();
    let other_runtime = ConfiguredRuntime::from_path(other_path, &TestCredential).unwrap();
    assert_eq!(
        instance
            .resume_held_import_processing(
                owner,
                &other_runtime,
                options.clone(),
                EpisodeId(episode)
            )
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
        .resume_held_import_processing(owner, &runtime, options.clone(), EpisodeId(episode))
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
    fixture.changed_profile.store(true, Ordering::Release);
    assert_eq!(
        instance
            .process_and_drain(owner, &runtime, options.clone())
            .unwrap_err()
            .kind(),
        ProductErrorKind::Conflict
    );
    fixture.changed_profile.store(false, Ordering::Release);
    assert_eq!(
        instance
            .process_and_drain(owner, &runtime, options)
            .unwrap()
            .state,
        ProcessingState::Ready
    );
    drop((management, instance, admission, gateway, rel, phy));
    assert_eq!(Rel::verify(&rel_path).unwrap().memory_count, 1);
    assert_eq!(Phylactery::verify(&phy_path).unwrap().memory_count, 1);
    drop(fixture);
    fs::remove_dir_all(root).unwrap();
}
