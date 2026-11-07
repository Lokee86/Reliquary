from __future__ import annotations

import json
import sys
import unittest
from contextlib import redirect_stderr
from io import StringIO
from tempfile import TemporaryDirectory
from unittest.mock import patch
from pathlib import Path
from hashlib import sha256

HERE = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(Path(__file__).resolve().parent))
import compare_synthetic as comparator  # noqa: E402
import synthetic_inventory as oracle  # noqa: E402

SCHEMA_PATH = HERE / "contract-v1" / "schemas" / "semantic-row-v1.schema.json"
FIXTURES = HERE / "fixtures"


def _row_bytes(rows: list[dict]) -> bytes:
    return b"".join(
        (json.dumps(row, ensure_ascii=False, separators=(",", ":")) + "\n").encode("utf-8")
        for row in rows
    )


def _rows(raw: bytes) -> list[dict]:
    return [json.loads(line) for line in raw.decode("utf-8").splitlines()]


class SyntheticComparatorTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.schema, cls.schema_bytes = comparator.load_schema(SCHEMA_PATH)
        cls.source_bytes = (FIXTURES / "d02-multiowner-source.jsonl").read_bytes()
        cls.target_bytes = (FIXTURES / "d02-multiowner-target.jsonl").read_bytes()
        cls.inventory_bytes = oracle.EXPECTED_INVENTORY_BYTES
        cls.target_coverage_bytes = oracle.TARGET_COVERAGE_BYTES

    def _compare(
        self,
        source: bytes | None = None,
        target: bytes | None = None,
        target_coverage: bytes | None = None,
    ) -> dict:
        return comparator.compare_bytes(
            self.source_bytes if source is None else source,
            self.target_bytes if target is None else target,
            self.schema,
            self.schema_bytes,
            self.inventory_bytes,
            self.target_coverage_bytes if target_coverage is None else target_coverage,
        )

    def test_output_cannot_overwrite_existing_evidence(self) -> None:
        with TemporaryDirectory() as temporary:
            evidence = Path(temporary) / "existing.jsonl"
            evidence.write_bytes(self.source_bytes)
            argv = []
            for option in ("source", "target", "inventory", "target-coverage", "output"):
                argv.extend((f"--{option}", str(evidence)))
            with patch.object(comparator, "compare_bytes", return_value={"status": "PASS"}):
                with redirect_stderr(StringIO()):
                    self.assertEqual(comparator.main(argv), 2)
            self.assertEqual(evidence.read_bytes(), self.source_bytes)

    def test_sidecar_hashes_are_pinned_and_not_derived_from_rows(self) -> None:
        self.assertEqual(
            sha256(self.inventory_bytes).hexdigest(),
            "cefa88789f8b649c762f72b80d9653604af418a3aeb4bd5a22263f93f9b705df",
        )
        self.assertEqual(
            sha256(self.target_coverage_bytes).hexdigest(),
            "519e258cdc357bd31b5e4d7fdbdb612debdf63d45e07f3f54f3474ba67c2394c",
        )
        self.assertNotEqual(self.inventory_bytes, self.source_bytes)
        self.assertNotEqual(self.target_coverage_bytes, self.target_bytes)

    def test_hand_authored_equivalent_witnesses_pass_despite_file_order(self) -> None:
        self.assertNotEqual(
            [row["family"] for row in _rows(self.source_bytes)],
            [row["family"] for row in _rows(self.target_bytes)],
        )
        report = self._compare()
        self.assertEqual(report["status"], "PASS")
        self.assertEqual(report["source_row_count"], "3")
        self.assertEqual(report["target_row_count"], "3")
        self.assertEqual(report["mismatch_count"], "0")
        self.assertEqual(report["inventory_authority"], "synthetic_only_not_authenticated")
        rows = _rows(self.source_bytes)
        self.assertEqual({row["owner"]["owner_kind"] for row in rows}, {"REL", "PHY"})
        self.assertIn("fixture.vector_probe.v1", {row["family"] for row in rows})
        memory = next(row for row in rows if row["family"] == "fixture.memory_revision.v1")
        self.assertEqual(memory["coordinate"], oracle.COORDINATE_ORIGIN)
        self.assertLess(
            int(memory["coordinate"]["global_version"]),
            int(oracle.COORDINATE_C2["global_version"]),
        )
        self.assertLess(
            int(oracle.COORDINATE_C2["global_version"]),
            int(oracle.COORDINATE_C1["global_version"]),
        )
        memory_inventory = next(
            row
            for owner in oracle.EXPECTED_OWNER_ROWS
            for row in owner["families"]["fixture.memory_revision.v1"]
        )
        self.assertEqual(memory_inventory["cut_ids"], [oracle.C2, oracle.C1])

    def test_omitted_row_fails(self) -> None:
        lines = self.target_bytes.splitlines(keepends=True)
        report = self._compare(target=b"".join(lines[:-1]))
        self.assertEqual(report["status"], "FAIL")
        self.assertEqual(report["mismatch_count"], "1")
        self.assertTrue(any(
            item["kind"] in {"missing_target_row", "target_omits_independent_inventory_row"}
            for item in report["diagnostics"]
        ))

    def test_extra_row_fails(self) -> None:
        rows = _rows(self.target_bytes)
        extra = dict(rows[0])
        extra["ordinal"] = str(int(extra["ordinal"]) + 100)
        report = self._compare(target=self.target_bytes + _row_bytes([extra]))
        self.assertEqual(report["status"], "FAIL")
        self.assertEqual(report["mismatch_count"], "1")
        self.assertTrue(any(
            item["kind"] in {"extra_target_row", "target_row_not_in_independent_inventory"}
            for item in report["diagnostics"]
        ))

    def test_changed_coordinate_fails_without_count_change(self) -> None:
        rows = _rows(self.target_bytes)
        rows[0]["coordinate"]["global_version"] = "99"
        report = self._compare(target=_row_bytes(rows))
        self.assertEqual(report["status"], "FAIL")
        self.assertEqual(report["source_row_count"], report["target_row_count"])
        changed = next(item for item in report["diagnostics"] if item["kind"] == "changed_row")
        self.assertIn("/coordinate/global_version", changed["different_fields"])

    def test_ordered_array_change_fails(self) -> None:
        target = _rows(self.target_bytes)
        target_row = next(row for row in target if row["family"] == "fixture.memory_revision.v1")
        target_row["value"]["fields"]["source_refs"]["items"].reverse()
        report = self._compare(target=_row_bytes(target))
        self.assertEqual(report["status"], "FAIL")
        self.assertTrue(any(
            any(path.startswith("/value/fields/source_refs/items/") for path in item["different_fields"])
            for item in report["diagnostics"] if item["kind"] == "changed_row"
        ))

    def test_unknown_value_schema_fails_closed(self) -> None:
        rows = _rows(self.target_bytes)
        rows[0]["value"]["schema_id"] = "fixture.unknown.v1"
        with self.assertRaises(comparator.WitnessInputError):
            self._compare(target=_row_bytes(rows))

    def test_family_group_and_value_schema_mapping_is_frozen(self) -> None:
        rows = _rows(self.target_bytes)
        memory = next(row for row in rows if row["family"] == "fixture.memory_revision.v1")
        memory["group_id"] = "archive_echo"
        with self.assertRaisesRegex(comparator.WitnessInputError, "family/group mapping"):
            self._compare(target=_row_bytes(rows))

        rows = _rows(self.target_bytes)
        memory = next(row for row in rows if row["family"] == "fixture.memory_revision.v1")
        memory["family"] = "fixture.archive_turn_revision.v1"
        memory["group_id"] = "archive_echo"
        with self.assertRaisesRegex(comparator.WitnessInputError, "value.schema_id"):
            self._compare(target=_row_bytes(rows))

    def test_unknown_schema_identity_or_same_id_changed_bytes_rejects(self) -> None:
        changed = dict(self.schema)
        changed["$id"] = "fixture.unknown.semantic-row.v2"
        with self.assertRaisesRegex(comparator.WitnessInputError, "unsupported semantic schema"):
            comparator.compare_bytes(
                self.source_bytes, self.target_bytes, changed,
                json.dumps(changed, sort_keys=True).encode(),
                self.inventory_bytes, self.target_coverage_bytes,
            )
        changed_bytes = self.schema_bytes + b" "
        with self.assertRaisesRegex(comparator.WitnessInputError, "schema bytes/hash"):
            comparator.compare_bytes(
                self.source_bytes, self.target_bytes, self.schema,
                changed_bytes, self.inventory_bytes, self.target_coverage_bytes,
            )

    def test_same_omission_from_both_bundles_fails_independent_inventory(self) -> None:
        source = _rows(self.source_bytes)
        target = _rows(self.target_bytes)
        source = [row for row in source if row["family"] != "fixture.vector_probe.v1"]
        target = [row for row in target if row["family"] != "fixture.vector_probe.v1"]
        report = self._compare(source=_row_bytes(source), target=_row_bytes(target))
        self.assertEqual(report["status"], "FAIL")
        self.assertTrue(any(
            item["kind"] == "source_omits_independent_inventory_row"
            for item in report["diagnostics"]
        ))
        self.assertTrue(any(
            item["kind"] == "target_omits_independent_inventory_row"
            for item in report["diagnostics"]
        ))

    def test_unlisted_owner_kind_scope_tuple_fails_without_global_scope_assumption(self) -> None:
        source = _rows(self.source_bytes)
        target = _rows(self.target_bytes)
        for rows in (source, target):
            row = next(row for row in rows if row["family"] == "fixture.vector_probe.v1")
            row["owner"] = {
                "owner_uuid": "00112233445566778899aabbccddeeff",
                "owner_kind": "PHY",
                "scope": "project",
            }
        report = self._compare(source=_row_bytes(source), target=_row_bytes(target))
        self.assertEqual(report["status"], "FAIL")
        self.assertEqual(report["source_row_count"], report["target_row_count"])
        self.assertTrue(any(
            item["kind"] == "source_row_not_in_independent_inventory"
            for item in report["diagnostics"]
        ))

    def test_target_owner_identity_swap_fails_at_equal_row_count(self) -> None:
        rows = _rows(self.target_bytes)
        vector = next(row for row in rows if row["family"] == "fixture.vector_probe.v1")
        vector["owner"] = {
            "owner_uuid": "10112233445566778899aabbccddeeff",
            "owner_kind": "PHY",
            "scope": "unspecified",
        }
        report = self._compare(target=_row_bytes(rows))
        self.assertEqual(report["status"], "FAIL")
        self.assertEqual(report["source_row_count"], report["target_row_count"])
        self.assertTrue(any(
            item["kind"] == "target_row_not_in_independent_inventory"
            for item in report["diagnostics"]
        ))

    def test_vector_bit_change_fails_at_equal_count(self) -> None:
        rows = _rows(self.target_bytes)
        vector = next(row for row in rows if row["family"] == "fixture.vector_probe.v1")
        vector["value"]["fields"]["scalar_bits"]["hex"] = "0100803f"
        report = self._compare(target=_row_bytes(rows))
        self.assertEqual(report["status"], "FAIL")
        self.assertEqual(report["source_row_count"], report["target_row_count"])
        self.assertTrue(any(
            item["kind"] == "changed_row" for item in report["diagnostics"]
        ))

    def test_matching_nonfinite_float_bits_reject_even_when_both_sides_match(self) -> None:
        source = _rows(self.source_bytes)
        target = _rows(self.target_bytes)
        for rows in (source, target):
            vector = next(row for row in rows if row["family"] == "fixture.vector_probe.v1")
            vector["value"]["fields"]["scalar_bits"]["hex"] = "0000c07f"
        with self.assertRaisesRegex(comparator.WitnessInputError, "non-finite IEEE"):
            self._compare(source=_row_bytes(source), target=_row_bytes(target))

    def test_typed_unsigned_and_coordinate_signed_ranges_are_enforced(self) -> None:
        source = _rows(self.source_bytes)
        target = _rows(self.target_bytes)
        for rows in (source, target):
            memory = next(row for row in rows if row["family"] == "fixture.memory_revision.v1")
            memory["value"]["fields"]["revision"]["decimal"] = "18446744073709551616"
        with self.assertRaisesRegex(comparator.WitnessInputError, "exceeds u64"):
            self._compare(source=_row_bytes(source), target=_row_bytes(target))

        rows = _rows(self.target_bytes)
        memory = next(row for row in rows if row["family"] == "fixture.memory_revision.v1")
        memory["coordinate"]["transaction_time"]["value"] = "9223372036854775808"
        with self.assertRaisesRegex(comparator.WitnessInputError, "exceeds i64"):
            self._compare(target=_row_bytes(rows))

    def test_oversized_decimal_rejects_before_integer_conversion(self) -> None:
        source = _rows(self.source_bytes)
        target = _rows(self.target_bytes)
        for rows in (source, target):
            memory = next(row for row in rows if row["family"] == "fixture.memory_revision.v1")
            memory["value"]["fields"]["revision"]["decimal"] = "9" * 4301
        with self.assertRaisesRegex(comparator.WitnessInputError, "exceeds u64"):
            self._compare(source=_row_bytes(source), target=_row_bytes(target))

        inventory = json.loads(self.inventory_bytes)
        rel = next(owner for owner in inventory["owners"] if owner["owner"]["owner_kind"] == "REL")
        memory = next(family for family in rel["families"] if family["family"] == "fixture.memory_revision.v1")
        memory["rows"][0]["ordinal"] = "9" * 4301
        altered_inventory = (json.dumps(inventory, sort_keys=True, separators=(",", ":")) + "\n").encode()
        with self.assertRaisesRegex(comparator.WitnessInputError, "exceeds u64"):
            comparator.compare_bytes(
                self.source_bytes,
                self.target_bytes,
                self.schema,
                self.schema_bytes,
                altered_inventory,
                self.target_coverage_bytes,
            )

    def test_final_lf_in_hex_payloads_and_digest_rejects(self) -> None:
        cases = ("memory_id", "provenance_sha256", "unavailable_digest")
        for case in cases:
            source = _rows(self.source_bytes)
            target = _rows(self.target_bytes)
            for rows in (source, target):
                memory = next(row for row in rows if row["family"] == "fixture.memory_revision.v1")
                if case == "memory_id":
                    memory["value"]["fields"]["memory_id"]["hex"] += "\n"
                elif case == "provenance_sha256":
                    memory["source_binding"]["value"]["fields"]["source_body_sha256"]["hex"] += "\n"
                else:
                    memory["availability"] = "unavailable"
                    memory["value"] = {
                        "kind": "unavailable",
                        "schema_id": "fixture.memory_revision.v1",
                        "reason_code": "unavailable",
                        "evidence_sha256": "aa" * 32 + "\n",
                    }
            with self.subTest(case=case), self.assertRaisesRegex(
                comparator.WitnessInputError,
                "hex must be lowercase|exactly 64 lowercase hex",
            ):
                self._compare(source=_row_bytes(source), target=_row_bytes(target))

    def test_coordinate_domain_versions_must_be_unique_and_byte_ordered(self) -> None:
        for mode in ("reversed", "duplicate"):
            source = _rows(self.source_bytes)
            target = _rows(self.target_bytes)
            for rows in (source, target):
                memory = next(row for row in rows if row["family"] == "fixture.memory_revision.v1")
                domains = memory["coordinate"]["domain_versions"]
                if mode == "reversed":
                    domains.reverse()
                else:
                    memory["coordinate"]["domain_versions"] = [domains[0], domains[0]]
            with self.subTest(mode=mode), self.assertRaisesRegex(
                comparator.WitnessInputError,
                "unique and UTF-8-byte ordered",
            ):
                self._compare(source=_row_bytes(source), target=_row_bytes(target))

    def test_unknown_coordinate_domain_rejects_matching_rows(self) -> None:
        source = _rows(self.source_bytes)
        target = _rows(self.target_bytes)
        for rows in (source, target):
            memory = next(row for row in rows if row["family"] == "fixture.memory_revision.v1")
            memory["coordinate"]["domain_versions"][0]["domain_id"] = "fixture.unknown"
        with self.assertRaisesRegex(comparator.WitnessInputError, "not one of|unknown contract domain"):
            self._compare(source=_row_bytes(source), target=_row_bytes(target))

    def test_invalid_provenance_span_rejects_matching_rows(self) -> None:
        source = _rows(self.source_bytes)
        target = _rows(self.target_bytes)
        for rows in (source, target):
            memory = next(row for row in rows if row["family"] == "fixture.memory_revision.v1")
            fields = memory["source_binding"]["value"]["fields"]
            fields["source_body_span_start"]["decimal"] = "15"
            fields["source_body_span_end"]["decimal"] = "14"
        with self.assertRaisesRegex(comparator.WitnessInputError, "half-open byte range"):
            self._compare(source=_row_bytes(source), target=_row_bytes(target))

    def test_unavailable_reason_must_match_availability(self) -> None:
        source = _rows(self.source_bytes)
        target = _rows(self.target_bytes)
        for rows in (source, target):
            memory = next(row for row in rows if row["family"] == "fixture.memory_revision.v1")
            memory["availability"] = "tombstoned"
            memory["value"] = {
                "kind": "unavailable",
                "schema_id": "fixture.memory_revision.v1",
                "reason_code": "pruned",
                "evidence_sha256": "ee" * 32,
            }
        with self.assertRaisesRegex(comparator.WitnessInputError, "reason code must match"):
            self._compare(source=_row_bytes(source), target=_row_bytes(target))

    def test_freshness_intra_settlement_unavailable_exception_remains_valid(self) -> None:
        from jsonschema import Draft202012Validator

        legacy_rows = _rows((FIXTURES / "comparator-pass-source-rows.v1.jsonl").read_bytes())
        row = next(
            item for item in legacy_rows
            if item["family"] == "fixture.freshness_event.v1"
            and item["revision_or_event"] == "settlement:intra"
        )
        self.assertEqual(row["availability"], "unavailable")
        self.assertEqual(row["value"]["reason_code"], "intra_settlement_unavailable")
        validated = comparator._validated_rows(
            _row_bytes([row]),
            "freshness exception fixture",
            Draft202012Validator(self.schema),
        )
        self.assertEqual(len(validated), 1)

    def test_provenance_change_fails_at_equal_count(self) -> None:
        rows = _rows(self.target_bytes)
        memory = next(row for row in rows if row["family"] == "fixture.memory_revision.v1")
        memory["source_binding"]["value"]["fields"]["source_body_sha256"]["hex"] = "ee" * 32
        report = self._compare(target=_row_bytes(rows))
        self.assertEqual(report["status"], "FAIL")
        self.assertEqual(report["source_row_count"], report["target_row_count"])
        self.assertTrue(any(
            item["kind"] == "changed_row" for item in report["diagnostics"]
        ))

    def test_target_coverage_family_cut_omission_rejects(self) -> None:
        document = json.loads(self.target_coverage_bytes)
        rel = next(owner for owner in document["owners"] if owner["owner"]["owner_kind"] == "REL")
        rel["families"] = [
            family for family in rel["families"]
            if family["family"] != "fixture.vector_probe.v1"
        ]
        altered = (json.dumps(document, sort_keys=True, separators=(",", ":")) + "\n").encode()
        with self.assertRaisesRegex(comparator.WitnessInputError, "exact sorted v1 family catalog"):
            self._compare(target_coverage=altered)

        document = json.loads(self.target_coverage_bytes)
        rel = next(owner for owner in document["owners"] if owner["owner"]["owner_kind"] == "REL")
        rel["cuts"].pop()
        altered = (json.dumps(document, sort_keys=True, separators=(",", ":")) + "\n").encode()
        with self.assertRaisesRegex(comparator.WitnessInputError, "family must enumerate every owner cut"):
            self._compare(target_coverage=altered)

    def test_target_coverage_membership_is_independent_of_origin_coordinate(self) -> None:
        document = json.loads(self.target_coverage_bytes)
        rel = next(owner for owner in document["owners"] if owner["owner"]["owner_kind"] == "REL")
        memory = next(family for family in rel["families"] if family["family"] == "fixture.memory_revision.v1")
        memory["rows"][0]["cut_ids"] = [oracle.C1]
        cut2 = next(cut for cut in memory["cuts"] if cut["cut_id"] == oracle.C2)
        cut2.update({
            "presence": "absent",
            "row_count": "0",
            "reason": "no hand-authored synthetic rows at this family/cut",
        })
        altered = (json.dumps(document, sort_keys=True, separators=(",", ":")) + "\n").encode()
        with self.assertRaisesRegex(comparator.WitnessInputError, "does not reconcile"):
            self._compare(target_coverage=altered)

    def test_empty_oracle_does_not_hide_extra_target_rows(self) -> None:
        import copy

        expected = copy.deepcopy(oracle.EXPECTED_OWNER_ROWS)
        observed = copy.deepcopy(oracle.TARGET_COVERAGE_OWNER_ROWS)
        for entries in (expected, observed):
            for owner in entries:
                for family in owner["families"]:
                    owner["families"][family] = []
        inventory = oracle._document(
            "reliquary-d02-synthetic-inventory-v1", oracle.INVENTORY_TRUST, expected
        )
        coverage = oracle._document(
            "reliquary-d02-synthetic-target-coverage-v1", oracle.TARGET_TRUST, observed
        )
        report = comparator.compare_bytes(
            b"",
            self.target_bytes,
            self.schema,
            self.schema_bytes,
            inventory,
            coverage,
        )
        self.assertEqual(report["status"], "FAIL")
        self.assertEqual(report["mismatch_count"], "3")
        self.assertTrue(any(
            item["kind"] == "target_row_not_in_independent_inventory"
            for item in report["diagnostics"]
        ))

    def test_duplicate_semantic_key_rejects(self) -> None:
        first = self.target_bytes.splitlines(keepends=True)[0]
        with self.assertRaises(comparator.WitnessInputError):
            self._compare(target=self.target_bytes + first)

    def test_duplicate_json_object_key_rejects(self) -> None:
        with self.assertRaises(comparator.WitnessInputError):
            comparator._read_jsonl(b'{"family":"a","family":"b"}\n', "synthetic")

    def test_json_numeric_token_rejects(self) -> None:
        with self.assertRaises(comparator.WitnessInputError):
            comparator._read_jsonl(b'{"ordinal":1}\n', "synthetic")

    def test_bom_crlf_and_missing_final_lf_reject(self) -> None:
        for raw in (b"\xef\xbb\xbf{}\n", b"{}\r\n", b"{}"):
            with self.subTest(raw=raw):
                with self.assertRaises(comparator.WitnessInputError):
                    comparator._read_jsonl(raw, "synthetic")

    def test_empty_expected_witness_is_not_a_pass(self) -> None:
        report = self._compare(source=b"")
        self.assertEqual(report["status"], "FAIL")
        self.assertEqual(report["reason"], "empty source omits independently inventoried rows")


if __name__ == "__main__":
    unittest.main()
