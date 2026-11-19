"""Read-only D02 oracle for the ordered synthetic PHY Memory candidate."""
from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import subprocess


OWNER = {
    "kind": "PHY",
    "owner_uuid": "45454545454545454545454545454545",
    "scope": "unspecified",
    "history_generation_sha256": "fec1cddead319cb7f5f681cf993df3ffc9f9d93ec345e2e6ef5a67227bd2c4a1",
}
MEMORY_ID = "c30df370d053280ec83a3af8bbac9dbe638551448e14f9663510b1661ed84f5f"
BODY_ID = "60f02b8580389599ffcb830eac3321fdc9380a70e579276451819de6a1992653"
PREIMAGE = b"reliquary:d02:memory-history:v1\0"


def digest_file(path: Path) -> str:
    with path.open("rb") as file:
        return hashlib.file_digest(file, "sha256").hexdigest()


def projection_digest(value: dict) -> str:
    body = json.dumps(value, ensure_ascii=False, separators=(",", ":"), sort_keys=True)
    return hashlib.sha256(PREIMAGE + body.encode("utf-8")).hexdigest()


def expected_projection(cut: int) -> dict:
    first = {
        "memory_id": MEMORY_ID,
        "body_id": BODY_ID,
        "revision": "1",
        "payload": {
            "availability": "present",
            "title_sha256": "43dabb25fc58fbbe9515df52abd788bd526a73aa1710a53e439461976b750997",
            "title_utf8_length": "15",
            "content_sha256": "739bc0cab4e32069284810a36144d0ef2bdc0ee3f85c76839ca2fde5ead98783",
            "content_utf8_length": "18",
        },
        "category": "fact",
        "memory_type": "semantic",
        "authority_kind": "direct",
        "temporal_status": "current",
        "scope": "private",
        "lifecycle_state": "knowledge",
        "archived": False,
        "parent_id": None,
        "superseded_by": None,
        "source_node_id": None,
        "content_source_conversation_id": None,
        "content_source_node_id": None,
        "grounding_source_conversation_id": None,
        "grounding_source_node_id": None,
        "source_episode_id": None,
        "source_ref": None,
        "source_time_ns": "-9007199254740995",
        "temporal_inference": None,
        "mutation_id": "phy-create-v1",
        "created_at_ns": "9007199254740993",
        "updated_at_ns": "9007199254740993",
        "memory_version": "1",
        "standalone_origin": {
            "global_version": "1",
            "transaction_time_ns": "9007199254741010",
            "bound_reservation": False,
        },
    }
    second = {
        **first,
        "revision": "2",
        "category": "decision",
        "temporal_status": "historical",
        "archived": True,
        "mutation_id": "phy-revise-v2",
        "updated_at_ns": "9007199254741021",
        "memory_version": "2",
        "standalone_origin": {
            "global_version": "2",
            "transaction_time_ns": "9007199254741030",
            "bound_reservation": False,
        },
    }
    return {
        "memory_id": MEMORY_ID,
        "memory_version": str(cut),
        "presence": "retained_history" if cut else "absent_at_cut",
        "history": {"revisions": [first, second][:cut], "tombstone": None} if cut else None,
    }


def run_reader(reader: Path, candidate: Path, cut: int) -> dict:
    process = subprocess.run(
        [
            str(reader), "--kind", "phy", "--candidate", str(candidate),
            "--memory-history", f"{MEMORY_ID}:{cut}",
        ],
        capture_output=True,
        timeout=30,
        check=False,
    )
    if process.returncode != 2 or process.stderr:
        raise ValueError(f"unexpected reader exit/output at Memory cut {cut}")
    return json.loads(process.stdout)


def check(reader: Path, candidate: Path) -> dict:
    before = digest_file(candidate)
    results = []
    for cut in range(3):
        expected = expected_projection(cut)
        expected_digest = projection_digest(expected)
        report = run_reader(reader, candidate, cut)
        if report != run_reader(reader, candidate, cut):
            raise ValueError(f"repeat differs at Memory cut {cut}")
        bindings = {
            "schema_id": "reliquary-d02-memory-history-observation-v1",
            "status": "INCOMPLETE",
            "memory_observation_status": "OBSERVED",
            "requested_memory_version": str(cut),
            "requested_memory_id": MEMORY_ID,
            "owner": OWNER,
            "candidate_sha256_before": before,
            "candidate_sha256_after": before,
            "candidate_unchanged": True,
            "memory_head": "2",
        }
        if any(report.get(key) != value for key, value in bindings.items()):
            raise ValueError(f"owner, cut, or candidate custody differs at Memory cut {cut}")
        if report.get("projection") != expected or report.get("projection_sha256") != expected_digest:
            raise ValueError(f"Memory projection differs at cut {cut}")
        results.append({"memory_version": str(cut), "projection_sha256": expected_digest})
    if before != digest_file(candidate):
        raise ValueError("candidate bytes changed during verification")
    return {
        "status": "PASS",
        "scope": "synthetic_ordered_phy_memory_fixture_only",
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
