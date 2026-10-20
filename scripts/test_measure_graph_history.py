from __future__ import annotations

import hashlib
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import measure_graph_history as measure


def literal_container() -> bytes:
    """Independent hand-laid V1.0 Graph frames, including each wire family."""
    header = bytes.fromhex(
        "435641000d0a1a0a"  # Container magic
        "0100 0000 28000000"  # V1.0, typed 40-byte header
        "0101 000000000000"  # Reliquary, Project scope, zero padding
        "000102030405060708090a0b0c0d0e0f"  # owner UUID
    )
    mem_a = bytes.fromhex("11" * 32)
    mem_b = bytes.fromhex("22" * 32)
    ent = bytes.fromhex("33" * 32)
    obs = bytes.fromhex("44" * 32)
    marker = bytes.fromhex("43564147464d5431 01000000")
    node_v1 = bytes.fromhex("435641474e4f4445") + mem_a + bytes.fromhex("07000000")
    node_v2_entity = bytes.fromhex("435641474e4f4432 02") + ent + bytes.fromhex("08000000")
    node_v2_observation = bytes.fromhex("435641474e4f4432 03") + obs + bytes.fromhex("09000000")
    version = bytes.fromhex(
        "4356414756455231"  # CVAGVER1
        "0100000000000000"  # global ticket
        "0200000000000000"  # Graph-local clock
        "7856341200000000"  # ObjectRef offset
        "4e00000000000000"  # ObjectRef payload length
    )
    mut_v1_no_origin = (
        bytes.fromhex("435641474d555431") + mem_a + mem_b
        + bytes.fromhex("0100 01")  # Topical, active; origin is implicit Dream
    )
    mut_v1_origin = (
        bytes.fromhex("435641474d555431") + mem_b + mem_a
        + bytes.fromhex("0200 00 03")  # Factual, inactive, Perception
    )
    node_ref_mem = b"\x01" + mem_a
    node_ref_entity = b"\x02" + ent
    node_ref_observation = b"\x03" + obs
    mut_v2 = (
        bytes.fromhex("435641474d555432") + node_ref_mem + node_ref_entity
        + bytes.fromhex("6400 01 02")  # EntityAssociation, active, User
    )
    batch_v1_legacy = bytes.fromhex("4356414742415431 01000000") + mem_a + mem_b + bytes.fromhex("0300 01")
    batch_v1_current = bytes.fromhex("4356414742415431 01000000") + mem_b + mem_a + bytes.fromhex("0400 01 02")
    batch_v2 = bytes.fromhex("4356414742415432 01000000") + node_ref_observation + node_ref_mem + bytes.fromhex("6500 00 03")
    baseline_body = (
        bytes.fromhex("43564147424c3031")  # CVAGBL01
        + bytes.fromhex("aa" * 16)  # owner
        + bytes.fromhex("bb" * 16)  # generation
        + bytes.fromhex("cc" * 32)  # policy digest
        + bytes.fromhex("01000000")  # one ordered Memory ID
        + mem_a
    )
    baseline = baseline_body + hashlib.sha256(baseline_body).digest()
    payloads = [marker, node_v1, node_v2_entity, node_v2_observation, version,
                mut_v1_no_origin, mut_v1_origin, mut_v2, batch_v1_legacy,
                batch_v1_current, batch_v2, baseline]
    frames = b"".join(len(payload).to_bytes(8, "little") + payload for payload in payloads)
    result = header + frames
    assert [len(payload) for payload in payloads] == [12, 44, 45, 45, 40, 75, 76, 78, 79, 80, 82, 140]
    return result


class GraphHistoryParserTests(unittest.TestCase):
    def test_literal_container_inventories_every_graph_wire_family_and_frame(self) -> None:
        data = literal_container()
        header, version, count, records = measure.walk_container(data)
        self.assertEqual((header, version, count), (40, 1, 12))
        self.assertEqual(
            [record["kind"] for record in records],
            [
                "format_marker", "memory_node_mapping_v1", "semantic_node_mapping_v2",
                "semantic_node_mapping_v2", "version_link", "memory_mutation_v1",
                "memory_mutation_v1", "semantic_mutation_v2", "memory_batch_v1",
                "memory_batch_v1", "semantic_batch_v2", "rewrite_baseline",
            ],
        )
        self.assertEqual(
            [record["payload_bytes"] for record in records],
            [12, 44, 45, 45, 40, 75, 76, 78, 79, 80, 82, 140],
        )
        self.assertEqual(sum(record["framed_bytes"] for record in records), sum(record["payload_bytes"] for record in records) + 12 * 8)
        self.assertEqual(records[4]["object_ref_offset"], 0x12345678)
        self.assertEqual(records[5]["mutations"][0]["origin"], "dream (implicit in V1 no-origin encoding)")
        self.assertEqual(records[6]["mutations"][0]["origin"], "perception")
        self.assertEqual(records[7]["mutations"][0]["relation_kind"], "entity_association")
        self.assertEqual(records[10]["mutations"][0]["target"]["kind"], "memory")

    def test_semantic_endpoint_width_and_dictionary_break_even_include_mapping_overhead(self) -> None:
        records = measure.walk_container(literal_container())[3]
        projection = measure.dictionary_projection(records)
        self.assertEqual(projection["endpoint_occurrences"], 12)
        self.assertEqual(projection["distinct_owner_local_endpoints"], 4)
        self.assertEqual(projection["chosen_ordinal_width_bytes"], 1)
        self.assertEqual(projection["current_inline_endpoint_bytes"], 388)
        self.assertEqual(projection["projected_dictionary_entry_bytes"], 132)
        self.assertEqual(projection["projected_dictionary_header_bytes"], 20)
        self.assertEqual(projection["projected_dictionary_container_frame_bytes"], 8)
        self.assertEqual(projection["projected_typed_entry_kind_tag_bytes"], 4)
        self.assertEqual(projection["projected_dictionary_id_bytes"], 128)
        self.assertEqual(projection["projected_ordinal_reference_bytes"], 12)
        self.assertEqual(projection["projected_total_bytes"], 164)
        self.assertEqual(projection["projected_net_savings_bytes"], 224)
        typed_records = [record for record in records if record["kind"] in ("semantic_mutation_v2", "semantic_batch_v2")]
        typed_memory_refs = sum(
            1 for record in typed_records for kind, _ in record["endpoint_keys"] if kind == 1
        )
        self.assertEqual(typed_memory_refs, 2)
        self.assertEqual(typed_memory_refs * 33, 66)
        self.assertEqual(measure.ordinal_width(256), 1)
        self.assertEqual(measure.ordinal_width(257), 2)
        self.assertEqual(measure.ordinal_width(65_537), 4)

        endpoints_257 = [{"kind": "semantic_batch_v2", "endpoint_keys": [(1, index.to_bytes(32, "little"))]} for index in range(257)]
        too_narrow = measure.dictionary_projection(endpoints_257, 1)
        self.assertFalse(too_narrow["ordinal_width_feasible"])
        self.assertEqual(too_narrow["ordinal_width_capacity"], 256)
        self.assertIsNone(too_narrow["projected_total_bytes"])
        self.assertIsNone(too_narrow["projected_net_savings_bytes"])
        self.assertIsNone(too_narrow["break_even"])
        self.assertEqual(measure.dictionary_projection(endpoints_257)["chosen_ordinal_width_bytes"], 2)

        empty = measure.dictionary_projection([])
        self.assertEqual(empty["projected_total_bytes"], 0)
        self.assertEqual(empty["projected_dictionary_header_bytes"], 0)
        self.assertEqual(empty["projected_net_savings_bytes"], 0)

    def test_graph_shape_flags_enums_versions_and_trailing_bytes_fail_closed(self) -> None:
        payloads = [record_payload for record_payload in [
            bytes.fromhex("435641474d555431") + bytes(64) + bytes.fromhex("0100 01 01"),
            bytes.fromhex("435641474d555432") + b"\x01" + bytes(32) + b"\x01" + bytes(32) + bytes.fromhex("0100 01 01"),
        ]]
        self.assertIsNotNone(measure.parse_graph_payload(payloads[0]))
        self.assertIsNotNone(measure.parse_graph_payload(payloads[1]))

        bad_active = bytearray(payloads[0])
        bad_active[-2] = 2
        bad_relation = bytearray(payloads[0])
        bad_relation[72:74] = bytes.fromhex("0900")
        bad_origin = bytearray(payloads[1])
        bad_origin[-1] = 9
        bad_kind = bytearray(payloads[1])
        bad_kind[8] = 8
        for bad in [bad_active, bad_relation, bad_origin, bad_kind, payloads[0] + b"x"]:
            with self.assertRaises(measure.MeasurementError):
                measure.parse_graph_payload(bytes(bad))
        for unsupported in [b"CVAGMUT9", b"CVAGBAT9", b"CVAGVER2", b"CVAGNOD9", b"CVAGFMT2", b"CVAGBL99"]:
            with self.assertRaises(measure.MeasurementError):
                measure.parse_graph_payload(unsupported)

    def test_batch_count_lengths_checksum_and_container_truncation_fail_closed(self) -> None:
        batch = bytearray(bytes.fromhex("4356414742415431 01000000") + bytes(64) + bytes.fromhex("0100 01"))
        self.assertEqual(measure.parse_graph_payload(bytes(batch))["count"], 1)
        batch[8:12] = (2).to_bytes(4, "little")
        with self.assertRaises(measure.MeasurementError):
            measure.parse_graph_payload(bytes(batch))

        # A V2 typed batch with a V1-sized 68-byte body must not be parsed as
        # though the V1 family permits that stride.
        typed_batch_v1_length = bytes.fromhex("4356414742415432 01000000") + bytes(68)
        with self.assertRaises(measure.MeasurementError):
            measure.parse_graph_payload(typed_batch_v1_length)

        baseline = bytearray(literal_container())
        baseline[-1] ^= 1
        with self.assertRaises(measure.MeasurementError):
            measure.walk_container(bytes(baseline))
        with self.assertRaises(measure.MeasurementError):
            measure.walk_container(literal_container()[:-1])

    def test_version_and_node_mapping_layouts_are_exact(self) -> None:
        records = measure.walk_container(literal_container())[3]
        node1, node2, node3 = records[1:4]
        self.assertEqual(node1["semantic_id_bytes"], 32)
        self.assertEqual(node1["derived_internal_ordinal_bytes"], 4)
        self.assertEqual(node2["semantic_id_bytes"], 33)
        self.assertEqual(node3["node"]["kind"], "observation")
        version = records[4]
        self.assertEqual(version["semantic_clock_bytes"], 16)
        self.assertEqual(version["reference_bytes"], 16)
        self.assertEqual(version["framing_bytes"], 24)

    def test_all_other_graph_families_reject_wrong_shapes_and_mapping_codes(self) -> None:
        bad_format = bytearray(bytes.fromhex("43564147464d5431 01000000"))
        bad_format[8:12] = (2).to_bytes(4, "little")
        bad_memory_node = bytes.fromhex("435641474e4f4445") + bytes(37) + b"x"
        bad_typed_node = bytearray(bytes.fromhex("435641474e4f4432 01") + bytes(36))
        bad_typed_node[8] = 9
        bad_version = bytes.fromhex("4356414756455231") + bytes(31)
        for payload in [bad_format, bad_memory_node, bad_typed_node, bad_version]:
            with self.assertRaises(measure.MeasurementError):
                measure.parse_graph_payload(bytes(payload))


if __name__ == "__main__":
    unittest.main(verbosity=2)
