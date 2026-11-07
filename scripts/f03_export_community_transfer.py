"""Translate exact pinned Community generations into current typed Community frames."""
import argparse
from pathlib import Path

from f03_verify_archived_oracle import PINS, require
from f03_verify_community_cuts import decode_snapshot, derive_lineage, encode, expected_records, sha256
from f03_verify_independent_repeat import (
    MANIFEST, SCHEMA_V2, decimal, read_document, read_exact, regular, snapshot, take_stream,
)
from f03_verify_prefix_availability import write_jsonl


def observe(source_path, output, artifacts, owner):
    expected = expected_records(output, artifacts, owner, SCHEMA_V2)
    uuid = bytes.fromhex(owner["owner_uuid"])
    frames, handled = [], []
    previous = None
    before = regular(source_path)
    with source_path.open("rb") as source:
        import hashlib
        require(hashlib.file_digest(source, "sha256").hexdigest() == owner["source_sha256"],
                "source Community bytes differ from their pin")
        for ordinal, row in take_stream(output, artifacts, owner, "frames", owner["frame_stream"], schema_id=SCHEMA_V2):
            if row["carrier"] != "CVACOMM1":
                require(not row["carrier"].startswith(("CVACOMM", "CVACNAM")), "unknown retained Community carrier")
                continue
            offset, end = decimal(row["offset"]), decimal(row["end_offset"])
            source.seek(offset)
            size = int.from_bytes(read_exact(source, 8), "little")
            require(size == end - offset - 8 and 60 <= size <= 16 * 1024 * 1024, "invalid Community frame extent")
            payload = read_exact(source, size)
            require(payload.startswith(b"CVACOMM1") and sha256(payload) == row["payload_sha256"],
                    "Community source frame differs from the verified witness")
            current = decode_snapshot(payload, uuid)
            generation = current["generation"]
            require(generation == (1 if previous is None else previous["generation"] + 1),
                    "Community generation differs from retained order")
            values = {"community_snapshot": current}
            if previous is not None:
                values["community_lineage"] = derive_lineage(previous, current)
            for family, value in values.items():
                canonical = encode(value)
                key = family, sha256(generation.to_bytes(8, "big"))
                require(expected.pop(key, None) == (sha256(canonical), str(len(canonical))),
                        "Community value differs from its native source proof")
            body = b"RELCOM02" + uuid + payload[12:]
            frames.append({"source_ordinal": str(ordinal), "kind": "frame", "body_hex": body.hex()})
            handled.append({"source_ordinal": str(ordinal), "source_carrier": "CVACOMM1",
                            "disposition": "typed_community_generation"})
            previous = current
    require(not expected and snapshot(before) == snapshot(regular(source_path)),
            "incomplete or changed Community source")
    return frames, handled


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("source-copies", "source-coverage", "output-directory"):
        parser.add_argument(f"--{name}", type=Path, required=True)
    args = parser.parse_args()
    output = args.source_coverage.parent
    coverage, coverage_sha = read_document(args.source_coverage, 2 * 1024 * 1024)
    manifest, manifest_sha = read_document(output / MANIFEST, 2 * 1024 * 1024)
    require(coverage["schema_id"] == manifest["schema_id"] == SCHEMA_V2
            and coverage_sha == manifest["source_coverage_sha256"], "Community source witness custody differs")
    owners = {row["source_sha256"]: row for row in coverage["sources"]}
    require(len(coverage["sources"]) == len(PINS) and owners.keys() == set(PINS.values()), "expected six exact pins")
    destination = args.output_directory.resolve()
    regular(destination.parent, directory=True)
    regular(args.source_copies, directory=True)
    require(not any(destination.is_relative_to(path.resolve()) for path in (output, args.source_copies)),
            "Community output overlaps protected inputs")
    artifacts = dict(manifest["artifact_sha256"])
    results = []
    for fixture, pin in sorted(PINS.items()):
        owner = owners[pin]
        frames, handled = observe(args.source_copies / fixture, output, artifacts, owner)
        results.append((owner, frames, handled))
    destination.mkdir()
    sources = []
    for owner, frames, handled in results:
        copy = owner["copy_id"]
        frame_name, handled_name = f"{copy}-community-frames.jsonl", f"{copy}-community-handled.jsonl"
        sources.append({
            "copy_id": copy, "source_sha256": owner["source_sha256"], "generations": len(frames),
            "frames": frame_name, "frames_sha256": write_jsonl(destination / frame_name, frames),
            "handled": handled_name, "handled_sha256": write_jsonl(destination / handled_name, handled),
        })
    receipt = {
        "scope": "exact retained Community generations; lineage derived from checked memberships",
        "source_coverage_sha256": coverage_sha, "source_manifest_sha256": manifest_sha, "sources": sources,
    }
    with (destination / "community-transfer-receipt.json").open("xb") as file:
        file.write(encode(receipt) + b"\n")


if __name__ == "__main__":
    main()
