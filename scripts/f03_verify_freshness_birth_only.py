"""Independently fingerprint the birth-only Freshness candidates in a checked replay plan."""
import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path

from f03_verify_archived_oracle import PINS, require
from f03_verify_dream_journal_frontiers import NATIVE_DOMAIN, decode_completion, encode, framed, sha, u64
from f03_verify_freshness_prefixes import INPUT_DOMAIN, POLICY
from f03_verify_independent_repeat import (
    MANIFEST, SCHEMA_V2, decimal, exact_object, read_document, read_exact, regular, snapshot, take_stream,
)
from f03_verify_prefix_availability import write_jsonl

PLAN_SHA256 = "1d921a8a2602e439b60ad769792a27f76f2b4ff23a9dfcad9a64fc24dae76e70"
UNADMITTED = {"score": 100, "admitted_at_turn": None, "accounted_turn": None}


def fingerprint(uuid, activity, births):
    selected = [(identity, value) for identity, value in sorted(births.items()) if value["source_turn"] <= activity]
    require(bool(selected), "birth-only cut lacks a physically accepted birth")
    rows = 3 + 2 * len(selected)
    digest = hashlib.sha256(NATIVE_DOMAIN + b"\1\0\0\0" + uuid + framed(b"freshness")
                            + u64(activity) + b"\0" + u64(rows))
    evidence = [("activation", b"current", {"seen": False, "activation": None}),
                ("cut", b"cut", {"mode": "activity", "rel_turn": activity,
                                 "event_id_inclusive": None, "graph_version": None})]
    evidence.extend(("initialize", identity, value) for identity, value in selected)
    evidence.append(("policy", b"policy", POLICY))
    evidence.extend(("record", identity, UNADMITTED) for identity, _ in selected)
    for family, key, value in evidence:
        digest.update(framed(family.encode()) + framed(key) + framed(encode(value)))
    return {"cut": activity, "event_cut": None, "rows": rows, "sha256": digest.hexdigest()}


def birth_inputs(path, output, artifacts, owner, representatives):
    uuid = bytes.fromhex(owner["owner_uuid"])
    before = regular(path)
    chain = hashlib.sha256(INPUT_DOMAIN + uuid)
    birth_frames = []
    all_births = {}
    with path.open("rb") as source:
        for ordinal, row in take_stream(output, artifacts, owner, "frames", owner["frame_stream"], schema_id=SCHEMA_V2):
            if row["carrier"] != "CVAINSC8":
                continue
            start, end = decimal(row["offset"]), decimal(row["end_offset"])
            require(end <= before.st_size and 16 <= end - start <= 16 * 1024 * 1024 + 8,
                    "invalid birth carrier extent")
            source.seek(start)
            size = int.from_bytes(read_exact(source, 8), "little")
            require(size == end - start - 8, "birth carrier length differs from its frame witness")
            payload = read_exact(source, size)
            require(payload.startswith(b"CVAINSC8") and sha(payload) == row["payload_sha256"],
                    "birth carrier differs from its verified witness")
            _, _, activity, identities = decode_completion(payload)
            if not identities:
                continue
            entries = {identity: {"memory_id": list(identity), "source_turn": activity, "admitted_at_birth": False}
                       for identity in identities}
            require(not entries.keys() & all_births.keys(), "repeated accepted birth identity")
            all_births.update(entries)
            birth_frames.append((end, ordinal, decimal(row["latest_global"]), row["payload_sha256"], entries))
        source.seek(0)
        raw_digest = hashlib.sha256()
        prefix_end = 0
        births = {}
        next_frame = iter(birth_frames)
        current = next(next_frame, None)
        results = {}
        for row in sorted(representatives, key=lambda row: row["global"]):
            target_end = row["end_offset"]
            require(prefix_end <= target_end <= before.st_size, "nonmonotone exact birth-only prefix")
            while prefix_end < target_end:
                block = read_exact(source, min(target_end - prefix_end, 1024 * 1024))
                raw_digest.update(block)
                prefix_end += len(block)
            require(raw_digest.hexdigest() == row["physical_prefix_sha256"],
                    "birth-only representative prefix differs from the checked plan")
            while current is not None and current[0] <= target_end:
                _, ordinal, global_, payload_sha, entries = current
                chain.update(framed(encode([ordinal, global_, payload_sha])))
                births.update(entries)
                current = next(next_frame, None)
            require(chain.hexdigest() == row["input_chain_sha256"]
                    and all(value["source_turn"] <= row["activity"] for value in births.values()),
                    "birth-only representative has non-equivalent physical inputs")
            results[row["global"]] = {**row, "freshness_fingerprint": fingerprint(uuid, row["activity"], births)}
        for block in iter(lambda: source.read(1024 * 1024), b""):
            raw_digest.update(block)
        require(raw_digest.hexdigest() == owner["source_sha256"]
                and snapshot(before) == snapshot(regular(path)), "source copy differs from its verified pin")
    expected = {sha(identity): encode(value) for identity, value in all_births.items()}
    for _, row in take_stream(output, artifacts, owner, "records", owner["native"]["record_stream"], schema_id=SCHEMA_V2):
        if row["projection_domain"] == "freshness" and row["family"] == "initialize":
            value = expected.pop(row["stable_key_sha256"], None)
            require(value is not None and row["canonical_value_sha256"] == sha(value)
                    and decimal(row["canonical_byte_length"]) == len(value) and row["availability"] == "retained",
                    "birth identity or turn differs from the accepted native commitment")
    require(not expected, "native witness omitted an accepted birth")
    return results, len(all_births)


def bind_points(path, plan, results, *, stage="birth_only"):
    before = regular(path)
    require(0 < before.st_size <= 16 * 1024 * 1024, "invalid checked point stream length")
    data = path.read_bytes()
    require(snapshot(before) == snapshot(regular(path)) and sha(data) == plan["point_stream_sha256"],
            "point stream differs from the verified replay plan")
    points = []
    counts = Counter()
    for raw in data.splitlines():
        row = json.loads(raw, object_pairs_hook=exact_object)
        if row["stage"] != stage:
            continue
        representative = results[row["representative_global"]]
        require(representative["input_chain_sha256"] == row["input_chain_sha256"]
                and representative["activity"] == row["activity"], "cut differs from its exact physical Freshness inputs")
        counts[representative["global"]] += 1
        points.append({**row, "freshness_fingerprint": representative["freshness_fingerprint"]})
    require(len(points) == plan["counts"][stage]
            and dict(counts) == {r["global"]: r["members"] for r in results.values()},
            "replay plan does not account for every backing-cleared point in this stage")
    return points


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("source-copies", "source-coverage", "prefix-plan", "output-directory"):
        parser.add_argument(f"--{name}", type=Path, required=True)
    args = parser.parse_args()
    output = args.source_coverage.parent
    coverage, coverage_sha = read_document(args.source_coverage, 2 * 1024 * 1024)
    manifest, manifest_sha = read_document(output / MANIFEST, 2 * 1024 * 1024)
    plan, plan_sha = read_document(args.prefix_plan, 2 * 1024 * 1024)
    require(plan_sha == PLAN_SHA256 and manifest_sha == plan["source_manifest_sha256"]
            and coverage_sha == manifest["source_coverage_sha256"] == plan["source_coverage_sha256"]
            and coverage["schema_id"] == manifest["schema_id"] == SCHEMA_V2,
            "source witness or replay plan provenance differs")
    destination = args.output_directory.resolve()
    regular(destination.parent, directory=True)
    regular(args.source_copies, directory=True)
    require(not any(destination.is_relative_to(path.resolve())
                    for path in (output, args.source_copies, args.prefix_plan.parent)),
            "birth-only evidence destination overlaps protected inputs")
    owners = {row["copy_id"]: row for row in coverage["sources"]}
    artifacts = dict(manifest["artifact_sha256"])
    sources = []
    for item in plan["sources"]:
        copy, fixture = item["copy_id"], item["fixture"]
        owner = owners[copy]
        require(owner["source_sha256"] == item["source_sha256"] == PINS[fixture]
                and owner["native"]["owner_kind"] == "reliquary", "birth-only source identity differs")
        representatives = [row for row in item["representatives"] if row["stage"] == "birth_only"]
        results, births = birth_inputs(args.source_copies / fixture, output, artifacts, owner, representatives)
        points = bind_points(args.prefix_plan.parent / f"{copy}-freshness-prefix-points.jsonl", item, results)
        sources.append(({
            "copy_id": copy, "source_sha256": owner["source_sha256"], "accepted_birth_rows_verified": births,
            "source_point_stream_sha256": item["point_stream_sha256"], "birth_only_cuts": len(points),
            "representatives": list(results.values()),
        }, points))
    destination.mkdir()
    for source, points in sources:
        source["point_stream_sha256"] = write_jsonl(
            destination / f'{source["copy_id"]}-freshness-birth-only-points.jsonl', points)
    result = {
        "schema_id": "reliquary-f03-freshness-birth-only-v1",
        "scope": "independent exact Freshness components at physical prefixes with only grouped births",
        "authentic_admission": False, "whole_owner_positive_availability_assessed": False,
        "native_interior_replay_checked": False, "prefix_plan_sha256": plan_sha,
        "source_manifest_sha256": manifest_sha, "source_coverage_sha256": coverage_sha,
        "sources": [source for source, _ in sources],
    }
    with (destination / "freshness-birth-only-fingerprints.json").open("xb") as file:
        file.write(encode(result) + b"\n")


if __name__ == "__main__":
    main()
