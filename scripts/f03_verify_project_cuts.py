"""Verify retained project correlations and file binding memberships at each source cut."""
import argparse
import hashlib
import io
import json
from pathlib import Path

from f03_legacy_frame_clock import LegacyFrameClock, integer
from f03_verify_archived_oracle import PINS, READER, require
from f03_verify_independent_repeat import (
    COVERAGE, HEADS, MANIFEST, SCHEMAS, open_header, read_document, read_exact, read_frame,
    regular, snapshot, take_stream,
)
from f03_verify_prefix_availability import hash_field, write_jsonl
from f03_verify_project_backing import PROJECT, REPOSITORY_ID, REVISION, file_binding

DOMAIN = b"reliquary:f03:project-operational-cuts:v1\0"
NATIVE = b"reliquary:historical-owner-state:v1\0"


def canonical(value):
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode("utf-8")


def decode_correlation(payload):
    sequence = integer(payload, 8)
    global_ = integer(payload, 16)
    archive = integer(payload, 24)
    memory = integer(payload, 32)
    kind = {1: "lore", 2: "git"}.get(integer(payload, 40, 1))
    management = {1: "warlock_managed", 2: "external"}.get(integer(payload, 41, 1))
    require(sequence > 0 and kind is not None and management is not None,
            "invalid retained project correlation header")
    with io.BytesIO(payload[42:]) as data:
        fields = []
        for _ in range(3):
            size = int.from_bytes(read_exact(data, 4), "little")
            require(size <= 32 * 1024, "project correlation field exceeds native limit")
            fields.append(read_exact(data, size).decode("utf-8", errors="strict"))
        require(not data.read(1), "trailing project correlation bytes")
    repository, project, revision = fields
    require(repository and revision, "empty retained project repository or revision")
    return sequence, {
        "sequence": sequence, "global_version": global_, "archive_version": archive,
        "memory_version": memory, "repository_management": management,
        "project_revision": {"kind": kind, "repository_id": repository,
                             "project_path": project, "revision": revision},
    }


def decode_file_binding(payload):
    file_id, (content, path) = file_binding(payload)
    key = bytes.fromhex(file_id)
    return key, {
        "file_id": list(key),
        "project_revision": {"kind": "git", "repository_id": REPOSITORY_ID,
                             "project_path": PROJECT, "revision": REVISION},
        "path": path, "content_hash": list(bytes.fromhex(content)),
    }


def native_head(owner, rows):
    digest = hashlib.sha256(NATIVE + (1).to_bytes(4, "little"))
    digest.update(bytes.fromhex(owner["owner_uuid"]) + hash_field(b"project_operational"))
    digest.update(b"\0" * 9 + len(rows).to_bytes(8, "little"))
    for (family, key), (_, _, value) in sorted(rows.items()):
        digest.update(hash_field(family.encode("ascii")) + hash_field(key) + hash_field(canonical(value)))
    return digest.hexdigest()


def scan(output, artifacts, owner, schema, path, kind, head):
    before = regular(path)
    require(before.st_size == int(owner["source_byte_length"]), "project source extent differs from custody")
    clock = LegacyFrameClock()
    accepted = {}
    sequence = 0
    repository_identity = None
    with path.open("rb") as source:
        source_hash = open_header(source, kind, owner)
        for ordinal, frame in take_stream(output, artifacts, owner, "frames", owner["frame_stream"], schema_id=schema):
            payload = read_frame(source, frame, source_hash, before.st_size)
            visible = clock.consume(payload)
            require(visible == int(frame["latest_global"]), "project record frontier clock differs from source")
            marker = payload[:8]
            if marker == b"PRJCOR02":
                require(kind == 1, "project correlation in a PHY")
                next_sequence, record = decode_correlation(payload)
                require(next_sequence == sequence + 1, "noncontiguous project correlation history")
                require(record["global_version"] <= head, "project correlation refers past source head")
                revision = record["project_revision"]
                identity = (revision["kind"], revision["repository_id"])
                require(repository_identity is None or repository_identity == identity,
                        "project correlation changes repository identity")
                repository_identity = identity
                sequence = next_sequence
                accepted[("project_revision_correlation", sequence.to_bytes(8, "big"))] = (
                    visible, ordinal, record)
            elif marker == b"PRJFILE1":
                require(kind == 1, "project file binding in a PHY")
                key, record = decode_file_binding(payload)
                _, _, old = accepted.setdefault(("project_file_binding", key), (visible, ordinal, record))
                require(old == record, "conflicting project file binding")
            elif marker.startswith((b"PRJCOR", b"PRJFILE")):
                raise ValueError("unclassified project authority carrier")
        require(clock.head == head and source.tell() == before.st_size and source_hash.hexdigest() == owner["source_sha256"]
                and snapshot(before) == snapshot(regular(path)), "project prefix scan differs from pinned bytes")
    retained = [row for row in owner["native"]["native_history_projections"] if row["domain"] == "project_operational"]
    expected = native_head(owner, accepted) if kind == 1 else None
    if kind == 1:
        require(len(retained) == 1 and retained[0]["sha256"] == expected
                and retained[0]["rows"] == str(len(accepted)) and retained[0]["local_cut"] == "0",
                "independently decoded project history differs from native typed head")
    else:
        require(not retained and not accepted, "PHY has unexpected Project authority")
    return accepted, expected


def membership_rows(accepted, head):
    for (family, key), (visible, ordinal, value) in sorted(accepted.items(), key=lambda item: item[1][:2]):
        row = {
            "family": family, "stable_key_sha256": hashlib.sha256(key).hexdigest(),
            "value_sha256": hashlib.sha256(canonical(value)).hexdigest(),
            "first_physical_ordinal": str(ordinal), "first_global": str(visible), "last_global": str(head),
        }
        if family == "project_revision_correlation":
            row["declared_rel_cut"] = {field: str(value[field]) for field in
                                       ("global_version", "archive_version", "memory_version")}
            row["correlation_sequence"] = str(value["sequence"])
        yield row


def cut_rows(memberships, head, kind):
    digest = hashlib.sha256(DOMAIN)
    index = 0
    sequence = 0
    files = 0
    for global_ in range(head + 1):
        while index < len(memberships) and int(memberships[index]["first_global"]) == global_:
            row = memberships[index]
            # The retained interval ends at this source's head. The earlier
            # prefix commitment must stay stable if the source is later extended.
            committed = {field: value for field, value in row.items() if field != "last_global"}
            digest.update(hash_field(canonical(committed)))
            if row["family"] == "project_file_binding":
                files += 1
            else:
                sequence = int(row["correlation_sequence"])
            index += 1
        yield {
            "global_version": str(global_), "availability": "supported" if kind == 1 else "not_applicable",
            "retained_file_bindings": str(files), "latest_correlation_sequence": str(sequence),
            "prefix_sha256": digest.hexdigest() if kind == 1 else None,
        }
    require(index == len(memberships), "project membership lies past its physical head")


def verify(copies, coverage_path, destination):
    output = coverage_path.parent
    coverage, coverage_sha = read_document(coverage_path, 2 * 1024 * 1024)
    manifest, manifest_sha = read_document(output / MANIFEST, 2 * 1024 * 1024)
    schema = coverage["schema_id"]
    require(schema in SCHEMAS and schema == manifest["schema_id"]
            and manifest["base_reader_commit"] == READER
            and coverage_sha == manifest["source_coverage_sha256"] == manifest["artifact_sha256"][COVERAGE],
            "invalid project source witness provenance")
    owners = {row["source_sha256"]: row for row in coverage["sources"]}
    require(len(coverage["sources"]) == 6 and owners.keys() == set(PINS.values()), "incomplete project owner set")
    regular(destination.parent, directory=True)
    resolved = destination.resolve()
    require(not any(resolved == parent or resolved.is_relative_to(parent)
                    for parent in (copies.resolve(), output.resolve())),
            "project evidence destination overlaps source custody")
    destination.mkdir()
    artifacts = dict(manifest["artifact_sha256"])
    results = []
    for ((relative, pin), head) in zip(sorted(PINS.items()), HEADS):
        owner = owners[pin]
        kind = 1 if relative.endswith(".rel") else 2
        accepted, native = scan(output, artifacts, owner, schema, copies / relative, kind, head)
        members = list(membership_rows(accepted, head))
        copy = owner["copy_id"]
        results.append({
            "copy_id": copy, "source_sha256": pin, "global_cuts": head + 1, "native_typed_head_sha256": native,
            "project_revision_correlations": sum(row["family"] == "project_revision_correlation" for row in members),
            "project_file_bindings": sum(row["family"] == "project_file_binding" for row in members),
            "memberships_sha256": write_jsonl(destination / f"{copy}-project-memberships.jsonl", members),
            "cut_rows_sha256": write_jsonl(destination / f"{copy}-project-cuts.jsonl", cut_rows(members, head, kind)),
        })
    return {
        "status": "PASS", "authentic_admission": False,
        "scope": "retained Project correlation and file binding membership at every source cut",
        "source_manifest_sha256": manifest_sha, "source_coverage_sha256": coverage_sha, "owners": results,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source-copies", type=Path, required=True)
    parser.add_argument("--source-coverage", type=Path, required=True)
    parser.add_argument("--private-output", type=Path, required=True,
                        help="fresh directory under the caller's protected evidence root")
    args = parser.parse_args()
    print(json.dumps(verify(args.source_copies, args.source_coverage, args.private_output), indent=2))


if __name__ == "__main__":
    main()
