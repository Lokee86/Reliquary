"""Independent regression tests for the provisional synthetic PG-C01 contract."""
import copy
import json
import pathlib
import sys
import unittest

import jsonschema

ROOT = pathlib.Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))
import corpus_evidence_v1 as contract


class CorpusEvidenceV1Tests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.groups, cls.records, cls.carriers = contract.load_registries(ROOT)
        cls.domains = contract.domain_ids_from_registry(cls.records)
        cls.rows = contract.parse_jsonl((ROOT / "fixtures" / "source-rows.v1.jsonl").read_bytes())

    def test_hand_authored_rows_validate_and_schema_matches(self):
        encoded = contract.canonical_rows(self.rows, self.groups, self.records, self.domains)
        self.assertEqual(len(encoded), 7)
        self.assertTrue(contract.canonical_jsonl(encoded).endswith(b"\n"))
        schema = json.loads((ROOT / "schemas" / "semantic-row-v1.schema.json").read_bytes())
        validator = jsonschema.Draft202012Validator(schema)
        validator.check_schema(schema)
        for row in self.rows:
            validator.validate(row)

    def test_strict_json_rejects_ambiguous_or_nonportable_inputs(self):
        bad = [
            b"\xef\xbb\xbf{}",
            b'{"a":1}',
            b'{"a":NaN}',
            b'{"a":"x","\\u0061":"y"}',
            b'{"a":"\\ud800"}',
        ]
        for raw in bad:
            with self.subTest(raw=raw):
                with self.assertRaises(contract.ContractError):
                    contract.strict_json_loads(raw)

    def test_jcs_uses_utf16_code_unit_key_order(self):
        value = {"\ue000": "bmp", "\U00010000": "nonbmp", "a": "M\u00fcnchen e\u0301"}
        expected = b'{"a":"M\xc3\xbcnchen e\xcc\x81","\xf0\x90\x80\x80":"nonbmp","\xee\x80\x80":"bmp"}'
        self.assertEqual(contract.canonical_json_bytes(value), expected)

    def test_independent_golden_vectors_and_alternate_target_encoding(self):
        golden = json.loads((ROOT / "fixtures" / "golden-vectors-v1.json").read_bytes())["vectors"]
        target = contract.parse_jsonl((ROOT / "fixtures" / "target-rows.v1.jsonl").read_bytes())
        source = next(row for row in self.rows if row["family"] == "fixture.memory_revision.v1" and row["revision_or_event"] == "revision-2")
        self.assertEqual(len(target), 1)
        self.assertEqual(contract.canonical_json_bytes(source), contract.canonical_json_bytes(target[0]))
        jcs = golden["jcs_utf16"]
        value = json.loads(jcs["input_json_escaped"])
        self.assertEqual(contract.canonical_json_bytes(value).hex(), jcs["canonical_utf8_hex"])
        self.assertEqual([contract.u64be(int(n)).hex() for n in golden["u64be"]["decimal_values"]], golden["u64be"]["bytes_hex"])
        self.assertEqual(contract.lp(golden["length_prefix"]["payload_utf8"].encode()).hex(), golden["length_prefix"]["encoded_hex"])
        family = golden["empty_family_digest"]
        preimage, digest = contract.family_digest(family["owner"], family["coverage"], [])
        self.assertEqual(preimage.hex(), family["preimage_hex"])
        self.assertEqual(digest, family["sha256"])

    def test_ordinal_sort_is_numeric_and_duplicate_events_are_retained(self):
        events = [row for row in self.rows if row["family"] == "fixture.freshness_event.v1" and row["availability"] == "present"]
        self.assertEqual(len(events), 2)
        by_ordinal = {row["ordinal"]: row for row in events}
        self.assertLess(contract.row_sort_key(by_ordinal["2"]), contract.row_sort_key(by_ordinal["10"]))
        encoded = contract.canonical_rows(events, self.groups, self.records, self.domains)
        self.assertEqual(len(encoded), 2)
        self.assertNotEqual(encoded[0], encoded[1])

    def test_registry_required_logical_provenance_is_enforced(self):
        archive = next(row for row in self.rows if row["family"] == "fixture.archive_turn_revision.v1")
        omitted = copy.deepcopy(archive)
        omitted["source_binding"] = {"kind": "optional", "state": "absent"}
        with self.assertRaisesRegex(contract.ContractError, "requires logical source provenance"):
            contract.validate_semantic_row(omitted, self.groups, self.records, self.domains)

    def test_unknown_schema_and_extra_record_fields_fail_closed(self):
        row = copy.deepcopy(self.rows[0])
        row["family"] = "fixture.unreviewed.v1"
        with self.assertRaises(contract.ContractError):
            contract.validate_semantic_row(row, self.groups, self.records, self.domains)
        row = copy.deepcopy(next(r for r in self.rows if r["availability"] == "present"))
        row["value"]["fields"]["unreviewed"] = {"kind": "text", "value": "ignored"}
        with self.assertRaisesRegex(contract.ContractError, "exactly match"):
            contract.validate_semantic_row(row, self.groups, self.records, self.domains)

    def test_intermediate_history_and_logical_provenance_affect_semantic_digest(self):
        memories = [row for row in self.rows if row["family"] == "fixture.memory_revision.v1"]
        coverage = {
            "family": "fixture.memory_revision.v1", "group_id": "memory",
            "presence": "supported", "completeness": "exhaustive",
            "row_count": "2", "cuts": [],
        }
        canonical = contract.canonical_rows(memories, self.groups, self.records, self.domains)
        owner = memories[0]["owner"]
        original_digest = contract.family_digest(owner, coverage, canonical)[1]
        mutated = copy.deepcopy(memories)
        old = next(row for row in mutated if row["revision_or_event"] == "revision-1")
        old["value"]["fields"]["body"]["value"]["value"] = "intermediate body changed"
        mutated_digest = contract.family_digest(
            owner, coverage, contract.canonical_rows(mutated, self.groups, self.records, self.domains)
        )[1]
        self.assertNotEqual(original_digest, mutated_digest)
        self.assertEqual(
            next(row for row in memories if row["revision_or_event"] == "revision-2"),
            next(row for row in mutated if row["revision_or_event"] == "revision-2"),
        )
        provenance_changed = copy.deepcopy(memories)
        provenance_changed[0]["source_binding"]["value"]["fields"]["source_body_sha256"]["hex"] = "c" * 64
        provenance_digest = contract.family_digest(
            owner, coverage, contract.canonical_rows(provenance_changed, self.groups, self.records, self.domains)
        )[1]
        self.assertNotEqual(original_digest, provenance_digest)

    def test_physical_offsets_change_custody_not_semantic_rows(self):
        custody_a = {
            "source_file_sha256": "a" * 64, "target_file_sha256": "b" * 64,
            "relocations": [{
                "owner_uuid": "00112233445566778899aabbccddeeff",
                "family": "memory", "source_object_sha256": "1" * 64,
                "source_offset": "100", "source_length": "20",
                "target_object_sha256": "2" * 64, "target_offset": "300",
                "target_length": "20", "payload_identity": "abcd",
                "reference_kind": "body",
            }],
        }
        custody_b = copy.deepcopy(custody_a)
        custody_b["relocations"][0]["target_offset"] = "400"
        self.assertNotEqual(contract.custody_digest(custody_a)[1], contract.custody_digest(custody_b)[1])
        row = next(row for row in self.rows if row["family"] == "fixture.memory_revision.v1")
        coverage = {"family": row["family"], "group_id": row["group_id"], "presence": "supported",
                    "completeness": "exhaustive", "row_count": "1", "cuts": []}
        encoded = contract.canonical_rows([row], self.groups, self.records, self.domains)
        semantic_before = contract.family_digest(row["owner"], coverage, encoded)[1]
        custody_before = contract.custody_digest(custody_a)[1]
        custody_after = contract.custody_digest(custody_b)[1]
        semantic_after = contract.family_digest(row["owner"], coverage, encoded)[1]
        self.assertNotEqual(custody_before, custody_after)
        self.assertEqual(semantic_before, semantic_after)

    def test_authentic_admission_is_blocked_for_synthetic_registries(self):
        with self.assertRaises(contract.ContractError):
            contract.validate_authentic_admission(self.groups, self.records, self.carriers)

    def test_registry_bundle_is_bound_and_hash_is_stable(self):
        first = contract.registry_hashes(ROOT)
        second = contract.registry_hashes(ROOT)
        self.assertEqual(first, second)
        self.assertEqual(set(first), {"semantic_groups", "record_schemas", "physical_carriers", "contract_bundle"})
        self.assertEqual(self.carriers["coverage"], "incomplete")
        self.assertEqual(self.carriers["entries"], [])


if __name__ == "__main__":
    unittest.main()
