use reliquary_memory::{OwnerScope, Phylactery, Rel, RewritePolicy};
use serde_json::Value;
use std::{
    fs,
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(1);
struct Temp(PathBuf);
impl Temp {
    fn new(tag: &str) -> Self {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(".test-tmp")
            .join(format!(
                "cli-{tag}-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn command(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_reliquary"))
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn help_has_explicit_owner_families_and_rejects_legacy_aliases() {
    let help = command(&["--help"]);
    assert!(help.status.success());
    let text = String::from_utf8(help.stdout).unwrap();
    assert!(text.contains("reliquary rel"));
    assert!(text.contains("reliquary phy"));
    assert!(command(&["cva", "info", "missing"]).status.code().unwrap() != 0);
    assert!(command(&["phy", "info", "missing"]).status.code().unwrap() != 0);
}

#[test]
fn phy_create_verify_and_history_reopen_without_changing_bytes() {
    let temp = Temp::new("phy");
    let path = temp.path("user.phy");
    let path = path.to_str().unwrap();
    let created = command(&["phy", "create", path]);
    assert!(created.status.success(), "{:?}", created.stderr);
    let created: Value = serde_json::from_slice(&created.stdout).unwrap();
    assert_eq!(created["owner"]["kind"], "phylactery");
    let before = fs::read(path).unwrap();
    let verified = command(&["phy", "verify", path]);
    assert!(verified.status.success(), "{:?}", verified.stderr);
    let verified: Value = serde_json::from_slice(&verified.stdout).unwrap();
    assert_eq!(verified["owner"], created["owner"]);
    assert_eq!(verified["verified"], true);
    assert!(
        command(&["phy", "memory", "history", path, &"0".repeat(64)])
            .status
            .success()
    );
    assert!(!command(&["phy", "create", path]).status.success());
    assert_eq!(fs::read(path).unwrap(), before);
}

#[test]
fn rel_memory_inspection_matches_the_created_revision_and_is_read_only() {
    let temp = Temp::new("memory-inspection");
    let path = temp.path("project.rel");
    let path = path.to_str().unwrap();
    assert!(command(&["rel", "create", path]).status.success());
    let added = command(&[
        "rel",
        "memory",
        "add",
        path,
        "--title",
        "Collision geometry",
        "--content",
        "The server uses convex hulls.",
        "--category",
        "project",
        "--type",
        "fact",
    ]);
    assert!(
        added.status.success(),
        "{}",
        String::from_utf8_lossy(&added.stderr)
    );
    let added: Value = serde_json::from_slice(&added.stdout).unwrap();
    let memory = &added["memory"];
    let id = memory["id"].as_str().unwrap();
    let before = fs::read(path).unwrap();
    let listed = command(&["rel", "memory", "list", path, "--limit", "1"]);
    assert!(listed.status.success());
    let listed: Value = serde_json::from_slice(&listed.stdout).unwrap();
    assert_eq!(&listed["page"]["items"][0]["value"], memory);
    let found = command(&["rel", "memory", "get", path, id]);
    assert!(found.status.success());
    let found: Value = serde_json::from_slice(&found.stdout).unwrap();
    assert_eq!(&found["memory"], memory);
    assert!(
        !command(&[
            "rel",
            "memory",
            "get",
            path,
            &format!("aé{}", "0".repeat(61))
        ])
        .status
        .success()
    );
    assert_eq!(fs::read(path).unwrap(), before);
}

#[test]
fn create_info_verify_are_json_and_wrong_kind_fails() {
    let temp = Temp::new("owner");
    let rel = temp.path("owner.rel");
    let created = command(&[
        "rel",
        "create",
        rel.to_str().unwrap(),
        "--scope",
        "connection",
    ]);
    assert!(created.status.success());
    let value: Value = serde_json::from_slice(&created.stdout).unwrap();
    assert_eq!(value["owner"]["format"], "2.1");
    assert!(value["owner"]["owner_id"].as_str().unwrap().len() == 32);
    assert!(
        command(&["rel", "info", rel.to_str().unwrap()])
            .status
            .success()
    );
    assert!(
        command(&["rel", "verify", rel.to_str().unwrap()])
            .status
            .success()
    );
    let phy = temp.path("owner.phy");
    Phylactery::create(&phy).unwrap();
    assert!(
        !command(&["rel", "verify", phy.to_str().unwrap()])
            .status
            .success()
    );
}

#[test]
fn rewrite_dry_run_never_creates_destination() {
    let temp = Temp::new("dry-run");
    let source = temp.path("source.rel");
    let output = temp.path("output.rel");
    let policy = temp.path("policy.json");
    Rel::create(&source, OwnerScope::Project).unwrap();
    fs::write(
        &policy,
        serde_json::to_vec(&RewritePolicy::default()).unwrap(),
    )
    .unwrap();
    let before = fs::read(&source).unwrap();
    let result = command(&[
        "rel",
        "rewrite",
        source.to_str().unwrap(),
        output.to_str().unwrap(),
        "--policy",
        policy.to_str().unwrap(),
        "--dry-run",
    ]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(!output.exists());
    assert_eq!(fs::read(source).unwrap(), before);
}
#[test]
fn unknown_and_duplicate_options_refuse_without_publication() {
    let temp = Temp::new("options");
    let source = temp.path("source.rel");
    let forbidden = temp.path("should-not-exist.rel");
    Rel::create(&source, OwnerScope::Project).unwrap();
    let original = fs::read(&source).unwrap();
    for args in [
        vec![
            "rel",
            "create",
            forbidden.to_str().unwrap(),
            "--unexpected",
            "yes",
        ],
        vec![
            "rel",
            "create",
            forbidden.to_str().unwrap(),
            "--scope",
            "project",
            "--scope",
            "connection",
        ],
        vec![
            "rel",
            "history",
            "archive",
            source.to_str().unwrap(),
            "--silent",
            "true",
        ],
        vec![
            "rel",
            "history",
            "memory",
            source.to_str().unwrap(),
            &"f".repeat(64),
            "--unknown",
            "yes",
        ],
        vec![
            "rel",
            "rewrite",
            source.to_str().unwrap(),
            forbidden.to_str().unwrap(),
            "--dry-run",
        ],
    ] {
        let result = command(&args);
        assert!(!result.status.success(), "unexpected success: {args:?}");
        assert!(!forbidden.exists());
        assert_eq!(fs::read(&source).unwrap(), original);
    }
}

#[test]
fn rewrite_rejects_unsupported_selection_override_without_creating_output() {
    let temp = Temp::new("override");
    let source = temp.path("source.rel");
    let output = temp.path("output.rel");
    let policy = temp.path("policy.json");
    Rel::create(&source, OwnerScope::Project).unwrap();
    fs::write(
        &policy,
        serde_json::to_vec(&RewritePolicy::default()).unwrap(),
    )
    .unwrap();
    let source_bytes = fs::read(&source).unwrap();
    let invalid_id = "a".repeat(64);
    let result = command(&[
        "rel",
        "rewrite",
        source.to_str().unwrap(),
        output.to_str().unwrap(),
        "--policy",
        policy.to_str().unwrap(),
        "--include-memory",
        &invalid_id,
        "--dry-run",
    ]);
    assert!(
        !result.status.success(),
        "unknown explicitly included Memory must reject"
    );
    assert!(!output.exists());
    assert_eq!(fs::read(&source).unwrap(), source_bytes);
}
#[test]
fn reconcile_cli_preserves_both_inputs_and_refuses_collision() {
    let temp = Temp::new("reconcile");
    let left = temp.path("left.rel");
    let right = temp.path("right.rel");
    let output = temp.path("merged.rel");
    Rel::create(&left, OwnerScope::Project).unwrap();
    let left_bytes = fs::read(&left).unwrap();
    fs::copy(&left, &right).unwrap();
    let args = [
        "rel",
        "reconcile",
        left.to_str().unwrap(),
        right.to_str().unwrap(),
        output.to_str().unwrap(),
    ];
    let result = command(&args);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let report: Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(report["operation"], "reconcile");
    assert_eq!(fs::read(&left).unwrap(), left_bytes);
    assert_eq!(fs::read(&right).unwrap(), left_bytes);
    assert_eq!(fs::read(&output).unwrap(), left_bytes);
    assert!(!command(&args).status.success());
    assert_eq!(fs::read(&output).unwrap(), left_bytes);
}

#[test]
fn history_proof_cli_is_read_only() {
    let temp = Temp::new("history");
    let source = temp.path("owner.rel");
    Rel::create(&source, OwnerScope::Project).unwrap();
    let before = fs::read(&source).unwrap();
    let result = command(&["rel", "history", "proof", source.to_str().unwrap()]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let report: Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(report["operation"], "history_proof");
    assert!(report["proof"]["digest"].as_str().is_some());
    assert_eq!(fs::read(&source).unwrap(), before);
}

#[test]
fn import_stages_exact_transcript_and_resumes_without_appending() {
    let temp = Temp::new("import");
    let rel = temp.path("project.rel");
    let source = temp.path("conversations.json");
    let rel = rel.to_str().unwrap();
    let source = source.to_str().unwrap();
    assert!(command(&["rel", "create", rel]).status.success());
    fs::write(source, br#"[{"id":"chat","title":"Project source","current_node":"answer","mapping":{
        "root":{"id":"root","parent":null,"children":["question"],"message":null},
        "question":{"id":"question","parent":"root","children":["answer"],"message":{"id":"user-msg","author":{"role":"user"},"create_time":1000,"content":{"content_type":"text","parts":["Use convex hulls."]}}},
        "answer":{"id":"answer","parent":"question","children":[],"message":{"id":"assistant-msg","author":{"role":"assistant"},"create_time":1001,"content":{"content_type":"text","parts":["Understood."]}}}
    }}]"#).unwrap();
    let args = [
        "rel", "import", rel, "--source", source, "--format", "chatgpt",
    ];
    let first = command(&args);
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    let first: Value = serde_json::from_slice(&first.stdout).unwrap();
    assert_eq!(first["coverage"]["turns"], 2);
    let before = fs::read(rel).unwrap();
    let second = command(&args);
    assert!(
        second.status.success(),
        "{}",
        String::from_utf8_lossy(&second.stderr)
    );
    let second: Value = serde_json::from_slice(&second.stdout).unwrap();
    assert_eq!(first["import_id"], second["import_id"]);
    let archive = command(&["rel", "archive", "conversations", rel]);
    let archive: Value = serde_json::from_slice(&archive.stdout).unwrap();
    let conversation = &archive["page"]["items"][0]["value"];
    let transcript = command(&[
        "rel",
        "archive",
        "show",
        rel,
        conversation["conversation_id"].as_str().unwrap(),
        conversation["branch"]["leaf_node_id"].as_str().unwrap(),
    ]);
    assert!(
        transcript.status.success(),
        "{}",
        String::from_utf8_lossy(&transcript.stderr)
    );
    let transcript: Value = serde_json::from_slice(&transcript.stdout).unwrap();
    assert_eq!(
        transcript["page"]["items"][0]["value"]["content"],
        "Use convex hulls."
    );
    assert_eq!(
        transcript["page"]["items"][0]["value"]["timestamp_ns"],
        1_000_000_000_000i64
    );
    let status = command(&[
        "rel",
        "work",
        "status",
        rel,
        "--import",
        first["import_id"].as_str().unwrap(),
    ]);
    assert!(status.status.success());
    assert_eq!(fs::read(rel).unwrap(), before);
    fs::write(source, b"[] trailing garbage").unwrap();
    assert!(!command(&args).status.success());
    assert_eq!(fs::read(rel).unwrap(), before);
}

#[test]
fn lexical_search_and_script_inspection_leave_owner_bytes_unchanged() {
    let temp = Temp::new("interact");
    let path = temp.path("project.rel");
    let script = temp.path("requests.jsonl");
    let path = path.to_str().unwrap();
    assert!(command(&["rel", "create", path]).status.success());
    let added = command(&[
        "rel",
        "memory",
        "add",
        path,
        "--title",
        "Collision geometry",
        "--content",
        "The server uses convex hulls.",
        "--category",
        "project",
        "--type",
        "fact",
    ]);
    assert!(added.status.success());
    let added: Value = serde_json::from_slice(&added.stdout).unwrap();
    let id = added["memory"]["id"].as_str().unwrap();
    let before = fs::read(path).unwrap();
    assert!(
        command(&[
            "rel",
            "search",
            path,
            "--query",
            "convex hulls",
            "--mode",
            "lexical"
        ])
        .status
        .success()
    );
    assert!(
        !command(&[
            "rel",
            "search",
            path,
            "--query",
            "convex hulls",
            "--mode",
            "semantic"
        ])
        .status
        .success()
    );
    let rows = [
        serde_json::json!({"request_id":"status","command":"status"}),
        serde_json::json!({"request_id":"show","command":"show","memory_id":id}),
        serde_json::json!({"request_id":"reset","command":"new-turn"}),
        serde_json::json!({"request_id":"query","command":"query","text":"convex hulls"}),
        serde_json::json!({"request_id":"quit","command":"quit"}),
    ]
    .map(|row| row.to_string())
    .join("\n");
    fs::write(&script, rows).unwrap();
    let result = command(&[
        "rel",
        "interact",
        path,
        "--commands",
        script.to_str().unwrap(),
    ]);
    assert!(!result.status.success());
    let replies = String::from_utf8(result.stdout)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(replies.len(), 5);
    assert_eq!(replies[0]["result"]["active_query"], false);
    assert_eq!(replies[1]["request_id"], "show");
    assert_eq!(replies[2]["result"]["turn"], 2);
    assert_eq!(replies[3]["error"]["kind"], "unavailable");
    assert_eq!(replies[4]["request_id"], "quit");
    assert_eq!(fs::read(path).unwrap(), before);
}
