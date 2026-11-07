"""Read custody-bound retained record commitments from an F03 v2 source witness."""
from __future__ import annotations

import hashlib
from pathlib import Path
import re

from archive_binding_evidence import hex_digest
from evidence_io import WitnessInputError, strict_json_document

SCHEMA = "reliquary-f03-historical-state-v2"
ROW_FIELDS = {
    "projection_domain", "family", "stable_key_sha256", "canonical_value_sha256",
    "canonical_byte_length", "origin_coordinate", "availability",
}


def require(condition: bool, message: str) -> None:
    if not condition:
        raise WitnessInputError(message)


def unsigned(value: str) -> int:
    require(isinstance(value, str) and bool(re.fullmatch(r"0|[1-9][0-9]{0,19}", value)),
            "noncanonical unsigned decimal in F03 evidence")
    return int(value)


def pinned_document(path: Path, digest: str, limit: int) -> dict:
    hex_digest(digest, 64, "artifact SHA-256")
    with path.open("rb") as file:
        raw = file.read(limit + 1)
    require(0 < len(raw) <= limit and hashlib.sha256(raw).hexdigest() == digest,
            f"source artifact size or SHA-256 differs for {path.name}")
    return strict_json_document(raw, path.name)


def read_source(coverage_path: Path, coverage_digest: str, manifest_digest: str, source_digest: str):
    hex_digest(source_digest, 64, "source owner SHA-256")
    coverage = pinned_document(coverage_path, coverage_digest, 2 * 1024 * 1024)
    manifest = pinned_document(coverage_path.with_name("source-witness-manifest.json"),
                               manifest_digest, 2 * 1024 * 1024)
    require(coverage.get("schema_id") == manifest.get("schema_id") == SCHEMA
            and manifest.get("source_coverage_sha256") == coverage_digest,
            "source coverage and manifest provenance differ")
    copies = [copy for copy in coverage["sources"] if copy["source_sha256"] == source_digest]
    require(len(copies) == 1 and bool(re.fullmatch(r"copy-[0-9]{4}", copies[0]["copy_id"])),
            "source hash does not identify one F03 owner")
    source = copies[0]
    header = source["header_observation"]
    owner = {
        "kind": header["owner_kind_observed_from_header"],
        "owner_uuid": source["owner_uuid"],
        "scope": header["native_scope_observed_from_header"] or "unspecified",
    }
    require(owner["kind"] in ("REL", "PHY") and owner["scope"] in (
        "unspecified", "organization", "project", "connection"), "invalid F03 owner kind or scope")
    hex_digest(owner["owner_uuid"], 32, "source owner UUID")
    return source, owner, manifest["artifact_sha256"]


def retained_records(directory: Path, source: dict, artifacts: dict):
    return witness_batches(directory, source, artifacts, "records", source["native"]["record_stream"])


def witness_batches(directory: Path, source: dict, artifacts: dict, stream: str, expected: dict):
    predecessor, ordinal = "0" * 64, 0
    for index in range(unsigned(expected["batches"])):
        name = f'{source["copy_id"]}-{stream}-{index:06}.json'
        digest = artifacts[name]
        batch = pinned_document(directory / name, digest, 16 * 1024 * 1024)
        rows = batch["rows"]
        require(isinstance(rows, list) and 0 < len(rows) <= 128 and batch == {
            "schema_id": SCHEMA, "copy_id": source["copy_id"], "source_sha256": source["source_sha256"],
            "stream": stream, "batch_index": str(index), "previous_sha256": predecessor,
            "first_ordinal": str(ordinal), "rows": rows,
        }, f"invalid F03 batch chain at {name}")
        yield from rows
        ordinal += len(rows)
        predecessor = digest
    require(ordinal == unsigned(expected["rows"]) and predecessor == expected["last_batch_sha256"],
            "F03 native stream population differs")


def index_rows(rows, domain: str, families: set[str]) -> dict:
    indexed = {}
    for row in rows:
        require(isinstance(row, dict) and row.keys() == ROW_FIELDS
                and row["projection_domain"] == domain and row["family"] in families
                and isinstance(row["origin_coordinate"], dict)
                and row["availability"] in ("retained", "tombstoned", "pruned", "inert")
                and unsigned(row["canonical_byte_length"]) > 0, "invalid retained commitment")
        key = row["family"], hex_digest(row["stable_key_sha256"], 64, "stable key SHA-256")
        hex_digest(row["canonical_value_sha256"], 64, "canonical value SHA-256")
        require(key not in indexed, "duplicate retained commitment")
        indexed[key] = row
    return indexed
