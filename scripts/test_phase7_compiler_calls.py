"""Negative fixtures for compiler implementation ownership (including aliases)."""
import hashlib
import json
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parent))
from check_phase7_compiler_calls import (
    analyze, callable_const_value, hir_owner_spans, thir_call_sites, source_fingerprint,
    validate_provenance, without_generics, primitive_foreign_receiver,
    concrete_external_trait_receiver,
    external_trait_arguments_are_closed,
    load_built_mir_manifest, mir_calls, thir_call_sites, thir_calls_for_mir_call,
)
from emit_phase7_compiler_evidence import crate_mir_from_cargo_log

ROOT = Path(__file__).resolve().parents[1]


class CompilerCallTests(unittest.TestCase):
    def test_built_mir_identity_joins_same_span_derive_methods_exactly(self):
        scratch = Path(".verification-tmp/phase7")
        scratch.mkdir(parents=True, exist_ok=True)
        with tempfile.TemporaryDirectory(prefix="built-mir-identity-", dir=scratch) as temp:
            root = Path(temp).resolve()
            source = root / "fixture.rs"
            dump_dir = root / "dump"
            dump_dir.mkdir()
            source.write_text(
                "#![allow(dead_code)]\n"
                "mod nested { #[derive(Clone, Eq, Ord, PartialEq, PartialOrd)] "
                "pub struct Foo { field: String } }\n", encoding="utf-8")
            environment = os.environ.copy()
            environment["RUSTC_BOOTSTRAP"] = "1"
            command = ["rustc", "--crate-name", "body_owner_fixture", "--crate-type", "lib",
                       "--edition=2021", "--emit=mir", "-Zdump-mir=built",
                       f"-Zdump-mir-dir={dump_dir}", "-Zmir-include-spans=yes",
                       str(source), "-o", str(root / "fixture.mir")]
            subprocess.run(command, check=True, cwd=ROOT, env=environment,
                           stdout=subprocess.PIPE, stderr=subprocess.PIPE)
            thir = subprocess.run(["rustc", "--crate-name", "body_owner_fixture",
                                   "--crate-type", "lib", "--edition=2021",
                                   "-Zunpretty=thir-tree", str(source)], check=True,
                                  cwd=ROOT, env=environment, stdout=subprocess.PIPE,
                                  stderr=subprocess.PIPE).stdout.decode("utf-8")
            thir_path = root / "fixture.thir"
            thir_path.write_text(thir, encoding="utf-8")
            files = sorted(dump_dir.rglob("*.built.after.mir"),
                           key=lambda path: path.relative_to(ROOT).as_posix())
            entries = [{"path": path.relative_to(ROOT).as_posix(),
                        "sha256": hashlib.sha256(path.read_bytes()).hexdigest()}
                       for path in files]
            manifest_path = root / "built-mir-manifest.json"
            manifest_path.write_text(json.dumps({
                "schema": "phase7-built-mir-manifest.v1",
                "directory": dump_dir.relative_to(ROOT).as_posix(),
                "files": entries,
            }), encoding="utf-8")
            index, _, errors, unindexed = load_built_mir_manifest(manifest_path)
            self.assertEqual([], errors)
            with thir_path.open(encoding="utf-8") as thir_stream:
                rows = [row for row in thir_call_sites(thir_stream)
                        if row["callee_span"]["line"] == 2]
            targets_by_method = {
                "clone": "core[", "cmp": "core[", "eq": "core[",
                "partial_cmp": "core[",
            }
            rows = [row for row in rows if row["caller_owner_identity"] and
                    row["caller_owner_identity"].rsplit("::", 1)[-1] in targets_by_method]
            self.assertEqual(4, len(rows))
            self.assertEqual(1, len({(row["callee_span"]["path"],
                                     row["callee_span"]["line"],
                                     row["callee_span"]["column"],
                                     row["callee_span"]["end_column"]) for row in rows}))
            thir_by_span = {}
            for row in rows:
                for span_name in ("callee_span", "expr_span"):
                    span = row.get(span_name)
                    if span:
                        key = tuple(span.get(name) for name in
                                    ("path", "line", "column", "end_line", "end_column"))
                        thir_by_span.setdefault(key, []).append(row)
            with (root / "fixture.mir").open(encoding="utf-8") as mir_stream:
                mir_rows = list(mir_calls(mir_stream, index))
            expected_owners = {row["caller_owner_identity"] for row in rows}
            matching_mir_calls = [call for call in mir_rows
                                  if call.get("caller_owner_identity") in expected_owners
                                  and call.get("source_line") == 2]
            self.assertEqual(4, len(matching_mir_calls))
            for mir_call in matching_mir_calls:
                owner = mir_call["caller_owner_identity"]
                body = next(body for body in index.values() if body["identity"] == owner)
                self.assertEqual(body["header"], mir_call["caller_body_header"])
                self.assertEqual(body["sha256"], mir_call["caller_body_artifact_sha256"])
                selected = thir_calls_for_mir_call(mir_call, thir_by_span)
                self.assertEqual(1, len(selected))
                self.assertEqual(owner, selected[0]["caller_owner_identity"])
                ambiguous = thir_calls_for_mir_call(
                    {key: value for key, value in mir_call.items()
                     if key != "caller_owner_identity"}, thir_by_span)
                self.assertEqual(4, len(ambiguous))

            changed = files[0].read_bytes()
            files[0].write_bytes(changed + b"\\n")
            changed_index, _, changed_errors, _ = load_built_mir_manifest(manifest_path)
            self.assertEqual({}, changed_index)
            self.assertTrue(any("hash does not match" in error for error in changed_errors))
            files[0].write_bytes(changed)
            files[0].unlink()
            missing_index, _, missing_errors, _ = load_built_mir_manifest(manifest_path)
            self.assertEqual({}, missing_index)
            self.assertTrue(any("file set differs" in error for error in missing_errors))
            files[0].write_bytes(changed)

            wrong_owner_call = dict(mir_call, caller_owner_identity="body_owner_fixture::wrong")
            self.assertEqual([], thir_calls_for_mir_call(wrong_owner_call, thir_by_span))

            duplicate = dump_dir / "body_owner_fixture.nested-{impl#9}-clone.1-1-000.built.after.mir"
            clone_file = next(path for path in files if "{impl#0}-clone" in path.name)
            wrong_path = dump_dir / "body_owner_fixture.wrong-{impl#0}-clone.1-1-000.built.after.mir"
            wrong_path.write_bytes(clone_file.read_bytes())
            wrong_rows = sorted(entries + [{"path": wrong_path.relative_to(ROOT).as_posix(),
                                              "sha256": hashlib.sha256(wrong_path.read_bytes()).hexdigest()}],
                                 key=lambda row: row["path"])
            manifest_path.write_text(json.dumps({
                "schema": "phase7-built-mir-manifest.v1",
                "directory": dump_dir.relative_to(ROOT).as_posix(),
                "files": wrong_rows,
            }), encoding="utf-8")
            wrong_index, _, wrong_errors, _ = load_built_mir_manifest(manifest_path)
            self.assertEqual({}, wrong_index)
            self.assertTrue(any("filename and full body header disagree" in error
                                for error in wrong_errors))
            wrong_path.unlink()

            duplicate.write_bytes(clone_file.read_bytes())
            duplicate_rows = sorted(entries + [{"path": duplicate.relative_to(ROOT).as_posix(),
                                                  "sha256": hashlib.sha256(duplicate.read_bytes()).hexdigest()}],
                                    key=lambda row: row["path"])
            manifest_path.write_text(json.dumps({
                "schema": "phase7-built-mir-manifest.v1",
                "directory": dump_dir.relative_to(ROOT).as_posix(),
                "files": duplicate_rows,
            }), encoding="utf-8")
            ambiguous_index, _, ambiguous_errors, _ = load_built_mir_manifest(manifest_path)
            self.assertEqual({}, ambiguous_index)
            self.assertTrue(any("ambiguous built MIR full body header" in error
                                for error in ambiguous_errors))

    def test_built_mir_manifest_keeps_unsupported_bodies_unindexed(self):
        scratch = Path(".verification-tmp/phase7")
        scratch.mkdir(parents=True, exist_ok=True)
        with tempfile.TemporaryDirectory(prefix="built-mir-unindexed-", dir=scratch) as temp:
            root = Path(temp).resolve()
            dump_dir = root / "dump"
            dump_dir.mkdir()
            supported = dump_dir / "fixture.{impl#0}-clone.1-1-000.built.after.mir"
            supported.write_text(
                "// MIR for `<impl at src/z.rs:1:1: 1:8>::clone` after built\n\n"
                "fn <impl at src/z.rs:1:1: 1:8>::clone(_1: &Foo) -> Foo {\n}\n",
                encoding="utf-8")
            free_function = dump_dir / "fixture.run.1-1-000.built.after.mir"
            free_function.write_text("fn run() -> () {\n}\n", encoding="utf-8")
            rows = [{"path": path.relative_to(root).as_posix(),
                     "sha256": hashlib.sha256(path.read_bytes()).hexdigest()}
                    for path in sorted(dump_dir.glob("*.built.after.mir"), key=lambda p: p.name)]
            manifest_path = root / "manifest.json"
            manifest_path.write_text(json.dumps({
                "schema": "phase7-built-mir-manifest.v1",
                "directory": "dump", "files": rows,
            }), encoding="utf-8")
            with mock.patch("check_phase7_compiler_calls.ROOT", root):
                index, files, errors, unindexed = load_built_mir_manifest(manifest_path)
            self.assertEqual([], errors)
            self.assertEqual(1, len(index))
            self.assertEqual(2, len(files))
            self.assertEqual(["dump/fixture.run.1-1-000.built.after.mir"],
                             [row["path"] for row in unindexed])

            malformed_supported = dump_dir / "fixture.{impl#1}-clone.1-1-000.built.after.mir"
            malformed_supported.write_text("fn unrelated() {}\n", encoding="utf-8")
            rows.append({"path": malformed_supported.relative_to(root).as_posix(),
                         "sha256": hashlib.sha256(malformed_supported.read_bytes()).hexdigest()})
            rows.sort(key=lambda row: row["path"])
            manifest_path.write_text(json.dumps({
                "schema": "phase7-built-mir-manifest.v1",
                "directory": "dump", "files": rows,
            }), encoding="utf-8")
            with mock.patch("check_phase7_compiler_calls.ROOT", root):
                malformed_index, _, malformed_errors, _ = load_built_mir_manifest(manifest_path)
            self.assertEqual({}, malformed_index)
            self.assertTrue(any("lacks one exact full body header" in error
                                for error in malformed_errors))

    def test_argument_drop_types_come_from_current_function_signature(self):
        rows = list(mir_calls([
            'fn consume(_1: std::option::Option<std::string::String>, _2: fn(u8) -> u8) -> () {\n',
            '    drop(_1) -> [return: bb1, unwind continue]; // scope 0 at src/runtime/run.rs:20:1: 20:8\n',
            '}\n',
            'fn other(_1: &owner::Owner) -> () {\n',
            '    drop(_1) -> [return: bb1, unwind continue]; // scope 0 at src/runtime/run.rs:21:1: 21:8\n',
            '}\n',
            'fn no_arguments() -> () {\n',
            '    drop(_1) -> [return: bb1, unwind continue]; // scope 0 at src/runtime/run.rs:22:1: 22:8\n',
            '}\n',
        ]))
        self.assertEqual('std::option::Option<std::string::String>', rows[0]['drop_type'])
        self.assertEqual('&owner::Owner', rows[1]['drop_type'])
        self.assertIsNone(rows[2]['drop_type'])

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

    def test_constant_count_excludes_destructors_and_indirect_terminators(self):
        doc, mir, mapping, dag = self.fixture("src/owner/api.rs")
        mir.extend([
            "    drop(_6) -> [return: bb1, unwind continue]; // scope 0 at src/runtime/run.rs:21:1: 21:8\n",
            "}\n",
            "    _0 = move _7() -> [return: bb1, unwind continue]; // scope 0 at src/runtime/run.rs:22:1: 22:8\n",
            "}\n",
        ])
        report = analyze(doc, mir, mapping, dag)
        self.assertEqual(1, report["constant_calls"])
        self.assertEqual(3, report["mir_call_and_drop_instances"])
        self.assertEqual(1, report["drop_analysis"]["raw_mir_drop_terminators"])

    def test_mir_relative_parent_path_maps_to_component(self):
        doc, mir, mapping, dag = self.fixture(source="src/runtime/sub/../run.rs")
        report = analyze(doc, mir, mapping, dag)
        self.assertEqual(1, len(report["violations"]))
        self.assertEqual("runtime", report["violations"][0]["source_component"])
        self.assertEqual([], report["unknown_calls"])

    def test_rustdoc_unit_shape_is_empty_but_unknown_plain_shape_stays_open(self):
        doc, _, mapping, dag = self.fixture()
        doc["index"]["1"] = {"crate_id": 0, "name": "Owner",
            "span": {"filename": "src/owner/api.rs", "begin": [1, 1], "end": [2, 1]},
            "inner": {"struct": {"kind": "unit", "generics": {"params": []}}}}
        mir = ["fn run() {\n", "    let _6: owner::Owner; // in scope 0\n",
               "    drop(_6) -> [return: bb1, unwind continue]; // scope 0 at src/runtime/run.rs:20:1: 20:8\n",
               "}\n"]
        report = analyze(doc, mir, mapping, dag)
        self.assertEqual([], report["unknown_calls"])
        self.assertEqual(1, report["drop_analysis"]["fully_resolved_drop_terminators"])

        doc["index"]["1"]["inner"]["struct"]["kind"] = {"plain": "unsupported"}
        unresolved = analyze(doc, mir, mapping, dag)
        self.assertEqual(1, len(unresolved["unknown_calls"]))
        self.assertEqual(0, unresolved["drop_analysis"]["fully_resolved_drop_terminators"])

    def test_rustdoc_plain_enum_variant_is_empty(self):
        doc, _, mapping, dag = self.fixture()
        doc["index"]["1"] = {"crate_id": 0, "name": "Owner", "span": {
            "filename": "src/owner/api.rs", "begin": [1, 1], "end": [2, 1]},
            "inner": {"enum": {"variants": ["2"], "generics": {"params": []}}}}
        doc["index"]["2"] = {"crate_id": 0, "name": "Empty", "inner": {
            "variant": {"kind": "plain"}}}
        mir = ["fn run() {\n", "    let _6: owner::Owner; // in scope 0\n",
               "    drop(_6) -> [return: bb1, unwind continue]; // scope 0 at src/runtime/run.rs:20:1: 20:8\n",
               "}\n"]
        report = analyze(doc, mir, mapping, dag)
        self.assertEqual([], report["unknown_calls"])
        self.assertEqual(1, report["drop_analysis"]["fully_resolved_drop_terminators"])

    def test_local_type_alias_expands_to_exact_drop_target(self):
        doc, _, mapping, dag = self.fixture()
        doc["paths"].update({
            "1": {"crate_id": 0, "path": ["reliquary_memory", "owner", "Owner"]},
            "2": {"crate_id": 0, "path": ["reliquary_memory", "owner", "Resource"]},
            "3": {"crate_id": 0, "path": ["reliquary_memory", "owner", "ResourceAlias"]},
            "4": {"crate_id": 1, "path": ["core", "ops", "Drop"]},
        })
        doc["index"]["1"] = {"crate_id": 0, "name": "Owner", "span": {
            "filename": "src/owner/api.rs", "begin": [1, 1], "end": [3, 1]},
            "inner": {"struct": {"kind": {"plain": {"fields": [5]}},
                                  "generics": {"params": []}}}}
        doc["index"]["5"] = {"crate_id": 0, "name": "resource", "inner": {
            "struct_field": {"resolved_path": {"id": "3"}}}}
        doc["index"]["3"] = {"crate_id": 0, "name": "ResourceAlias", "inner": {
            "type_alias": {"type": {"resolved_path": {"id": "2"}},
                           "generics": {"params": []}}}}
        doc["index"]["2"] = {"crate_id": 0, "name": "Resource", "span": {
            "filename": "src/owner/api.rs", "begin": [5, 1], "end": [7, 1]},
            "inner": {"struct": {"kind": "unit", "generics": {"params": []}}}}
        doc["index"]["6"] = {"crate_id": 0, "name": None, "inner": {"impl": {
            "for": {"resolved_path": {"id": "2"}}, "trait": {"id": "4"},
            "items": ["7"]}}}
        doc["index"]["7"] = {"crate_id": 0, "name": "drop", "span": {
            "filename": "src/owner/api.rs", "begin": [9, 1], "end": [10, 1]},
            "inner": {"function": {}}}
        mir = ["fn run() {\n", "    let _6: owner::Owner; // in scope 0\n",
               "    drop(_6) -> [return: bb1, unwind continue]; // scope 0 at src/runtime/run.rs:20:1: 20:8\n",
               "}\n"]
        report = analyze(doc, mir, mapping, dag)
        self.assertEqual([], report["unknown_calls"])
        witnesses = report["drop_analysis"]["direct_drop_witnesses"]
        self.assertTrue(any(definition.get("type_path") == "owner::Resource"
                            for row in witnesses
                            for definition in row.get("target_definitions", [])))

    def test_type_alias_cycle_stays_unknown(self):
        doc, _, mapping, dag = self.fixture()
        doc["paths"].update({
            "1": {"crate_id": 0, "path": ["reliquary_memory", "owner", "Owner"]},
            "2": {"crate_id": 0, "path": ["reliquary_memory", "owner", "AliasA"]},
            "3": {"crate_id": 0, "path": ["reliquary_memory", "owner", "AliasB"]},
        })
        doc["index"]["1"] = {"crate_id": 0, "name": "Owner", "span": {
            "filename": "src/owner/api.rs", "begin": [1, 1], "end": [2, 1]},
            "inner": {"type_alias": {"type": {"resolved_path": {"id": "2"}},
                                      "generics": {"params": []}}}}
        doc["index"]["2"] = {"crate_id": 0, "name": "AliasA", "inner": {
            "type_alias": {"type": {"resolved_path": {"id": "3"}},
                           "generics": {"params": []}}}}
        doc["index"]["3"] = {"crate_id": 0, "name": "AliasB", "inner": {
            "type_alias": {"type": {"resolved_path": {"id": "2"}},
                           "generics": {"params": []}}}}
        mir = ["fn run() {\n", "    let _6: owner::Owner; // in scope 0\n",
               "    drop(_6) -> [return: bb1, unwind continue]; // scope 0 at src/runtime/run.rs:20:1: 20:8\n",
               "}\n"]
        report = analyze(doc, mir, mapping, dag)
        self.assertEqual(1, len(report["unknown_calls"]))
        self.assertEqual(0, report["drop_analysis"]["fully_resolved_drop_terminators"])

    def test_unbound_generic_type_alias_stays_unknown(self):
        doc, _, mapping, dag = self.fixture()
        doc["paths"].update({
            "1": {"crate_id": 0, "path": ["reliquary_memory", "owner", "Alias"]},
        })
        doc["index"]["1"] = {"crate_id": 0, "name": "Alias", "span": {
            "filename": "src/owner/api.rs", "begin": [1, 1], "end": [2, 1]},
            "inner": {"type_alias": {"type": {"generic": "T"}, "generics": {
                "params": [{"name": "T", "kind": {"type": {}}}]}}}}
        mir = ["fn run() {\n", "    let _6: owner::Alias; // in scope 0\n",
               "    drop(_6) -> [return: bb1, unwind continue]; // scope 0 at src/runtime/run.rs:20:1: 20:8\n",
               "}\n"]
        report = analyze(doc, mir, mapping, dag)
        self.assertEqual(1, len(report["unknown_calls"]))
        self.assertEqual(0, report["drop_analysis"]["fully_resolved_drop_terminators"])

    def test_enum_payload_keeps_local_drop_target(self):
        doc, _, mapping, dag = self.fixture()
        doc["paths"].update({
            "1": {"crate_id": 0, "path": ["reliquary_memory", "owner", "Owner"]},
            "2": {"crate_id": 0, "path": ["reliquary_memory", "owner", "Resource"]},
            "3": {"crate_id": 1, "path": ["core", "ops", "Drop"]},
        })
        doc["index"]["1"] = {"crate_id": 0, "name": "Owner", "span": {
            "filename": "src/owner/api.rs", "begin": [1, 1], "end": [3, 1]},
            "inner": {"enum": {"variants": ["4"], "generics": {"params": []}}}}
        doc["index"]["4"] = {"crate_id": 0, "name": "HasResource", "inner": {
            "variant": {"kind": {"tuple": ["5"]}}}}
        doc["index"]["5"] = {"crate_id": 0, "name": "0", "inner": {
            "struct_field": {"resolved_path": {"id": "2"}}}}
        doc["index"]["2"] = {"crate_id": 0, "name": "Resource", "span": {
            "filename": "src/owner/api.rs", "begin": [5, 1], "end": [7, 1]},
            "inner": {"struct": {"kind": "unit", "generics": {"params": []}}}}
        doc["index"]["6"] = {"crate_id": 0, "name": None, "inner": {"impl": {
            "for": {"resolved_path": {"id": "2"}}, "trait": {"id": "3"},
            "items": ["7"]}}}
        doc["index"]["7"] = {"crate_id": 0, "name": "drop", "span": {
            "filename": "src/owner/api.rs", "begin": [9, 1], "end": [10, 1]},
            "inner": {"function": {}}}
        mir = ["fn run() {\n", "    let _6: owner::Owner; // in scope 0\n",
               "    drop(_6) -> [return: bb1, unwind continue]; // scope 0 at src/runtime/run.rs:20:1: 20:8\n",
               "}\n"]
        report = analyze(doc, mir, mapping, dag)
        self.assertEqual([], report["unknown_calls"])
        self.assertTrue(any(definition.get("type_path") == "owner::Resource"
                            for row in report["drop_analysis"]["direct_drop_witnesses"]
                            for definition in row.get("target_definitions", [])))

    def test_missing_local_target_fails_closed(self):
        report = analyze(*self.fixture(call="owner::Owner::missing"))
        self.assertEqual(1, len(report["unknown_calls"]))

    def test_unresolved_trait_call_does_not_fall_back_to_inherent_method(self):
        report = analyze(*self.fixture(call="<owner::Owner as core::fmt::Debug>::read"))
        self.assertEqual([], report["violations"])
        self.assertEqual(1, len(report["unknown_calls"]))
        self.assertEqual([], report["cross_component_calls"])

    def test_exact_external_thir_for_builtin_receiver_uses_orphan_rule(self):
        self.assertTrue(primitive_foreign_receiver("<str as PartialEq>::eq"))
        self.assertTrue(primitive_foreign_receiver("<&'{erased} [u8] as Into<Vec<u8>>>::into"))
        self.assertTrue(primitive_foreign_receiver("<[u16; 8] as Into<Vec<u16>>>::into"))
        self.assertFalse(primitive_foreign_receiver("<Owner as core::fmt::Debug>::fmt"))
        self.assertFalse(primitive_foreign_receiver("<Box<Local> as core::fmt::Debug>::fmt"))
        self.assertFalse(primitive_foreign_receiver("<__A as MapAccess>::next_key"))

        doc, mir, mapping, dag = self.fixture(call="<str as PartialEq>::eq")
        doc["external_crates"]["2"] = {"name": "core"}
        call_line = "_0 = <str as PartialEq>::eq(copy _1) -> [return: bb1, unwind continue]; // scope 0 at src/runtime/run.rs:20:1: 20:30\n"
        mir = [call_line,
               " // + const_: Const { ty: for<Region(BrNamed(DefId(2:700 ~ core[abcd]::cmp::PartialEq::eq::'_)))> fn(&str, &str) -> bool {<str as PartialEq>::eq}, val: Value(ConstValue(ZeroSized: fn(&str, &str) -> bool {<str as PartialEq>::eq})) }\n"]
        external_def = "DefId(2:700 ~ core[abcd]::cmp::PartialEq::eq)"
        thir = [{"target_def_id": external_def,
                 "callee_span": {"path": "src/runtime/run.rs", "line": 20,
                                 "column": 1, "end_line": 20, "end_column": 30}}]
        report = analyze(doc, mir, mapping, dag, thir_calls=thir)
        self.assertEqual(1, report["external_constant_calls"])
        self.assertEqual([], report["unknown_calls"])
        self.assertEqual(
            "exact-external-thir-defid-builtin-receiver-orphan-rule",
            report["external_resolution_witnesses"][0]["resolved_by"])

        # The DefId in this MIR function type is nested under BrNamed and may
        # identify a bound region, not the Const callee. With no exact THIR
        # FnDef span, keep the external-trait call unresolved.
        report = analyze(doc, mir, mapping, dag, thir_calls=[])
        self.assertEqual(0, report["external_constant_calls"])
        self.assertEqual(1, len(report["unknown_calls"]))
        self.assertEqual("unresolved local compiler target",
                         report["unknown_calls"][0]["reason"])

    def test_external_thir_trait_on_local_or_generic_receiver_stays_unknown(self):
        external_def = "DefId(2:700 ~ core[abcd]::cmp::PartialEq::eq)"
        for target in ("<owner::Owner as PartialEq>::eq", "<__A as MapAccess>::next_key"):
            doc, mir, mapping, dag = self.fixture(call=target)
            doc["external_crates"]["2"] = {"name": "core"}
            thir = [{"target_def_id": external_def,
                     "callee_span": {"path": "src/runtime/run.rs", "line": 20,
                                     "column": 1, "end_line": 20, "end_column": 30}}]
            report = analyze(doc, mir, mapping, dag, thir_calls=thir)
            self.assertEqual(1, len(report["unknown_calls"]))

    def test_external_trait_receiver_root_does_not_hide_local_or_generic_types(self):
        core_def = "DefId(2:700 ~ core[abcd]::fmt::Debug::fmt)"
        targets = (
            "<std::boxed::Box<Local> as core::fmt::Debug>::fmt",
            "<&Local as core::fmt::Debug>::fmt",
            "<&[Local] as core::fmt::Debug>::fmt",
            "<[Local] as core::fmt::Debug>::fmt",
            "<std::vec::Vec<Local> as core::fmt::Debug>::fmt",
            "<std::vec::Vec<T> as core::fmt::Debug>::fmt",
        )
        self.assertFalse(concrete_external_trait_receiver(
            "<std::boxed::Box<Local> as core::fmt::Debug>::fmt", {}, {"std", "core"}))
        self.assertFalse(concrete_external_trait_receiver(
            "<&Local as core::fmt::Debug>::fmt", {}, {"std", "core"}))
        self.assertFalse(concrete_external_trait_receiver(
            "<std::vec::Vec<Local> as core::fmt::Debug>::fmt", {}, {"std", "core"}))
        self.assertTrue(concrete_external_trait_receiver(
            "<std::vec::Vec<u8> as core::fmt::Debug>::fmt", {}, {"std", "core"}))
        for target in targets:
            with self.subTest(target=target):
                doc, mir, mapping, dag = self.fixture(call=target)
                doc["external_crates"]["2"] = {"name": "core"}
                doc["external_crates"]["3"] = {"name": "std"}
                thir = [{"target_def_id": core_def,
                         "callee_span": {"path": "src/runtime/run.rs", "line": 20,
                                         "column": 1, "end_line": 20, "end_column": 30}}]
                report = analyze(doc, mir, mapping, dag, thir_calls=thir)
                self.assertEqual(0, report["external_constant_calls"])
                self.assertEqual(1, len(report["unknown_calls"]))
                # Neither the rendered crate root nor its MIR-associated
                # DefId hint substitutes for an exact THIR FnDef identity.
                report_without_thir = analyze(doc, mir, mapping, dag, thir_calls=[])
                self.assertEqual(0, report_without_thir["external_constant_calls"])
                self.assertEqual(1, len(report_without_thir["unknown_calls"]))

    def test_concrete_external_generic_receiver_can_use_exact_external_thir(self):
        target = "<std::vec::Vec<u8> as core::fmt::Debug>::fmt"
        doc, mir, mapping, dag = self.fixture(call=target)
        doc["external_crates"]["2"] = {"name": "core"}
        doc["external_crates"]["3"] = {"name": "std"}
        thir = [{"target_def_id": "DefId(2:700 ~ core[abcd]::fmt::Debug::fmt)",
                 "callee_span": {"path": "src/runtime/run.rs", "line": 20,
                                 "column": 1, "end_line": 20, "end_column": 30}}]
        report = analyze(doc, mir, mapping, dag, thir_calls=thir)
        self.assertEqual(1, report["external_constant_calls"])
        self.assertEqual([], report["unknown_calls"])

    def test_local_or_unresolved_external_trait_type_arguments_stay_unknown(self):
        targets = (
            "<u8 as foreign::ForeignTrait<Local>>::call",
            "<std::vec::Vec<u8> as foreign::ForeignTrait<Local>>::call",
            "<u8 as foreign::ForeignTrait<&Local>>::call",
            "<u8 as foreign::ForeignTrait<T>>::call",
        )
        self.assertFalse(external_trait_arguments_are_closed(
            "<u8 as foreign::ForeignTrait<Local>>::call", {}, {"foreign"}))
        self.assertTrue(external_trait_arguments_are_closed(
            "<u8 as foreign::ForeignTrait<u16>>::call", {}, {"foreign"}))
        foreign_def = "DefId(2:700 ~ foreign[abcd]::traits::ForeignTrait::call)"
        for target in targets:
            with self.subTest(target=target):
                doc, mir, mapping, dag = self.fixture(call=target)
                doc["external_crates"]["2"] = {"name": "foreign"}
                doc["external_crates"]["3"] = {"name": "std"}
                thir = [{"target_def_id": foreign_def,
                         "callee_span": {"path": "src/runtime/run.rs", "line": 20,
                                         "column": 1, "end_line": 20, "end_column": 30}}]
                report = analyze(doc, mir, mapping, dag, thir_calls=thir)
                self.assertEqual(0, report["external_constant_calls"])
                self.assertEqual(1, len(report["unknown_calls"]))

        doc, mir, mapping, dag = self.fixture(
            call="<u8 as foreign::ForeignTrait<u16>>::call")
        doc["external_crates"]["2"] = {"name": "foreign"}
        thir = [{"target_def_id": foreign_def,
                 "callee_span": {"path": "src/runtime/run.rs", "line": 20,
                                 "column": 1, "end_line": 20, "end_column": 30}}]
        report = analyze(doc, mir, mapping, dag, thir_calls=thir)
        self.assertEqual(1, report["external_constant_calls"])
        self.assertEqual([], report["unknown_calls"])

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

    def test_external_trait_callee_with_unresolved_closure_type_stays_unknown(self):
        target = ("<std::iter::Map<Iter, {owner::run::{closure#0} closure_kind_ty=i16 "
                  "closure_sig_as_fn_ptr_ty=fn() -> () upvar_tys=()}> as Iterator>::collect")
        doc, mir, mapping, dag = self.fixture(call=target)
        doc["external_crates"]["2"] = {"name": "core"}
        mir[1] = (" // + const_: Const { ty: for<Region(BrNamed(DefId(2:700 ~ "
                  f"core[abcd]::iter::traits::iterator::Iterator::collect)))> fn() {{{target}}}, "
                  f"val: Value(ConstValue(ZeroSized: fn() {{{target}}})) }}\n")
        report = analyze(doc, mir, mapping, dag)
        self.assertEqual(0, report["external_constant_calls"])
        self.assertEqual(1, len(report["unknown_calls"]))

    def test_local_closure_defid_stays_unknown_without_exact_hir_owner(self):
        doc, mir, mapping, dag = self.fixture(call="owner::Owner::{closure#0}")
        mir[1] = (" // + const_: Const { ty: for<Region(BrNamed(DefId(0:77 ~ "
                  "reliquary_memory[abcd]::owner::Owner::{closure#0})))> "
                  "fn() {owner::Owner::{closure#0}}, "
                  "val: Value(ConstValue(ZeroSized: fn() {owner::Owner::{closure#0}})) }\n")
        report = analyze(doc, mir, mapping, dag)
        self.assertEqual(0, report["external_constant_calls"])
        self.assertEqual(1, len(report["unknown_calls"]))
        self.assertEqual("unresolved compiler closure target", report["unknown_calls"][0]["reason"])

    def test_local_closure_mir_hint_does_not_resolve_with_hir_alone(self):
        doc, mir, mapping, dag = self.fixture(call="owner::Owner::{closure#0}")
        def_id = "DefId(0:77 ~ reliquary_memory[abcd]::owner::Owner::{closure#0})"
        mir[1] = (" // + const_: Const { ty: for<Region(BrNamed(DefId(0:77 ~ "
                  "reliquary_memory[abcd]::owner::Owner::{closure#0})))> "
                  "fn() {owner::Owner::{closure#0}}, "
                  "val: Value(ConstValue(ZeroSized: fn() {owner::Owner::{closure#0}})) }\n")
        hir = [f"{def_id} => OwnerNodes {{\n", "    node: ParentedNode {\n",
               f"                owner_id: {def_id},\n",
               "                span: src/owner/api.rs:15:1: 16:1 (#0),\n", "}\n"]
        report = analyze(doc, mir, mapping, dag, hir_owner_spans(hir))
        self.assertEqual(0, report["external_constant_calls"])
        self.assertEqual([], report["cross_component_calls"])
        self.assertEqual(1, len(report["unknown_calls"]))

    def test_nested_hir_closure_defid_resolves_direct_mir_closure_call(self):
        doc, mir, mapping, dag = self.fixture(call="owner::Owner::{closure#0}")
        def_id = ("DefId(0:77 ~ reliquary_memory[abcd]::owner::Owner::"
                  "{closure#0})")
        mir[1] = (" // + const_: Const { ty: for<Region(BrNamed(" + def_id +
                   "))> fn() {owner::Owner::{closure#0}}, val: "
                   "Value(ConstValue(ZeroSized: fn() {owner::Owner::{closure#0}})) }\n")
        hir = ["kind: Closure(\n", "    Closure {\n", f"        def_id: {def_id},\n",
               "        fn_decl_span: src/owner/api.rs:15:21: 15:23 (#0),\n",
               "    },\n"]
        spans = hir_owner_spans(hir)
        self.assertEqual([{"path": "src/owner/api.rs", "line": 15, "end_line": 15,
                           "column": 21, "end_column": 23}], spans[def_id])
        thir = [{"target_def_id": def_id,
                 "callee_span": {"path": "src/runtime/run.rs", "line": 20,
                                 "column": 1, "end_line": 20, "end_column": 30}}]
        report = analyze(doc, mir, mapping, dag, spans, thir)
        self.assertEqual([], report["unknown_calls"])
        self.assertEqual("owner", report["cross_component_calls"][0]["target_component"])

    def test_duplicate_nested_hir_closure_spans_remain_ambiguous(self):
        def_id = "DefId(0:77 ~ reliquary_memory[abcd]::owner::{closure#0})"
        hir = []
        for path in ("src/owner/one.rs", "src/facade/two.rs"):
            hir.extend(["kind: Closure(\n", "    Closure {\n", f"        def_id: {def_id},\n",
                        f"        fn_decl_span: {path}:15:1: 15:4 (#0),\n", "    },\n"])
        spans = hir_owner_spans(hir)
        self.assertEqual(2, len(spans[def_id]))


    def test_external_target_separately_counted(self):
        report = analyze(*self.fixture(call="std::mem::drop"))
        self.assertEqual(1, report["external_constant_calls"])
        self.assertEqual([], report["unknown_calls"])

    def test_external_target_without_local_span_is_not_material_unknown(self):
        report = analyze(*self.fixture(call="std::fmt::format", source="/rustc/library/alloc/src/macros.rs"))
        self.assertEqual(1, report["external_constant_calls"])
        self.assertEqual(1, report["external_calls_without_mapped_caller"])
        self.assertEqual([], report["unknown_calls"])

    def test_external_impl_short_name_mir_defpath_hint_stays_unknown(self):
        doc, mir, mapping, dag = self.fixture(call="Vec::push")
        doc["external_crates"]["2"] = {"name": "alloc"}
        mir[1] = (" // + const_: Const { ty: for<Region(BrNamed(DefId(3:42 ~ "
                  "alloc[abcd]::vec::{impl#4}::push::'_)))> fn(&mut Vec<u8>, u8) "
                  "{Vec::<u8>::push}, val: Value(ConstValue(ZeroSized: "
                  "fn(&mut Vec<u8>, u8) {Vec::<u8>::push})) }\n")
        report = analyze(doc, mir, mapping, dag)
        self.assertEqual(0, report["external_constant_calls"])
        self.assertEqual(1, len(report["unknown_calls"]))

    def test_exact_external_trait_method_defid_classifies_callee(self):
        doc, mir, mapping, dag = self.fixture(call="<owner::Owner as Deref>::deref")
        doc["external_crates"]["2"] = {"name": "core"}
        mir[1] = (" // + const_: Const { ty: for<Region(BrNamed(DefId(2:627 ~ "
                  "core[abcd]::ops::Deref::deref)))> fn(&Owner) -> &Target "
                  "{<Owner as Deref>::deref}, val: Value(ConstValue(ZeroSized: "
                  "fn(&Owner) -> &Target {<Owner as Deref>::deref})) }\n")
        report = analyze(doc, mir, mapping, dag)
        self.assertEqual(0, report["external_constant_calls"])
        self.assertEqual(1, len(report["unknown_calls"]))

    def test_thir_fn_def_and_exact_span_resolve_local_call(self):
        doc, mir, mapping, dag = self.fixture(call="Owner::read")
        # MIR carries the full call expression; THIR also exposes the narrower
        # callee token span and the enclosing Expr span.
        mir[0] = " _0 = Owner::read(copy _1) -> [return: bb1, unwind continue]; // scope 0 at src/runtime/run.rs:20:5: 20:15\n"
        mir[1] = " // + const_: Const { ty: fn(&Owner) -> u8 {Owner::read}, val: Value(Owner::read) }\n"
        def_id = "DefId(0:7 ~ reliquary_memory[abcd]::owner::Owner::read)"
        thir = ["DefId(0:8 ~ reliquary_memory[abcd]::runtime::run):\n",
                "                                                                Expr {\n",
                "                                                                    span: src/runtime/run.rs:20:5: 20:15 (#0)\n",
                "                                                                        Call {\n",
                f"                                                                            ty: FnDef({def_id}, [])\n",
                "                                                                            fn_span: src/runtime/run.rs:20:8: 20:14 (#0)\n",
                "                                                                        }\n"]
        hir = [f"{def_id} => OwnerNodes {{\n", "    node: ParentedNode {\n",
               f"                owner_id: {def_id},\n",
               "                span: src/owner/api.rs:10:1: 12:1 (#0),\n", "}\n"]
        rows = thir_call_sites(thir)
        self.assertEqual(1, len(rows))
        report = analyze(doc, mir, mapping, dag, hir_owner_spans(hir), rows)
        self.assertEqual(1, len(report["cross_component_calls"]))
        self.assertEqual("exact-thir-defid-hir-owner-span", report["cross_component_calls"][0]["resolved_by"])

    def test_thir_span_mismatch_and_ambiguous_calls_remain_unknown(self):
        doc, mir, mapping, dag = self.fixture(call="Owner::read")
        mir[0] = " _0 = Owner::read(copy _1) -> [return: bb1, unwind continue]; // scope 0 at src/runtime/run.rs:20:5: 20:15\n"
        mir[1] = " // + const_: Const { ty: fn(&Owner) -> u8 {Owner::read}, val: Value(Owner::read) }\n"
        def_id = "DefId(0:7 ~ reliquary_memory[abcd]::owner::Owner::read)"
        thir = [{"callee_span": {"path": "src/runtime/run.rs", "line": 20, "column": 6,
                                 "end_line": 20, "end_column": 15}, "target_def_id": def_id}]
        report = analyze(doc, mir, mapping, dag, {}, thir)
        self.assertEqual(1, len(report["unknown_calls"]))

    def test_bound_region_mir_defid_does_not_disambiguate_overlapping_thir_calls(self):
        doc, mir, mapping, dag = self.fixture(call="owner::Owner::read")
        target = "DefId(0:7 ~ reliquary_memory[abcd]::owner::Owner::read)"
        other = "DefId(2:8 ~ core[abcd]::fmt::Debug::fmt)"
        mir[0] = " _0 = owner::Owner::read(copy _1) -> [return: bb1, unwind continue]; // scope 0 at src/runtime/run.rs:20:5: 20:15\n"
        mir[1] = (" // + const_: Const { ty: fn(&Owner) -> u8 {owner::Owner::read}, "
                  "val: Value(ConstValue(ZeroSized: for<Region(BrNamed(" + target +
                  "::'_)))> fn(&Owner) -> u8 {owner::Owner::read})) }\n")
        hir = [f"{target} => OwnerNodes {{\n", "    node: ParentedNode {\n",
               f"                owner_id: {target},\n",
               "                span: src/owner/api.rs:10:1: 12:1 (#0),\n", "}\n"]
        span = {"path": "src/runtime/run.rs", "line": 20, "column": 5,
                "end_line": 20, "end_column": 15}
        thir = [{"target_def_id": target, "callee_span": span},
                {"target_def_id": other, "callee_span": span}]
        report = analyze(doc, mir, mapping, dag, hir_owner_spans(hir), thir)
        self.assertEqual(1, len(report["unknown_calls"]))
        self.assertEqual([], report["cross_component_calls"])

    def test_thir_local_defid_without_hir_owner_blocks_name_guess(self):
        doc, mir, mapping, dag = self.fixture(call="owner::Owner::read")
        mir[0] = " _0 = owner::Owner::read(copy _1) -> [return: bb1, unwind continue]; // scope 0 at src/runtime/run.rs:20:5: 20:15\n"
        mir[1] = " // + const_: Const { ty: fn(&Owner), val: Value(owner::Owner::read) }\n"
        thir = [{"callee_span": {"path": "src/runtime/run.rs", "line": 20, "column": 5,
                                 "end_line": 20, "end_column": 15},
                 "target_def_id": "DefId(0:7 ~ reliquary_memory[abcd]::owner::Owner::read)"}]
        report = analyze(doc, mir, mapping, dag, {}, thir)
        self.assertEqual(1, len(report["unknown_calls"]))
        self.assertEqual([], report["cross_component_calls"])
        thir.append(dict(thir[0]))
        thir[0]["callee_span"]["column"] = 5
        thir[1]["callee_span"]["column"] = 5
        report = analyze(doc, mir, mapping, dag, {}, thir)
        self.assertEqual(1, len(report["unknown_calls"]))

    def test_thir_exact_external_defid_classifies_trait_callee(self):
        doc, mir, mapping, dag = self.fixture(call="<core::fmt::Formatter as core::fmt::Debug>::fmt")
        doc["external_crates"]["2"] = {"name": "core"}
        mir[0] = " _0 = Debug::fmt(copy _1) -> [return: bb1, unwind continue]; // scope 0 at src/runtime/run.rs:20:5: 20:13\n"
        mir[1] = (" // + const_: Const { ty: fn(&Owner), val: Value(ConstValue(ZeroSized: "
                   "fn(&Owner) {<core::fmt::Formatter as core::fmt::Debug>::fmt})) }\n")
        thir = [{"callee_span": {"path": "src/runtime/run.rs", "line": 20, "column": 5,
                                 "end_line": 20, "end_column": 13},
                 "target_def_id": "DefId(2:9 ~ core[abcd]::fmt::Debug::fmt)"}]
        report = analyze(doc, mir, mapping, dag, {}, thir)
        self.assertEqual(1, report["external_constant_calls"])
        self.assertEqual([], report["unknown_calls"])

    def test_thir_trait_declaration_defid_does_not_externalize_local_receiver(self):
        doc, mir, mapping, dag = self.fixture(call="<owner::Owner as core::fmt::Debug>::fmt")
        doc["external_crates"]["2"] = {"name": "core"}
        mir[0] = " _0 = Debug::fmt(copy _1) -> [return: bb1, unwind continue]; // scope 0 at src/runtime/run.rs:20:5: 20:13\n"
        mir[1] = (" // + const_: Const { ty: fn(&Owner), val: Value(ConstValue(ZeroSized: "
                   "fn(&Owner) {<owner::Owner as core::fmt::Debug>::fmt})) }\n")
        thir = [{"callee_span": {"path": "src/runtime/run.rs", "line": 20, "column": 5,
                                 "end_line": 20, "end_column": 13},
                 "target_def_id": "DefId(2:9 ~ core[abcd]::fmt::Debug::fmt)"}]
        report = analyze(doc, mir, mapping, dag, {}, thir)
        self.assertEqual(0, report["external_constant_calls"])
        self.assertEqual(1, len(report["unknown_calls"]))

    def test_thir_parser_reads_resolved_fn_def_call(self):
        rows = thir_call_sites(["DefId(0:8 ~ crate[abcd]::caller):\n", "    Call {\n",
                               "        ty: FnDef(DefId(0:7 ~ crate[abcd]::callee), [])\n",
                               "        fn_span: src/lib.rs:4:8: 4:14 (#0)\n", "    }\n"])
        self.assertEqual("DefId(0:7 ~ crate[abcd]::callee)", rows[0]["target_def_id"])
        self.assertEqual(8, rows[0]["callee_span"]["column"])

    def test_thir_parser_can_retain_only_requested_exact_source_spans(self):
        lines = ["DefId(0:8 ~ crate[abcd]::caller):\n",
                 "    Call {\n",
                 "        ty: FnDef(DefId(0:7 ~ crate[abcd]::callee), [])\n",
                 "        fn_span: src/lib.rs:4:8: 4:14 (#0)\n", "    }\n",
                 "    Call {\n",
                 "        ty: FnDef(DefId(0:9 ~ crate[abcd]::other), [])\n",
                 "        fn_span: src/lib.rs:8:8: 8:13 (#0)\n", "    }\n"]
        selected = thir_call_sites(lines, {
            ("src/lib.rs", 8, 8, 8, 13),
        })
        self.assertEqual(1, len(selected))
        self.assertEqual("DefId(0:9 ~ crate[abcd]::other)",
                         selected[0]["target_def_id"])

    def test_hir_owner_parser_can_retain_only_requested_exact_defids(self):
        first = "DefId(0:7 ~ crate[abcd]::first)"
        second = "DefId(0:9 ~ crate[abcd]::second)"
        lines = [f"{first} => OwnerNodes {{\n", "    node: ParentedNode {\n",
                 f"                owner_id: {first},\n",
                 "                span: src/first.rs:4:1: 4:8 (#0),\n", "}\n",
                 f"{second} => OwnerNodes {{\n", "    node: ParentedNode {\n",
                 f"                owner_id: {second},\n",
                 "                span: src/second.rs:8:1: 8:9 (#0),\n", "}\n"]
        selected = hir_owner_spans(lines, {second})
        self.assertEqual({second: [{"path": "src/second.rs", "line": 8,
                                    "end_line": 8}]}, selected)

    def test_local_trait_impl_symbol_precedes_external_trait_defid(self):
        doc, mir, mapping, dag = self.fixture(call="<owner::Owner as core::fmt::Debug>::fmt")
        doc["external_crates"]["2"] = {"name": "core"}
        doc["paths"]["4"] = {"crate_id": 2, "path": ["core", "fmt", "Debug"]}
        doc["index"]["2"] = {"crate_id": 0, "inner": {"impl": {
            "for": {"resolved_path": {"id": 1}}, "trait": {"id": 4},
            "items": [5]}}}
        doc["index"]["5"] = {"crate_id": 0, "name": "fmt", "span": {
            "filename": "src/owner/api.rs", "begin": [10, 1], "end": [12, 1]},
            "inner": {"function": {}}}
        mir[1] = (" // + const_: Const { ty: for<Region(BrNamed(DefId(2:900 ~ "
                   "core[abcd]::fmt::Debug::fmt)))> fn(&Owner) -> Result<(), Error> "
                   "{<owner::Owner as core::fmt::Debug>::fmt}, val: Value(ConstValue(ZeroSized: "
                   "fn(&Owner) -> Result<(), Error> {<owner::Owner as core::fmt::Debug>::fmt})) }\n")
        report = analyze(doc, mir, mapping, dag)
        self.assertEqual(0, report["external_constant_calls"])
        self.assertEqual(1, len(report["cross_component_calls"]))
        self.assertEqual("owner", report["cross_component_calls"][0]["target_component"])

    def test_unrelated_external_defid_does_not_externalize_direct_call(self):
        doc, mir, mapping, dag = self.fixture(call="owner::Owner::missing")
        mir[1] = (" // + const_: Const { ty: for<Region(BrNamed(DefId(2:42 ~ "
                  "core[abcd]::ops::Deref::deref::'_)))> fn(&Owner) "
                  "{owner::Owner::missing}, val: Value(ConstValue(ZeroSized: "
                  "fn(&Owner) {owner::Owner::missing})) }\n")
        report = analyze(doc, mir, mapping, dag)
        self.assertEqual(0, report["external_constant_calls"])
        self.assertEqual(1, len(report["unknown_calls"]))

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
        self.assertEqual("Owner::read", report["unknown_calls"][0]["target"])

    def test_nested_erased_regions_do_not_replace_function_symbol(self):
        rendered = ("ConstValue(ZeroSized: for<Region(DefId(0:1 ~ crate::owner::read))> "
                    "fn(&Owner<'{erased}>) -> u8 {Owner::<'{erased}>::read})")
        self.assertEqual("Owner::<'{erased}>::read", callable_const_value(rendered))

    def test_shortened_target_without_exact_hir_defid_remains_unknown(self):
        doc, mir, mapping, dag = self.fixture(call="Owner::read")
        mir[1] = (" // + const_: Const { ty: for<Region(BrNamed(DefId(0:42 ~ "
                  "reliquary_memory[abcd]::owner::{impl#0}::read::'_)))> "
                  "fn(&Owner) {Owner::read}, val: Value(ConstValue(ZeroSized: "
                  "fn(&Owner) {Owner::read})) }\n")
        report = analyze(doc, mir, mapping, dag)
        self.assertEqual([], report["violations"])
        self.assertEqual(1, len(report["unknown_calls"]))

    def test_mir_boundregion_defid_hint_does_not_resolve_shortened_target(self):
        doc, mir, mapping, dag = self.fixture(call="Owner::read")
        def_id = "DefId(0:42 ~ reliquary_memory[abcd]::owner::{impl#0}::read)"
        mir[1] = (" // + const_: Const { ty: for<Region(BrNamed(DefId(0:42 ~ "
                  "reliquary_memory[abcd]::owner::{impl#0}::read::'_)))> "
                  "fn(&Owner) {Owner::read}, val: Value(ConstValue(ZeroSized: "
                  "fn(&Owner) {Owner::read})) }\n")
        hir = [f"{def_id} => OwnerNodes {{\n",
               "    node: ParentedNode {\n",
               f"                owner_id: {def_id},\n",
               "                span: src/facade/api.rs:10:1: 12:1 (#0),\n",
               "                                span: src/facade/nested.rs:1:1: 99:1 (#0),\n",
               "}\n"]
        report = analyze(doc, mir, mapping, dag, hir_owner_spans(hir))
        self.assertEqual([], report["violations"])
        self.assertEqual([], report["cross_component_calls"])
        self.assertEqual(1, len(report["unknown_calls"]))

    def test_hir_owner_defid_mismatch_does_not_resolve_shortened_target(self):
        doc, mir, mapping, dag = self.fixture(call="Owner::read")
        def_id = "DefId(0:42 ~ reliquary_memory[abcd]::owner::{impl#0}::read)"
        mir[1] = (" // + const_: Const { ty: for<Region(BrNamed(DefId(0:42 ~ "
                  "reliquary_memory[abcd]::owner::{impl#0}::read::'_)))> "
                  "fn(&Owner) {Owner::read}, val: Value(ConstValue(ZeroSized: "
                  "fn(&Owner) {Owner::read; DefId(0:42 ~ reliquary_memory[abcd]::owner::{impl#0}::read)})) }\n")
        other = "DefId(0:43 ~ reliquary_memory[abcd]::owner::{impl#0}::read)"
        report = analyze(doc, mir, mapping, dag, hir_owner_spans([
            f"{other} => OwnerNodes {{\n", f"                owner_id: {other},\n",
            "                span: src/owner/api.rs:20:1: 22:1 (#0),\n", "}\n"]))
        self.assertEqual([], report["cross_component_calls"])
        self.assertEqual(1, len(report["unknown_calls"]))

    def test_ambiguous_hir_owner_spans_fail_closed(self):
        doc, mir, mapping, dag = self.fixture(call="Owner::read")
        def_id = "DefId(0:42 ~ reliquary_memory[abcd]::owner::{impl#0}::read)"
        mir[1] = (" // + const_: Const { ty: for<Region(BrNamed(DefId(0:42 ~ "
                  "reliquary_memory[abcd]::owner::{impl#0}::read::'_)))> "
                  "fn(&Owner) {Owner::read}, val: Value(ConstValue(ZeroSized: "
                  "fn(&Owner) {Owner::read})) }\n")
        report = analyze(doc, mir, mapping, dag, {def_id: [
            {"path": "src/owner/api.rs", "line": 10, "end_line": 12},
            {"path": "src/facade/api.rs", "line": 20, "end_line": 22}]})
        self.assertEqual(1, len(report["unknown_calls"]))

    def test_drop_terminator_remains_explicit_destructor_unknown(self):
        doc, _, mapping, dag = self.fixture()
        mir = [" drop(_3) -> [return: bb1, unwind continue]; // scope 0 at src/runtime/run.rs:20:1: 20:30\n",
               "}\n"]
        report = analyze(doc, mir, mapping, dag)
        self.assertEqual("drop terminator has unresolved generic or dynamic field glue",
                         report["unknown_calls"][0]["reason"])

    def test_exact_local_drop_impl_resolves_only_the_direct_mir_type(self):
        doc, _, mapping, dag = self.fixture()
        doc["index"]["1"] = {"crate_id": 0, "name": "Owner", "inner": {"struct": {
            "kind": {"plain": {"fields": [], "has_stripped_fields": False}},
            "generics": {"params": []}}}}
        doc["paths"]["4"] = {"crate_id": 1, "path": ["core", "ops", "Drop"]}
        doc["index"]["6"] = {"crate_id": 0, "name": None, "inner": {"impl": {
            "for": {"resolved_path": {"id": 1}}, "trait": {"id": 4}, "items": [5]}}}
        doc["index"]["5"] = {"crate_id": 0, "name": "drop", "span": {
            "filename": "src/owner/api.rs", "begin": [30, 5], "end": [32, 6]},
            "inner": {"function": {}}}
        mir = ["fn run() {\n", "    let _6: owner::Owner; // in scope 0\n",
               "    drop(_6) -> [return: bb1, unwind continue]; // scope 0 at src/runtime/run.rs:20:1: 20:8\n",
               "}\n"]
        report = analyze(doc, mir, mapping, dag)
        self.assertEqual(1, report["drop_analysis"]["raw_mir_drop_terminators"])
        self.assertEqual(1, report["drop_analysis"]["fully_resolved_drop_terminators"])
        self.assertEqual([], report["unknown_calls"])
        self.assertEqual("owner", report["cross_component_calls"][0]["target_component"])
        self.assertEqual("exact-rustdoc-nominal-field-drop-graph",
                         report["cross_component_calls"][0]["resolved_by"])

    def test_unknown_external_drop_wrapper_remains_unresolved(self):
        doc, _, mapping, dag = self.fixture()
        doc["index"]["1"] = {"crate_id": 0, "name": "Owner", "inner": {"struct": {
            "kind": {"plain": {"fields": [], "has_stripped_fields": False}},
            "generics": {"params": []}}}}
        doc["paths"]["4"] = {"crate_id": 1, "path": ["core", "ops", "Drop"]}
        doc["index"]["6"] = {"crate_id": 0, "name": None, "inner": {"impl": {
            "for": {"resolved_path": {"id": 1}}, "trait": {"id": 4}, "items": [5]}}}
        doc["index"]["5"] = {"crate_id": 0, "name": "drop", "span": {
            "filename": "src/owner/api.rs", "begin": [30, 5], "end": [32, 6]},
            "inner": {"function": {}}}
        mir = ["fn run() {\n", "    let _6: unknown_crate::Wrapper<owner::Owner>; // in scope 0\n",
               "    drop(_6) -> [return: bb1, unwind continue]; // scope 0 at src/runtime/run.rs:20:1: 20:8\n",
               "}\n"]
        report = analyze(doc, mir, mapping, dag)
        self.assertEqual(0, report["drop_analysis"]["fully_resolved_drop_terminators"])
        self.assertEqual(1, report["drop_analysis"]["unresolved_or_structural_drop_terminators"])
        self.assertEqual(1, len(report["unknown_calls"]))

    def test_custom_type_named_like_standard_wrapper_is_not_traversed_as_std(self):
        doc, _, mapping, dag = self.fixture()
        doc["index"]["1"] = {"crate_id": 0, "name": "Owner", "inner": {"struct": {
            "kind": {"plain": {"fields": [], "has_stripped_fields": False}},
            "generics": {"params": []}}}}
        doc["paths"]["8"] = {"crate_id": 0, "path": ["reliquary_memory", "owner", "Option"]}
        doc["index"]["8"] = {"crate_id": 0, "name": "Option", "inner": {"struct": {
            "kind": {"plain": {"fields": [9], "has_stripped_fields": False}},
            "generics": {"params": [{"name": "T", "kind": {"type": {}}}]}}}}
        doc["index"]["9"] = {"crate_id": 0, "name": "value", "inner": {
            "struct_field": {"generic": "T"}}}
        mir = ["fn run() {\n",
               "    let _6: other_crate::Option<owner::Owner>; // in scope 0\n",
               "    drop(_6) -> [return: bb1, unwind continue]; // scope 0 at src/runtime/run.rs:20:1: 20:8\n",
               "}\n"]
        report = analyze(doc, mir, mapping, dag)
        self.assertEqual(0, report["drop_analysis"]["fully_resolved_drop_terminators"])
        self.assertEqual(1, report["drop_analysis"]["unresolved_or_structural_drop_terminators"])
        self.assertEqual(1, len(report["unknown_calls"]))

    def test_missing_local_qualified_nominal_does_not_look_external(self):
        doc, _, mapping, dag = self.fixture()
        doc["index"]["1"] = {"crate_id": 0, "name": "Owner", "inner": {"struct": {
            "kind": {"plain": {"fields": [], "has_stripped_fields": False}},
            "generics": {"params": []}}}}
        mir = ["fn run() {\n",
               "    let _6: owner_lifecycle::PrivateMissing<owner::Owner>; // in scope 0\n",
               "    drop(_6) -> [return: bb1, unwind continue]; // scope 0 at src/runtime/run.rs:20:1: 20:8\n",
               "}\n"]
        report = analyze(doc, mir, mapping, dag)
        self.assertEqual(0, report["drop_analysis"]["fully_resolved_drop_terminators"])
        self.assertEqual(1, report["drop_analysis"]["unresolved_or_structural_drop_terminators"])
        self.assertEqual(1, len(report["unknown_calls"]))

    def test_function_pointer_drop_is_proven_without_following_signature_types(self):
        doc, _, mapping, dag = self.fixture()
        doc["index"]["1"] = {"crate_id": 0, "name": "Owner", "inner": {"struct": {
            "kind": {"plain": {"fields": [], "has_stripped_fields": False}},
            "generics": {"params": []}}}}
        mir = ["fn run() {\n",
               "    let _6: unsafe extern \"C\" fn(owner::Owner) -> owner::Owner; // in scope 0\n",
               "    drop(_6) -> [return: bb1, unwind continue]; // scope 0 at src/runtime/run.rs:20:1: 20:8\n",
               "}\n"]
        report = analyze(doc, mir, mapping, dag)
        self.assertEqual(1, report["drop_analysis"]["fully_resolved_drop_terminators"])
        self.assertEqual(1, report["drop_analysis"]["no_local_drop_path_terminators"])
        self.assertEqual([], report["unknown_calls"])

    def test_drop_glue_edges_follow_nominal_owners_without_flattening(self):
        doc, _, _, _ = self.fixture()
        # A runtime-owned root contains an owner-lifecycle object, which owns
        # a container resource.  Flattening descendants onto the drop caller
        # would invent runtime -> container and hide the actual field boundary.
        paths = {
            "10": ["reliquary_memory", "rel_execution", "RelExecutionOwner"],
            "11": ["reliquary_memory", "owner_lifecycle", "ReliquaryOwner"],
            "12": ["reliquary_memory", "container", "Container"],
        }
        rows = {
            "10": ("RelExecutionOwner", "src/runtime/api.rs", 10, [13]),
            "11": ("ReliquaryOwner", "src/owner/api.rs", 20, [14]),
            "12": ("Container", "src/container/api.rs", 30, []),
        }
        for item_id, (name, path, line, fields) in rows.items():
            doc["paths"][item_id] = {"crate_id": 0, "path": paths[item_id]}
            doc["index"][item_id] = {"crate_id": 0, "name": name, "span": {
                "filename": path, "begin": [line, 1], "end": [line + 2, 2]},
                "inner": {"struct": {"kind": {"plain": {
                    "fields": fields, "has_stripped_fields": False}},
                    "generics": {"params": []}}}}
        for field_id, parent, type_id in (("13", "root", "11"), ("14", "nested", "12")):
            doc["index"][field_id] = {"crate_id": 0, "name": parent, "inner": {
                "struct_field": {"resolved_path": {"id": type_id}}}}
        # Only Container has an explicit Drop implementation. The compiler's
        # passive layout hop runtime -> owner_lifecycle is not an executable
        # call, while the owner-owned field glue calls Container::drop.
        doc["paths"]["20"] = {"crate_id": 1, "path": ["core", "ops", "Drop"]}
        for impl_id, method_id, nominal_id, source_path, line in (
                ("25", "26", "12", "src/container/api.rs", 35),):
            doc["index"][impl_id] = {"crate_id": 0, "name": None, "inner": {"impl": {
                "for": {"resolved_path": {"id": nominal_id}},
                "trait": {"id": "20"}, "items": [method_id]}}}
            doc["index"][method_id] = {"crate_id": 0, "name": "drop", "span": {
                "filename": source_path, "begin": [line, 1], "end": [line + 1, 2]},
                "inner": {"function": {}}}
        mapping = {"files": [
            {"path": "src/runtime/run.rs", "component": "runtime"},
            {"path": "src/runtime/api.rs", "component": "runtime"},
            {"path": "src/owner/api.rs", "component": "owner_lifecycle"},
            {"path": "src/container/api.rs", "component": "container"},
        ]}
        dag = {"components": [
            {"id": "runtime", "depends_on": [{"to": "owner_lifecycle"}]},
            {"id": "owner_lifecycle", "depends_on": [{"to": "container"}]},
            {"id": "container", "depends_on": []},
        ]}
        mir = ["fn run() {\n",
               "    let _6: rel_execution::RelExecutionOwner; // in scope 0\n",
               "    drop(_6) -> [return: bb1, unwind continue]; // scope 0 at src/runtime/run.rs:20:1: 20:8\n",
               "}\n"]
        report = analyze(doc, mir, mapping, dag)
        edges = {(row["source_component"], row["target_component"])
                 for row in report["cross_component_calls"]}
        self.assertEqual({("owner_lifecycle", "container")}, edges)
        self.assertNotIn(("runtime", "container"), edges)
        passive = {(row["source_component"], row["target_component"])
                   for row in report["drop_analysis"]["passive_field_layout_witnesses"]}
        self.assertIn(("runtime", "owner_lifecycle"), passive)
        self.assertEqual([], [row for row in report["violations"]
                              if (row["source_component"], row["target_component"])
                              == ("runtime", "owner_lifecycle")])
        dag["components"][1]["depends_on"] = []
        forbidden = analyze(doc, mir, mapping, dag)
        self.assertEqual(1, len(forbidden["violations"]))
        self.assertEqual(("owner_lifecycle", "container"),
                         (forbidden["violations"][0]["source_component"],
                          forbidden["violations"][0]["target_component"]))

    def test_standard_owned_wrapper_and_concrete_generic_fields_resolve_drop_graph(self):
        doc, _, mapping, dag = self.fixture()
        doc["index"]["1"] = {"crate_id": 0, "name": "Owner", "inner": {"struct": {
            "kind": {"plain": {"fields": [], "has_stripped_fields": False}},
            "generics": {"params": []}}}}
        doc["paths"]["4"] = {"crate_id": 1, "path": ["core", "ops", "drop", "Drop"]}
        doc["index"]["6"] = {"crate_id": 0, "name": None, "inner": {"impl": {
            "for": {"resolved_path": {"id": 1}}, "trait": {"id": 4}, "items": [5]}}}
        doc["index"]["5"] = {"crate_id": 0, "name": "drop", "span": {
            "filename": "src/owner/api.rs", "begin": [30, 5], "end": [32, 6]},
            "inner": {"function": {}}}
        doc["paths"]["8"] = {"crate_id": 0, "path": ["reliquary_memory", "owner", "Envelope"]}
        doc["index"]["8"] = {"crate_id": 0, "name": "Envelope", "span": {
            "filename": "src/owner/api.rs", "begin": [40, 1], "end": [42, 2]}, "inner": {"struct": {
            "kind": {"plain": {"fields": [9], "has_stripped_fields": False}},
            "generics": {"params": [{"name": "T", "kind": {"type": {}}}]}}}}
        doc["index"]["9"] = {"crate_id": 0, "name": "value", "inner": {"struct_field": {"generic": "T"}}}
        mir = ["fn run() {\n", "    let _6: std::option::Option<owner::Owner>; // in scope 0\n",
               "    drop(_6) -> [return: bb1, unwind continue]; // scope 0 at src/runtime/run.rs:20:1: 20:8\n",
               "    }\n", "    bb1: {\n",
               "    let _7: owner::Envelope<owner::Owner>; // in scope 0\n",
               "    drop(_7) -> [return: bb2, unwind continue]; // scope 0 at src/runtime/run.rs:21:1: 21:8\n",
               "}\n"]
        report = analyze(doc, mir, mapping, dag)
        self.assertEqual(2, report["drop_analysis"]["fully_resolved_drop_terminators"])
        self.assertEqual([], report["unknown_calls"])
        self.assertEqual(1, len(report["cross_component_calls"]))
        envelope_edges = [row for row in report["drop_analysis"]["direct_drop_witnesses"]
                          if row.get("drop_type", "").startswith("owner::Envelope")]
        self.assertTrue(any(row["source_component"] == "runtime" and row["target_component"] == "owner"
                            for row in envelope_edges))
        self.assertTrue(any(row["source_component"] == "owner" and row["target_component"] == "owner"
                            for row in envelope_edges))

    def test_unresolved_generic_field_keeps_drop_terminator_unknown(self):
        doc, _, mapping, dag = self.fixture()
        doc["paths"]["8"] = {"crate_id": 0, "path": ["reliquary_memory", "owner", "Envelope"]}
        doc["index"]["8"] = {"crate_id": 0, "name": "Envelope", "inner": {"struct": {
            "kind": {"plain": {"fields": [9], "has_stripped_fields": False}},
            "generics": {"params": [{"name": "T", "kind": {"type": {}}}]}}}}
        doc["index"]["9"] = {"crate_id": 0, "name": "value", "inner": {"struct_field": {"generic": "T"}}}
        mir = ["fn run() {\n", "    let _6: owner::Envelope<T>; // in scope 0\n",
               "    drop(_6) -> [return: bb1, unwind continue]; // scope 0 at src/runtime/run.rs:20:1: 20:8\n",
               "}\n"]
        report = analyze(doc, mir, mapping, dag)
        self.assertEqual(0, report["drop_analysis"]["fully_resolved_drop_terminators"])
        self.assertEqual(1, report["drop_analysis"]["unresolved_or_structural_drop_terminators"])
        self.assertEqual(1, len(report["unknown_calls"]))

    def test_provenance_must_match_artifacts_and_current_sources(self):
        scratch = Path(".verification-tmp/phase7")
        scratch.mkdir(parents=True, exist_ok=True)
        with tempfile.TemporaryDirectory(prefix="compiler-provenance-test-", dir=scratch) as temp:
            root = Path(temp)
            source_file = root / "src/z.rs"
            second_source_file = root / "src/a.rs"
            source_file.parent.mkdir()
            source_file.write_text("pub fn current() {}\n", encoding="utf-8")
            second_source_file.write_text("pub fn another() {}\n", encoding="utf-8")
            entries = [{"path": "src/z.rs",
                        "sha256": hashlib.sha256(source_file.read_bytes()).hexdigest()},
                       {"path": "src/a.rs",
                        "sha256": hashlib.sha256(second_source_file.read_bytes()).hexdigest()}]
            fingerprint = source_fingerprint(entries)
            manifest = {"schema": "phase7-compiler-source-manifest.v1",
                        "files": entries, "fingerprint_sha256": fingerprint}
            manifest_path = root / "compiler-source-manifest.json"
            manifest_path.write_text(json.dumps(manifest), encoding="utf-8")
            artifact = Path(__file__).resolve()
            artifact_hash = hashlib.sha256(artifact.read_bytes()).hexdigest()
            receipt = {"schema": "phase7-compiler-provenance.v1",
                       "source_snapshot": {"before_sha256": fingerprint,
                                           "after_sha256": fingerprint,
                                           "file_count": 2,
                                           "manifest_path": "compiler-source-manifest.json",
                                           "manifest_sha256": hashlib.sha256(
                                               manifest_path.read_bytes()).hexdigest()},
                       "artifacts": {"rustdoc": {"path": str(artifact), "sha256": artifact_hash},
                                     "mir": {"path": str(artifact), "sha256": artifact_hash},
                                     "hir": {"path": str(artifact), "sha256": artifact_hash}}}
            receipt.update({"build_profile": "dev", "features": [], "cfg": ["unix"],
                            "environment": {}, "rustc_version": "rustc test"})
            with mock.patch("check_phase7_compiler_calls.ROOT", root):
                policy_paths = ["Cargo.toml", "Cargo.lock",
                                "tools/pitlord/production-components.json",
                                "tools/pitlord/convergence-component-dag.json"]
                for relative in policy_paths:
                    policy = root / relative
                    policy.parent.mkdir(parents=True, exist_ok=True)
                    policy.write_text(relative, encoding="utf-8")
                relative = "scripts/emit_phase7_compiler_evidence.py"
                tool = root / relative
                tool.parent.mkdir(parents=True, exist_ok=True)
                tool.write_text("emitter", encoding="utf-8")
                tool_hash = hashlib.sha256(tool.read_bytes()).hexdigest()
                tool_records = {"emitter": {"path": relative, "sha256_before": tool_hash,
                                             "sha256_after": tool_hash}}
                log_path = root / "compiler.log"
                log_path.write_text("ok\n", encoding="utf-8")
                receipt["policy_hashes"] = {
                    relative: hashlib.sha256((root / relative).read_bytes()).hexdigest()
                    for relative in policy_paths}
                receipt["policy_hashes_after"] = dict(receipt["policy_hashes"])
                receipt["commands"] = [
                    {"name": name, "exit": 0, "log": "compiler.log",
                     "log_sha256": hashlib.sha256(log_path.read_bytes()).hexdigest()}
                    for name in ("rustdoc", "mir", "hir", "thir")]
                receipt["tools"] = tool_records
                valid = validate_provenance(receipt, artifact, artifact, artifact)
                self.assertTrue(valid["validated"], valid["errors"])
                dump_dir = root / "built-mir"
                dump_dir.mkdir()
                dump = dump_dir / "fixture.{impl#0}-clone.1-1-000.built.after.mir"
                dump.write_text("// MIR for `<impl at src/z.rs:1:1: 1:8>::clone` after built\n\n"
                                "fn <impl at src/z.rs:1:1: 1:8>::clone(_1: &Foo) -> Foo {\n}\n",
                                encoding="utf-8")
                built_manifest = root / "built-mir-manifest.json"
                built_manifest.write_text(json.dumps({
                    "schema": "phase7-built-mir-manifest.v1",
                    "directory": "built-mir",
                    "files": [{"path": "built-mir/" + dump.name,
                               "sha256": hashlib.sha256(dump.read_bytes()).hexdigest()}],
                }), encoding="utf-8")
                receipt["artifacts"]["built_mir_manifest"] = {
                    "path": str(built_manifest),
                    "sha256": hashlib.sha256(built_manifest.read_bytes()).hexdigest()}
                next(row for row in receipt["commands"] if row["name"] == "mir")["command"] = [
                    "cargo", "rustc", "-Zdump-mir=built", f"-Zdump-mir-dir={dump_dir}"]
                valid_built = validate_provenance(receipt, artifact, artifact, artifact,
                                                  built_mir_manifest=built_manifest)
                self.assertTrue(valid_built["validated"], valid_built["errors"])
                dump.write_text(dump.read_text(encoding="utf-8") + "// changed\n",
                                encoding="utf-8")
                changed_dump = validate_provenance(receipt, artifact, artifact, artifact,
                                                   built_mir_manifest=built_manifest)
                self.assertFalse(changed_dump["validated"])
                self.assertTrue(any("hash does not match manifest" in error
                                    for error in changed_dump["errors"]))
                dump.write_text("// MIR for `<impl at src/z.rs:1:1: 1:8>::clone` after built\n\n"
                                "fn <impl at src/z.rs:1:1: 1:8>::clone(_1: &Foo) -> Foo {\n}\n",
                                encoding="utf-8")
                invalid_hir = validate_provenance(receipt, artifact, artifact, source_file)
                self.assertFalse(invalid_hir["validated"])
                self.assertIn("hir artifact path does not match the supplied input",
                              invalid_hir["errors"])
                source_file.write_text("pub fn changed() {}\n", encoding="utf-8")
                changed = validate_provenance(receipt, artifact, artifact)
                self.assertFalse(changed["validated"])
                self.assertIn("current Rust source files differ from the emitted source manifest",
                              changed["errors"])
                wrong_artifact = validate_provenance(receipt, artifact, source_file)
                self.assertFalse(wrong_artifact["validated"])

                failed_command = dict(receipt)
                failed_command["commands"] = [dict(row) for row in receipt["commands"]]
                failed_command["commands"][0]["exit"] = 101
                failed = validate_provenance(failed_command, artifact, artifact)
                self.assertIn("rustdoc compiler command did not succeed", failed["errors"])

                (root / "tools/pitlord/convergence-component-dag.json").write_text(
                    "changed", encoding="utf-8")
                stale_policy = validate_provenance(receipt, artifact, artifact)
                self.assertIn("current policy input differs from compiler provenance: "
                              "tools/pitlord/convergence-component-dag.json",
                              stale_policy["errors"])
                changed_during_build = dict(receipt)
                changed_during_build["policy_hashes_after"] = dict(receipt["policy_hashes"])
                changed_during_build["policy_hashes_after"]["Cargo.toml"] = "0" * 64
                self.assertIn("compiler policy inputs changed during emission",
                              validate_provenance(changed_during_build, artifact, artifact)["errors"])

                changed_tool = dict(receipt)
                changed_tool["tools"] = {"emitter": dict(tool_records["emitter"])}
                changed_tool["tools"]["emitter"]["sha256_after"] = "0" * 64
                unstable_tool = validate_provenance(changed_tool, artifact, artifact)
                self.assertIn("emitter tool identity is missing or changed during capture",
                              unstable_tool["errors"])

    def test_mir_selection_uses_exact_cargo_crate_hash_not_newest_file(self):
        Path(".verification-tmp/phase7").mkdir(parents=True, exist_ok=True)
        with tempfile.TemporaryDirectory(prefix="compiler-artifact-test-",
                                          dir=".verification-tmp/phase7") as temp:
            root = Path(temp)
            target = root / "target"
            deps = target / "debug/deps"
            deps.mkdir(parents=True)
            selected = deps / "reliquary_memory-abc123.mir"
            selected.write_text("current", encoding="utf-8")
            (deps / "reliquary_memory-def456.mir").write_text("stale profile",
                                                                  encoding="utf-8")
            log = root / "mir.log"
            artifact = {"reason": "compiler-artifact",
                        "target": {"name": "reliquary_memory", "kind": ["lib"]},
                        "filenames": ["C:/target/debug/deps/libreliquary_memory-abc123.rlib"]}
            log.write_text(json.dumps(artifact) + "\n", encoding="utf-8")
            self.assertEqual(selected, crate_mir_from_cargo_log(log, target))
            log.write_text("", encoding="utf-8")
            with self.assertRaisesRegex(RuntimeError, "resolves 0 crate MIR artifacts"):
                crate_mir_from_cargo_log(log, target)
            artifact["filenames"].append(
                "C:/target/debug/deps/libreliquary_memory-def456.rmeta")
            log.write_text(json.dumps(artifact) + "\n", encoding="utf-8")
            with self.assertRaisesRegex(RuntimeError, "resolves 2 crate MIR artifacts"):
                crate_mir_from_cargo_log(log, target)


if __name__ == "__main__":
    unittest.main()
