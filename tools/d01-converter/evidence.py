"""PG-C01-compatible INCOMPLETE report skeleton for zero-source synthetic checks."""

from __future__ import annotations

from datetime import datetime, timezone
import hashlib
from pathlib import Path
import subprocess
import sys
from typing import Any

from pgc01_adapter import PGC01Snapshot


_ZERO_SHA256 = "0" * 64
_CODE_FILES = ("admission.py", "evidence.py", "pgc01_adapter.py", "synthetic.py")


def _sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def _code_digest(tool_root: Path) -> str:
    """Hash only the explicitly named D01 Python modules, never source data."""
    digest = hashlib.sha256()
    digest.update(b"reliquary-d01-synthetic-code-v1\0")
    for name in sorted(_CODE_FILES):
        body = (tool_root / name).read_bytes()
        encoded_name = name.encode("utf-8")
        digest.update(len(encoded_name).to_bytes(4, "big"))
        digest.update(encoded_name)
        digest.update(len(body).to_bytes(8, "big"))
        digest.update(body)
    return digest.hexdigest()


def _revision(tool_root: Path) -> str:
    repo_root = tool_root.parents[1]
    result = subprocess.run(
        ["git", "-C", str(repo_root), "rev-parse", "HEAD"],
        check=False,
        capture_output=True,
        text=True,
        encoding="utf-8",
    )
    if result.returncode != 0:
        raise RuntimeError("cannot identify the D01 worktree revision")
    revision = result.stdout.strip()
    if len(revision) != 40 or any(char not in "0123456789abcdef" for char in revision):
        raise RuntimeError("D01 Git revision is not a full lowercase commit ID")
    return revision


def build_incomplete_report(
    snapshot: PGC01Snapshot,
    tool_root: Path,
    *,
    captured_at_utc: str | None = None,
) -> dict[str, Any]:
    """Build and validate a report skeleton with no owner or source evidence."""
    tool_root = Path(tool_root).resolve(strict=True)
    if tool_root.name != "d01-converter" or tool_root.parent.name != "tools":
        raise ValueError("expected tools/d01-converter as the tooling root")
    if snapshot.authentic_admission or snapshot.carriers.get("entries") != []:
        raise RuntimeError("synthetic evidence cannot be built against an admitting registry")

    if captured_at_utc is None:
        captured_at_utc = datetime.now(timezone.utc).replace(microsecond=0).isoformat().replace(
            "+00:00", "Z"
        )
    try:
        captured = datetime.strptime(captured_at_utc, "%Y-%m-%dT%H:%M:%SZ")
    except ValueError as error:
        raise ValueError("captured_at_utc must be a whole-second UTC timestamp") from error

    binding_hash = _sha256((tool_root / "pgc01-binding.provisional.json").read_bytes())
    coverage_hash = _sha256((tool_root / "codec-coverage.provisional.json").read_bytes())
    lock_hash = _sha256((tool_root / "requirements.lock").read_bytes())
    report: dict[str, Any] = {
        "schema_id": "reliquary-corpus-artifact-report-v1",
        "contract_id": "reliquary-corpus-evidence-v1",
        "producer_role": "converter",
        "evidence_scope": "synthetic_fixture",
        "producer_revision": _revision(tool_root),
        "producer_binary_sha256": _ZERO_SHA256,
        "producer_code_sha256": _code_digest(tool_root),
        "validation_patch_sha256": None,
        "lockfile_sha256": lock_hash,
        "toolchain_id": (
            f"python-{sys.version_info.major}.{sys.version_info.minor}.{sys.version_info.micro}; "
            "standard-library fixture harness; converter not executed"
        ),
        "registry_hashes": dict(snapshot.registry_hashes),
        "status": "INCOMPLETE",
        "captured_at_utc": captured_at_utc,
        "reporting_date_local": captured.strftime("%Y-%m-%d"),
        "reporting_timezone": "Etc/UTC",
        "command_argv": ["not executed: zero-source report skeleton only"],
        "exit_status": "-1",
        "source_hashes": [],
        "source_owner_bindings": [],
        "parent_artifact_hashes": sorted({binding_hash, coverage_hash}),
        "coverage": {
            "owner_count": "0",
            "family_count": "0",
            "cut_count": "0",
            "row_count": "0",
            "completeness": "incomplete",
        },
        "diagnostics": {"total_count": "0", "sample_limit": "0", "sample": []},
    }
    snapshot.api.validate_artifact_report(report, snapshot.contract_root)
    return report


def validate_incomplete_report(
    report: dict[str, Any],
    snapshot: PGC01Snapshot,
) -> None:
    """Validate and enforce the D01 zero-source INCOMPLETE boundary."""
    if (
        report.get("producer_role") != "converter"
        or report.get("evidence_scope") != "synthetic_fixture"
        or report.get("status") != "INCOMPLETE"
        or report.get("source_hashes") != []
        or report.get("source_owner_bindings") != []
        or report.get("coverage", {}).get("owner_count") != "0"
        or report.get("coverage", {}).get("completeness") != "incomplete"
    ):
        raise ValueError("D01 synthetic evidence must remain zero-source and INCOMPLETE")
    snapshot.api.validate_artifact_report(report, snapshot.contract_root)