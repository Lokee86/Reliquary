from __future__ import annotations

import os
from pathlib import Path
from typing import Any

from .files import (
    D03Error, canonical_bytes, is_reparse_point, member, parse_json,
    read_bytes, reject_reparse, reject_reparse_chain, sha256,
)
from .manifest import (
    CANDIDATE, CONTRACT_ID, OWNER_FIXTURE_SCHEMA, OWNER_SPECS,
    PREVIOUS, REPORT_SCHEMA, SCHEMA_ID, owner_uuid,
)


def validate_generation(generation: Path, expected_digest: str) -> dict[str, Any]:
    reject_reparse_chain(generation, "generation path")
    if not generation.is_dir():
        raise D03Error("generation directory is missing or aliased")
    manifest_path = generation / "generation-manifest.json"
    reject_reparse(manifest_path, "generation manifest")
    raw = read_bytes(manifest_path, "generation manifest")
    manifest = parse_json(raw)
    if not isinstance(manifest, dict):
        raise D03Error("generation manifest must be a JSON object")
    try:
        canonical_manifest = canonical_bytes(manifest)
    except (TypeError, ValueError, RecursionError) as error:
        raise D03Error(f"generation manifest is not canonicalizable JSON: {error}") from error
    if raw != canonical_manifest + b"\n":
        raise D03Error("generation manifest is not canonical JSON")
    if (
        not isinstance(expected_digest, str)
        or len(expected_digest) != 64
        or any(character not in "0123456789abcdef" for character in expected_digest)
    ):
        raise D03Error("expected manifest digest is missing or invalid")
    if generation.name != manifest.get("generation_id"):
        raise D03Error("generation directory and manifest ID differ")
    if manifest["generation_id"] not in {PREVIOUS, CANDIDATE}:
        raise D03Error("unknown synthetic generation ID")
    expected_keys = {
        "schema_id", "contract_id", "evidence_scope", "generation_id", "runtime",
        "owners", "pairings", "d02_report", "operational_journal_policy",
        "previous_generation_id", "activation_preconditions",
    }
    if set(manifest) != expected_keys:
        raise D03Error("manifest has missing or unknown fields")
    if manifest["schema_id"] != SCHEMA_ID or manifest["contract_id"] != CONTRACT_ID:
        raise D03Error("unsupported manifest or evidence contract")
    if manifest["evidence_scope"] != "synthetic_fixture":
        raise D03Error("only synthetic-fixture manifests are accepted by this harness")
    digest = sha256(canonical_manifest)
    if digest != expected_digest:
        raise D03Error("manifest digest differs from the recorded digest")

    runtime = manifest["runtime"]
    if not isinstance(runtime, dict) or set(runtime) != {"version", "path", "sha256"}:
        raise D03Error("runtime descriptor has missing or unknown fields")
    if (
        runtime["version"] != "2.1"
        or not isinstance(runtime["path"], str)
        or not isinstance(runtime["sha256"], str)
    ):
        raise D03Error("synthetic runtime version/digest is invalid")
    report = manifest["d02_report"]
    if not isinstance(report, dict) or set(report) != {"path", "sha256", "status", "evidence_scope"}:
        raise D03Error("D02 report binding has missing or unknown fields")
    if (
        report["status"] != "PASS"
        or report["evidence_scope"] != "synthetic_fixture"
        or not isinstance(report["path"], str)
        or not isinstance(report["sha256"], str)
    ):
        raise D03Error("only the explicit synthetic D02 report fixture is accepted")

    entries = manifest["owners"]
    if not isinstance(entries, list) or len(entries) != 6:
        raise D03Error("synthetic cohort must contain exactly six owners")
    owner_ids: set[str] = set()
    owner_paths: set[str] = set()
    by_kind: dict[str, list[dict[str, Any]]] = {"REL": [], "PHY": []}
    for entry in entries:
        if not isinstance(entry, dict) or set(entry) != {"owner_uuid", "owner_kind", "scope", "path", "sha256"}:
            raise D03Error("owner descriptor has missing or unknown fields")
        owner_id, kind, scope, relative, file_hash = (
            entry["owner_uuid"], entry["owner_kind"], entry["scope"], entry["path"], entry["sha256"]
        )
        if not isinstance(owner_id, str) or len(owner_id) != 32 or any(c not in "0123456789abcdef" for c in owner_id):
            raise D03Error("owner UUID must be 32 lowercase hexadecimal characters")
        if owner_id in owner_ids:
            raise D03Error("duplicate owner UUID")
        owner_ids.add(owner_id)
        if not isinstance(kind, str) or kind not in by_kind:
            raise D03Error("owner kind is invalid")
        if not isinstance(scope, str) or not isinstance(relative, str) or not isinstance(file_hash, str):
            raise D03Error("owner scope/path/digest must be strings")
        if len(file_hash) != 64 or any(c not in "0123456789abcdef" for c in file_hash):
            raise D03Error("owner file digest is invalid")
        if relative in owner_paths:
            raise D03Error("duplicate owner path")
        owner_paths.add(relative)
        by_kind[kind].append(entry)

    expected_owners = {
        (owner_uuid(kind, index), kind, scope, f"owners/{owner_uuid(kind, index)}.{kind.lower()}.fake.json")
        for kind, scope, index in OWNER_SPECS
    }
    actual_owners = {(row["owner_uuid"], row["owner_kind"], row["scope"], row["path"]) for row in entries}
    if actual_owners != expected_owners:
        raise D03Error("synthetic owner IDs, kinds, scopes, or paths differ from the fixed fixture catalog")
    if len(by_kind["REL"]) != 3 or len(by_kind["PHY"]) != 3:
        raise D03Error("synthetic cohort requires three REL and three PHY fixture owners")
    pairings = manifest["pairings"]
    if not isinstance(pairings, list) or len(pairings) != 3:
        raise D03Error("synthetic fixture requires three explicit REL/PHY pairings")
    expected_pairs = {
        (f"synthetic-pair-{index + 1}", owner_uuid("REL", index), owner_uuid("PHY", index))
        for index in range(3)
    }
    actual_pairs: set[tuple[str, str, str]] = set()
    for pair in pairings:
        if not isinstance(pair, dict) or set(pair) != {"pair_id", "rel_owner_uuid", "phy_owner_uuid"}:
            raise D03Error("pair descriptor has missing or unknown fields")
        if not all(isinstance(pair[key], str) for key in pair):
            raise D03Error("pairing fields must be strings")
        actual_pairs.add((pair["pair_id"], pair["rel_owner_uuid"], pair["phy_owner_uuid"]))
    if actual_pairs != expected_pairs:
        raise D03Error("synthetic REL/PHY pairing map differs from fixture catalog")
    if manifest["previous_generation_id"] not in (None, PREVIOUS) or manifest["generation_id"] == manifest["previous_generation_id"]:
        raise D03Error("previous generation binding is invalid")
    if manifest["operational_journal_policy"] != {
        "unknown_external_outcomes": "held",
        "first_accepted_write_is_rollback_boundary": True,
    }:
        raise D03Error("operational journal policy differs from the safe fixture contract")
    if manifest["activation_preconditions"] != [
        "six synthetic fixture owners validate",
        "runtime and report digests match",
        "no writer admission during rehearsal",
    ]:
        raise D03Error("activation precondition list differs from fixture contract")

    expected_files = {"generation-manifest.json", runtime["path"], report["path"]}
    expected_files.update(entry["path"] for entry in entries)
    actual_files: set[str] = set()
    try:
        items = list(generation.rglob("*"))
    except OSError as error:
        raise D03Error(f"synthetic generation contents could not be listed: {error}") from error
    for item in items:
        if is_reparse_point(item):
            raise D03Error("reparse point in synthetic generation")
        if item.is_file():
            actual_files.add(item.relative_to(generation).as_posix())
    if actual_files != expected_files:
        raise D03Error("generation has missing or extra files")

    members = [member(generation, relative) for relative in sorted(expected_files)]
    for index, left in enumerate(members):
        for right in members[index + 1 :]:
            try:
                if os.path.samefile(left, right):
                    raise D03Error("generation member paths alias the same file")
            except OSError as error:
                raise D03Error(f"generation member identity check failed: {error}") from error
    for entry in entries:
        data = read_bytes(member(generation, entry["path"]), "owner fixture")
        if sha256(data) != entry["sha256"]:
            raise D03Error("owner file digest mismatch")
        owner = parse_json(data)
        if owner != {
            "schema_id": OWNER_FIXTURE_SCHEMA,
            "generation_id": manifest["generation_id"],
            "owner_uuid": entry["owner_uuid"],
            "owner_kind": entry["owner_kind"],
            "scope": entry["scope"],
        }:
            raise D03Error("owner fixture identity/kind/scope does not match manifest")
    runtime_data = read_bytes(member(generation, runtime["path"]), "runtime fixture")
    if sha256(runtime_data) != runtime["sha256"] or not runtime_data.startswith(b"D03 SYNTHETIC RUNTIME FIXTURE ONLY"):
        raise D03Error("runtime fixture hash/type mismatch")
    report_data = read_bytes(member(generation, report["path"]), "D02 report fixture")
    if sha256(report_data) != report["sha256"]:
        raise D03Error("D02 report fixture digest mismatch")
    d02 = parse_json(report_data)
    if d02 != {
        "schema_id": REPORT_SCHEMA,
        "contract_id": CONTRACT_ID,
        "evidence_scope": "synthetic_fixture",
        "status": "PASS",
        "generation_id": manifest["generation_id"],
    }:
        raise D03Error("D02 report fixture is not the bound synthetic PASS example")
    return {"manifest": manifest, "manifest_sha256": digest}
