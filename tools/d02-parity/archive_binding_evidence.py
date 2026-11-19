"""Canonical Archive publication binding evidence for the independent checker."""
from __future__ import annotations

import hashlib
import json
import re

from evidence_io import WitnessInputError

EXPECTATION_SCHEMA = "reliquary-archive-binding-expectation-v1"
READER_SCHEMA = "reliquary-d02-archive-binding-cut-observation-v1"
PROJECTION_DOMAIN = b"reliquary:d02:archive-publication-bindings:v1\0"
SCOPES = {"unspecified", "organization", "project", "connection"}
ROW_FIELDS = {
    "archive_version", "global_version", "transaction_time_ns", "clock_mode",
    "first_applying_archive_version", "first_applying_global_version",
}


def hex_digest(value: object, length: int, label: str) -> str:
    if not isinstance(value, str) or len(value) != length or not re.fullmatch("[0-9a-f]+", value):
        raise WitnessInputError(f"invalid {label}")
    return value


def version(value: object, label: str) -> int:
    return integer(value, 0, (1 << 64) - 1, label)


def integer(value: object, minimum: int, maximum: int, label: str) -> int:
    if not isinstance(value, str) or len(value) > 20 or not re.fullmatch(r"0|-?[1-9][0-9]*", value):
        raise WitnessInputError(f"{label} is not a canonical decimal string")
    number = int(value)
    if not minimum <= number <= maximum:
        raise WitnessInputError(f"{label} is outside the supported integer range")
    return number


def binding_projection_digest(projection: dict) -> str:
    encoded = json.dumps(projection, ensure_ascii=False, separators=(",", ":"), sort_keys=True)
    return hashlib.sha256(PROJECTION_DOMAIN + encoded.encode("utf-8")).hexdigest()


def validate_binding_projection(projection: object) -> str:
    if not isinstance(projection, dict) or set(projection) != {"archive_version", "bindings"}:
        raise WitnessInputError("invalid Archive binding projection shape")
    cut = version(projection["archive_version"], "Archive cut")
    rows = projection["bindings"]
    if not isinstance(rows, list) or len(rows) != cut:
        raise WitnessInputError("Archive bindings do not cover the complete local cut")
    globals_seen = set()
    for local, row in enumerate(rows, 1):
        if not isinstance(row, dict) or set(row) != ROW_FIELDS:
            raise WitnessInputError(f"invalid Archive binding row shape at local version {local}")
        if version(row["archive_version"], "Archive local version") != local:
            raise WitnessInputError(f"Archive bindings are not in local order at version {local}")
        global_version = version(row["global_version"], "global version")
        if not global_version or global_version in globals_seen:
            raise WitnessInputError(f"invalid or reused Archive global version at local version {local}")
        globals_seen.add(global_version)
        time = row["transaction_time_ns"]
        if time is not None:
            integer(time, -(1 << 63), (1 << 63) - 1, "transaction time")
        if row["clock_mode"] not in ("allocate", "bind_reserved"):
            raise WitnessInputError(f"unknown Archive clock mode at local version {local}")
        first = version(row["first_applying_archive_version"], "first-applying Archive version")
        first_global = version(row["first_applying_global_version"], "first-applying global version")
        if not 1 <= first <= local:
            raise WitnessInputError(f"invalid Archive first-applying version at local version {local}")
        origin = rows[first - 1]
        if origin["first_applying_archive_version"] != str(first) or origin["global_version"] != str(first_global):
            raise WitnessInputError(f"Archive first-applying binding does not resolve at local version {local}")
    return binding_projection_digest(projection)


def validate_expectation(document: dict) -> str:
    if set(document) != {"schema_id", "source_file_sha256", "owner", "binding_projection"}:
        raise WitnessInputError("invalid Archive binding expectation shape")
    if document["schema_id"] != EXPECTATION_SCHEMA:
        raise WitnessInputError("unknown Archive binding expectation schema")
    hex_digest(document["source_file_sha256"], 64, "source file SHA-256")
    owner = document["owner"]
    if not isinstance(owner, dict) or set(owner) != {"kind", "owner_uuid", "scope"}:
        raise WitnessInputError("invalid expected Archive owner shape")
    if owner["kind"] != "REL" or not isinstance(owner["scope"], str) or owner["scope"] not in SCOPES:
        raise WitnessInputError("invalid expected Archive owner kind or scope")
    hex_digest(owner["owner_uuid"], 32, "Archive owner UUID")
    return validate_binding_projection(document["binding_projection"])
