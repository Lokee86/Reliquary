"""Read-only D02 oracle for the ordered synthetic Archive candidate.

Invoke this after building the D02 reader and the D01 synthetic fixture.
The expected owner and history are literals, independent of converter output.
"""
from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import subprocess


OWNER = {
    "kind": "REL",
    "owner_uuid": "32323232323232323232323232323232",
    "scope": "project",
    "history_generation_sha256": "e1df88795ef889209b94ec297e22596fc172a2ff5096a2ed8d5cf980eeaa686c",
}
SCHEMA = "reliquary-d02-archive-cut-observation-v1"
PREIMAGE = b"reliquary:d02:archive-cut:v1\0"
CONVERSATION = "synthetic-conversation"


def digest_file(path: Path) -> str:
    with path.open("rb") as file:
        return hashlib.file_digest(file, "sha256").hexdigest()


def projection_digest(value: dict) -> str:
    body = json.dumps(value, ensure_ascii=False, separators=(",", ":"), sort_keys=True)
    return hashlib.sha256(PREIMAGE + body.encode("utf-8")).hexdigest()


def expected_projection(cut: int) -> dict:
    root = "850f7dc43910ff890f8879c0ed26fe697c93a067ad93a7d50f466a7028a9bf4e"
    reply = "a9f4b3d22a523fdada41c85c175425bcd15b32b4cd0f54d9433accd52d7195a1"
    turns = [
        {
            "node_id": "root",
            "conversation_id": CONVERSATION,
            "parent_id": None,
            "role": "user",
            "principal_id": "fixture-principal",
            "timestamp_ns": "9007199254740993",
            "content_id": root,
            "body": {"availability": "present", "utf8_length": "5", "sha256": root},
            "archive_version": "1",
            "activity_position": "1",
            "attachments": [],
        },
        {
            "node_id": "reply",
            "conversation_id": CONVERSATION,
            "parent_id": "root",
            "role": "assistant",
            "principal_id": None,
            "timestamp_ns": "9007199254741009",
            "content_id": reply,
            "body": {"availability": "present", "utf8_length": "8", "sha256": reply},
            "archive_version": "2",
            "activity_position": "2",
            "attachments": [],
        },
    ]
    branches = [
        {
            "archive_version": str(version),
            "branch": {
                "id": "main",
                "conversation_id": CONVERSATION,
                "leaf_node_id": leaf,
                "canonical": True,
            },
        }
        for version, leaf in ((3, "root"), (4, "reply"))
        if version <= cut
    ]
    return {
        "archive_version": str(cut),
        "activity_cut": str(min(cut, 2)),
        "turns": turns[:cut],
        "branch_revisions": branches,
        "branches": [branches[-1]["branch"]] if branches else [],
        "episodes": [],
        "import_receipts": [],
        "fragment_definitions": [],
        "conversation_tombstones": [],
    }


def run_reader(reader: Path, candidate: Path, cut: int) -> dict:
    process = subprocess.run(
        [
            str(reader), "--kind", "rel", "--candidate", str(candidate),
            "--archive-cut", str(cut),
        ],
        capture_output=True,
        timeout=30,
        check=False,
    )
    if process.returncode != 2 or process.stderr:
        raise ValueError(f"unexpected reader exit/output at Archive cut {cut}")
    return json.loads(process.stdout)


def check(reader: Path, candidate: Path) -> dict:
    before = digest_file(candidate)
    results = []
    for cut in range(5):
        expected = expected_projection(cut)
        expected_digest = projection_digest(expected)
        report = run_reader(reader, candidate, cut)
        if report != run_reader(reader, candidate, cut):
            raise ValueError(f"repeat differs at Archive cut {cut}")
        bindings = {
            "schema_id": SCHEMA,
            "status": "INCOMPLETE",
            "archive_observation_status": "OBSERVED",
            "requested_archive_version": str(cut),
            "owner": OWNER,
            "candidate_sha256_before": before,
            "candidate_sha256_after": before,
            "candidate_unchanged": True,
            "archive_head": "4",
        }
        if any(report.get(key) != value for key, value in bindings.items()):
            raise ValueError(f"owner, cut, or candidate custody differs at Archive cut {cut}")
        if report.get("projection") != expected or report.get("projection_sha256") != expected_digest:
            raise ValueError(f"Archive projection differs at cut {cut}")
        results.append({"archive_version": str(cut), "projection_sha256": expected_digest})
    if before != digest_file(candidate):
        raise ValueError("candidate bytes changed during verification")
    return {
        "status": "PASS",
        "scope": "synthetic_ordered_archive_fixture_only",
        "candidate_sha256": before,
        "reader_sha256": digest_file(reader),
        "oracle_sha256": digest_file(Path(__file__)),
        "cuts": results,
        "fresh_process_repeats": "2",
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--reader", type=Path, required=True)
    parser.add_argument("--candidate", type=Path, required=True)
    args = parser.parse_args()
    try:
        result = check(args.reader.resolve(strict=True), args.candidate.resolve(strict=True))
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        print(json.dumps({"status": "FAIL", "reason": str(error)}))
        return 2
    print(json.dumps(result, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
