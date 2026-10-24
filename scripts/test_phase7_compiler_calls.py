"""Negative fixtures for compiler implementation ownership (including aliases)."""
import hashlib
import json
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parent))
from check_phase7_compiler_calls import analyze, source_fingerprint, validate_provenance, without_generics


class CompilerCallTests(unittest.TestCase):
    def fixture(self, implementation="src/facade/api.rs", call="owner::Owner::read",
                source="src/runtime/run.rs"):
        doc = {"external_crates": {"1": {"name": "std"}},
               "paths": {"1": {"crate_id": 0, "path": ["reliquary_memory", "owner", "Owner"]}},
               "index": {
                   "2": {"crate_id": 0, "name": None, "inner": {"impl": {
                       "for": {"resolved_path": {"id": 1}}, "trait": None, "items": [3]}}},
                   "3": {"crate_id": 0, "name": "read", "span": {
                       "filename": implementation, "begin": [10, 1], "end": [12, 1]},
                       "inner": {"function": {}}}}}
        mir = [f" _0 = {call}(copy _1) -> [return: bb1, unwind continue]; // scope 0 at {source}:20:1: 20:30\n",
               f" // + const_: Const {{ ty: fn(&Owner) -> u8 {{{call}}}, val: Value({call}) }}\n"]
        mapping = {"files": [{"path": "src/runtime/run.rs", "component": "runtime"},
                              {"path": "src/facade/api.rs", "component": "facade"},
                              {"path": "src/owner/api.rs", "component": "owner"}]}
        dag = {"components": [{"id": "runtime", "depends_on": [{"to": "owner"}]},
                              {"id": "owner", "depends_on": []},
                              {"id": "facade", "depends_on": [{"to": "runtime"}]}]}
        return doc, mir, mapping, dag

    def test_lower_alias_does_not_hide_facade_implementation(self):
        report = analyze(*self.fixture())
        self.assertEqual(1, len(report["violations"]))
        self.assertEqual("facade", report["violations"][0]["target_component"])

    def test_allowed_owner_implementation_passes(self):
        report = analyze(*self.fixture("src/owner/api.rs"))
        self.assertEqual([], report["violations"])
        self.assertEqual([], report["unknown_calls"])

    def test_mir_relative_parent_path_maps_to_component(self):
        doc, mir, mapping, dag = self.fixture(source="src/runtime/sub/../run.rs")
        report = analyze(doc, mir, mapping, dag)
        self.assertEqual(1, len(report["violations"]))
        self.assertEqual("runtime", report["violations"][0]["source_component"])
        self.assertEqual([], report["unknown_calls"])

    def test_missing_local_target_fails_closed(self):
        report = analyze(*self.fixture(call="owner::Owner::missing"))
        self.assertEqual(1, len(report["unknown_calls"]))

    def test_unresolved_trait_call_does_not_fall_back_to_inherent_method(self):
        report = analyze(*self.fixture(call="<owner::Owner as core::fmt::Debug>::read"))
        self.assertEqual([], report["violations"])
        self.assertEqual(1, len(report["unknown_calls"]))
        self.assertEqual([], report["cross_component_calls"])

    def test_missing_local_trait_impl_is_not_misclassified_as_external(self):
        report = analyze(*self.fixture(call="<owner::Owner as std::fmt::Debug>::fmt"))
        self.assertEqual(0, report["external_constant_calls"])
        self.assertEqual(1, len(report["unknown_calls"]))

    def test_closure_targets_remain_unknown_even_with_exact_rustdoc_symbol(self):
        doc, mir, mapping, dag = self.fixture(call="owner::Owner::{closure#0}")
        doc["paths"]["4"] = {"crate_id": 0,
                               "path": ["reliquary_memory", "owner", "Owner", "{closure#0}"]}
        doc["index"]["4"] = {"crate_id": 0, "name": "{closure#0}",
                              "span": {"filename": "src/owner/api.rs", "begin": [15, 1],
                                       "end": [16, 1]}, "inner": {"function": {}}}
        report = analyze(doc, mir, mapping, dag)
        self.assertEqual(1, len(report["unknown_calls"]))
        self.assertEqual("unresolved compiler closure target", report["unknown_calls"][0]["reason"])

    def test_external_target_separately_counted(self):
        report = analyze(*self.fixture(call="std::mem::drop"))
        self.assertEqual(1, report["external_constant_calls"])
        self.assertEqual([], report["unknown_calls"])

    def test_external_target_without_local_span_is_not_material_unknown(self):
        report = analyze(*self.fixture(call="std::fmt::format", source="/rustc/library/alloc/src/macros.rs"))
        self.assertEqual(1, report["external_constant_calls"])
        self.assertEqual(1, report["external_calls_without_mapped_caller"])
        self.assertEqual([], report["unknown_calls"])

    def test_unrecognized_unqualified_crate_root_remains_unknown(self):
        report = analyze(*self.fixture(call="untracked_crate::Thing::run"))
        self.assertEqual(0, report["external_constant_calls"])
        self.assertEqual(1, len(report["unknown_calls"]))

    def test_turbofish_does_not_change_implementation_identity(self):
        self.assertEqual("owner::Owner::read", without_generics("owner::Owner::<Vec<u8>>::read::<u32>"))

    def test_unique_shortened_compiler_symbol_remains_unknown(self):
        report = analyze(*self.fixture(call="Owner::read"))
        self.assertEqual([], report["cross_component_calls"])
        self.assertEqual(1, len(report["unknown_calls"]))

    def test_ambiguous_shortened_compiler_symbol_remains_unknown(self):
        doc, mir, mapping, dag = self.fixture(call="Owner::read")
        doc["paths"]["4"] = {"crate_id": 0, "path": ["reliquary_memory", "other", "Owner"]}
        doc["index"]["4"] = {"crate_id": 0, "name": None, "inner": {"impl": {
            "for": {"resolved_path": {"id": 4}}, "trait": None, "items": [5]}}}
        doc["index"]["5"] = {"crate_id": 0, "name": "read", "span": {
            "filename": "src/owner/api.rs", "begin": [20, 1], "end": [22, 1]},
            "inner": {"function": {}}}
        report = analyze(doc, mir, mapping, dag)
        self.assertEqual([], report["cross_component_calls"])
        self.assertEqual(1, len(report["unknown_calls"]))

    def test_external_and_local_same_short_name_remains_unknown(self):
        doc, mir, mapping, dag = self.fixture(call="Owner::read")
        doc["external_crates"]["2"] = {"name": "external_owner"}
        doc["paths"]["8"] = {"crate_id": 2, "path": ["external_owner", "Owner"]}
        report = analyze(doc, mir, mapping, dag)
        self.assertEqual(0, report["external_constant_calls"])
        self.assertEqual(1, len(report["unknown_calls"]))

    def test_indirect_call_retained_as_unknown(self):
        doc, _, mapping, dag = self.fixture()
        report = analyze(doc, ["_0 = move _3(copy _1) -> [return: bb1, unwind continue]; // scope 0 at src/runtime/run.rs:20:1: 20:30\n", "}\n"], mapping, dag)
        self.assertEqual(1, len(report["unknown_calls"]))

    def test_verbose_internal_constvalue_preserves_short_symbol_as_unknown(self):
        doc, _, mapping, dag = self.fixture()
        mir = [" _0 = Owner::read(copy _1) -> [return: bb1, unwind continue]; // scope 0 at src/runtime/run.rs:20:1: 20:30\n",
               " // + const_: Const { ty: fn(&Owner) -> u8 {owner::Owner::read}, val: Value(ConstValue(ZeroSized: fn(&Owner) -> u8 {Owner::read})) }\n"]
        report = analyze(doc, mir, mapping, dag)
        self.assertEqual([], report["cross_component_calls"])
        self.assertEqual(1, len(report["unknown_calls"]))

    def test_drop_terminator_remains_explicit_destructor_unknown(self):
        doc, _, mapping, dag = self.fixture()
        mir = [" drop(_3) -> [return: bb1, unwind continue]; // scope 0 at src/runtime/run.rs:20:1: 20:30\n",
               "}\n"]
        report = analyze(doc, mir, mapping, dag)
        self.assertEqual("drop terminator requires destructor analysis",
                         report["unknown_calls"][0]["reason"])

    def test_provenance_must_match_artifacts_and_current_sources(self):
        scratch = Path(".verification-tmp/phase7")
        scratch.mkdir(parents=True, exist_ok=True)
        with tempfile.TemporaryDirectory(prefix="compiler-provenance-test-", dir=scratch) as temp:
            root = Path(temp)
            source_file = root / "src/lib.rs"
            source_file.parent.mkdir()
            source_file.write_text("pub fn current() {}\n", encoding="utf-8")
            entry = {"path": "src/lib.rs",
                     "sha256": hashlib.sha256(source_file.read_bytes()).hexdigest()}
            fingerprint = source_fingerprint([entry])
            manifest = {"schema": "phase7-compiler-source-manifest.v1",
                        "files": [entry], "fingerprint_sha256": fingerprint}
            manifest_path = root / "compiler-source-manifest.json"
            manifest_path.write_text(json.dumps(manifest), encoding="utf-8")
            artifact = Path(__file__).resolve()
            artifact_hash = hashlib.sha256(artifact.read_bytes()).hexdigest()
            receipt = {"schema": "phase7-compiler-provenance.v1",
                       "source_snapshot": {"before_sha256": fingerprint,
                                           "after_sha256": fingerprint,
                                           "file_count": 1,
                                           "manifest_path": "compiler-source-manifest.json",
                                           "manifest_sha256": hashlib.sha256(
                                               manifest_path.read_bytes()).hexdigest()},
                       "artifacts": {"rustdoc": {"path": str(artifact), "sha256": artifact_hash},
                                     "mir": {"path": str(artifact), "sha256": artifact_hash}}}
            with mock.patch("check_phase7_compiler_calls.ROOT", root):
                valid = validate_provenance(receipt, artifact, artifact)
                self.assertTrue(valid["validated"], valid["errors"])
                source_file.write_text("pub fn changed() {}\n", encoding="utf-8")
                changed = validate_provenance(receipt, artifact, artifact)
                self.assertFalse(changed["validated"])
                self.assertIn("current Rust source files differ from the emitted source manifest",
                              changed["errors"])
                wrong_artifact = validate_provenance(receipt, artifact, source_file)
                self.assertFalse(wrong_artifact["validated"])


if __name__ == "__main__":
    unittest.main()
