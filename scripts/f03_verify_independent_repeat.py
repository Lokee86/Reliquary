"""Independently check F03 batch custody and compare two historical exports.

A matching repeat is evidence about the streams that were exported. An
INCOMPLETE exporter remains INCOMPLETE even if both runs match.
"""
import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path
import re
import stat

from f03_verify_archived_oracle import PINS, READER, require

SCHEMA = "reliquary-f03-historical-state-v1"
SCHEMA_V2 = "reliquary-f03-historical-state-v2"
SCHEMAS = {SCHEMA, SCHEMA_V2}
COVERAGE = "source-coverage.json"
MANIFEST = "source-witness-manifest.json"
CUSTODY = "bd2c6bee9966a3e62bdd1deb6a0e29e34f74cc13bad622f7898c151084153c5d"
HEADS = (18938, 1088, 52804, 2215, 11725, 5221)
REPARSE_POINT = 0x400


def exact_object(pairs):
    result = dict(pairs)
    require(len(result) == len(pairs), "duplicate JSON property")
    return result


def bad_constant(value):
    raise ValueError(f"non-JSON numeric constant {value}")


def regular(path, directory=False):
    info = path.lstat()
    correct_kind = stat.S_ISDIR if directory else stat.S_ISREG
    require(correct_kind(info.st_mode)
            and not getattr(info, "st_file_attributes", 0) & REPARSE_POINT
            and (directory or info.st_nlink == 1),
            f"unsafe file or directory {path.name}")
    return info


def snapshot(info):
    return info.st_dev, info.st_ino, info.st_size, info.st_mtime_ns


def read_document(path, size_limit):
    before = regular(path)
    require(0 < before.st_size <= size_limit, f"invalid artifact size for {path.name}")
    content = path.read_bytes()
    require(snapshot(before) == snapshot(regular(path)), f"artifact changed {path.name}")
    return json.loads(content.decode("utf-8"), object_pairs_hook=exact_object,
                      parse_constant=bad_constant), hashlib.sha256(content).hexdigest()


def decimal(value):
    require(isinstance(value, str) and re.fullmatch(r"0|[1-9][0-9]{0,19}", value),
            "noncanonical unsigned decimal")
    return int(value)


def read_exact(file, size):
    data = file.read(size)
    require(len(data) == size, "truncated historical container")
    return data


def open_header(file, kind, observed):
    header = read_exact(file, 40)
    require(header[:16] == b"CVA\0\r\n\x1a\n\x02\0\0\0\x28\0\0\0"
            and header[16] == kind and not any(header[18:24])
            and header[24:40].hex() == observed["owner_uuid"],
            "owner header differs from source witness")
    scope = {0: None, 1: "organization", 2: "project", 3: "connection"}.get(header[17])
    require(header[17] in range(4) and (kind == 1 or scope is None)
            and scope == observed["header_observation"]["native_scope_observed_from_header"],
            "owner scope differs from source witness")
    return hashlib.sha256(header)


def read_frame(file, witness, source_hash, owner_bytes):
    require(file.tell() == decimal(witness["offset"]), "frame offset differs from source")
    prefix = read_exact(file, 8)
    size = int.from_bytes(prefix, "little")
    require(8 <= size <= 16 * 1024 * 1024 and file.tell() + size <= owner_bytes,
            "frame length exceeds its source bound")
    payload = read_exact(file, size)
    require(file.tell() == decimal(witness["end_offset"])
            and hashlib.sha256(payload).hexdigest() == witness["payload_sha256"],
            "frame length or SHA differs from source")
    carrier = payload[:8]
    if carrier == b"CVAPUB01":
        require(len(payload) >= 25 and payload[16] in (0, 1, 2), "invalid publication framing")
        body_start = 25 if payload[16] == 1 else 17
        carrier = payload[body_start:body_start + 8]
        detail = witness["details"]
        require(detail["outer_carrier"] == "CVAPUB01"
                and decimal(detail["global_version"]) == int.from_bytes(payload[8:16], "little")
                and detail["clock"] == ("bind_reserved" if payload[16] == 2 else "allocate"),
                "publication coordinates differ from source")
        time = str(int.from_bytes(payload[17:25], "little", signed=True)) if payload[16] == 1 else None
        require(detail["transaction_time_ns_in_envelope"] == time,
                "publication transaction timestamp differs from source")
    require(carrier.decode("ascii") == witness["carrier"], "carrier differs from source")
    source_hash.update(prefix)
    source_hash.update(payload)


def take_stream(output, artifacts, owner, stream, metadata, *, schema_id=SCHEMA):
    ordinal = 0
    predecessor = "0" * 64
    for index in range(decimal(metadata["batches"])):
        name = f'{owner["copy_id"]}-{stream}-{index:06}.json'
        require(name in artifacts, f"missing {name}")
        batch, digest = read_document(output / name, 16 * 1024 * 1024)
        require(artifacts.pop(name) == digest, f"SHA mismatch for {name}")
        rows = batch["rows"]
        require(batch == {
            "schema_id": schema_id, "copy_id": owner["copy_id"], "source_sha256": owner["source_sha256"],
            "stream": stream, "batch_index": str(index), "previous_sha256": predecessor,
            "first_ordinal": str(ordinal), "rows": rows,
        } and isinstance(rows, list) and 0 < len(rows) <= 128, f"invalid batch chain at {name}")
        for row in rows:
            yield ordinal, row
            ordinal += 1
        predecessor = digest
    require(metadata == {"rows": str(ordinal), "batches": metadata["batches"],
                         "last_batch_sha256": predecessor}, f"wrong {stream} stream totals")


def verify_record_rows(output, artifacts, owner, stream, schema_id):
    require(decimal(stream["rows"]) > 0, "owner has no native record witness")
    by_domain = Counter()
    seen = set()
    for _, row in take_stream(output, artifacts, owner, "records", stream, schema_id=schema_id):
        require(row.keys() == {"projection_domain", "family", "stable_key_sha256",
                               "canonical_value_sha256", "canonical_byte_length", "availability",
                               "origin_coordinate"}
                and isinstance(row["origin_coordinate"], dict)
                and all(isinstance(value, str) and re.fullmatch(r"0|-?[1-9][0-9]{0,19}", value)
                        for value in row["origin_coordinate"].values())
                and all(isinstance(row[key], str) and re.fullmatch(r"[a-f0-9]{64}", row[key])
                        for key in ("stable_key_sha256", "canonical_value_sha256"))
                and decimal(row["canonical_byte_length"]) > 0
                and row["availability"] in {"retained", "pruned", "tombstoned", "inert"},
                "invalid native record witness")
        key = (row["projection_domain"], row["family"], row["stable_key_sha256"])
        require(all(isinstance(value, str) and value for value in key) and key not in seen,
                "duplicate or malformed native record identity")
        seen.add(key)
        by_domain[key[0]] += 1
    projections = owner["native"]["native_history_projections"]
    declared = {proof["domain"]: decimal(proof["rows"]) for proof in projections}
    require(len(declared) == len(projections) and by_domain.keys() <= declared.keys()
            and all(by_domain[key] == count for key, count in declared.items()),
            "record stream does not match the native projection row counts")


def verify_v2_streams(output, artifacts, owner, kind):
    native = owner["native"]
    membership = native["clocked_cut_membership"]
    streams = {
        "clocked-membership": membership["stream"],
        "clocked-composition": native["clocked_composition_stream"],
    }
    if kind == 1:
        bindings = native["archive_publication_bindings"]
        streams.update({"archive-bindings": bindings["bindings"], "archive-binding-cuts": bindings["cuts"]})
        freshness = native["freshness_activity_proofs"]
        if freshness["coverage"] == "all_activity_cuts":
            streams["freshness-activity"] = freshness["stream"]
        else:
            require(freshness == {
                "coverage": "unaudited", "reason": "exhaustive_activity_proof_not_requested",
            }, "invalid Freshness activity coverage")
    for name, metadata in streams.items():
        for _, row in take_stream(output, artifacts, owner, name, metadata, schema_id=SCHEMA_V2):
            require(isinstance(row, dict), f"invalid {name} row")


def verify_owner(output, artifacts, owner, path, relative, digest, head, schema_id):
    kind = 1 if relative.endswith(".rel") else 2
    before = regular(path)
    require(owner["source_sha256"] == digest and before.st_size == decimal(owner["source_byte_length"]),
            "owner differs from pinned custody")
    with path.open("rb") as file:
        source_hash = open_header(file, kind, owner)
        counts = Counter()
        groups = Counter()
        global_max = 0
        for _, row in take_stream(output, artifacts, owner, "frames", owner["frame_stream"], schema_id=schema_id):
            read_frame(file, row, source_hash, before.st_size)
            counts[row["carrier"]] += 1
            groups[row["group"]] += 1
            latest = decimal(row["latest_global"])
            require(global_max <= latest <= head, "invalid frame high-water clock")
            global_max = latest
        require(file.tell() == before.st_size and source_hash.hexdigest() == digest
                and snapshot(before) == snapshot(regular(path)), "frame scan did not consume stable pinned bytes")
    require(global_max == head and dict(counts) == {
        key: decimal(value) for key, value in owner["physical_carrier_counts"].items()
    }, "frame carrier or clock counts differ from coverage")
    require(all(groups[key] == decimal(value["physical_carrier_count"])
                for key, value in owner["groups"].items())
            and groups.keys() <= owner["groups"].keys(), "frame group counts differ from coverage")
    native = owner["native"]
    require(native["owner_kind"] == ("reliquary" if kind == 1 else "phylactery"), "owner kind mismatch")
    global_stream = native["global_stream"]
    require(decimal(global_stream["rows"]) == head + 1, "global coordinate count differs from V01 head")
    for ordinal, row in take_stream(output, artifacts, owner, "global", global_stream, schema_id=schema_id):
        require(decimal(row["global_version"]) == ordinal, "global coordinate is missing or out of order")
    freshness_stream = native["freshness_stream"]
    if kind == 1:
        require(decimal(freshness_stream["rows"]) > 0, "REL has no Freshness witness")
        for _, row in take_stream(output, artifacts, owner, "freshness", freshness_stream, schema_id=schema_id):
            require(isinstance(row["kind"], str), "Freshness row has no type")
    else:
        require(freshness_stream == {"availability": "unsupported", "reason": "REL_only"},
                "PHY Freshness availability is invalid")
    if "record_stream" in native:
        verify_record_rows(output, artifacts, owner, native["record_stream"], schema_id)
    if schema_id == SCHEMA_V2:
        verify_v2_streams(output, artifacts, owner, kind)
    return snapshot(before)


def verify_output(output, copies, exporter_hash):
    regular(output, directory=True)
    regular(copies, directory=True)
    manifest, manifest_hash = read_document(output / MANIFEST, 2 * 1024 * 1024)
    coverage, coverage_hash = read_document(output / COVERAGE, 2 * 1024 * 1024)
    require(manifest["base_reader_commit"] == READER and manifest["expected_custody_sha256"] == CUSTODY
            and manifest["producer_binary_sha256"] == exporter_hash
            and manifest["source_coverage_sha256"] == coverage_hash,
            "manifest provenance mismatch")
    schema_id = manifest["schema_id"]
    require(schema_id in SCHEMAS, "unknown historical source witness schema")
    for document in (manifest, coverage):
        require(document["schema_id"] == schema_id and document["producer_role"] == "source_reader"
                and document["status"] == "INCOMPLETE" and document["authentic_admission"] is False,
                "unsupported source witness acceptance contract")
    artifacts = dict(manifest["artifact_sha256"])
    inventory = set(artifacts) | {MANIFEST}
    require({entry.name for entry in output.iterdir()} == inventory
            and artifacts.pop(COVERAGE, None) == coverage_hash, "artifact membership or coverage hash mismatch")
    source_rows = coverage["sources"]
    require(len(source_rows) == len(PINS), "expected all six pinned source files")
    owner_groups = {}
    for row in source_rows:
        owner_groups.setdefault(row["owner_uuid"], []).append(row["copy_id"])
    repeated_owners = sorted(ids for ids in owner_groups.values() if len(ids) > 1)
    snapshots = []
    for index, ((relative, digest), head, owner) in enumerate(zip(sorted(PINS.items()), HEADS, source_rows), 1):
        require(owner["copy_id"] == f"copy-{index:04}", "owner ordering mismatch")
        owner_path = copies / relative
        regular(owner_path.parent, directory=True)
        snapshots.append(verify_owner(output, artifacts, owner, owner_path, relative, digest, head, schema_id))
    require(not artifacts and {entry.name for entry in output.iterdir()} == inventory,
            "unconsumed or changed artifact inventory")
    mtimes = manifest.pop("source_file_mtimes_unix_ns")
    require(mtimes == {r["copy_id"]: str(s[-1]) for r, s in zip(source_rows, snapshots)},
            "source mtime differs from export")
    totals = {
        "frames": sum(decimal(r["frame_stream"]["rows"]) for r in source_rows),
        "global_points": sum(decimal(r["native"]["global_stream"]["rows"]) for r in source_rows),
        "record_commitments": sum(decimal(r["native"]["record_stream"]["rows"])
                                  for r in source_rows if "record_stream" in r["native"]),
    }
    return manifest, manifest_hash, snapshots, repeated_owners, totals


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--first-output", type=Path, required=True)
    parser.add_argument("--first-copies", type=Path, required=True)
    parser.add_argument("--repeat-output", type=Path, required=True)
    parser.add_argument("--repeat-copies", type=Path, required=True)
    parser.add_argument("--exporter", type=Path, required=True)
    args = parser.parse_args()
    with args.exporter.open("rb") as file:
        exporter_hash = hashlib.file_digest(file, "sha256").hexdigest()
    first, first_hash, first_sources, owner_groups, totals = verify_output(
        args.first_output, args.first_copies, exporter_hash)
    repeat, repeat_hash, repeat_sources, repeat_groups, repeat_totals = verify_output(
        args.repeat_output, args.repeat_copies, exporter_hash)
    require({row[:2] for row in first_sources}.isdisjoint(row[:2] for row in repeat_sources),
            "independent repeat must use separate physical copies")
    require(first == repeat and owner_groups == repeat_groups and totals == repeat_totals,
            "historical export differed between the two verified copy cohorts")
    print(json.dumps({
        "status": "INCOMPLETE",
        "repeat_equality": "PASS",
        "scope": "two matching historical export inventories and independent physical frame comparison",
        "authentic_admission": False,
        "sources": len(PINS),
        "shared_owner_id_copy_groups": owner_groups,
        "identical_artifacts": len(first["artifact_sha256"]),
        **totals,
        "source_coverage_sha256": first["source_coverage_sha256"],
        "first_manifest_sha256": first_hash,
        "repeat_manifest_sha256": repeat_hash,
        "exporter_sha256": exporter_hash,
    }, indent=2))


if __name__ == "__main__":
    main()
