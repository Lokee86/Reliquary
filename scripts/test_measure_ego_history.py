"""Positive and fail-closed tests for the read-only Ego history parser."""

from __future__ import annotations

import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import measure_ego_history as measure


REPEATED = b"R" * 64


def golden_container() -> bytes:
    """Hand-authored V1.0 PHY header and one literal frame per Ego family."""
    header = bytes.fromhex(
        "435641000d0a1a0a"  # Container magic
        "0100 0000 28000000"  # V1.0, 40-byte typed header
        "0200 000000000000"  # Phylactery, no scope, zero padding
        "000102030405060708090a0b0c0d0e0f"  # fixed fixture UUID
    )
    legacy_identity = (
        bytes.fromhex("4356414549444e31 0100000000000000 0100000000000000 40000000")
        + REPEATED
    )
    identity = (
        bytes.fromhex(
            "4356414549444e32"  # CVAEIDN2
            "0200000000000000"  # Ego version
            "11111111111111111111111111111111"  # Identity ID
            "0100000000000000"  # revision
            "0000"  # deleted=false, activate=false
            "07000000"  # name length
        )
        + b"persona"
        + bytes.fromhex("40000000")
        + REPEATED
    )
    active = bytes.fromhex(
        "4356414549444131"  # CVAEIDA1
        "0300000000000000"  # Ego version
        "11111111111111111111111111111111"  # active Identity ID
    )
    personality = (
        bytes.fromhex(
            "4356414550455231"  # CVAEPER1
            "0400000000000000"  # Ego version
            "0100000000000000"  # revision
            "0a00000000000000"  # source Memory version
            "40000000"  # text length
        )
        + REPEATED
    )
    anchor = (
        bytes.fromhex(
            "43564145414e4331"  # CVAEANC1
            "0500000000000000"  # Ego version
            "22222222222222222222222222222222"  # Anchor ID
            "0100000000000000"  # revision
            "0001"  # deleted=false, priority=High
            "06000000"  # text length
        )
        + b"anchor"
    )
    synthesis = (
        bytes.fromhex(
            "4356414553594e31"  # CVAESYN1
            "0600000000000000"  # Ego version
            "0100000000000000"  # revision
            "0a00000000000000"  # source Memory version
            "40000000"  # text length
        )
        + REPEATED
    )
    # Literal little-endian u64 lengths are each followed by the exact payload.
    chunks = b"".join(
        [
            bytes.fromhex("5c00000000000000") + legacy_identity,  # 92 bytes
            bytes.fromhex("7900000000000000") + identity,  # 121 bytes
            bytes.fromhex("2000000000000000") + active,  # 32 bytes
            bytes.fromhex("6400000000000000") + personality,  # 100 bytes
            bytes.fromhex("3400000000000000") + anchor,  # 52 bytes
            bytes.fromhex("6400000000000000") + synthesis,  # 100 bytes
        ]
    )
    golden = header + chunks
    assert [len(value) for value in (legacy_identity, identity, active, personality, anchor, synthesis)] == [92, 121, 32, 100, 52, 100]
    assert len(golden) == 585
    return golden


class EgoHistoryParserTests(unittest.TestCase):
    def test_literal_container_covers_all_six_ego_families_and_frame_sizes(self) -> None:
        data = golden_container()
        header_bytes, version, chunk_count, owner_kind, records = measure.walk_container(data)

        self.assertEqual((header_bytes, version, chunk_count, owner_kind), (40, 1, 6, 2))
        self.assertEqual(
            [record["kind"] for record in records],
            [
                "legacy_identity",
                "identity",
                "identity_activation",
                "personality",
                "anchor",
                "synthesis",
            ],
        )
        self.assertEqual([record["payload_bytes"] for record in records], [92, 121, 32, 100, 52, 100])
        self.assertEqual([record["framed_bytes"] for record in records], [100, 129, 40, 108, 60, 108])
        self.assertEqual(sum(record["payload_bytes"] for record in records), 497)
        self.assertEqual(sum(record["framed_bytes"] for record in records), 545)
        self.assertEqual(len(data), 40 + 545)

    def test_utf8_field_totals_exact_repeats_and_projection_overhead(self) -> None:
        data = golden_container()
        _, _, _, _, records = measure.walk_container(data)
        by_field: dict[str, list[bytes]] = {}
        all_values: list[tuple[str, bytes]] = []
        for record in records:
            for field in record["text_fields"]:
                key = f"{record['kind']}.{field['field']}"
                raw = field["text"].encode("utf-8")
                by_field.setdefault(key, []).append(raw)
                all_values.append((key, raw))

        self.assertEqual(sum(map(len, (value for _, value in all_values))), 64 * 4 + 7 + 6)
        self.assertEqual(len(all_values), 6)
        self.assertEqual(sum(len(values) for values in by_field.values()), 6)
        owner_counts = measure.Counter(value for _, value in all_values)
        self.assertEqual(owner_counts[REPEATED], 4)
        self.assertEqual(sum(count - 1 for count in owner_counts.values()), 3)
        self.assertEqual(sum(len(value) * (count - 1) for value, count in owner_counts.items()), 192)

        pooled = measure.projected_pool_savings(all_values, 4)
        self.assertEqual(pooled["current_inline_text_bytes"], 64 * 4 + 7 + 6 + 6 * 4)
        self.assertEqual(pooled["current_tagged_inline_text_bytes"], 299)
        self.assertEqual(pooled["pooled_exact_values"], 1)
        self.assertEqual(pooled["pooled_occurrences"], 4)
        self.assertEqual(pooled["pool_entry_fixed_overhead_bytes"], 32)
        self.assertEqual(pooled["pool_entry_framing_bytes"], 8)
        self.assertEqual(pooled["pool_entry_magic_bytes"], 8)
        self.assertEqual(pooled["pool_entry_owner_sequence_bytes"], 8)
        self.assertEqual(pooled["pool_entry_length_prefix_bytes"], 4)
        self.assertEqual(pooled["projected_text_and_pool_bytes"], 139)
        self.assertEqual(pooled["net_savings_bytes"], 154)
        self.assertEqual(measure.projected_pool_savings(all_values, 8)["net_savings_bytes"], 134)
        self.assertEqual(measure.projected_pool_savings(all_values, 16)["net_savings_bytes"], 94)

        unique_only = measure.projected_pool_savings([("anchor.text", b"unique")], 4)
        self.assertEqual(unique_only["current_inline_text_bytes"], 10)
        self.assertEqual(unique_only["current_tagged_inline_text_bytes"], 11)
        self.assertEqual(unique_only["projected_text_and_pool_bytes"], 11)
        self.assertEqual(unique_only["pooled_exact_values"], 0)
        self.assertEqual(unique_only["net_savings_bytes"], -1)

        # No identical whole record bytes: sequence/revision/IDs differ even
        # when the exact text body repeats in different families.
        self.assertEqual(len({record["payload_sha256"] for record in records}), 6)

    def test_truncated_header_chunk_and_ego_string_fail_closed(self) -> None:
        data = golden_container()
        with self.assertRaises(measure.MeasurementError):
            measure.walk_container(data[:15])
        with self.assertRaises(measure.MeasurementError):
            measure.walk_container(data[:-1])

        # Keep the valid chunk prefix but shorten the first Ego body by one
        # byte, which must fail the string length check.
        truncated_body = bytearray(data[:40 + 8 + 91])
        truncated_body[40:48] = bytes.fromhex("5b00000000000000")
        with self.assertRaises(measure.MeasurementError):
            measure.walk_container(bytes(truncated_body))

    def test_invalid_ego_flags_priority_trailing_bytes_and_rel_owner_reject(self) -> None:
        data = bytearray(golden_container())
        # Identity flags follow the 92-byte Legacy Identity frame and its
        # 8-byte chunk prefix, then 40 fixed bytes in the Identity payload.
        data[40 + 8 + 92 + 8 + 40] = 2
        with self.assertRaises(measure.MeasurementError):
            measure.walk_container(bytes(data))

        data = bytearray(golden_container())
        # Anchor priority byte follows its deletion flag.
        anchor_offset = 40 + (8 + 92) + (8 + 121) + (8 + 32) + (8 + 100)
        data[anchor_offset + 8 + 41] = 4
        with self.assertRaises(measure.MeasurementError):
            measure.walk_container(bytes(data))

        data = golden_container() + bytes.fromhex("00")
        with self.assertRaises(measure.MeasurementError):
            measure.walk_container(data)

        # Convert only the typed file-kind byte to REL; Identity and Personality
        # families are explicitly forbidden by the current owner boundary.
        data = bytearray(golden_container())
        data[16] = 1
        with self.assertRaises(measure.MeasurementError):
            measure.walk_container(bytes(data))

    def test_whole_text_values_can_repeat_without_identical_records(self) -> None:
        records = measure.walk_container(golden_container())[4]
        repeated = [record for record in records if any(
            field["text"].encode("utf-8") == REPEATED for field in record["text_fields"]
        )]
        self.assertEqual(len(repeated), 4)
        self.assertEqual(len({record["payload_sha256"] for record in repeated}), 4)

    def test_utf8_lengths_are_measured_in_bytes_not_characters(self) -> None:
        # One Anchor text value "é": one Unicode scalar, two UTF-8 bytes.
        payload = bytes.fromhex(
            "43564145414e4331"  # CVAEANC1
            "0100000000000000"  # Ego version
            "33333333333333333333333333333333"  # Anchor ID
            "0100000000000000"  # revision
            "0001"  # deleted=false, priority=High
            "02000000"  # UTF-8 byte length, not character count
            "c3a9"
        )
        parsed = measure.parse_ego_payload(payload)
        self.assertIsNotNone(parsed)
        self.assertEqual(parsed["text_fields"][0]["text"], "é")
        self.assertEqual(parsed["text_fields"][0]["bytes"], 2)


if __name__ == "__main__":
    unittest.main(verbosity=2)
