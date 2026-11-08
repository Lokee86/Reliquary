//! Configured CLI acceptance through separate processes and loopback HTTP.

use serde_json::{Value, json};
use std::{
    fs,
    io::{self, BufRead, BufReader, Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    path::PathBuf,
    process::Command,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    thread,
    time::Duration,
};

const CREDENTIAL: &str = "RELIQUARY_OPERATIONAL_FIXTURE_KEY";
const KEY: &str = "synthetic-operational-cli-test-key";
const PROJECT: &str = "Lantern release uses the amber checklist.";
const PRIVATE: &str = "My preferred meeting window is afternoons.";
const QUERY: &str = "Lantern amber checklist";
const REFINEMENT: &str = "Focus on the release";
const REWRITTEN: &str = "Lantern release amber checklist";

struct Provider {
    addr: SocketAddr,
    stop: Arc<AtomicBool>,
    requests: Arc<AtomicUsize>,
    worker: Option<thread::JoinHandle<()>>,
}

impl Provider {
    fn start() -> Self {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let addr = listener.local_addr().unwrap();
        listener.set_nonblocking(true).unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let requests = Arc::new(AtomicUsize::new(0));
        let shutdown = stop.clone();
        let count = requests.clone();
        let worker = thread::spawn(move || {
            while !shutdown.load(Ordering::Acquire) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        assert!(
                            count.fetch_add(1, Ordering::Relaxed) < 64,
                            "provider call limit"
                        );
                        serve(stream);
                    }
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(5));
                    }
                    Err(error) => panic!("accepting fixture connection: {error}"),
                }
            }
        });
        Self {
            addr,
            stop,
            requests,
            worker: Some(worker),
        }
    }
}

impl Drop for Provider {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        let result = self.worker.take().unwrap().join();
        if !thread::panicking() {
            result.expect("loopback provider failed");
        }
    }
}

fn serve(mut stream: TcpStream) {
    const MAX_REQUEST: u64 = 256 * 1024;
    let timeout = Some(Duration::from_secs(5));
    stream.set_nonblocking(false).unwrap();
    stream.set_read_timeout(timeout).unwrap();
    stream.set_write_timeout(timeout).unwrap();
    let mut reader = BufReader::new(&stream).take(MAX_REQUEST);
    let mut line = String::new();
    reader.read_line(&mut line).unwrap();
    assert!(line.starts_with("POST / HTTP/1.1"));
    let mut length = None;
    let mut authorized = false;
    loop {
        line.clear();
        assert_ne!(
            reader.read_line(&mut line).unwrap(),
            0,
            "incomplete HTTP headers"
        );
        if line == "\r\n" {
            break;
        }
        if let Some((name, value)) = line.split_once(':') {
            if name.eq_ignore_ascii_case("content-length") {
                length = Some(value.trim().parse::<u64>().unwrap());
            } else if name.eq_ignore_ascii_case("authorization") {
                authorized = value.trim().strip_prefix("Bearer ") == Some(KEY);
            }
        }
    }
    assert!(authorized, "expected the synthetic subprocess credential");
    let length = length.unwrap();
    assert!(
        length <= reader.limit(),
        "provider request exceeds fixture bound"
    );
    let mut body = vec![0; length as usize];
    reader.read_exact(&mut body).unwrap();
    let request: Value = serde_json::from_slice(&body).unwrap();
    let body = provider_reply(&request).to_string();
    write!(
        stream,
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    ).unwrap();
}

fn provider_reply(request: &Value) -> Value {
    if let Some(inputs) = request["input"].as_array() {
        assert_eq!(request["dimensions"], 2);
        let data: Vec<_> = (0..inputs.len())
            .map(|index| json!({"index":index,"embedding":[1.0,0.0]}))
            .collect();
        return json!({"data":data});
    }
    if let Some(state) = request.get("state") {
        return decision_reply(state);
    }

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
    let schema = request["response_format"]["json_schema"]["name"]
        .as_str()
        .unwrap();
    let output = match schema {
        "readiness_probe" => json!({"ok":true}),
        "insomnia_authority_disposition_ledger" => json!({
            "turns":{"accepted":[clause("fact","project",PROJECT),clause("preference","communication",PRIVATE)]},
            "evidence_requests":[]
        }),
        "insomnia_memory_ownership" => json!({"ownership":{"g000":"project","g001":"user"}}),
        "insomnia_memory_wording" => json!({"candidates":[
            candidate("fact","project","project","Amber checklist",PROJECT),
            candidate("preference","communication","user","Meeting window",PRIVATE)
        ]}),
        other => panic!("unexpected general provider schema {other}"),
    };
    json!({"choices":[{"message":{"content":output.to_string()}}]})
}

fn decision_reply(state: &Value) -> Value {
    match state["operation"].as_str().unwrap() {
        "readiness_probe" => json!({"ok":true}),
        "refine_query_rewrite" => {
            assert_eq!(state["current_query"], QUERY);
            assert_eq!(state["refinement"], REFINEMENT);
            json!({"version":1,"query":REWRITTEN})
        }
        operation @ ("memory_query" | "refine_query") => {
            let query = if operation == "memory_query" {
                QUERY
            } else {
                REWRITTEN
            };
            assert_eq!(state["query"], query);
            let candidate = only(&state["candidates"]);
            assert_eq!(candidate["content"], PROJECT);
            json!({"version":1,"approved":[candidate["memory"]]})
        }
        other => panic!("unexpected decision operation {other}"),
    }
}

struct Cli(PathBuf);

impl Cli {
    fn new(port: u16) -> Self {
        let parent = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".test-tmp");
        fs::create_dir_all(&parent).unwrap();
        let root = parent.join(format!("operational-{}-{port}", std::process::id()));
        fs::create_dir(&root).unwrap();
        Self(root)
    }

    fn run(&self, args: &[&str]) -> String {
        let mut command = Command::new(env!("CARGO_BIN_EXE_reliquary"));
        command
            .current_dir(&self.0)
            .args(args)
            .env_clear()
            .env(CREDENTIAL, KEY);
        // Keep the Windows system directory available in the isolated child.
        if let Some(root) = std::env::var_os("SystemRoot") {
            command.env("SystemRoot", root);
        }
        let output = command.output().unwrap();
        let stdout = String::from_utf8(output.stdout).unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            !stdout.contains(KEY) && !stderr.contains(KEY),
            "credential leaked"
        );
        assert!(
            output.status.success(),
            "{args:?} failed: {stdout}\n{stderr}"
        );
        stdout
    }

    fn json(&self, args: &[&str]) -> Value {
        serde_json::from_str(&self.run(args)).unwrap()
    }
}

impl Drop for Cli {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn only(items: &Value) -> &Value {
    let items = items.as_array().expect("expected a JSON array");
    assert_eq!(items.len(), 1, "{items:?}");
    &items[0]
}

#[test]
fn configured_cli_import_processes_routes_and_reopens_grounded_memories() {
    let provider = Provider::start();
    let cli = Cli::new(provider.addr.port());
    let (rel, phy, config, input, commands) = (
        "./project.rel",
        "./user.phy",
        "runtime.json",
        "conversations.json",
        "commands.jsonl",
    );
    let base = format!("http://{}", provider.addr);
    let routes = json!({
        "general":{"model":"fixture","url":base,"credential_id":CREDENTIAL},
        "memory_retrieval_decision":{"model":"decision-fixture","url":base,"credential_id":CREDENTIAL},
        "embedding":{"model":"embed-fixture","url":base,"credential_id":CREDENTIAL,
            "dimensions":2,"normalization":"l2"}
    });
    fs::write(cli.0.join(config), routes.to_string()).unwrap();
    let source_text = format!("{PROJECT} {PRIVATE}");
    let export = json!([{
        "id":"chat","current_node":"accepted","mapping":{
            "accepted":{"id":"accepted","parent":null,"children":[],"message":{
                "id":"accepted","author":{"role":"user"},"create_time":1000,
                "content":{"content_type":"text","parts":[source_text]}
            }}
        }
    }]);
    fs::write(cli.0.join(input), export.to_string()).unwrap();

    let offline = cli.json(&["config", "check", config]);
    assert_eq!(offline["operation"], "config_check");
    assert_eq!(offline["online"], false);
    assert_eq!(provider.requests.load(Ordering::Relaxed), 0);
    assert_eq!(
        cli.json(&["config", "check", config, "--online"])["online"],
        true
    );
    assert_eq!(provider.requests.load(Ordering::Relaxed), 3);

    let owners = [("rel", rel), ("phy", phy)];
    let [rel_id, phy_id] = owners.map(|(family, path)| {
        let created = cli.json(&[family, "create", path]);
        assert_eq!(created["owner"]["memory_count"], 0);
        created["owner"]["owner_id"].as_str().unwrap().to_owned()
    });

    let imported = cli.json(&[
        "rel", "import", rel, "--source", input, "--format", "chatgpt",
    ]);
    assert_eq!(imported["coverage"]["turns"], 1);
    assert_eq!(imported["staged"]["already_staged"], false);
    let import_id = imported["import_id"].as_str().unwrap();
    let process = [
        "rel",
        "process",
        rel,
        "--config",
        config,
        "--until",
        "ready",
        "--import",
        import_id,
        "--phy",
        phy,
        "--max-episodes",
        "1",
    ];
    let report = cli.json(&process);
    assert_eq!(report["report"]["state"], "ready");
    assert_eq!(report["report"]["import_id"], import_id);
    assert_eq!(report["report"]["insomnia"]["rel_memories"], 1);
    assert_eq!(report["report"]["insomnia"]["phy_memories"], 1);
    assert_eq!(report["report"]["insomnia"]["import_complete"], true);
    assert_eq!(cli.json(&process)["report"]["state"], "ready");

    let search = cli.json(&[
        "rel", "search", rel, "--mode", "hybrid", "--query", QUERY, "--config", config,
    ]);
    assert_eq!(search["result"]["memories_without_vectors"], 0);
    let memory = only(&search["result"]["memories"]);
    assert_eq!(memory["content"], PROJECT);
    assert!(memory["semantic_score"].as_f64().unwrap() > 0.0);
    let id = memory["memory_id"].as_str().unwrap();
    assert_eq!(memory["source"]["node_id"], "accepted");
    assert_eq!(memory["source"]["owner_id"], rel_id.as_str());

    let query = cli.json(&["rel", "query", rel, "--query", QUERY, "--config", config]);
    let preview = &query["result"]["preview"];
    assert_eq!(only(&preview["memories"])["memory_id"], id);
    assert_eq!(preview["cut"]["owner_id"], rel_id.as_str());

    let script = [
        json!({"request_id":1,"command":"query","text":QUERY}),
        json!({"request_id":2,"command":"refine","text":REFINEMENT}),
        json!({"request_id":3,"command":"traverse","memory_ids":[id]}),
    ];
    let jsonl = script.map(|row| row.to_string()).join("\n");
    fs::write(cli.0.join(commands), jsonl).unwrap();
    let replies: Vec<Value> = cli
        .run(&[
            "rel",
            "interact",
            rel,
            "--config",
            config,
            "--commands",
            commands,
        ])
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(replies.len(), 3);
    for (index, operation) in ["query", "refine", "traverse"].iter().enumerate() {
        assert_eq!(replies[index]["request_id"], (index + 1) as u64);
        assert_eq!(replies[index]["result"]["operation"], *operation);
    }
    assert_eq!(
        only(&replies[0]["result"]["outcome"]["preview"]["memories"])["memory_id"],
        id
    );
    let refined = &replies[1]["result"]["outcome"]["preview"];
    assert_eq!(refined["query"], REWRITTEN);
    assert_eq!(only(&refined["memories"])["memory_id"], id);
    let traversed = &replies[2]["result"]["outcome"];
    let anchor = only(&traversed["anchors"]);
    // The one REL Memory is a real anchor with no Memory neighbors.
    assert_eq!(anchor["anchor"], id);
    assert_eq!(anchor["unseen_neighbors"], 0);
    assert_eq!(anchor["relations"], json!([]));
    assert_eq!(traversed["preview"]["memories"], json!([]));

    let rel_proof = cli.json(&["rel", "memory", "provenance", rel, id]);
    let proof = &only(&rel_proof["page"]["items"])["value"];
    assert_eq!(proof["memory_id"], id);
    assert_eq!(proof["body_id"], memory["body_id"]);
    assert_eq!(proof["revision"], memory["revision"]);
    assert_eq!(proof["availability"], "available");
    assert_eq!(proof["source_turn"]["conversation_id"], "chat");
    assert_eq!(proof["source_turn"]["node_id"], "accepted");
    assert_eq!(proof["source_turn"]["content"], source_text);

    let private = cli.json(&["phy", "memory", "list", phy]);
    let private = &only(&private["page"]["items"])["value"];
    assert_eq!(private["content"], PRIVATE);
    let private_id = private["id"].as_str().unwrap();
    let private_proof = cli.json(&["phy", "memory", "provenance", phy, private_id]);
    let proof = &only(&private_proof["page"]["items"])["value"];
    assert_eq!(proof["memory_id"], private_id);
    assert_eq!(proof["availability"], "restricted");
    assert!(proof["source_turn"].is_null());

    for ((family, path), id) in owners.into_iter().zip([rel_id, phy_id]) {
        let verified = cli.json(&[family, "verify", path]);
        assert_eq!(verified["verified"], true);
        assert_eq!(verified["owner"]["owner_id"], id.as_str());
        assert_eq!(verified["owner"]["memory_count"], 1);
    }
}
