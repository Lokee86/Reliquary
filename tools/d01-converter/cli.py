"""Separate-process synthetic-only D01 command surfaces.

These commands inspect, copy-test, and structurally verify D01SYN01 fixtures.
They are not an FRS07 reader, 2.1 owner writer, or candidate acceptance tool.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import sys
import tempfile
from typing import Any

from admission import (
    AdmissionDenied,
    AdmissionScope,
    admit_for_scope,
    classify_synthetic_frame,
)
from coverage_map import (
    CoverageMapError,
    ProvisionalCoverageMap,
    load_provisional_coverage_map,
)
from evidence import _code_digest
from pgc01_adapter import ContractSnapshotError, PGC01Snapshot, load_pinned_snapshot
from synthetic import SyntheticFrameError, UnsupportedSyntheticFamily
from synthetic_transaction import (
    CandidateAlreadyExists,
    SyntheticPublicationError,
    create_only_synthetic_artifact,
)


_MAX_FRAME_BYTES = 1_049_000
_MAX_RECEIPT_BYTES = 65_536
_SHA256 = re.compile(r"^[0-9a-f]{64}$")
_RECEIPT_SCHEMA = "reliquary-d01-synthetic-command-receipt-v2"


class CliFailure(Exception):
    def __init__(self, error_code: str, message: str, exit_code: int) -> None:
        super().__init__(message)
        self.error_code = error_code
        self.message = message
        self.exit_code = exit_code


class StructuredArgumentParser(argparse.ArgumentParser):
    def error(self, _message: str) -> None:
        raise CliFailure("invalid_arguments", "command arguments are invalid", 2)


def _json_line(value: dict[str, Any]) -> str:
    return json.dumps(value, ensure_ascii=True, sort_keys=True, separators=(",", ":"))


def _sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def _stat_token(path: Path) -> tuple[int, int, int, int, int]:
    info = path.stat()
    return (
        info.st_dev,
        info.st_ino,
        info.st_size,
        info.st_mtime_ns,
        info.st_ctime_ns,
    )


def _temporary_root() -> Path:
    try:
        return Path(tempfile.gettempdir()).resolve(strict=True)
    except OSError as error:
        raise CliFailure("temporary_root_unavailable", "temporary output root is unavailable", 7) from error


def _inside_temporary_directory(path: Path) -> bool:
    try:
        path.relative_to(_temporary_root())
        return True
    except ValueError:
        return False


def _input_path(value: str) -> Path:
    try:
        path = Path(value).expanduser().resolve(strict=True)
    except (OSError, RuntimeError, ValueError) as error:
        raise CliFailure("input_unavailable", "input fixture could not be opened", 7) from error
    if not _inside_temporary_directory(path):
        raise CliFailure("input_outside_temporary_scope", "input must be under the system temporary directory", 3)
    if not path.is_file():
        raise CliFailure("input_unavailable", "input fixture is not a regular file", 7)
    try:
        if path.stat().st_size > _MAX_FRAME_BYTES:
            raise CliFailure("input_too_large", "synthetic fixture exceeds the size bound", 4)
    except OSError as error:
        raise CliFailure("input_unavailable", "input fixture could not be statted", 7) from error
    return path


def _output_path(value: str) -> Path:
    try:
        raw = Path(value).expanduser()
        parent = raw.parent.resolve(strict=True)
        output = parent / raw.name
        resolved = output.resolve(strict=False)
    except (OSError, RuntimeError, ValueError) as error:
        raise CliFailure("output_unavailable", "output location could not be resolved", 7) from error
    if not _inside_temporary_directory(parent) or not _inside_temporary_directory(resolved):
        raise CliFailure("output_outside_temporary_scope", "output must be under the system temporary directory", 3)
    if not parent.is_dir():
        raise CliFailure("output_unavailable", "output parent must already exist", 7)
    return output


def _receipt_path(value: str) -> Path:
    try:
        path = Path(value).expanduser().resolve(strict=True)
    except (OSError, RuntimeError, ValueError) as error:
        raise CliFailure("receipt_unavailable", "receipt file could not be opened", 7) from error
    if not _inside_temporary_directory(path):
        raise CliFailure("receipt_outside_temporary_scope", "receipt must be under the system temporary directory", 3)
    if not path.is_file():
        raise CliFailure("receipt_unavailable", "receipt is not a regular file", 7)
    try:
        if path.stat().st_size > _MAX_RECEIPT_BYTES:
            raise CliFailure("receipt_invalid", "receipt exceeds the size bound", 6)
    except OSError as error:
        raise CliFailure("receipt_unavailable", "receipt could not be statted", 7) from error
    return path


def _read_input(path: Path) -> tuple[bytes, tuple[int, int, int, int, int]]:
    try:
        before = _stat_token(path)
        data = path.read_bytes()
        after = _stat_token(path)
    except OSError as error:
        raise CliFailure("input_unavailable", "input fixture could not be read", 7) from error
    if before != after or len(data) != after[2]:
        raise CliFailure("input_changed", "input fixture changed during inspection", 3)
    if len(data) > _MAX_FRAME_BYTES:
        raise CliFailure("input_too_large", "synthetic fixture exceeds the size bound", 4)
    return data, after


def _snapshot(contract_root: str | None) -> PGC01Snapshot:
    root = contract_root or os.environ.get("RELIQUARY_PG_C01_ROOT")
    if not root:
        raise CliFailure("contract_unavailable", "accepted synthetic PG-C01 snapshot is required", 3)
    try:
        return load_pinned_snapshot(Path(root))
    except (OSError, ContractSnapshotError) as error:
        raise CliFailure("contract_mismatch", "accepted synthetic PG-C01 snapshot verification failed", 3) from error


def _coverage_map(
    snapshot: PGC01Snapshot,
    path: Path | None = None,
) -> ProvisionalCoverageMap:
    if path is None:
        path = Path(__file__).resolve().parent / "codec-coverage.provisional.json"
    try:
        return load_provisional_coverage_map(path, snapshot)
    except CoverageMapError as error:
        raise CliFailure(error.error_code, error.message, 3) from error


def _classify(
    data: bytes,
    snapshot: PGC01Snapshot,
    coverage_map: ProvisionalCoverageMap,
):
    try:
        return classify_synthetic_frame(data, snapshot, coverage_map)
    except UnsupportedSyntheticFamily as error:
        raise CliFailure("unsupported_family", "synthetic family is not in the closed fixture map", 4) from error
    except SyntheticFrameError as error:
        raise CliFailure("unclassified_or_malformed_input", "input is not a valid D01 synthetic fixture", 4) from error
    except AdmissionDenied as error:
        raise CliFailure("admission_closed", "synthetic fixture did not pass the closed contract gate", 3) from error


def _strict_object(pairs: list[tuple[str, object]]) -> dict[str, object]:
    value: dict[str, object] = {}
    for key, item in pairs:
        if key in value:
            raise ValueError("duplicate receipt field")
        value[key] = item
    return value


def _read_receipt(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(
            path.read_text(encoding="utf-8"),
            object_pairs_hook=_strict_object,
        )
    except (OSError, UnicodeError, json.JSONDecodeError, ValueError) as error:
        raise CliFailure("receipt_invalid", "receipt is not strict JSON", 6) from error
    if not isinstance(value, dict):
        raise CliFailure("receipt_invalid", "receipt must be a JSON object", 6)
    return value


def _synthetic_receipt(
    input_sha256: str,
    output_sha256: str,
    byte_count: int,
    output_name: str,
    classification: Any,
    snapshot: PGC01Snapshot,
    coverage_map: ProvisionalCoverageMap,
) -> dict[str, Any]:
    return {
        "schema_id": _RECEIPT_SCHEMA,
        "command": "convert",
        "status": "INCOMPLETE",
        "evidence_scope": "synthetic_fixture",
        "artifact_state": "synthetic-test-artifact",
        "target_candidate": False,
        "authentic_source_admission": False,
        "source_sha256_before": input_sha256,
        "source_sha256_after": input_sha256,
        "output_sha256": output_sha256,
        "byte_count": str(byte_count),
        "output_name": output_name,
        "carrier_marker": classification.marker,
        "carrier_version": str(classification.carrier_version),
        "family_id": classification.family_id,
        "target_schema_id": classification.target_schema_id,
        "contract_revision": snapshot.revision,
        "registry_hashes": dict(snapshot.registry_hashes),
        "coverage_map_schema": coverage_map.schema_id,
        "coverage_map_status": coverage_map.status,
        "coverage_map_sha256": coverage_map.sha256,
        "d01_t01_acceptance": coverage_map.t01_acceptance,
        "producer_code_sha256": _code_digest(Path(__file__).resolve().parent),
        "payload_emitted": False,
        "current_only_owner_reopen": False,
    }


def _inspect(
    args: argparse.Namespace,
    snapshot: PGC01Snapshot,
    coverage_map: ProvisionalCoverageMap,
) -> dict[str, Any]:
    input_path = _input_path(args.input)
    data, _ = _read_input(input_path)
    classification = _classify(data, snapshot, coverage_map)
    return {
        "schema_id": "reliquary-d01-synthetic-command-report-v2",
        "command": "inspect",
        "status": "INCOMPLETE",
        "evidence_scope": "synthetic_fixture",
        "classification": "synthetic_fixture_only",
        "input_sha256": _sha256(data),
        "byte_count": str(len(data)),
        "carrier_marker": classification.marker,
        "carrier_version": str(classification.carrier_version),
        "family_id": classification.family_id,
        "target_schema_id": classification.target_schema_id,
        "authentic_source_admission": False,
        "payload_emitted": False,
        "target_candidate": False,
        "registry_hashes": dict(snapshot.registry_hashes),
        "coverage_map_schema": coverage_map.schema_id,
        "coverage_map_status": coverage_map.status,
        "coverage_map_sha256": coverage_map.sha256,
        "d01_t01_acceptance": coverage_map.t01_acceptance,
        "producer_code_sha256": _code_digest(Path(__file__).resolve().parent),
    }


def _convert(
    args: argparse.Namespace,
    snapshot: PGC01Snapshot,
    coverage_map: ProvisionalCoverageMap,
) -> dict[str, Any]:
    input_path = _input_path(args.input)
    data, before_token = _read_input(input_path)
    digest = _sha256(data)
    if not _SHA256.fullmatch(args.expected_sha256) or args.expected_sha256 != digest:
        raise CliFailure("input_admission_mismatch", "expected input hash does not match the fixture", 3)
    classification = _classify(data, snapshot, coverage_map)
    grant = admit_for_scope(
        classification,
        snapshot,
        AdmissionScope.SYNTHETIC_FIXTURE_TEST,
        coverage_map,
    )
    if not grant.allowed_for_synthetic_tests or grant.authentic_source_admission:
        raise CliFailure("admission_closed", "synthetic fixture grant is not valid", 3)
    output_path = _output_path(args.output)
    if output_path.resolve(strict=False) == input_path:
        raise CliFailure("source_alias", "input and output paths must be distinct", 3)
    try:
        artifact = create_only_synthetic_artifact(
            output_path.parent,
            output_path.name,
            data,
            snapshot,
            coverage_map,
        )
    except CandidateAlreadyExists as error:
        raise CliFailure("conflict_collision", "output already exists; no overwrite was attempted", 5) from error
    except SyntheticPublicationError as error:
        raise CliFailure("io_failure", "atomic no-replace publication is unavailable", 7) from error
    except (OSError, ValueError, TypeError) as error:
        raise CliFailure("io_failure", "synthetic test artifact could not be created", 7) from error

    after_data, after_token = _read_input(input_path)
    if _sha256(after_data) != digest or after_token != before_token:
        raise CliFailure("input_changed", "fixture changed across synthetic materialization", 3)
    return _synthetic_receipt(
        digest,
        artifact.sha256,
        artifact.byte_count,
        artifact.file_name,
        classification,
        snapshot,
        coverage_map,
    )


def _verify_structure(
    args: argparse.Namespace,
    snapshot: PGC01Snapshot,
    coverage_map: ProvisionalCoverageMap,
) -> dict[str, Any]:
    candidate_path = _input_path(args.candidate)
    receipt_path = _receipt_path(args.receipt)
    data, _ = _read_input(candidate_path)
    classification = _classify(data, snapshot, coverage_map)
    receipt = _read_receipt(receipt_path)
    expected_keys = {
        "schema_id",
        "command",
        "status",
        "evidence_scope",
        "artifact_state",
        "target_candidate",
        "authentic_source_admission",
        "source_sha256_before",
        "source_sha256_after",
        "output_sha256",
        "byte_count",
        "output_name",
        "carrier_marker",
        "carrier_version",
        "family_id",
        "target_schema_id",
        "contract_revision",
        "registry_hashes",
        "coverage_map_schema",
        "coverage_map_status",
        "coverage_map_sha256",
        "d01_t01_acceptance",
        "producer_code_sha256",
        "payload_emitted",
        "current_only_owner_reopen",
    }
    if set(receipt) != expected_keys:
        raise CliFailure("target_mismatch", "synthetic receipt fields do not match the closed command contract", 6)
    digest = _sha256(data)
    if (
        receipt["schema_id"] != _RECEIPT_SCHEMA
        or receipt["command"] != "convert"
        or receipt["status"] != "INCOMPLETE"
        or receipt["evidence_scope"] != "synthetic_fixture"
        or receipt["artifact_state"] != "synthetic-test-artifact"
        or receipt["target_candidate"] is not False
        or receipt["authentic_source_admission"] is not False
        or receipt["source_sha256_before"] != receipt["source_sha256_after"]
        or receipt["source_sha256_before"] != digest
        or receipt["output_sha256"] != digest
        or receipt["byte_count"] != str(len(data))
        or receipt["output_name"] != candidate_path.name
        or receipt["carrier_marker"] != classification.marker
        or receipt["carrier_version"] != str(classification.carrier_version)
        or receipt["family_id"] != classification.family_id
        or receipt["target_schema_id"] != classification.target_schema_id
        or receipt["contract_revision"] != snapshot.revision
        or receipt["registry_hashes"] != snapshot.registry_hashes
        or receipt["coverage_map_schema"] != coverage_map.schema_id
        or receipt["coverage_map_status"] != coverage_map.status
        or receipt["coverage_map_sha256"] != coverage_map.sha256
        or receipt["d01_t01_acceptance"] != coverage_map.t01_acceptance
        or receipt["producer_code_sha256"] != _code_digest(Path(__file__).resolve().parent)
        or receipt["payload_emitted"] is not False
        or receipt["current_only_owner_reopen"] is not False
    ):
        raise CliFailure("target_mismatch", "synthetic fixture and receipt do not agree", 6)
    return {
        "schema_id": "reliquary-d01-synthetic-command-report-v2",
        "command": "verify-structure",
        "status": "INCOMPLETE",
        "evidence_scope": "synthetic_fixture",
        "structure_valid": True,
        "input_sha256": digest,
        "byte_count": str(len(data)),
        "family_id": classification.family_id,
        "target_schema_id": classification.target_schema_id,
        "authentic_source_admission": False,
        "target_candidate": False,
        "current_only_owner_reopen": False,
        "payload_emitted": False,
        "registry_hashes": dict(snapshot.registry_hashes),
        "coverage_map_schema": coverage_map.schema_id,
        "coverage_map_status": coverage_map.status,
        "coverage_map_sha256": coverage_map.sha256,
        "d01_t01_acceptance": coverage_map.t01_acceptance,
        "producer_code_sha256": _code_digest(Path(__file__).resolve().parent),
    }


def _parser() -> argparse.ArgumentParser:
    parser = StructuredArgumentParser(
        description="Synthetic-only D01 fixture command surfaces; no authentic source mode exists."
    )
    commands = parser.add_subparsers(dest="command", required=True)

    inspect = commands.add_parser("inspect", help="classify one D01SYN01 fixture")
    inspect.add_argument("--synthetic-fixture", action="store_true", required=True)
    inspect.add_argument("--input", required=True)
    inspect.add_argument("--contract-root")

    convert = commands.add_parser("convert", help="create a no-replace synthetic test artifact")
    convert.add_argument("--synthetic-fixture", action="store_true", required=True)
    convert.add_argument("--input", required=True)
    convert.add_argument("--expected-sha256", required=True)
    convert.add_argument("--output", required=True)
    convert.add_argument("--contract-root")

    verify = commands.add_parser("verify-structure", help="check a synthetic artifact and receipt")
    verify.add_argument("--synthetic-fixture", action="store_true", required=True)
    verify.add_argument("--candidate", required=True)
    verify.add_argument("--receipt", required=True)
    verify.add_argument("--contract-root")

    resume = commands.add_parser(
        "resume",
        help="unsupported: no checkpoint resume is implemented",
        description="Refuse explicitly; synthetic copy artifacts have no resumable conversion checkpoint.",
    )
    resume.set_defaults(command="resume")

    return parser


def main(argv: list[str] | None = None) -> int:
    try:
        args = _parser().parse_args(argv)
        if args.command == "resume":
            raise CliFailure(
                "resume_unsupported",
                "checkpoint resume is unsupported; no resumable synthetic checkpoint exists",
                3,
            )
        snapshot = _snapshot(args.contract_root)
        coverage_map = _coverage_map(snapshot)
        if args.command == "inspect":
            result = _inspect(args, snapshot, coverage_map)
        elif args.command == "convert":
            result = _convert(args, snapshot, coverage_map)
        elif args.command == "verify-structure":
            result = _verify_structure(args, snapshot, coverage_map)
        else:
            raise CliFailure("invalid_arguments", "command is unavailable", 2)
        print(_json_line(result))
        return 0
    except CliFailure as error:
        print(
            _json_line(
                {
                    "schema_id": "reliquary-d01-command-diagnostic-v1",
                    "status": "INCOMPLETE",
                    "error_code": error.error_code,
                    "message": error.message,
                    "payload_emitted": False,
                }
            ),
            file=sys.stderr,
        )
        return error.exit_code
    except Exception:
        print(
            _json_line(
                {
                    "schema_id": "reliquary-d01-command-diagnostic-v1",
                    "status": "INCOMPLETE",
                    "error_code": "internal_command_error",
                    "message": "synthetic command failed without emitting input content",
                    "payload_emitted": False,
                }
            ),
            file=sys.stderr,
        )
        return 7


if __name__ == "__main__":
    raise SystemExit(main())