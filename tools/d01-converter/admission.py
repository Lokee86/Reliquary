"""Typed classification gates. No source decoder or authentic admission exists here."""

from __future__ import annotations

from dataclasses import dataclass
from enum import Enum

from pgc01_adapter import PGC01Snapshot
from synthetic import parse_frame


class ClassificationKind(str, Enum):
    SYNTHETIC_FIXTURE_ONLY = "synthetic_fixture_only"
    UNCLASSIFIED_SOURCE_MARKER = "unclassified_source_marker"


@dataclass(frozen=True)
class Classification:
    kind: ClassificationKind
    identifier: str
    source_admissible: bool = False


class AdmissionDenied(RuntimeError):
    """The current provisional contract cannot authorize source conversion."""


def classify_synthetic_frame(data: bytes) -> Classification:
    """Parse the isolated D01SYN01 test envelope and label it synthetic-only."""
    frame = parse_frame(data)
    return Classification(
        kind=ClassificationKind.SYNTHETIC_FIXTURE_ONLY,
        identifier=frame.family,
    )


def classify_source_marker(marker: str, snapshot: PGC01Snapshot) -> Classification:
    """Treat a caller-supplied marker as unknown; never scan a source file."""
    if not isinstance(marker, str) or not marker or len(marker.encode("utf-8")) > 128:
        raise ValueError("marker must be a non-empty bounded string")
    if snapshot.carriers.get("entries") != [] or snapshot.authentic_admission:
        raise AdmissionDenied("the pinned PG-C01 map is expected to remain empty and provisional")
    return Classification(
        kind=ClassificationKind.UNCLASSIFIED_SOURCE_MARKER,
        identifier=marker,
    )


def require_source_admission(classification: Classification, snapshot: PGC01Snapshot) -> None:
    """Fail closed for both synthetic frames and unknown source markers."""
    if classification.kind is ClassificationKind.SYNTHETIC_FIXTURE_ONLY:
        raise AdmissionDenied("synthetic fixtures cannot be admitted as source data")
    if classification.kind is not ClassificationKind.UNCLASSIFIED_SOURCE_MARKER:
        raise AdmissionDenied("unsupported classification cannot enter source admission")
    if not snapshot.authentic_admission:
        raise AdmissionDenied(
            "authentic source admission is closed: PG-C01 carrier inventory is empty and provisional"
        )
    raise AdmissionDenied("D01 authentic admission requires a separately accepted F03 source inventory")