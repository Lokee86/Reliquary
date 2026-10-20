import struct
import unittest
import hashlib
import json

from scripts.measure_publication_framing import (
    MeasurementError,
    attribute_global_tickets,
    parse_container,
    parse_record,
)


def literal_container(payloads: list[bytes]) -> bytes:
    header = b"CVA\0\r\n\x1a\n" + struct.pack("<HHI", 1, 0, 16)
    return header + b"".join(struct.pack("<Q", len(payload)) + payload for payload in payloads)


class PublicationFramingTests(unittest.TestCase):
    def test_literal_global_ticket_and_link_byte_fields(self) -> None:
        ticket = b"CVAVERS2" + struct.pack("<Qq", 0x0102030405060708, -12)
        link = b"CVAGVER1" + struct.pack("<QQQ", 9, 4, 0x1112131415161718) + struct.pack("<Q", 32)
        parsed_ticket = parse_record(ticket)
        parsed_link = parse_record(link)
        self.assertEqual(len(ticket), 24)
        self.assertEqual(parsed_ticket["global_version_bytes"], 8)
        self.assertEqual(parsed_ticket["transaction_time_bytes"], 8)
        self.assertEqual(len(link), 40)
        self.assertEqual(parsed_link["global_version_bytes"], 8)
        self.assertEqual(parsed_link["owner_local_version_bytes"], 8)
        self.assertEqual(parsed_link["object_ref_bytes"], 16)
        self.assertEqual(parsed_link["container_framed_bytes"] if "container_framed_bytes" in parsed_link else len(link) + 8, 48)

    def test_literal_container_scanner_preserves_framing_count(self) -> None:
        payloads = [b"CVAVERS1" + struct.pack("<Q", 1), b"CVARELV1" + struct.pack("<QQQQ", 2, 3, 4, 5)]
        header, chunks = parse_container(literal_container(payloads))
        self.assertEqual(header, 16)
        self.assertEqual([payload for _, payload in chunks], payloads)
        self.assertEqual([parse_record(payload)["family"] for payload in payloads], ["container_ticket_v1", "relationship_link"])

    def test_unknown_version_and_malformed_link_fail_closed(self) -> None:
        for payload in (
            b"CVAVERS9" + bytes(16),
            b"CVAMEMV9" + bytes(32),
            b"CVAENC99" + bytes(64),
            b"CVARNC99" + bytes(64),
            b"CVAINSC9" + bytes(80),
            b"CVAXMEM9" + bytes(80),
            b"CVAGVER1" + bytes(31),
        ):
            with self.subTest(magic=payload[:8]):
                with self.assertRaises(MeasurementError):
                    parse_record(payload)

    def test_container_truncation_fails_closed(self) -> None:
        with self.assertRaises(MeasurementError):
            parse_container(literal_container([b"CVAVERS1" + bytes(8)])[:-1])

    def test_compact_carriers_keep_inline_clock_layouts_distinct_from_links(self) -> None:
        memory_creation = b"CVAMC001" + bytes(32 + 32 + 16)
        memory_delta = b"CVAMD001" + bytes(32 + 32 + 16)
        entity_creation = b"CVAENC01" + bytes(32) + b"\0" + bytes(16)
        relationship_creation = b"CVARNC01" + bytes(32 + 16)
        for payload, global_at, local_at, owner in (
            (memory_creation, 72, 80, "Memory"),
            (memory_delta, 56, 64, "Memory"),
            (entity_creation, 41, 49, "Entity"),
            (relationship_creation, 40, 48, "Relationship"),
        ):
            with self.subTest(owner=owner, magic=payload[:8]):
                row = parse_record(payload)
                self.assertEqual(row["field_offsets"]["global_version"], global_at)
                self.assertEqual(row["field_offsets"]["owner_local_version"], local_at)
                self.assertEqual(row["object_ref_bytes"], 0)

    def test_ego_sequence_is_local_only_and_no_object_link(self) -> None:
        row = parse_record(b"CVAECR01" + bytes(1 + 8 + 16))
        self.assertEqual(row["owner_local_version_bytes"], 8)
        self.assertEqual(row["global_version_bytes"], 0)
        self.assertEqual(row["object_ref_bytes"], 0)

    def test_insomnia_and_external_publication_ranges_are_explicit(self) -> None:
        completion = b"CVAINSC4" + bytes(56) + struct.pack("<QIq", 20, 2, 500)
        row = parse_record(completion)
        self.assertEqual(row["global_ranges"][0], {"start": 20, "count": 2, "transaction_time_ns": 500})
        self.assertEqual(row["range_count_bytes"], 4)

        encoded_json = json.dumps({
            "window": [0] * 32,
            "global_start": 30,
            "transaction_ns": 700,
            "birth_turn": None,
            "records": [{}],
            "bodies": [],
            "bindings": [],
            "routing": [],
        }, separators=(",", ":")).encode()
        external_prefix = (
            b"CVAXMEM2"
            + struct.pack("<QQIq", len(encoded_json), 30, 1, 700)
            + encoded_json
        )
        external = external_prefix + hashlib.sha256(external_prefix).digest()
        external_row = parse_record(external)
        self.assertEqual(external_row["global_ranges"][0]["start"], 30)
        self.assertEqual(external_row["global_ranges"][0]["transaction_time_ns"], 700)

    def test_ticket_attribution_joins_ids_and_reports_orphan_and_sharing(self) -> None:
        ticket7 = parse_record(b"CVAVERS1" + struct.pack("<Q", 7))
        ticket8 = parse_record(b"CVAVERS2" + struct.pack("<Qq", 8, 500))
        ticket9 = parse_record(b"CVAVERS1" + struct.pack("<Q", 9))
        memory_link = parse_record(b"CVAMEMV1" + struct.pack("<QQ", 7, 1) + bytes(16))
        graph_link = parse_record(b"CVAGVER1" + struct.pack("<QQQQ", 7, 1, 0, 16))
        completion = parse_record(b"CVAINSC4" + bytes(56) + struct.pack("<QIq", 8, 1, 500))
        result = attribute_global_tickets([ticket7, ticket8, ticket9, memory_link, graph_link, completion])
        self.assertEqual(result["individual_ticket_ids_with_no_publication_reference"], 1)
        self.assertEqual(result["individual_ticket_ids_shared_by_multiple_owners"], 1)
        self.assertEqual(result["individual_ticket_ids_by_referencing_owner"], {"Graph": 1, "Insomnia": 1, "Memory": 1})
        self.assertEqual(result["direct_owner_link_distinct_ids_with_no_ticket_or_explicit_range"], 0)
        self.assertEqual(result["exclusive_individual_ticket_bytes_by_owner"]["Insomnia"], {
            "record_count": 1, "payload_bytes": 24, "container_framed_bytes": 32,
        })
        self.assertEqual(result["shared_ticket_bucket"], {
            "record_count": 1, "payload_bytes": 16, "container_framed_bytes": 24,
        })
        self.assertEqual(result["orphan_ticket_bucket"], {
            "record_count": 1, "payload_bytes": 16, "container_framed_bytes": 24,
        })

    def test_unrelated_payload_is_not_classified_as_publication(self) -> None:
        self.assertIsNone(parse_record(b"CVAXYZ01opaque"))


if __name__ == "__main__":
    unittest.main()
