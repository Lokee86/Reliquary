from __future__ import annotations

import hashlib
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location(
    "check_phase7_compiler_gate", ROOT / "scripts/check_phase7_compiler_gate.py"
)
gate = importlib.util.module_from_spec(SPEC)
assert SPEC.loader
SPEC.loader.exec_module(gate)


def sha(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


class CompilerGateTests(unittest.TestCase):
    def setUp(self) -> None:
        base = ROOT / ".verification-tmp/phase7"
        base.mkdir(parents=True, exist_ok=True)
        self.temp = tempfile.TemporaryDirectory(prefix="compiler-gate-", dir=base)
        self.root = Path(self.temp.name)
        for relative, content in {
            "src/lib.rs": "pub fn stable() {}\n",
            "Cargo.toml": "[package]\nname='fixture'\nversion='0.1.0'\nedition='2021'\n",
            "Cargo.lock": "version = 4\n",
            "tools/pitlord/production-components.json": "{}\n",
            "tools/pitlord/convergence-component-dag.json": "{\"components\": []}\n",
            "scripts/check_phase7_compiler_calls.py": "# checker fixture\n",
            "scripts/emit_phase7_compiler_evidence.py": "# emitter fixture\n",
        }.items():
            path = self.root / relative
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(content, encoding="utf-8")
        self.artifacts = {}
        for name in ("rustdoc", "mir", "hir", "thir"):
            path = self.root / f"{name}.artifact"
            path.write_text(name, encoding="utf-8")
            self.artifacts[name] = path
        self.logs = {}
        for name in ("rustdoc", "mir", "hir", "thir"):
            path = self.root / f"{name}.log"
            path.write_text("ok", encoding="utf-8")
            self.logs[name] = path
        manifest_files = [{"path": "src/lib.rs", "sha256": sha(self.root / "src/lib.rs")}]
        fingerprint = hashlib.sha256(json.dumps(
            manifest_files, sort_keys=True, separators=(",", ":")
        ).encode()).hexdigest()
        manifest = self.root / "source-manifest.json"
        manifest.write_text(json.dumps({
            "schema": "phase7-compiler-source-manifest.v1",
            "files": manifest_files,
            "fingerprint_sha256": fingerprint,
        }), encoding="utf-8")
        self.manifest = manifest
        policy_paths = (
            "Cargo.toml", "Cargo.lock", "tools/pitlord/production-components.json",
            "tools/pitlord/convergence-component-dag.json",
        )
        policy_hashes = {relative: sha(self.root / relative) for relative in policy_paths}
        provenance = {
            "schema": "phase7-compiler-provenance.v1",
            "policy_hashes": policy_hashes,
            "policy_hashes_after": policy_hashes,
            "tools": {"emitter": {
                "path": "scripts/emit_phase7_compiler_evidence.py",
                "sha256_before": sha(self.root / "scripts/emit_phase7_compiler_evidence.py"),
                "sha256_after": sha(self.root / "scripts/emit_phase7_compiler_evidence.py"),
            }},
            "build_profile": "dev", "features": [], "cfg": [],
            "environment": {}, "rustc_version": "rustc fixture",
            "commands": [{"name": name, "exit": 0,
                           "log": path.relative_to(self.root).as_posix(),
                           "log_sha256": sha(path)} for name, path in self.logs.items()],
            "source_snapshot": {
                "before_sha256": fingerprint, "after_sha256": fingerprint,
                "file_count": 1, "manifest_path": "source-manifest.json",
                "manifest_sha256": sha(manifest),
            },
            "artifacts": {name: {"path": str(path), "sha256": sha(path)}
                          for name, path in self.artifacts.items()},
        }
        self.provenance_path = self.root / "provenance.json"
        self.provenance_path.write_text(json.dumps(provenance), encoding="utf-8")
        inputs = {
            **{str(path.resolve()): sha(path) for path in self.artifacts.values()},
            **{str(path.resolve()): sha(path) for path in self.logs.values()},
            str((self.root / "tools/pitlord/production-components.json").resolve()):
                sha(self.root / "tools/pitlord/production-components.json"),
            str((self.root / "tools/pitlord/convergence-component-dag.json").resolve()):
                sha(self.root / "tools/pitlord/convergence-component-dag.json"),
            str((self.root / "scripts/check_phase7_compiler_calls.py").resolve()):
                sha(self.root / "scripts/check_phase7_compiler_calls.py"),
            str(self.provenance_path.resolve()): sha(self.provenance_path),
        }
        self.report = {
            "schema": "phase7-compiler-calls.v1", "inputs": inputs,
            "build_provenance": provenance,
            "provenance_validation": {"validated": True, "errors": []},
            "cross_component_calls": [], "violations": [], "unknown_calls": [],
        }
        self.report_path = self.root / "calls.json"
        self.write_report()

    def tearDown(self) -> None:
        self.temp.cleanup()

    def write_report(self) -> None:
        self.report_path.write_text(json.dumps(self.report), encoding="utf-8")

    def repin_dag_provenance(self) -> None:
        provenance = json.loads(self.provenance_path.read_text(encoding="utf-8"))
        relative = "tools/pitlord/convergence-component-dag.json"
        provenance["policy_hashes"][relative] = sha(self.root / relative)
        provenance["policy_hashes_after"] = dict(provenance["policy_hashes"])
        self.provenance_path.write_text(json.dumps(provenance), encoding="utf-8")
        self.report["build_provenance"] = provenance
        provenance_key = str(self.provenance_path.resolve())
        self.report["inputs"][provenance_key] = sha(self.provenance_path)

    def test_exact_zero_violation_and_unknown_report_passes(self) -> None:
        self.assertEqual(gate.validate_report(self.report_path, self.root)["status"], "pass")

    def test_stale_source_manifest_fails(self) -> None:
        (self.root / "src/lib.rs").write_text("pub fn changed() {}\n", encoding="utf-8")
        result = gate.validate_report(self.report_path, self.root)
        self.assertEqual(result["status"], "fail")
        self.assertTrue(any("current Rust source files differ" in error for error in result["errors"]))

    def test_tampered_policy_input_fails(self) -> None:
        (self.root / "tools/pitlord/convergence-component-dag.json").write_text("{\"changed\":true}\n")
        result = gate.validate_report(self.report_path, self.root)
        self.assertEqual(result["status"], "fail")
        self.assertTrue(any("component DAG" in error for error in result["errors"]))

    def test_stale_checker_hash_fails(self) -> None:
        key = str((self.root / "scripts/check_phase7_compiler_calls.py").resolve())
        self.report["inputs"][key] = "0" * 64
        self.write_report()
        result = gate.validate_report(self.report_path, self.root)
        self.assertEqual(result["status"], "fail")
        self.assertTrue(any("compiler checker" in error for error in result["errors"]))

    def test_missing_thir_input_fails(self) -> None:
        key = str(self.artifacts["thir"].resolve())
        del self.report["inputs"][key]
        self.write_report()
        result = gate.validate_report(self.report_path, self.root)
        self.assertEqual(result["status"], "fail")
        self.assertTrue(any("thir artifact" in error for error in result["errors"]))

    def test_built_mir_manifest_and_each_dump_are_revalidated(self) -> None:
        dump_dir = self.root / "built-mir"
        dump_dir.mkdir()
        dump = dump_dir / "fixture.{impl#0}-clone.1-1-000.built.after.mir"
        dump.write_text("// MIR for `<impl at src/lib.rs:1:1: 1:8>::clone` after built\n\n"
                        "fn <impl at src/lib.rs:1:1: 1:8>::clone(_1: &Foo) -> Foo {\n}\n",
                        encoding="utf-8")
        manifest = self.root / "built-mir-manifest.json"
        manifest.write_text(json.dumps({
            "schema": "phase7-built-mir-manifest.v1",
            "directory": "built-mir",
            "files": [{"path": "built-mir/" + dump.name, "sha256": sha(dump)}],
        }), encoding="utf-8")
        provenance = json.loads(self.provenance_path.read_text(encoding="utf-8"))
        provenance["artifacts"]["built_mir_manifest"] = {
            "path": str(manifest), "sha256": sha(manifest)}
        next(row for row in provenance["commands"] if row["name"] == "mir")["command"] = [
            "cargo", "rustc", "-Zdump-mir=built", f"-Zdump-mir-dir={dump_dir}"]
        self.provenance_path.write_text(json.dumps(provenance), encoding="utf-8")
        self.report["build_provenance"] = provenance
        self.report["inputs"][str(manifest.resolve())] = sha(manifest)
        self.report["inputs"][str(dump.resolve())] = sha(dump)
        self.report["inputs"][str(self.provenance_path.resolve())] = sha(self.provenance_path)
        self.write_report()
        self.assertEqual("pass", gate.validate_report(self.report_path, self.root)["status"])

        dump.write_text(dump.read_text(encoding="utf-8") + "// changed\n", encoding="utf-8")
        result = gate.validate_report(self.report_path, self.root)
        self.assertEqual("fail", result["status"])
        self.assertTrue(any("built MIR dump" in error or "manifest" in error
                            or str(dump.relative_to(self.root)) in error
                            for error in result["errors"]))

    def test_unapproved_call_fails(self) -> None:
        row = {"caller": "caller_alias", "callee": "forbidden",
               "source_component": "consumer", "target_component": "owner",
               "permitted": False}
        dag = {"components": [
            {"id": "consumer", "depends_on": []},
            {"id": "owner", "depends_on": []},
        ]}
        (self.root / "tools/pitlord/convergence-component-dag.json").write_text(
            json.dumps(dag), encoding="utf-8")
        dag_path = self.root / "tools/pitlord/convergence-component-dag.json"
        self.repin_dag_provenance()
        for key in list(self.report["inputs"]):
            if Path(key) == dag_path.resolve():
                del self.report["inputs"][key]
        self.report["inputs"][str(dag_path.resolve())] = sha(dag_path)
        self.report["cross_component_calls"] = [row]
        self.report["violations"] = [row]
        self.write_report()
        self.assertEqual(gate.validate_report(self.report_path, self.root)["status"], "fail")

    def test_hand_cleared_violations_fail(self) -> None:
        row = {"caller": "caller_alias", "callee": "forbidden",
               "source_component": "consumer", "target_component": "owner",
               "permitted": False}
        dag = {"components": [
            {"id": "consumer", "depends_on": []},
            {"id": "owner", "depends_on": []},
        ]}
        dag_path = self.root / "tools/pitlord/convergence-component-dag.json"
        dag_path.write_text(json.dumps(dag), encoding="utf-8")
        self.repin_dag_provenance()
        for key in list(self.report["inputs"]):
            if Path(key) == dag_path.resolve():
                del self.report["inputs"][key]
        self.report["inputs"][str(dag_path.resolve())] = sha(dag_path)
        self.report["cross_component_calls"] = [row]
        self.report["violations"] = []
        self.write_report()
        result = gate.validate_report(self.report_path, self.root)
        self.assertEqual(result["status"], "fail")
        self.assertTrue(any("DAG-derived" in error for error in result["errors"]))

    def test_unknown_call_fails(self) -> None:
        self.report["unknown_calls"] = [{"reason": "indirect call"}]
        self.write_report()
        self.assertEqual(gate.validate_report(self.report_path, self.root)["status"], "fail")


if __name__ == "__main__":
    unittest.main()
