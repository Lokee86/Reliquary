"""Reconcile two native prefix replays with the independently derived Freshness cuts."""
import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path
import re

from f03_verify_archived_oracle import PINS, require
from f03_verify_independent_repeat import exact_object, read_document, regular, snapshot

DOMAIN = b"reliquary:f03:freshness-at-backing-cleared-global:v1\0"
COORDINATES = ("global", "stage", "activity", "members", "input_chain_sha256", "physical_prefix_sha256")


def encode(value):
    return json.dumps(value, sort_keys=True, separators=(",", ":")).encode("ascii")


def read_points(path, expected):
    before = regular(path)
    require(0 < before.st_size <= 16 * 1024 * 1024, "invalid Freshness point stream size")
    payload = path.read_bytes()
    require(snapshot(before) == snapshot(regular(path))
            and hashlib.sha256(payload).hexdigest() == expected,
            "independent Freshness point stream changed")
    return [json.loads(line, object_pairs_hook=exact_object) for line in payload.splitlines()]


def source_index(sources):
    rows = {row["copy_id"]: row for row in sources}
    require(len(sources) == len(rows) == 3
            and {row["source_sha256"] for row in sources}
            == {pin for fixture, pin in PINS.items() if fixture.endswith(".rel")},
            "Freshness reconciliation requires exactly the three pinned RELs")
    return rows


def observe(plan, native, birth, points):
    require(plan["source_sha256"] == native["source_sha256"] == birth["source_sha256"],
            "Freshness component source differs")
    native_rows = native["representatives"]
    planned = plan["representatives"]
    require(len(native_rows) == len(planned), "native Freshness representative count differs")
    birth_rows = {row["global"]: row for row in birth["representatives"]}
    require(len(birth_rows) == len(birth["representatives"]), "duplicate birth-only representative")
    representatives = {}
    for got, want in zip(native_rows, planned):
        require(all(got[field] == want[field] for field in COORDINATES)
                and 0 <= got["physical_global"] <= got["global"]
                and got["rows"] >= 3 and re.fullmatch("[0-9a-f]{64}", got["sha256"]),
                "native Freshness representative differs from the independent plan")
        key = got["global"]
        require(key not in representatives, "duplicate native Freshness representative")
        if got["stage"] == "birth_only":
            old = birth_rows.pop(key, None)
            require(old == {field: got[field] for field in ("global", "activity", "rows", "sha256")},
                    "native Freshness birth-only proof differs from independent decoded values")
        else:
            require(got["stage"] == "journal", "unrecognized native Freshness stage")
        representatives[key] = got
    require(not birth_rows, "native replay omitted independent birth-only expectations")

    root = hashlib.sha256(DOMAIN + bytes.fromhex(plan["source_sha256"]))
    previous_global = -1
    stage_counts = Counter()
    membership = Counter()
    for point in points:
        global_ = point["global"]
        require(global_ > previous_global, "Freshness points are not strictly ordered")
        previous_global = global_
        stage = point["stage"]
        if stage in {"birth_only", "journal"}:
            rep_key = point["representative_global"]
            native_row = representatives.get(rep_key)
            require(native_row is not None and native_row["stage"] == stage
                    and native_row["activity"] == point["activity"]
                    and native_row["input_chain_sha256"] == point["input_chain_sha256"]
                    and rep_key <= global_, "Freshness point does not match its native input representative")
            if global_ == rep_key:
                require(native_row["physical_prefix_sha256"] == point["physical_prefix_sha256"],
                        "native representative was read at a different physical frontier")
            membership[rep_key] += 1
            proof = {"cut": native_row["activity"], "event_cut": None,
                     "rows": native_row["rows"], "sha256": native_row["sha256"]}
        else:
            require(stage in {"no_input", "terminal"}, "unknown independently computed Freshness stage")
            proof = point["freshness_fingerprint"]
        stage_counts[stage] += 1
        value = encode({"global": global_, "stage": stage, "physical_prefix_sha256": point["physical_prefix_sha256"],
                        "input_chain_sha256": point["input_chain_sha256"], "fingerprint": proof})
        root.update(len(value).to_bytes(8, "little") + value)
    require(membership == {key: row["members"] for key, row in representatives.items()},
            "Freshness native representative membership differs")
    return {"copy_id": plan["copy_id"], "source_sha256": plan["source_sha256"],
            "native_representatives": dict(Counter(row["stage"] for row in representatives.values())),
            "stage_counts": dict(stage_counts), "cut_count": len(points), "cut_proofs_sha256": root.hexdigest()}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--plan", type=Path, required=True)
    parser.add_argument("--birth-expectations", type=Path, required=True)
    parser.add_argument("--first-replay", type=Path, required=True)
    parser.add_argument("--repeat-replay", type=Path, required=True)
    args = parser.parse_args()
    plan, plan_sha = read_document(args.plan, 2 * 1024 * 1024)
    birth, birth_sha = read_document(args.birth_expectations, 2 * 1024 * 1024)
    first, first_sha = read_document(args.first_replay, 2 * 1024 * 1024)
    repeat, repeat_sha = read_document(args.repeat_replay, 2 * 1024 * 1024)
    require(first == repeat and first_sha == repeat_sha and first["status"] == "PASS"
            and not first["authentic_admission"] and first["plan_sha256"] == birth["prefix_plan_sha256"] == plan_sha
            and plan["schema_id"] == "reliquary-f03-freshness-prefix-reduction-v1"
            and not plan["authentic_admission"],
            "native repeats or independent plan bindings differ")
    plans, native, births = map(source_index, (plan["sources"], first["sources"], birth["sources"]))
    require(plans.keys() == native.keys() == births.keys(), "Freshness source keys differ")
    results = []
    for key, source in sorted(plans.items()):
        stream = args.plan.parent / f"{key}-freshness-prefix-points.jsonl"
        points = read_points(stream, source["point_stream_sha256"])
        results.append(observe(source, native[key], births[key], points))
    counts = Counter()
    representatives = Counter()
    for result in results:
        counts.update(result["stage_counts"])
        representatives.update(result["native_representatives"])
    require(counts == {"no_input": 4067, "birth_only": 10723, "journal": 798, "terminal": 3}
            and representatives == {"birth_only": 169, "journal": 111}, "Freshness source cut population differs")
    print(json.dumps({
        "status": "PASS", "scope": "Freshness component on the backing-cleared pinned REL frontiers",
        "authentic_admission": False, "whole_owner_positive_availability_assessed": False,
        "plan_sha256": plan_sha, "independent_birth_expectations_sha256": birth_sha,
        "native_repeat_receipt_sha256": first_sha, "cut_count": sum(counts.values()),
        "stage_counts": counts, "native_representatives": representatives, "owners": results,
    }, indent=2))


if __name__ == "__main__":
    main()
