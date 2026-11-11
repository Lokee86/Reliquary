#!/usr/bin/env python3
"""Prove documentation/architecture gates reject real fixture mutations."""
from __future__ import annotations

import json
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def run(cwd: Path, *args: str) -> subprocess.CompletedProcess[str]:
    return subprocess.run(args, cwd=cwd, text=True, capture_output=True, check=False)


def require_ok(result: subprocess.CompletedProcess[str]) -> None:
    if result.returncode:
        raise RuntimeError(result.stdout + result.stderr)


def docs(repo: Path, *args: str) -> dict:
    result = run(repo, sys.executable, ".standards/docs_policy/check.py", "--repo", ".", "--json", *args)
    if result.returncode not in (0, 1):
        raise RuntimeError(result.stdout + result.stderr)
    report = json.loads(result.stdout)
    if bool(result.returncode) != bool(report["findings"]):
        raise RuntimeError("checker exit status disagrees with findings")
    return report


def rejected(report: dict, code: str) -> None:
    if not any(finding["code"] == code for finding in report["findings"]):
        raise RuntimeError(f"negative control failed to detect {code}")
    print(f"PASS rejected {code}")


def main() -> int:
    # The Rust adapter ignores target/ even when it walks nested Cargo manifests.
    scratch = ROOT / "target" / "standards-gate-controls"
    scratch.mkdir(parents=True, exist_ok=True)
    tracked = run(ROOT, "git", "ls-files", "-z")
    require_ok(tracked)
    with tempfile.TemporaryDirectory(prefix="gate-controls-", dir=scratch) as temporary:
        repo = Path(temporary)
        for name in tracked.stdout.split("\0"):
            if name and (ROOT / name).is_file():
                destination = repo / name
                destination.parent.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(ROOT / name, destination)
        for args in (
            ("git", "init", "--quiet"),
            ("git", "config", "user.name", "Gate control fixture"),
            ("git", "config", "user.email", "fixture@invalid.local"),
            ("git", "config", "core.autocrlf", "false"),
            ("git", "add", "."),
            ("git", "-c", "core.hooksPath=", "commit", "--quiet", "-m", "fixture baseline"),
        ):
            require_ok(run(repo, *args))
        baseline = run(repo, "git", "rev-parse", "HEAD").stdout.strip()
        clean = docs(repo)
        if clean["findings"] or clean["baselined_findings"]:
            raise RuntimeError("clean fixture has documentation debt: " + json.dumps(clean))
        require_ok(run(repo, sys.executable, "scripts/check_architecture.py"))
        require_ok(run(repo, sys.executable, "scripts/check_documentation_entrypoints.py"))
        print("PASS clean documentation and exact DAG baseline")

        required = repo / "docs/architecture/system.md"
        original = required.read_bytes()
        required.unlink()
        rejected(docs(repo), "missing-required-path")
        required.write_bytes(original)

        index = repo / "docs/INDEX.md"
        original = index.read_bytes()
        with index.open("a", encoding="utf-8") as stream:
            stream.write("\n[Negative control](missing-negative-control.md)\n")
        rejected(docs(repo), "broken-link")
        index.write_bytes(original)

        entrypoint = repo / "README.md"
        original = entrypoint.read_bytes()
        with entrypoint.open("a", encoding="utf-8") as stream:
            stream.write("\n[Negative control](missing-entrypoint-negative-control.md)\n")
        result = run(repo, sys.executable, "scripts/check_documentation_entrypoints.py")
        if result.returncode != 1 or "README.md: broken link" not in result.stdout:
            raise RuntimeError("root entry-point negative control failed: " + result.stdout + result.stderr)
        print("PASS rejected broken root entry-point link")
        entrypoint.write_bytes(original)

        public = repo / "src/lib.rs"
        with public.open("a", encoding="utf-8") as stream:
            stream.write("\n// Gate-control public-surface change.\n")
        require_ok(run(repo, "git", "add", "src/lib.rs"))
        require_ok(run(repo, "git", "-c", "core.hooksPath=", "commit", "--quiet", "-m", "undocumented surface fixture"))
        rejected(docs(repo, "--changed-from", baseline), "missing-documentation-impact")
        # An unrelated owner must not satisfy the public Root mapping.
        with (repo / "docs/architecture/storage.md").open("a", encoding="utf-8") as stream:
            stream.write("\nUnrelated owner change for negative control.\n")
        require_ok(run(repo, "git", "add", "docs/architecture/storage.md"))
        require_ok(run(repo, "git", "-c", "core.hooksPath=", "commit", "--quiet", "-m", "unrelated docs fixture"))
        rejected(docs(repo, "--changed-from", baseline), "missing-documentation-impact")

        manifest = repo / "Cargo.toml"
        original = manifest.read_bytes()

        def architecture_result(content: str, label: str) -> subprocess.CompletedProcess[str]:
            manifest.write_text(content, encoding="utf-8")
            result = run(repo, sys.executable, "scripts/check_architecture.py")
            if result.returncode == 0 or "unapproved production import: reliquary-memory -> reliquary-container" not in result.stderr:
                raise RuntimeError(f"negative control failed to reject {label}: " + result.stdout + result.stderr)
            print(f"PASS rejected {label}")
            manifest.write_bytes(original)
            return result

        content = original.decode("utf-8")
        architecture_result(content.replace(
            "[dev-dependencies]",
            'leaky = { package = "reliquary-container", path = "crates/reliquary-container" }\n\n[dev-dependencies]',
            1,
        ), "aliased production dependency")
        architecture_result(content.replace(
            "[dev-dependencies]",
            'leaky = { path = "crates/reliquary-container" }\n\n[dev-dependencies]',
            1,
        ), "path-resolved production dependency alias")
        manifest.write_text(content.replace(
            "[dev-dependencies]",
            'leaky = { package = "reliquary-application", path = "crates/reliquary-container" }\n\n[dev-dependencies]',
            1,
        ), encoding="utf-8")
        result = run(repo, sys.executable, "scripts/check_architecture.py")
        if result.returncode == 0 or "dependency package/path mismatch" not in result.stderr:
            raise RuntimeError("negative control failed to reject a package/path name mismatch: " + result.stdout + result.stderr)
        print("PASS rejected dependency package/path name mismatch")
        manifest.write_bytes(original)
        architecture_result(content.replace(
            "[patch.crates-io]",
            "[target.'cfg(windows)'.dependencies]\n"
            'leaky = { package = "reliquary-container", path = "crates/reliquary-container" }\n\n'
            "[patch.crates-io]",
            1,
        ), "target-conditioned production dependency")
        architecture_result(content.replace(
            "[package]\n",
            '[workspace.dependencies]\n'
            'leaky = { package = "reliquary-container", path = "crates/reliquary-container" }\n\n'
            "[package]\n",
            1,
        ).replace(
            "[dev-dependencies]",
            "leaky = { workspace = true }\n\n[dev-dependencies]",
            1,
        ), "workspace-inherited production dependency")

        manifest.write_text(content.replace(
            'reliquary-application = { path = "crates/reliquary-application" }',
            'reliquary-application = "0.1"',
            1,
        ), encoding="utf-8")
        result = run(repo, sys.executable, "scripts/check_architecture.py")
        if result.returncode == 0 or "internal package dependency must use its canonical local path" not in result.stderr:
            raise RuntimeError("negative control failed to reject registry replacement of an internal package: " + result.stdout + result.stderr)
        print("PASS rejected external replacement of an internal package")
        manifest.write_bytes(original)

        # Dev dependencies are reported for context but remain outside the runtime DAG.
        manifest.write_text(content.replace(
            "[dev-dependencies]",
            '[dev-dependencies]\ntest-leaky = { package = "reliquary-container", path = "crates/reliquary-container" }',
            1,
        ), encoding="utf-8")
        result = run(repo, sys.executable, "scripts/check_architecture.py")
        if result.returncode or "Test-only dependency edges: 15; not part of production DAG." not in result.stdout:
            raise RuntimeError("permitted dev dependency was rejected or miscounted: " + result.stdout + result.stderr)
        print("PASS allowed test-only package dependency")
        manifest.write_bytes(original)
    print("All gate controls passed; temporary fixture removed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
