"""Check every historical Archive binding against the pinned source wire."""
import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path

from f03_verify_archived_oracle import PINS, require
from f03_verify_corpus_scope import SCOPES
from f03_verify_independent_repeat import MANIFEST, SCHEMA_V2, read_document, read_exact, regular, snapshot, take_stream

ARCHIVE_MARKERS = {
    b"CVANOD02", b"CVABRC02", b"CVACON04", b"CVACTM02", b"CVAFRA02",
    b"CVAEPI02", b"CVAFIL02", b"CVATUR03", b"CVAIMP02", b"CVAFME02", b"CVAARP02",
}
COMPARABLE_DOMAIN = b"reliquary:d02:archive-publication-bindings:v1\0"
PREFIX_DOMAIN = b"reliquary:f03:archive-binding-prefixes:v1\0"


def number(data, start, *, signed=False):
    end = start + 8
    require(len(data) >= end, "truncated historical integer")
    return int.from_bytes(data[start:end], "little", signed=signed)


def read_binding(payload, reservations, previous):
    tag = payload[16] if len(payload) >= 17 else None
    require(tag in (0, 1, 2), "invalid historical envelope mode")
    body_start = 25 if tag == 1 else 17
    require(len(payload) >= body_start + 8, "truncated historical publication body")
    body = payload[body_start:]
    marker = body[:8]
    if marker not in ARCHIVE_MARKERS:
        return None
    global_ = number(payload, 8)
    local = number(body, 8)
    require(local == len(previous) + 1 and global_ > 0
            and (not previous or int(previous[-1]["global_version"]) < global_),
            "Archive publications are not a dense ordered history")
    if tag == 2:
        require(global_ in reservations, "Archive binding has no standalone reservation")
        time = reservations[global_]
    else:
        require(global_ not in reservations, "Archive allocation overlaps a reservation")
        time = number(payload, 17, signed=True) if tag == 1 else None
    origin = local
    origin_global = global_
    if marker == b"CVAARP02":
        require(len(body) == 24, "invalid retained Archive repeat")
        origin = number(body, 16)
        require(0 < origin < local and previous[origin - 1]["first_applying_archive_version"] == str(origin),
                "Archive repeat does not identify its first applying publication")
        origin_global = int(previous[origin - 1]["global_version"])
    return {
        "archive_version": str(local),
        "global_version": str(global_),
        "transaction_time_ns": None if time is None else str(time),
        "clock_mode": "bind_reserved" if tag == 2 else "allocate",
        "first_applying_archive_version": str(origin),
        "first_applying_global_version": str(origin_global),
    }


def scan(copies, relative, pinned):
    path = copies / relative
    before = regular(path)
    rows = []
    reservations = {}
    with path.open("rb") as source:
        header = read_exact(source, 40)
        require(header[:16] == b"CVA\0\r\n\x1a\n\x02\0\0\0\x28\0\0\0" and header[16] == 1
                and header[17] in SCOPES and not any(header[18:24]),
                "expected a typed historical REL")
        digest = hashlib.sha256(header)
        while source.tell() < before.st_size:
            length = read_exact(source, 8)
            size = int.from_bytes(length, "little")
            require(8 <= size <= 16 * 1024 * 1024 and source.tell() + size <= before.st_size,
                    "invalid historical source frame length")
            payload = read_exact(source, size)
            digest.update(length)
            digest.update(payload)
            tag = payload[:8]
            if tag in {b"CVAVERS1", b"CVAVERS2"}:
                require(len(payload) == (24 if tag == b"CVAVERS2" else 16), "invalid standalone reservation")
                version = number(payload, 8)
                require(version > 0 and version not in reservations, "duplicate standalone reservation")
                reservations[version] = number(payload, 16, signed=True) if tag == b"CVAVERS2" else None
            elif tag == b"CVAPUB01":
                binding = read_binding(payload, reservations, rows)
                if binding is not None:
                    rows.append(binding)
    require(digest.hexdigest() == pinned and snapshot(before) == snapshot(regular(path)),
            "historical source differs from its pins or changed during verification")
    return rows, {"kind": "REL", "owner_uuid": header[24:40].hex(), "scope": SCOPES[header[17]]}


def verify_bindings(output, artifacts, owner, wire):
    evidence = owner["native"]["archive_publication_bindings"]
    head = str(len(wire))
    require(evidence["archive_head"] == head
            and evidence["cut_coverage"] == "exhaustive_ordered_prefixes"
            and evidence["schema_id"] == "reliquary-d02-archive-publication-bindings-v1"
            and evidence["bindings"]["rows"] == head
            and evidence["cuts"]["rows"] == str(len(wire) + 1),
            "Archive binding coverage does not describe the complete source history")
    for index, row in take_stream(output, artifacts, owner, "archive-bindings", evidence["bindings"], schema_id=SCHEMA_V2):
        require(row == wire[index], f"Archive binding differs from source at cut {index + 1}")
    prefix = hashlib.sha256(PREFIX_DOMAIN)
    for index, row in take_stream(output, artifacts, owner, "archive-binding-cuts", evidence["cuts"], schema_id=SCHEMA_V2):
        if index:
            encoded = json.dumps(wire[index - 1], sort_keys=True, separators=(",", ":")).encode("ascii")
            prefix.update(len(encoded).to_bytes(8, "little"))
            prefix.update(encoded)
        require(row == {
            "archive_version": str(index), "binding_rows": str(index),
            "availability": "retained", "prefix_sha256": prefix.hexdigest(),
        }, f"Archive binding prefix digest differs at cut {index}")
    canonical = json.dumps({"archive_version": head, "bindings": wire},
                           sort_keys=True, separators=(",", ":")).encode("ascii")
    comparable = hashlib.sha256(COMPARABLE_DOMAIN + canonical).hexdigest()
    require(comparable == evidence["binding_projection_sha256"], "Archive binding projection digest differs")
    return comparable


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source-copies", type=Path, required=True)
    parser.add_argument("--source-coverage", type=Path)
    parser.add_argument("--expectation-directory", type=Path,
                        help="fresh directory under the caller's protected evidence root")
    args = parser.parse_args()
    require(not args.expectation_directory or args.source_coverage,
            "expectations require a verified native source witness")
    owners = {}
    if args.source_coverage:
        output = args.source_coverage.parent
        coverage, coverage_hash = read_document(args.source_coverage, 2 * 1024 * 1024)
        manifest, _ = read_document(output / MANIFEST, 2 * 1024 * 1024)
        require(coverage["schema_id"] == manifest["schema_id"] == SCHEMA_V2
                and manifest["source_coverage_sha256"] == coverage_hash,
                "source coverage schema or digest mismatch")
        owners = {row["source_sha256"]: row for row in coverage["sources"]}
        require(owners.keys() == set(PINS.values()), "source coverage cohort differs from the pins")
        artifacts = manifest["artifact_sha256"]
    results = []
    expectation_files = []
    for relative, pinned in sorted(PINS.items()):
        if not relative.endswith(".rel"):
            continue
        wire, header = scan(args.source_copies, relative, pinned)
        counts = Counter(row["clock_mode"] for row in wire)
        repeated = sum(row["archive_version"] != row["first_applying_archive_version"] for row in wire)
        receipt = {
            "fixture": relative, "source_sha256": pinned, "archive_publications": len(wire),
            "allocation_count": counts["allocate"], "reserved_binding_count": counts["bind_reserved"],
            "first_applying_repeat_count": repeated,
        }
        if owners:
            owner = owners[pinned]
            observed_scope = owner["header_observation"]["native_scope_observed_from_header"]
            require(owner["owner_uuid"] == header["owner_uuid"]
                    and (observed_scope or "unspecified") == header["scope"],
                    "native Archive owner header differs from pinned bytes")
            receipt["binding_projection_sha256"] = verify_bindings(output, artifacts, owner, wire)
            if args.expectation_directory:
                filename = f'{owner["copy_id"]}-archive-binding-expectation.json'
                value = {
                    "schema_id": "reliquary-archive-binding-expectation-v1",
                    "source_file_sha256": pinned,
                    "owner": header,
                    "binding_projection": {"archive_version": str(len(wire)), "bindings": wire},
                }
                data = (json.dumps(value, sort_keys=True, separators=(",", ":")) + "\n").encode("ascii")
                expectation_files.append((filename, data))
                receipt["expectation_file"] = filename
                receipt["expectation_sha256"] = hashlib.sha256(data).hexdigest()
        results.append(receipt)
    if expectation_files:
        regular(args.expectation_directory.parent, directory=True)
        args.expectation_directory.mkdir()
        for filename, data in expectation_files:
            with (args.expectation_directory / filename).open("xb") as handle:
                handle.write(data)
    print(json.dumps({
        "status": "PASS", "authentic_admission": False, "scope": "historical Archive binding wire and cut verification" if owners
        else "independent historical Archive wire census",
        "fixtures": results,
    }, indent=2))


if __name__ == "__main__":
    main()
