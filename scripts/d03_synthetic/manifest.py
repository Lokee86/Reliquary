from __future__ import annotations

import uuid
from pathlib import Path
from typing import Any

from .files import D03Error, canonical_bytes, make_directory, sha256, write_new

CONTRACT_ID = "reliquary-corpus-evidence-v1"
SCHEMA_ID = "reliquary-d03-synthetic-generation-manifest-v1"
OWNER_FIXTURE_SCHEMA = "reliquary-d03-synthetic-owner-fixture-v1"
REPORT_SCHEMA = "reliquary-d03-synthetic-d02-report-v1"
PREVIOUS = "d03-synthetic-previous"
CANDIDATE = "d03-synthetic-candidate"
OWNER_SPECS = (
    ("REL", "project", 0),
    ("REL", "organization", 1),
    ("REL", "connection", 2),
    ("PHY", "unspecified", 0),
    ("PHY", "unspecified", 1),
    ("PHY", "unspecified", 2),
)


def owner_uuid(kind: str, index: int) -> str:
    return uuid.uuid5(uuid.NAMESPACE_URL, f"d03-synthetic-owner:{kind}:{index}").hex


def _owner_path(owner_id: str, kind: str) -> str:
    return f"owners/{owner_id}.{kind.lower()}.fake.json"


def create_generation(path: Path, generation_id: str, previous_id: str | None) -> str:
    if path.exists():
        raise D03Error("generation destination already exists")
    make_directory(path)
    make_directory(path / "owners")
    make_directory(path / "runtime")
    make_directory(path / "evidence")

    runtime_bytes = b"D03 SYNTHETIC RUNTIME FIXTURE ONLY\nformat=2.1\n"
    runtime_path = "runtime/runtime.fake"
    write_new(path / runtime_path, runtime_bytes)
    report = {
        "schema_id": REPORT_SCHEMA,
        "contract_id": CONTRACT_ID,
        "evidence_scope": "synthetic_fixture",
        "status": "PASS",
        "generation_id": generation_id,
    }
    report_bytes = canonical_bytes(report) + b"\n"
    report_path = "evidence/d02-report.fake.json"
    write_new(path / report_path, report_bytes)

    owners: list[dict[str, str]] = []
    rel_ids: list[str] = []
    phy_ids: list[str] = []
    for kind, scope, index in OWNER_SPECS:
        owner_id = owner_uuid(kind, index)
        owner_path = _owner_path(owner_id, kind)
        payload = {
            "schema_id": OWNER_FIXTURE_SCHEMA,
            "generation_id": generation_id,
            "owner_uuid": owner_id,
            "owner_kind": kind,
            "scope": scope,
        }
        owner_bytes = canonical_bytes(payload) + b"\n"
        write_new(path / owner_path, owner_bytes)
        owners.append(
            {
                "owner_uuid": owner_id,
                "owner_kind": kind,
                "scope": scope,
                "path": owner_path,
                "sha256": sha256(owner_bytes),
            }
        )
        (rel_ids if kind == "REL" else phy_ids).append(owner_id)

    pairings = [
        {
            "pair_id": f"synthetic-pair-{index + 1}",
            "rel_owner_uuid": rel_ids[index],
            "phy_owner_uuid": phy_ids[index],
        }
        for index in range(3)
    ]
    manifest: dict[str, Any] = {
        "schema_id": SCHEMA_ID,
        "contract_id": CONTRACT_ID,
        "evidence_scope": "synthetic_fixture",
        "generation_id": generation_id,
        "runtime": {"version": "2.1", "path": runtime_path, "sha256": sha256(runtime_bytes)},
        "owners": owners,
        "pairings": pairings,
        "d02_report": {
            "path": report_path,
            "sha256": sha256(report_bytes),
            "status": "PASS",
            "evidence_scope": "synthetic_fixture",
        },
        "operational_journal_policy": {
            "unknown_external_outcomes": "held",
            "first_accepted_write_is_rollback_boundary": True,
        },
        "previous_generation_id": previous_id,
        "activation_preconditions": [
            "six synthetic fixture owners validate",
            "runtime and report digests match",
            "no writer admission during rehearsal",
        ],
    }
    write_new(path / "generation-manifest.json", canonical_bytes(manifest) + b"\n")
    return sha256(canonical_bytes(manifest))
