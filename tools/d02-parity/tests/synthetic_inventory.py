"""Independent hand-authored synthetic cut inventory and target coverage."""
from __future__ import annotations

import hashlib
import json
from typing import Any

SEMANTIC_SCHEMA_ID = "https://reliquary.invalid/corpus-evidence/semantic-row-v1"
SEMANTIC_SCHEMA_SHA256 = "77499989921a42ff529fb768c1d3215aeb6ed73bbfd0564b2f057bbfdc8c1165"
INVENTORY_TRUST = "hand-authored synthetic test inventory; not authenticated real-source authority"
TARGET_TRUST = "synthetic target observation; not authenticated"

COORDINATE_C1 = {
    "global_version": "12",
    "domain_versions": [
        {"domain_id": "fixture.archive", "version": "6"},
        {"domain_id": "fixture.memory", "version": "4"},
    ],
    "transaction_time": {"state": "present", "value": "9007199254740995"},
    "rel_activity_position": {"state": "present", "value": "11"},
    "accepted_event_boundary": {"state": "present", "value": "7"},
}
COORDINATE_C2 = {
    "global_version": "10",
    "domain_versions": [
        {"domain_id": "fixture.archive", "version": "4"},
        {"domain_id": "fixture.memory", "version": "2"},
    ],
    "transaction_time": {"state": "present", "value": "9007199254740993"},
    "rel_activity_position": {"state": "present", "value": "8"},
    "accepted_event_boundary": {"state": "absent"},
}
COORDINATE_ORIGIN = {
    "global_version": "8",
    "domain_versions": [
        {"domain_id": "fixture.archive", "version": "3"},
        {"domain_id": "fixture.memory", "version": "1"},
    ],
    "transaction_time": {"state": "present", "value": "9007199254740992"},
    "rel_activity_position": {"state": "present", "value": "7"},
    "accepted_event_boundary": {"state": "absent"},
}


def _cut_id(coordinate: dict[str, Any]) -> str:
    encoded = json.dumps(coordinate, ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode("utf-8")
    payload = b"reliquary:corpus-evidence:cut:v1\0" + len(encoded).to_bytes(8, "big") + encoded
    return hashlib.sha256(payload).hexdigest()


C1 = _cut_id(COORDINATE_C1)
C2 = _cut_id(COORDINATE_C2)
assert C1 == "94686d439bd1ba550a933e0b328908864e65ba276615d1efa874129f770c65b4"
assert C2 == "6b77e0a298a88f9fc906a7c6b0d2938994ed3042e3883f404d79b38c9ffc7a2e"
COORDINATES = {C1: COORDINATE_C1, C2: COORDINATE_C2}
FAMILIES = (
    "fixture.archive_turn_revision.v1",
    "fixture.ego_anchor.v1",
    "fixture.freshness_event.v1",
    "fixture.memory_revision.v1",
    "fixture.vector_probe.v1",
)

# These literal identity inventories are authored independently of either JSONL
# witness. Target observations have their own separate literal inventory below.
EXPECTED_OWNER_ROWS = [
    {
        "owner": {"owner_uuid": "10112233445566778899aabbccddeeff", "owner_kind": "PHY", "scope": "unspecified"},
        "cut_ids": [C2],
        "families": {
            "fixture.archive_turn_revision.v1": [],
            "fixture.ego_anchor.v1": [
                {"stable_key": "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee", "revision_or_event": "anchor:1", "ordinal": "1", "cut_ids": [C2]},
            ],
            "fixture.freshness_event.v1": [],
            "fixture.memory_revision.v1": [],
            "fixture.vector_probe.v1": [],
        },
    },
    {
        "owner": {"owner_uuid": "00112233445566778899aabbccddeeff", "owner_kind": "REL", "scope": "project"},
        "cut_ids": [C2, C1],
        "families": {
            "fixture.archive_turn_revision.v1": [],
            "fixture.ego_anchor.v1": [],
            "fixture.freshness_event.v1": [],
            "fixture.memory_revision.v1": [
                {"stable_key": "77777777777777777777777777777777", "revision_or_event": "revision-1", "ordinal": "1", "cut_ids": [C2, C1]},
            ],
            "fixture.vector_probe.v1": [
                {"stable_key": "33333333333333333333333333333333", "revision_or_event": "probe:1", "ordinal": "1", "cut_ids": [C1]},
            ],
        },
    },
]

TARGET_COVERAGE_OWNER_ROWS = [
    {
        "owner": {"owner_uuid": "10112233445566778899aabbccddeeff", "owner_kind": "PHY", "scope": "unspecified"},
        "cut_ids": [C2],
        "families": {
            "fixture.archive_turn_revision.v1": [],
            "fixture.ego_anchor.v1": [
                {"stable_key": "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee", "revision_or_event": "anchor:1", "ordinal": "1", "cut_ids": [C2]},
            ],
            "fixture.freshness_event.v1": [],
            "fixture.memory_revision.v1": [],
            "fixture.vector_probe.v1": [],
        },
    },
    {
        "owner": {"owner_uuid": "00112233445566778899aabbccddeeff", "owner_kind": "REL", "scope": "project"},
        "cut_ids": [C2, C1],
        "families": {
            "fixture.archive_turn_revision.v1": [],
            "fixture.ego_anchor.v1": [],
            "fixture.freshness_event.v1": [],
            "fixture.memory_revision.v1": [
                {"stable_key": "77777777777777777777777777777777", "revision_or_event": "revision-1", "ordinal": "1", "cut_ids": [C2, C1]},
            ],
            "fixture.vector_probe.v1": [
                {"stable_key": "33333333333333333333333333333333", "revision_or_event": "probe:1", "ordinal": "1", "cut_ids": [C1]},
            ],
        },
    },
]


def _owners(entries: list[dict[str, Any]]) -> list[dict[str, Any]]:
    output = []
    for entry in entries:
        cut_ids = entry["cut_ids"]
        cuts = [{"cut_id": cut_id, "coordinate": COORDINATES[cut_id]} for cut_id in cut_ids]
        family_docs = []
        for family in FAMILIES:
            rows = sorted(
                entry["families"][family],
                key=lambda row: (
                    row["stable_key"].encode("utf-8"),
                    row["revision_or_event"].encode("utf-8"),
                    int(row["ordinal"]),
                ),
            )
            cut_docs = []
            for cut_id in cut_ids:
                count = sum(cut_id in row["cut_ids"] for row in rows)
                presence = "supported" if count else "absent"
                cut_docs.append({
                    "cut_id": cut_id,
                    "presence": presence,
                    "completeness": "exhaustive",
                    "row_count": str(count),
                    "reason": None if count else "no hand-authored synthetic rows at this family/cut",
                })
            family_docs.append({
                "family": family,
                "group_id": {
                    "fixture.archive_turn_revision.v1": "archive_echo",
                    "fixture.ego_anchor.v1": "ego",
                    "fixture.freshness_event.v1": "freshness",
                    "fixture.memory_revision.v1": "memory",
                    "fixture.vector_probe.v1": "vectors",
                }[family],
                "presence": "supported" if rows else "absent",
                "completeness": "exhaustive",
                "row_count": str(len(rows)),
                "cuts": cut_docs,
                "rows": rows,
            })
        output.append({"owner": entry["owner"], "cuts": cuts, "families": family_docs})
    return output


def _document(schema_id: str, trust_note: str, entries: list[dict[str, Any]]) -> bytes:
    document = {
        "schema_id": schema_id,
        "evidence_scope": "synthetic_fixture_only",
        "semantic_schema_id": SEMANTIC_SCHEMA_ID,
        "semantic_schema_sha256": SEMANTIC_SCHEMA_SHA256,
        "trust_note": trust_note,
        "owners": _owners(entries),
    }
    return (json.dumps(document, ensure_ascii=False, sort_keys=True, separators=(",", ":")) + "\n").encode("utf-8")


EXPECTED_INVENTORY_BYTES = _document(
    "reliquary-d02-synthetic-inventory-v1", INVENTORY_TRUST, EXPECTED_OWNER_ROWS
)
TARGET_COVERAGE_BYTES = _document(
    "reliquary-d02-synthetic-target-coverage-v1", TARGET_TRUST, TARGET_COVERAGE_OWNER_ROWS
)
