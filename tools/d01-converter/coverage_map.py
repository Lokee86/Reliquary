"""Fail-closed binding for the provisional synthetic D01 coverage map."""

from __future__ import annotations

from dataclasses import dataclass, field
import hashlib
import json
from pathlib import Path

from pgc01_adapter import PGC01Snapshot


COVERAGE_MAP_SCHEMA = "reliquary-d01-codec-coverage-provisional-v0"
COVERAGE_MAP_STATUS = "provisional-incomplete"
D01_T01_ACCEPTANCE = "not-accepted"
PINNED_COVERAGE_MAP_SHA256 = "2e4a1546bc68054999c044d873c3ddf4fb4ce2382577145547ce3da1cc2823e4"
_EXPECTED_FIXTURE_SCHEMAS = {
    "synthetic.archive_turn.v1": "fixture.archive_turn_revision.v1",
    "synthetic.memory_revision.v1": "fixture.memory_revision.v1",
}
_COVERAGE_MAP_SEAL = object()


class CoverageMapError(RuntimeError):
    """The provisional synthetic coverage map is missing or changed."""

    def __init__(self, error_code: str, message: str) -> None:
        super().__init__(message)
        self.error_code = error_code
        self.message = message


@dataclass(frozen=True)
class ProvisionalCoverageMap:
    schema_id: str
    status: str
    t01_acceptance: str
    sha256: str
    contract_revision: str
    registry_hashes: tuple[tuple[str, str], ...]
    fixture_schemas: tuple[tuple[str, str], ...]
    _seal: object = field(default=None, repr=False, compare=False)

    def target_schema_for(self, family_id: str) -> str | None:
        return dict(self.fixture_schemas).get(family_id)


def is_pinned_coverage_map(value: object) -> bool:
    return (
        isinstance(value, ProvisionalCoverageMap)
        and value._seal is _COVERAGE_MAP_SEAL
        and value.schema_id == COVERAGE_MAP_SCHEMA
        and value.status == COVERAGE_MAP_STATUS
        and value.t01_acceptance == D01_T01_ACCEPTANCE
        and value.sha256 == PINNED_COVERAGE_MAP_SHA256
    )


def is_bound_to_snapshot(
    value: object,
    snapshot: PGC01Snapshot,
) -> bool:
    return (
        is_pinned_coverage_map(value)
        and value.contract_revision == snapshot.revision
        and value.registry_hashes == tuple(sorted(snapshot.registry_hashes.items()))
    )


def _strict_object(pairs: list[tuple[str, object]]) -> dict[str, object]:
    result: dict[str, object] = {}
    for key, value in pairs:
        if key in result:
            raise ValueError("duplicate object key")
        result[key] = value
    return result


def _require(condition: bool, message: str) -> None:
    if not condition:
        raise CoverageMapError("coverage_map_invalid", message)


def load_provisional_coverage_map(
    path: Path,
    snapshot: PGC01Snapshot,
) -> ProvisionalCoverageMap:
    """Load the pinned map bytes and return the exact family mapping they define."""
    try:
        raw = Path(path).read_bytes()
    except FileNotFoundError as error:
        raise CoverageMapError(
            "coverage_map_unavailable", "provisional synthetic coverage map is missing"
        ) from error
    except OSError as error:
        raise CoverageMapError(
            "coverage_map_unavailable", "provisional synthetic coverage map cannot be read"
        ) from error

    digest = hashlib.sha256(raw).hexdigest()
    if digest != PINNED_COVERAGE_MAP_SHA256:
        raise CoverageMapError(
            "coverage_map_changed", "provisional synthetic coverage map differs from its exact pin"
        )
    try:
        document = json.loads(
            raw.decode("utf-8", errors="strict"), object_pairs_hook=_strict_object
        )
    except (UnicodeDecodeError, json.JSONDecodeError, ValueError) as error:
        raise CoverageMapError(
            "coverage_map_invalid", "provisional synthetic coverage map is not strict JSON"
        ) from error
    _require(isinstance(document, dict), "provisional synthetic coverage map must be an object")
    assert isinstance(document, dict)
    _require(
        document.get("schema") == COVERAGE_MAP_SCHEMA
        and document.get("status") == COVERAGE_MAP_STATUS
        and document.get("d01_t01_acceptance") == D01_T01_ACCEPTANCE,
        "coverage map schema, status, or T01 acceptance label is not provisional",
    )

    shared = document.get("shared_contract")
    _require(isinstance(shared, dict), "coverage map is missing its PG-C01 contract binding")
    assert isinstance(shared, dict)
    _require(
        shared.get("pg_c01_status") == "accepted-synthetic-v1"
        and shared.get("pinned_snapshot_at_inspection") == snapshot.revision
        and shared.get("registry_hashes") == dict(snapshot.registry_hashes)
        and shared.get("physical_carrier_registry_status")
        == "unclassified-no-authenticated-inventory"
        and shared.get("physical_carrier_entries") == 0
        and shared.get("unclassified_carrier_entries") == 0
        and shared.get("authentic_admission") is False,
        "coverage map does not match the exact synthetic-only PG-C01 snapshot",
    )

    inventory = document.get("source_inventory")
    _require(isinstance(inventory, dict), "coverage map is missing source-inventory status")
    assert isinstance(inventory, dict)
    _require(
        inventory.get("authenticated_status")
        == "pending-f03-v02-v04-historical-source-witness"
        and inventory.get("admitted_carriers") == [],
        "coverage map must remain source-unadmitted pending F03",
    )

    entries = document.get("synthetic_fixture_map")
    _require(isinstance(entries, list), "coverage map has no synthetic fixture map")
    assert isinstance(entries, list)
    fixture_schemas: dict[str, str] = {}
    for entry in entries:
        _require(isinstance(entry, dict), "synthetic fixture map entry must be an object")
        assert isinstance(entry, dict)
        family = entry.get("d01_fixture_family")
        schema = entry.get("pg_c01_schema")
        _require(
            isinstance(family, str)
            and isinstance(schema, str)
            and family not in fixture_schemas
            and entry.get("fixture_carrier") == "D01SYN01"
            and entry.get("observed_source_carrier") is False,
            "synthetic fixture map entry is duplicated or claims a source carrier",
        )
        fixture_schemas[family] = schema
    _require(
        fixture_schemas == _EXPECTED_FIXTURE_SCHEMAS,
        "synthetic fixture map differs from the pinned closed D01 fixture families",
    )

    command_surface = document.get("command_surface")
    _require(isinstance(command_surface, dict), "coverage map is missing its command binding")
    assert isinstance(command_surface, dict)
    map_contract = command_surface.get("coverage_map_contract")
    _require(
        isinstance(map_contract, dict)
        and map_contract.get("schema_id") == COVERAGE_MAP_SCHEMA
        and map_contract.get("status") == COVERAGE_MAP_STATUS
        and map_contract.get("binding") == "pinned-exact-file-bytes"
        and map_contract.get("digest_field") == "coverage_map_sha256"
        and map_contract.get("missing_error_code") == "coverage_map_unavailable"
        and map_contract.get("changed_error_code") == "coverage_map_changed"
        and map_contract.get("t01_acceptance") == D01_T01_ACCEPTANCE,
        "coverage map binding metadata is inconsistent",
    )
    return ProvisionalCoverageMap(
        schema_id=COVERAGE_MAP_SCHEMA,
        status=COVERAGE_MAP_STATUS,
        t01_acceptance=D01_T01_ACCEPTANCE,
        sha256=digest,
        contract_revision=snapshot.revision,
        registry_hashes=tuple(sorted(snapshot.registry_hashes.items())),
        fixture_schemas=tuple(sorted(fixture_schemas.items())),
        _seal=_COVERAGE_MAP_SEAL,
    )
