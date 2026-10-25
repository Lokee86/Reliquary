#!/usr/bin/env python3
from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
WORKSPACE = ROOT.parent
POLICY = ROOT / "tools" / "pitlord" / "policy.json"


def executable(name: str) -> str:
    return name + ".exe" if os.name == "nt" else name


def resolve(environment: str, name: str, candidates: list[Path]) -> Path:
    configured = os.environ.get(environment)
    if configured:
        path = Path(configured).expanduser().resolve()
        if path.is_file():
            return path
        raise RuntimeError(f"{environment} does not name a file: {path}")
    for candidate in candidates:
        if candidate.is_file():
            return candidate.resolve()
    discovered = shutil.which(name)
    if discovered:
        return Path(discovered).resolve()
    raise RuntimeError(
        f"{name} is unavailable; set {environment}, install it on PATH, or build the sibling tool"
    )


def analysis_roots() -> list[Path]:
    configured = os.environ.get("LEXICON_ARCANA_ROOT")
    roots: list[Path] = []
    if configured:
        roots.append(Path(configured).expanduser().resolve())
    roots.append(WORKSPACE / "lexicon-arcana")
    return roots


def lexicon_candidates() -> list[Path]:
    candidates: list[Path] = []
    for root in analysis_roots():
        candidates.append(root / "build" / "bin" / executable("lexicon"))
    return candidates


def arcana_candidates() -> list[Path]:
    candidates: list[Path] = []
    for root in analysis_roots():
        candidates.extend(
            [
                root / "build" / "bin" / executable("arcana"),
                root / "arcana" / "target" / "release" / executable("arcana"),
                root / "arcana" / "target-v3" / "release" / executable("arcana"),
            ]
        )
    return candidates


def adapter_root() -> Path:
    configured = os.environ.get("LEXICON_ADAPTERS")
    if configured:
        path = Path(configured).expanduser().resolve()
        if path.is_dir():
            return path
        raise RuntimeError(f"LEXICON_ADAPTERS does not name a directory: {path}")
    try:
        configuration = json.loads((ROOT / ".lexicon" / "config.json").read_text(encoding="utf-8"))
        path = Path(configuration.get("adapter_root", "")).expanduser().resolve()
        if path.is_dir():
            return path
    except (OSError, ValueError, TypeError):
        pass
    for root in analysis_roots():
        path = root / "lexicon" / "adapters"
        if path.is_dir():
            return path.resolve()
    raise RuntimeError(
        "Lexicon adapters are unavailable; set LEXICON_ADAPTERS or provide the sibling lexicon-arcana checkout"
    )


def run(
    command: list[str],
    *,
    environment: dict[str, str] | None = None,
    timeout: int | None = None,
) -> None:
    print("+", subprocess.list2cmdline(command), flush=True)
    process = subprocess.Popen(command, cwd=ROOT, env=environment)
    try:
        exit_code = process.wait(timeout=timeout)
    except subprocess.TimeoutExpired as error:
        tree_stopped = os.name != "nt"
        if os.name == "nt":
            try:
                stopped = subprocess.run(
                    ["taskkill", "/PID", str(process.pid), "/T", "/F"],
                    stdout=subprocess.DEVNULL,
                    stderr=subprocess.DEVNULL,
                    timeout=10,
                    check=False,
                )
                tree_stopped = stopped.returncode == 0
            except (OSError, subprocess.TimeoutExpired):
                tree_stopped = False
        try:
            process.kill()
        except OSError:
            pass
        process.wait()
        if not tree_stopped:
            raise RuntimeError(
                f"command exceeded its {timeout}s bound and process-tree cleanup was not confirmed: "
                f"{subprocess.list2cmdline(command)}"
            ) from error
        raise RuntimeError(
            f"command exceeded its {timeout}s bound; process tree stopped: "
            f"{subprocess.list2cmdline(command)}"
        ) from error
    if exit_code:
        raise subprocess.CalledProcessError(exit_code, command)


def refresh_evidence(lexicon: Path, arcana: Path, lexicon_timeout: int) -> None:
    # The Rust adapter launches cargo run; optimize analysis without changing
    # application build/test profiles or excluding authored source.
    adapter_environment = os.environ.copy()
    adapter_environment.setdefault("CARGO_PROFILE_DEV_OPT_LEVEL", "3")
    adapters = adapter_root()
    adapter_binary = adapters / "rust" / executable("lexicon-rust")
    adapter_build_receipt = ROOT / ".verification-tmp" / "phase7" / "lexicon-adapter-local-build-current.json"
    if not adapter_binary.is_file() or not adapter_build_receipt.is_file():
        raise RuntimeError("Phase 7 refresh requires the locally patched Rust adapter build and its receipt")
    build = json.loads(adapter_build_receipt.read_text(encoding="utf-8"))
    patch_path = ROOT / "tools" / "pitlord" / "lexicon-toolstate-exclusion.patch"
    expected_patch_hash = hashlib.sha256(patch_path.read_bytes()).hexdigest()
    expected_binary_hash = hashlib.sha256(adapter_binary.read_bytes()).hexdigest()
    sibling = WORKSPACE / "lexicon-arcana"
    sibling_revision = subprocess.run(
        ["git", "rev-parse", "HEAD"], cwd=sibling, capture_output=True, text=True, check=True
    ).stdout.strip()
    if (
        build.get("schema") != "phase7-local-lexicon-adapter-build.v2"
        or build.get("source_manifest_protocol") != "reliquary.production-source-manifest.v1"
        or build.get("patch_sha256") != expected_patch_hash
        or build.get("binary_sha256") != expected_binary_hash
        or build.get("source_revision") != sibling_revision
    ):
        raise RuntimeError("local Lexicon adapter receipt does not match the configured patch, binary, and sibling revision")
    component_map_path = ROOT / "tools" / "pitlord" / "production-components.json"
    dag_path = ROOT / "tools" / "pitlord" / "convergence-component-dag.json"
    component_map_hash = hashlib.sha256(component_map_path.read_bytes()).hexdigest()
    dag_hash = hashlib.sha256(dag_path.read_bytes()).hexdigest()
    mapping = json.loads(component_map_path.read_text(encoding="utf-8"))
    profile = mapping["cfg_profile"]
    cfg_facts = set(profile["rustc_cfg_facts"]) | {
        f'feature="{feature}"' for feature in profile.get("features", [])
    }
    sys.path.insert(0, str(ROOT / "scripts"))
    from check_owner_boundaries import module_tree

    module_paths = [
        path.replace("\\", "/")
        for path in module_tree(ROOT, cfg_facts).values()
        if path.startswith("src/")
    ]
    production_paths = sorted(set(module_paths))
    # `module_tree` records semantic modules, while Cargo's crate root is the
    # assembly point that exposes them. Keep it in the analyzer manifest with
    # an explicit role; it is not a semantic component ownership row.
    assembly_root = "src/lib.rs"
    manifest_paths = sorted(set(production_paths) | {assembly_root})
    if not (ROOT / assembly_root).is_file():
        raise RuntimeError("configured Rust crate assembly root src/lib.rs is missing")
    mapped: dict[str, list[dict]] = {}
    for row in mapping["files"]:
        mapped.setdefault(row["path"].replace("\\", "/"), []).append(row)
    dag = json.loads(dag_path.read_text(encoding="utf-8"))
    component_ids = {row["id"] for row in dag["components"]}
    if not production_paths or any(
        path not in mapped
        or len(mapped[path]) != 1
        or mapped[path][0].get("classification") != "production"
        or mapped[path][0].get("component") not in component_ids
        for path in production_paths
    ):
        raise RuntimeError("cfg-reachable production sources are not completely mapped")
    source_rows = []
    for relative in manifest_paths:
        path = ROOT / relative
        source_rows.append({"path": relative, "sha256": hashlib.sha256(path.read_bytes()).hexdigest()})
    manifest = ROOT / ".verification-tmp" / "phase7" / "production-source-manifest.json"
    manifest.parent.mkdir(parents=True, exist_ok=True)
    manifest.write_text(
        json.dumps({
            "schema": "reliquary.production-source-manifest.v1",
            "cfg_profile": profile,
            "cfg_profile_sha256": hashlib.sha256(
                json.dumps(profile, sort_keys=True, separators=(",", ":")).encode()
            ).hexdigest(),
            "component_map_sha256": component_map_hash,
            "module_tree_sha256": hashlib.sha256(
                json.dumps(manifest_paths, sort_keys=True, separators=(",", ":")).encode()
            ).hexdigest(),
            "module_tree_entry_count": len(manifest_paths),
            "semantic_production_module_count": len(production_paths),
            "duplicate_module_alias_count": len(module_paths) - len(production_paths),
            "crate_assembly_root": {"path": assembly_root, "role": "crate-assembly-root"},
            "file_count": len(source_rows),
            "files": source_rows,
        }, sort_keys=True, indent=2) + "\n",
        encoding="utf-8",
    )
    manifest_hash = hashlib.sha256(manifest.read_bytes()).hexdigest()
    adapter_environment["LEXICON_RUST_SOURCE_MANIFEST"] = str(manifest.resolve())
    progress_log = ROOT / ".verification-tmp" / "phase7" / "lexicon-progress-current.jsonl"
    progress_log.write_text("", encoding="utf-8")
    adapter_environment["LEXICON_PROGRESS_LOG"] = str(progress_log.resolve())
    try:
        configuration = json.loads((ROOT / ".lexicon" / "config.json").read_text(encoding="utf-8"))
        configured = (
            configuration.get("version") == 1
            and Path(configuration.get("adapter_root", "")).resolve() == adapters
            and configuration.get("enabled_languages") == ["rust"]
        )
    except (OSError, ValueError, TypeError):
        configured = False
    # An unchanged configuration needs one explicit scan, not init's scan plus
    # another scan. Reconfigure first when adapters or enabled languages differ.
    if not configured:
        run(
            [
                str(lexicon),
                "init",
                "--repo",
                str(ROOT),
                "--adapters",
                str(adapters),
                # semantic.json owns authored Rust under src; Python guards run above.
                "--languages",
                "rust",
            ],
            environment=adapter_environment,
            timeout=lexicon_timeout,
        )
    # `scan` may reuse an unchanged language snapshot and skip the adapter. The
    # manifest acknowledgement below is meaningful only if this refresh actually
    # invokes Rust extraction, so rebuild the configured Rust library explicitly.
    # The CLI rebuild path remains bounded by the same timeout and publishes a
    # normal immutable snapshot without exporting or rewriting source evidence.
    run(
        [str(lexicon), "rebuild", "--repo", str(ROOT), "--languages", "rust"],
        environment=adapter_environment,
        timeout=lexicon_timeout,
    )
    expected_ack = f"accepted production source manifest sha256={manifest_hash} files={len(source_rows)}"
    progress = progress_log.read_text(encoding="utf-8")
    if expected_ack not in progress:
        raise RuntimeError("configured Lexicon adapter did not acknowledge the exact production source manifest")
    extracted = re.search(r"extracted (\d+) callable symbols", progress)
    if extracted is None or int(extracted.group(1)) == 0:
        raise RuntimeError("Lexicon produced no callable symbol inventory for the production source manifest")
    rust_output = ROOT / ".lexicon" / "tmp" / "rust.jsonl"
    if not rust_output.is_file():
        raise RuntimeError("Lexicon did not retain its Rust source analysis output")
    expected_symbols = {
        ("src/container/container.rs", "create"),
        ("src/facade/cva_lifecycle.rs", "open"),
        ("src/facade/cva_import_completion.rs", "complete_native_import"),
    }
    observed_symbols: set[tuple[str, str]] = set()
    with rust_output.open(encoding="utf-8") as stream:
        for line in stream:
            try:
                fact = json.loads(line)
            except ValueError:
                continue
            if (
                fact.get("record") == "node"
                and fact.get("kind") in {"function", "method"}
                and isinstance(fact.get("path"), str)
                and isinstance(fact.get("name"), str)
            ):
                observed_symbols.add((fact["path"].replace("\\", "/"), fact["name"]))
    missing_symbols = sorted(expected_symbols - observed_symbols)
    if missing_symbols:
        raise RuntimeError(f"Lexicon production graph omitted known symbols: {missing_symbols}")
    for row in source_rows:
        if hashlib.sha256((ROOT / row["path"]).read_bytes()).hexdigest() != row["sha256"]:
            raise RuntimeError(f"source changed during Lexicon refresh: {row['path']}")
    if hashlib.sha256((ROOT / "tools" / "pitlord" / "production-components.json").read_bytes()).hexdigest() != component_map_hash:
        raise RuntimeError("production component map changed during Lexicon refresh")
    if hashlib.sha256((ROOT / "tools" / "pitlord" / "convergence-component-dag.json").read_bytes()).hexdigest() != dag_hash:
        raise RuntimeError("component DAG changed during Lexicon refresh")
    run(
        [
            str(arcana),
            "sync",
            "--lexicon",
            str(ROOT / ".lexicon"),
            "--state",
            str(ROOT / ".arcana"),
        ],
        timeout=5 * 60,
    )


def main() -> int:
    parser = argparse.ArgumentParser(description="Validate Reliquary architecture policy.")
    parser.add_argument(
        "--refresh",
        action="store_true",
        help="rebuild Lexicon/Arcana evidence before checking policy",
    )
    parser.add_argument(
        "--lexicon-timeout-seconds",
        type=int,
        default=20 * 60,
        help="bounded Lexicon init/scan cap (1..1200 seconds; default 1200)",
    )
    parser.add_argument(
        "--compiler-evidence",
        type=Path,
        help="current phase7-compiler-calls.v1 report (or PHASE7_COMPILER_EVIDENCE)",
    )
    parser.add_argument(
        "--source-only",
        action="store_true",
        help="run source/graph policy checks without claiming full compiler-backed architecture acceptance",
    )
    args = parser.parse_args()
    if not 1 <= args.lexicon_timeout_seconds <= 20 * 60:
        parser.error("--lexicon-timeout-seconds must be between 1 and 1200")

    run([sys.executable, "-m", "unittest", "scripts.test_phase7_child_registry"])
    run([sys.executable, "scripts/check_phase7_child_registry.py"])
    run([sys.executable, "-m", "unittest", "scripts.test_phase7_component_map"])
    run([sys.executable, "scripts/check_phase7_component_map.py"])
    run([sys.executable, "-m", "unittest", "discover", "-s", "scripts/tests", "-p", "test_phase7_dependencies.py"])
    run([sys.executable, "scripts/check_phase7_dependencies.py", "--check"])
    run([sys.executable, "-m", "unittest", "scripts.test_phase7_source_cuts", "scripts.test_phase7_compiler_calls"])
    run([sys.executable, "scripts/check_phase7_source_cuts.py"])

    run([sys.executable, "scripts/check_multiplexing_phase4_coverage.py", "--self-test"])

    run([sys.executable, "scripts/check_external_source_ownership.py", "--self-test"])

    run([sys.executable, "scripts/check_owner_boundaries.py", "--self-test"])
    run([sys.executable, "scripts/check_owner_boundaries.py"])
    run([sys.executable, "scripts/check_runtime_boundaries.py", "--self-test"])
    run([sys.executable, "scripts/check_facade_boundaries.py", "--self-test"])
    run([sys.executable, "scripts/check_history_boundaries.py", "--self-test"])
    run([sys.executable, "scripts/check_history_boundaries.py"])

    run([sys.executable, "scripts/check_physical_boundary.py"])

    pitlord = resolve(
        "PITLORD",
        "pitlord",
        [WORKSPACE / "pitlord" / "bin" / executable("pitlord")],
    )
    arcana = resolve("ARCANA", "arcana", arcana_candidates())

    if args.refresh:
        lexicon = resolve("LEXICON", "lexicon", lexicon_candidates())
        refresh_evidence(lexicon, arcana, args.lexicon_timeout_seconds)

    if not (ROOT / ".arcana" / "CURRENT").is_file():
        raise RuntimeError(
            "Reliquary has no current Arcana snapshot; run this command again with --refresh"
        )

    run([str(pitlord), "validate", "--policy", str(POLICY)])
    run(
        [
            str(pitlord),
            "check",
            "--repo",
            str(ROOT),
            "--policy",
            str(POLICY),
            "--arcana",
            str(arcana),
            "--timeout",
            "2m",
        ]
    )
    run([sys.executable, "scripts/check_dependency_convergence.py"])
    if args.source_only:
        print("SOURCE-ONLY: compiler call evidence was not checked; this is not full architecture acceptance.")
        return 0
    compiler_report = args.compiler_evidence or Path(
        os.environ.get(
            "PHASE7_COMPILER_EVIDENCE",
            ".verification-tmp/phase7/closure-forwarder-current-20261024/calls.json",
        )
    )
    run([sys.executable, "scripts/check_phase7_compiler_gate.py", "--report", str(compiler_report)])
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (RuntimeError, subprocess.CalledProcessError) as error:
        print(f"architecture check failed: {error}", file=sys.stderr)
        raise SystemExit(1)
