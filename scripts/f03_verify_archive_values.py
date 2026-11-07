"""Independently reproduce exact retained Archive value commitments from the pinned REL bytes."""
import argparse
from collections import Counter, defaultdict
import hashlib
import io
import json
from pathlib import Path

from f03_compare_archived_projections import ARCHIVED_SHA256
from f03_legacy_frame_clock import integer
from f03_verify_archived_oracle import PINS, READER, require, rows
from f03_verify_independent_repeat import (
    COVERAGE, MANIFEST, SCHEMAS, open_header, read_document, read_exact, read_frame,
    regular, snapshot, take_stream,
)
from f03_verify_prefix_availability import ARCHIVE_PUBLICATIONS, field, hash_field, write_jsonl
from f03_verify_project_backing import PROJECT, REPOSITORY_ID, REVISION, file_binding

NATIVE = b"reliquary:historical-owner-state:v1\0"
PREFIX = b"reliquary:f03:retained-archive-value-prefix:v1\0"


def canonical(value):
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode("utf-8")


def text(data, *, required=False):
    return field(data, required=required).decode("utf-8")


def number(data, size=8, *, signed=False):
    return int.from_bytes(read_exact(data, size), "little", signed=signed)


def boolean(data):
    value = number(data, 1)
    require(value in {0, 1}, "invalid retained Archive boolean")
    return bool(value)


def decode_turn(data):
    timestamp = number(data, signed=True)
    content_id = read_exact(data, 32).hex()
    node = {
        "id": text(data, required=True), "conversation": text(data, required=True),
        "parent": text(data) or None, "role": text(data, required=True), "principal": text(data) or None,
        "timestamp_ns": timestamp, "content_id": content_id,
    }
    count = number(data, 4)
    require(count <= (data.getbuffer().nbytes - data.tell()) // 80, "invalid retained attachment count")
    files = []
    for _ in range(count):
        files.append({
            "id": read_exact(data, 32).hex(), "content_id": read_exact(data, 32).hex(),
            "byte_length": number(data), "filename": text(data, required=True), "mime_type": text(data) or None,
        })
    return {"turn": {"node": node, "attachments": files}}


def decode_record(body):
    marker = body[:8]
    with io.BytesIO(body[16:]) as data:
        if marker == b"CVATUR03":
            value = decode_turn(data)
        elif marker == b"CVABRC02":
            canonical_branch = boolean(data)
            value = {"branch": {"id": text(data), "conversation": text(data),
                                "leaf": text(data), "canonical": canonical_branch}}
        elif marker == b"CVACON04":
            identity, title, active = text(data), text(data), boolean(data)
            policy = {1: "sequential_chronological", 2: "visible_auto_branch"}.get(number(data, 1))
            revision = number(data)
            require(policy is not None and revision > 0, "invalid retained conversation policy")
            value = {"conversation": {"id": identity, "title": title or None, "active": active,
                                      "policy": policy, "policy_revision": revision}}
        elif marker == b"CVAFRA02":
            value = {"fragment": {"id": read_exact(data, 32).hex(), "conversation": text(data),
                                  "start": text(data), "end": text(data)}}
        elif marker == b"CVAEPI02":
            identity = read_exact(data, 32).hex()
            origin, boundary = number(data, 1), number(data, 1)
            require(origin in {1, 2} and boundary in {1, 2, 3, 4, 5}, "unknown retained Episode tags")
            first, last = number(data, signed=True), number(data, signed=True)
            value = {"episode": {"id": identity, "origin": origin, "boundary": boundary,
                                 "source_through_ns": first, "finalized_at_ns": last,
                                 "conversation": text(data), "start": text(data), "end": text(data)}}
        else:
            raise ValueError("unclassified retained Archive value")
        require(not data.read(1), "trailing retained Archive publication bytes")
    return value


def scan(output, artifacts, owner, schema, path):
    before = regular(path)
    require(before.st_size == int(owner["source_byte_length"]), "Archive source extent differs from custody")
    native = owner["native"]
    binding = iter(take_stream(output, artifacts, owner, "archive-bindings",
                               native["archive_publication_bindings"]["bindings"], schema_id=schema))
    publications = []
    contents = {}
    projects = {}
    with path.open("rb") as source:
        source_hash = open_header(source, 1, owner)
        for _, frame in take_stream(output, artifacts, owner, "frames", owner["frame_stream"], schema_id=schema):
            payload = read_frame(source, frame, source_hash, before.st_size)
            marker = payload[:8]
            if marker == b"CVACONT1":
                content = payload[8:40]
                value = payload[44:]
                require(integer(payload, 40, 4) == len(value) and hashlib.sha256(value).digest() == content,
                        "Archive body differs from retained content address")
                require(contents.setdefault(content.hex(), value) == value, "conflicting Archive content bytes")
            elif marker == b"PRJFILE1":
                identity, (content, repository_path) = file_binding(payload)
                reference = {
                    "kind": "git", "repository_id": REPOSITORY_ID, "project_path": PROJECT,
                    "revision": REVISION, "path": repository_path, "content_hash": content,
                }
                require(projects.setdefault(identity, reference) == reference, "conflicting Archive repository backing")
            elif marker == b"CVAPUB01":
                body = payload[25 if payload[16] == 1 else 17:]
                if body[:8] not in ARCHIVE_PUBLICATIONS:
                    continue
                item = next(binding, None)
                require(item is not None, "unbound Archive value publication")
                ordinal, coordinates = item
                local = integer(body, 8)
                global_ = integer(payload, 8)
                require(local == ordinal + 1 and coordinates["archive_version"] == str(local)
                        and coordinates["global_version"] == str(global_)
                        and coordinates["first_applying_archive_version"] == str(local),
                        "Archive value disagrees with the verified first-applying binding")
                timestamp = coordinates["transaction_time_ns"]
                publications.append({
                    "archive_version": local, "global_version": global_,
                    "transaction_time_ns": None if timestamp is None else int(timestamp),
                    "record": decode_record(body),
                })
        require(next(binding, None) is None and source.tell() == before.st_size
                and source_hash.hexdigest() == owner["source_sha256"]
                and snapshot(before) == snapshot(regular(path)), "Archive value scan differs from pinned source")
    return publications, contents, projects


def retained_rows(publications, contents, projects):
    content_births = {}
    used_projects = set()
    for value in publications:
        local = value["archive_version"]
        turn = value["record"].get("turn")
        if turn:
            content = turn["node"]["content_id"]
            require(content in contents, "node lacks its retained inline body")
            content_births.setdefault(content, local)
            for file in turn["attachments"]:
                content = file["content_id"]
                reference = projects.get(file["id"])
                file["project_reference"] = reference
                if reference:
                    require(reference["content_hash"] == content, "file and repository body identities differ")
                    used_projects.add(file["id"])
                if content in contents:
                    require(len(contents[content]) == file["byte_length"], "inline Archive attachment extent differs")
                    content_births.setdefault(content, local)
                else:
                    require(reference is not None, "file lacks retained inline or repository backing")
        yield "publication", local.to_bytes(8, "big"), local, canonical(value)
    require(used_projects == projects.keys(), "unreferenced retained project file binding")
    for content, first in content_births.items():
        yield "content", bytes.fromhex(content), first, canonical({"bytes_hex": contents[content].hex()})


def commitment(family, key, value):
    return {
        "family": family, "stable_key_sha256": hashlib.sha256(key).hexdigest(),
        "canonical_value_sha256": hashlib.sha256(value).hexdigest(),
        "canonical_byte_length": str(len(value)), "availability": "retained",
    }


def verify_existing_rows(output, artifacts, owner, schema, indexed, head):
    observed_rows = {}
    observed_membership = {}
    for _, row in take_stream(output, artifacts, owner, "records", owner["native"]["record_stream"], schema_id=schema):
        if row["projection_domain"] != "archive":
            continue
        key = (row["family"], row["stable_key_sha256"])
        require(key not in observed_rows, "duplicate native Archive value")
        observed_rows[key] = {field: row[field] for field in
                              ("canonical_value_sha256", "canonical_byte_length", "availability")}
    cut_stream = owner["native"]["clocked_cut_membership"]["stream"]
    for _, interval in take_stream(output, artifacts, owner, "clocked-membership", cut_stream, schema_id=schema):
        row = interval["row"]
        if row["projection_domain"] != "archive":
            continue
        key = (row["family"], row["stable_key_sha256"])
        require(key not in observed_membership and interval["last_cut"] == str(head),
                "Archive row does not persist through its retained head")
        observed_membership[key] = (interval["first_cut"], row["canonical_value_sha256"], row["availability"])
    for (family, key), (first, value) in indexed.items():
        committed = commitment(family, key, value)
        identity = (family, committed["stable_key_sha256"])
        require(observed_rows.pop(identity, None) == {
            field: committed[field] for field in ("canonical_value_sha256", "canonical_byte_length", "availability")
        } and observed_membership.pop(identity, None) == (
            str(first), committed["canonical_value_sha256"], "retained"
        ), "independently decoded Archive value or first-cut membership differs from the native witness")
    require(not observed_rows and not observed_membership, "native Archive rows are missing from the independent value history")


class NativeArchive:
    def __init__(self, owner, indexed, head):
        self.prefix = NATIVE + (1).to_bytes(4, "little") + bytes.fromhex(owner["owner_uuid"]) + hash_field(b"archive")
        self.content = []
        self.publications = bytearray()
        self.publication_ends = [0]
        births = Counter()
        for (family, key), (first, value) in sorted(indexed.items()):
            preimage = hash_field(family.encode("ascii")) + hash_field(key) + hash_field(value)
            if family == "content":
                self.content.append((first, preimage))
                births[first] += 1
            else:
                self.publications.extend(preimage)
                self.publication_ends.append(len(self.publications))
        require(len(self.publication_ends) == head + 1, "independent Archive publication closure differs from its head")
        self.row_counts = []
        total = 0
        for cut in range(head + 1):
            total += births[cut]
            self.row_counts.append(cut + total)

    def at(self, cut):
        count = self.row_counts[cut]
        digest = hashlib.sha256(self.prefix + cut.to_bytes(8, "little") + b"\0" + count.to_bytes(8, "little"))
        for first, data in self.content:
            if first <= cut:
                digest.update(data)
        digest.update(memoryview(self.publications)[:self.publication_ends[cut]])
        return {"cut": cut, "event_cut": None, "rows": count, "sha256": digest.hexdigest()}


def cut_commitments(by_cut, head):
    digest = hashlib.sha256(PREFIX)
    count = 0
    for cut in range(head + 1):
        for row in sorted(by_cut.get(cut, ()), key=lambda row: (row["family"], row["stable_key_sha256"])):
            digest.update(cut.to_bytes(8, "big"))
            digest.update(hash_field(canonical(row)))
            count += 1
        yield {"archive_version": str(cut), "retained_rows": str(count), "prefix_sha256": digest.hexdigest()}


def verify(copies, coverage_path, archived_path, destination):
    output = coverage_path.parent
    coverage, coverage_sha = read_document(coverage_path, 2 * 1024 * 1024)
    manifest, manifest_sha = read_document(output / MANIFEST, 2 * 1024 * 1024)
    archive, archive_sha = read_document(archived_path, 128 * 1024 * 1024)
    schema = coverage["schema_id"]
    require(schema in SCHEMAS and schema == manifest["schema_id"] and manifest["base_reader_commit"] == READER
            and archive_sha == ARCHIVED_SHA256
            and coverage_sha == manifest["source_coverage_sha256"] == manifest["artifact_sha256"][COVERAGE],
            "unrecognized source witness or archive custody")
    archived = rows(archive)
    owners = {row["source_sha256"]: row for row in coverage["sources"]}
    require(len(coverage["sources"]) == 6 and owners.keys() == set(PINS.values()), "source coverage differs from reviewed pins")
    regular(destination.parent, directory=True)
    resolved = destination.resolve()
    require(not any(resolved == parent or resolved.is_relative_to(parent)
                    for parent in (copies.resolve(), output.resolve())),
            "Archive value evidence destination overlaps source custody")
    destination.mkdir()
    artifacts = dict(manifest["artifact_sha256"])
    results = []
    for relative, pin in sorted(PINS.items()):
        if not relative.endswith(".rel"):
            continue
        owner = owners[pin]
        publications, contents, projects = scan(output, artifacts, owner, schema, copies / relative)
        head = len(publications)
        indexed = {}
        for family, key, first, value in retained_rows(publications, contents, projects):
            require((family, key) not in indexed, "duplicate independently decoded Archive row")
            indexed[(family, key)] = first, value
        verify_existing_rows(output, artifacts, owner, schema, indexed, head)
        model = NativeArchive(owner, indexed, head)
        expected = archived[relative]["oracle"]["archive"]
        require(expected["head"] == head, "native Archive head differs from frozen oracle")
        known = next(row for row in owner["native"]["native_history_projections"] if row["domain"] == "archive")
        actual = model.at(head)
        require(actual["sha256"] == known["sha256"] and str(actual["rows"]) == known["rows"],
                "independent Archive head differs from native projection")
        for cut in expected["snapshots"]:
            require(model.at(cut["cut"]) == cut, "independent Archive snapshot differs from the frozen cut")
        by_cut = defaultdict(list)
        members = []
        for (family, key), (first, value) in sorted(indexed.items()):
            row = commitment(family, key, value)
            by_cut[first].append(row)
            members.append({"row": row, "first_archive_version": str(first), "last_archive_version": str(head)})
        copy = owner["copy_id"]
        results.append({
            "copy_id": copy, "source_sha256": pin, "archive_publication_values": head,
            "inline_content_values": len(indexed) - head, "repository_backed_files": len(projects),
            "retained_archive_cuts": head + 1, "native_archive_head_sha256": actual["sha256"],
            "archived_native_snapshot_digests_matched": len(expected["snapshots"]),
            "archived_native_snapshots_exhaustive": expected["snapshot_queries_exhaustive"],
            "value_memberships_sha256": write_jsonl(destination / f"{copy}-archive-value-memberships.jsonl", members),
            "value_cuts_sha256": write_jsonl(destination / f"{copy}-archive-value-cuts.jsonl", cut_commitments(by_cut, head)),
        })
    return {
        "status": "PASS", "authentic_admission": False,
        "scope": "retained Archive native value and local-cut membership for the three pinned RELs",
        "physical_prefix_availability_assessed": False,
        "archive_oracle_sha256": archive_sha, "source_manifest_sha256": manifest_sha,
        "source_coverage_sha256": coverage_sha, "owners": results,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source-copies", type=Path, required=True)
    parser.add_argument("--source-coverage", type=Path, required=True)
    parser.add_argument("--archived-oracle", type=Path, required=True)
    parser.add_argument("--private-output", type=Path, required=True,
                        help="fresh directory under the caller's protected evidence root")
    args = parser.parse_args()
    print(json.dumps(verify(args.source_copies, args.source_coverage, args.archived_oracle, args.private_output), indent=2))


if __name__ == "__main__":
    main()
