from __future__ import annotations

import json
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(HERE))
import compare_synthetic as comparator  # noqa: E402

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
        cls.source_bytes = (FIXTURES / "comparator-pass-source-rows.v1.jsonl").read_bytes()
        cls.target_bytes = (FIXTURES / "comparator-pass-target-rows.v1.jsonl").read_bytes()

    def _compare(self, source: bytes | None = None, target: bytes | None = None) -> dict:
        return comparator.compare_bytes(
            self.source_bytes if source is None else source,
            self.target_bytes if target is None else target,
            self.schema,
            self.schema_bytes,
        )

    def test_hand_authored_equivalent_witnesses_pass_despite_file_order(self) -> None:
        self.assertNotEqual(
            [row["family"] for row in _rows(self.source_bytes)],
            [row["family"] for row in _rows(self.target_bytes)],
        )
        report = self._compare()
        self.assertEqual(report["status"], "PASS")
        self.assertEqual(report["source_row_count"], "8")
        self.assertEqual(report["target_row_count"], "8")
        self.assertEqual(report["mismatch_count"], "0")

    def test_omitted_row_fails(self) -> None:
        lines = self.target_bytes.splitlines(keepends=True)
        report = self._compare(target=b"".join(lines[:-1]))
        self.assertEqual(report["status"], "FAIL")
        self.assertEqual(report["mismatch_count"], "1")
        self.assertEqual(report["diagnostics"][0]["kind"], "missing_target_row")

    def test_extra_row_fails(self) -> None:
        rows = _rows(self.target_bytes)
        extra = dict(rows[0])
        extra["ordinal"] = str(int(extra["ordinal"]) + 100)
        report = self._compare(target=self.target_bytes + _row_bytes([extra]))
        self.assertEqual(report["status"], "FAIL")
        self.assertEqual(report["mismatch_count"], "1")
        self.assertEqual(report["diagnostics"][0]["kind"], "extra_target_row")

    def test_changed_coordinate_fails_without_count_change(self) -> None:
        rows = _rows(self.target_bytes)
        rows[0]["coordinate"]["global_version"] = "99"
        report = self._compare(target=_row_bytes(rows))
        self.assertEqual(report["status"], "FAIL")
        self.assertEqual(report["source_row_count"], report["target_row_count"])
        self.assertEqual(report["diagnostics"][0]["kind"], "changed_row")
        self.assertIn("/coordinate/global_version", report["diagnostics"][0]["different_fields"])

    def test_ordered_array_change_fails(self) -> None:
        source = _rows(self.source_bytes)
        target = _rows(self.target_bytes)
        source_row = next(row for row in source if row["family"] == "fixture.memory_revision.v1")
        target_row = next(
            row for row in target
            if row["family"] == "fixture.memory_revision.v1"
            and row["revision_or_event"] == source_row["revision_or_event"]
            and row["ordinal"] == source_row["ordinal"]
        )
        first = source_row["value"]["fields"]["source_refs"]["items"][0]
        second = {"kind": "bytes", "hex": "cd" * 16}
        source_row["value"]["fields"]["source_refs"]["items"] = [first, second]
        target_row["value"]["fields"]["source_refs"]["items"] = [second, first]
        report = self._compare(source=_row_bytes(source), target=_row_bytes(target))
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
        self.assertEqual(report["status"], "INCOMPLETE")
        self.assertEqual(report["reason"], "empty expected witness is not a valid PASS")


if __name__ == "__main__":
    unittest.main()
