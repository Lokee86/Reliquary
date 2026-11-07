"""Temporary synthetic-only no-replace publication test seam, not a converter writer.

This models a single-file create-only boundary. It does not perform directory fsync,
checkpoint/resume, owner admission, durability fault recovery, or T04 acceptance.
"""

from __future__ import annotations

from dataclasses import dataclass
import hashlib
import os
from pathlib import Path
import re
import tempfile

from admission import AdmissionScope, admit_for_scope, classify_synthetic_frame
from coverage_map import ProvisionalCoverageMap
from pgc01_adapter import PGC01Snapshot


class CandidateAlreadyExists(FileExistsError):
    """The synthetic test destination already exists and was not overwritten."""


class SyntheticPublicationError(RuntimeError):
    """The temporary synthetic artifact could not be published create-only."""


@dataclass(frozen=True)
class SyntheticArtifactReceipt:
    file_name: str
    sha256: str
    byte_count: int
    semantic_family: str
    target_schema_id: str
    state: str = "synthetic-test-artifact"


_FILE_NAME = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._-]{0,63}$")


def create_only_synthetic_artifact(
    workspace: tempfile.TemporaryDirectory,
    file_name: str,
    data: bytes,
    snapshot: PGC01Snapshot,
    coverage_map: ProvisionalCoverageMap,
) -> SyntheticArtifactReceipt:
    """Write one verified D01SYN01 fixture under a TemporaryDirectory with no replacement."""
    if isinstance(workspace, tempfile.TemporaryDirectory):
        workspace_path = workspace.name
    elif isinstance(workspace, (str, os.PathLike)):
        workspace_path = os.fspath(workspace)
    else:
        raise TypeError("synthetic output requires a temporary directory path")
    if not isinstance(file_name, str) or not _FILE_NAME.fullmatch(file_name):
        raise ValueError("file_name must be one safe leaf name")
    root = Path(workspace_path).resolve(strict=True)
    allowed_temp_root = Path(tempfile.gettempdir()).resolve(strict=True)
    try:
        root.relative_to(allowed_temp_root)
    except ValueError as error:
        raise ValueError("synthetic output must stay under the system temporary directory") from error
    if not root.is_dir():
        raise ValueError("synthetic output root must be a directory")

    classification = classify_synthetic_frame(data, snapshot, coverage_map)
    grant = admit_for_scope(
        classification,
        snapshot,
        AdmissionScope.SYNTHETIC_FIXTURE_TEST,
        coverage_map,
    )
    if not grant.allowed_for_synthetic_tests or grant.authentic_source_admission:
        raise SyntheticPublicationError("synthetic fixture gate returned an unsafe grant")

    destination = root / file_name
    if destination.exists() or destination.is_symlink():
        raise CandidateAlreadyExists(f"synthetic destination already exists: {file_name}")

    descriptor, staging_name = tempfile.mkstemp(
        prefix=".d01-synthetic-stage-",
        suffix=".pending",
        dir=root,
    )
    staging = Path(staging_name)
    try:
        with os.fdopen(descriptor, "wb") as stream:
            stream.write(data)
            stream.flush()
            os.fsync(stream.fileno())
        try:
            # Same-directory hard-link creation is atomic and fails if destination exists.
            os.link(staging, destination)
        except FileExistsError as error:
            raise CandidateAlreadyExists(
                f"synthetic destination was created concurrently: {file_name}"
            ) from error
        except OSError as error:
            raise SyntheticPublicationError(
                "atomic no-replace link is unavailable; no weaker fallback is allowed"
            ) from error
    finally:
        try:
            staging.unlink()
        except FileNotFoundError:
            pass

    return SyntheticArtifactReceipt(
        file_name=file_name,
        sha256=hashlib.sha256(data).hexdigest(),
        byte_count=len(data),
        semantic_family=classification.family_id,
        target_schema_id=classification.target_schema_id,
    )