"""Typed synthetic carrier classification and fail-closed source admission."""

from __future__ import annotations

from dataclasses import dataclass, field
from enum import Enum

from coverage_map import ProvisionalCoverageMap, is_bound_to_snapshot
from pgc01_adapter import PGC01Snapshot
from synthetic import parse_frame


# These are test-envelope to shared synthetic-schema mappings only.
# They are not observations of physical source carriers.
_CLASSIFICATION_SEAL = object()

class CarrierKind(str, Enum):
    SYNTHETIC_FIXTURE = "synthetic_fixture"
    UNCLASSIFIED_SOURCE_MARKER = "unclassified_source_marker"


class AdmissionScope(str, Enum):
    SYNTHETIC_FIXTURE_TEST = "synthetic_fixture"
    AUTHENTIC_SOURCE = "authentic_source"


@dataclass(frozen=True)
class CarrierClassification:
    kind: CarrierKind
    marker: str
    family_id: str | None
    target_schema_id: str | None
    carrier_version: int | None
    evidence_scope: str
    source_admissible: bool = False
    _seal: object = field(default=None, repr=False, compare=False)


@dataclass(frozen=True)
class AdmissionGrant:
    scope: AdmissionScope
    allowed_for_synthetic_tests: bool
    authentic_source_admission: bool
    family_id: str
    target_schema_id: str
    contract_revision: str
    contract_bundle_sha256: str


class AdmissionDenied(RuntimeError):
    """The requested input or scope cannot pass this typed gate."""


def _record_schema(snapshot: PGC01Snapshot, schema_id: str) -> dict[str, object]:
    if snapshot.records.get("status") != "synthetic-fixture-only":
        raise AdmissionDenied("PG-C01 fixture schemas are not in synthetic-only state")
    if snapshot.records.get("authentic_admission") is not False:
        raise AdmissionDenied("PG-C01 fixture registry must not admit authentic sources")
    for record in snapshot.records.get("records", []):
        if isinstance(record, dict) and record.get("schema_id") == schema_id:
            if record.get("semantic_family") is True and record.get("requires_source_binding") is True:
                return record
            break
    raise AdmissionDenied("the mapped target schema is not an admissible PG-C01 fixture family")


def classify_synthetic_frame(
    data: bytes,
    snapshot: PGC01Snapshot,
    coverage_map: ProvisionalCoverageMap,
) -> CarrierClassification:
    """Parse only D01SYN01 using the exact-byte-pinned provisional fixture map."""
    if not is_bound_to_snapshot(coverage_map, snapshot):
        raise AdmissionDenied("coverage map is not pinned to the supplied PG-C01 snapshot")
    frame = parse_frame(data)
    target_schema_id = coverage_map.target_schema_for(frame.family)
    if target_schema_id is None:
        raise AdmissionDenied("synthetic family has no explicit fixture-schema mapping")
    _record_schema(snapshot, target_schema_id)
    return CarrierClassification(
        kind=CarrierKind.SYNTHETIC_FIXTURE,
        marker="D01SYN01",
        family_id=frame.family,
        target_schema_id=target_schema_id,
        carrier_version=1,
        evidence_scope="synthetic_fixture",
        _seal=_CLASSIFICATION_SEAL,
    )


def classify_source_marker(marker: str, snapshot: PGC01Snapshot) -> CarrierClassification:
    """Record only a caller-supplied bounded marker; never scans source files or bytes."""
    if not isinstance(marker, str) or not marker:
        raise ValueError("marker must be a non-empty bounded string")
    try:
        marker_bytes = marker.encode("utf-8", errors="strict")
    except UnicodeEncodeError as error:
        raise ValueError("marker must be valid UTF-8") from error
    if len(marker_bytes) > 128:
        raise ValueError("marker exceeds the synthetic classification bound")
    if snapshot.carriers.get("entries") != [] or snapshot.authentic_admission:
        raise AdmissionDenied("the pinned PG-C01 carrier map must remain empty and closed")
    return CarrierClassification(
        kind=CarrierKind.UNCLASSIFIED_SOURCE_MARKER,
        marker=marker,
        family_id=None,
        target_schema_id=None,
        carrier_version=None,
        evidence_scope="unclassified",
    )


def admit_for_scope(
    classification: CarrierClassification,
    snapshot: PGC01Snapshot,
    scope: AdmissionScope,
    coverage_map: ProvisionalCoverageMap | None = None,
) -> AdmissionGrant:
    """Grant fixture-test use only; authentic source admission is intentionally absent."""
    if scope is AdmissionScope.AUTHENTIC_SOURCE:
        raise AdmissionDenied(
            "authentic admission requires F03-V04 and an accepted source inventory; T01 has no source reader"
        )
    if scope is not AdmissionScope.SYNTHETIC_FIXTURE_TEST:
        raise AdmissionDenied("unsupported admission scope")
    if classification.kind is not CarrierKind.SYNTHETIC_FIXTURE:
        raise AdmissionDenied("unclassified source markers cannot enter the synthetic fixture path")
    if classification._seal is not _CLASSIFICATION_SEAL:
        raise AdmissionDenied("classification was not produced by the closed D01 fixture parser")
    if not is_bound_to_snapshot(coverage_map, snapshot):
        raise AdmissionDenied("coverage map is not pinned to the supplied PG-C01 snapshot")
    if (
        classification.source_admissible
        or classification.evidence_scope != "synthetic_fixture"
        or classification.marker != "D01SYN01"
        or classification.carrier_version != 1
        or classification.family_id is None
        or coverage_map.target_schema_for(classification.family_id) != classification.target_schema_id
    ):
        raise AdmissionDenied("synthetic classification does not match the closed D01 fixture map")
    _record_schema(snapshot, classification.target_schema_id)
    return AdmissionGrant(
        scope=AdmissionScope.SYNTHETIC_FIXTURE_TEST,
        allowed_for_synthetic_tests=True,
        authentic_source_admission=False,
        family_id=classification.family_id,
        target_schema_id=classification.target_schema_id,
        contract_revision=snapshot.revision,
        contract_bundle_sha256=snapshot.registry_hashes["contract_bundle"],
    )


def require_source_admission(
    classification: CarrierClassification,
    snapshot: PGC01Snapshot,
) -> None:
    """Compatibility guard for source-mode callers; this T01 lane always refuses."""
    admit_for_scope(classification, snapshot, AdmissionScope.AUTHENTIC_SOURCE)