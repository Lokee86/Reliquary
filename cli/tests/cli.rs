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
fn help_has_rel_only_and_rejects_legacy_aliases() {
    let help = command(&["--help"]);
    assert!(help.status.success());
    let text = String::from_utf8(help.stdout).unwrap();
    assert!(text.contains("reliquary rel"));
    assert!(command(&["cva", "info", "missing"]).status.code().unwrap() != 0);
    assert!(command(&["phy", "info", "missing"]).status.code().unwrap() != 0);
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
