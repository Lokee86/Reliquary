"""Independent oracle for the ordered synthetic Archive binding candidate."""
from __future__ import annotations

import argparse
import json
from pathlib import Path
import subprocess
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from archive_binding_evidence import READER_SCHEMA as SCHEMA, validate_binding_projection
from check_ordered_archive_candidate import OWNER, digest_file, expected_projection, projection_digest
from compare_archive_bindings import run_reader


# Local order is not global reservation order. The repeats point directly to
# the applying rows; the last repeat must leave the current branch unchanged.
EXPECTED_BINDINGS = (
    (1, 2, "9007199254741010", "allocate", 1, 2),
    (2, 1, "-9007199254740995", "bind_reserved", 2, 1),
    (3, 4, None, "allocate", 3, 4),
    (4, 3, None, "bind_reserved", 1, 2),
    (5, 5, "9007199254741030", "allocate", 5, 5),
    (6, 6, "-9007199254741050", "allocate", 2, 1),
)


def expected_binding_projection(cut: int) -> dict:
    return {
        "archive_version": str(cut),
        "bindings": [
            {
                "archive_version": str(local),
                "global_version": str(global_version),
                "transaction_time_ns": time,
                "clock_mode": mode,
                "first_applying_archive_version": str(first_local),
                "first_applying_global_version": str(first_global),
            }
            for local, global_version, time, mode, first_local, first_global in EXPECTED_BINDINGS[:cut]
        ],
    }


def expected_semantic_projection(cut: int) -> dict:
    full = expected_projection(4)
    root, reply = full["turns"]
    branch_root, branch_reply = full["branch_revisions"]
    turns = [
        turn
        for version, turn in ((1, root), (3, {**reply, "archive_version": "3"}))
        if version <= cut
    ]
    branches = [
        {**branch, "archive_version": str(version)}
        for version, branch in ((2, branch_root), (5, branch_reply))
        if version <= cut
    ]
    return {
        **full,
        "archive_version": str(cut),
        "activity_cut": str(len(turns)),
        "turns": turns,
        "branch_revisions": branches,
        "branches": [branches[-1]["branch"]] if branches else [],
    }


def verify_report(report: dict, candidate_sha256: str, cut: int) -> dict:
    required = {
        "schema_id": SCHEMA,
        "status": "INCOMPLETE",
        "archive_observation_status": "OBSERVED",
        "archive_binding_observation_status": "OBSERVED",
        "requested_archive_version": str(cut),
        "owner": OWNER,
        "candidate_sha256_before": candidate_sha256,
        "candidate_sha256_after": candidate_sha256,
        "candidate_unchanged": True,
        "archive_head": "6",
    }
    if any(report.get(key) != value for key, value in required.items()):
        raise ValueError(f"Archive binding coverage or candidate custody differs at cut {cut}")
    expected = expected_semantic_projection(cut)
    semantic_hash = projection_digest(expected)
    if report.get("projection") != expected or report.get("projection_sha256") != semantic_hash:
        raise ValueError(f"Archive semantic projection differs at cut {cut}")
    bindings = expected_binding_projection(cut)
    binding_hash = validate_binding_projection(bindings)
    if report.get("binding_projection") != bindings or report.get("binding_projection_sha256") != binding_hash:
        raise ValueError(f"Archive publication binding projection differs at cut {cut}")
    return {
        "archive_version": str(cut),
        "projection_sha256": semantic_hash,
        "binding_projection_sha256": binding_hash,
    }


def check(reader: Path, candidate: Path) -> dict:
    before = digest_file(candidate)
    cuts = []
    for cut in range(7):
        report = run_reader(reader, candidate, str(cut))
        if report != run_reader(reader, candidate, str(cut)):
            raise ValueError(f"fresh-process repeat differs at Archive binding cut {cut}")
        cuts.append(verify_report(report, before, cut))
    if digest_file(candidate) != before:
        raise ValueError("candidate bytes changed during verification")
    return {
        "status": "PASS",
        "scope": "synthetic_bound_archive_fixture_only",
        "candidate_sha256": before,
        "reader_sha256": digest_file(reader),
        "oracle_sha256": digest_file(Path(__file__)),
        "semantic_oracle_sha256": digest_file(Path(__file__).with_name("check_ordered_archive_candidate.py")),
        "cuts": cuts,
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
