"""Verify exact physical frontiers and Archive backing availability for the six pinned sources."""
import argparse
from collections import Counter, defaultdict
from dataclasses import dataclass
import hashlib
import io
import json
from pathlib import Path

from f03_compare_archived_projections import ARCHIVED_SHA256
from f03_legacy_frame_clock import LegacyFrameClock, integer
from f03_verify_archived_oracle import PINS, require, rows
from f03_verify_independent_repeat import (
    COVERAGE, HEADS, MANIFEST, SCHEMAS, open_header, read_document, read_exact, read_frame,
    regular, snapshot, take_stream,
)
from f03_verify_project_backing import PROJECT, REPOSITORY_ID, REVISION, file_binding

ARCHIVE_WITHOUT_DIRECT_BACKING = {b"CVABRC02", b"CVACON04", b"CVAEPI02", b"CVAFRA02"}
ARCHIVE_PUBLICATIONS = ARCHIVE_WITHOUT_DIRECT_BACKING | {
    b"CVANOD02", b"CVACTM02", b"CVAFIL02", b"CVATUR03", b"CVAIMP02", b"CVAFME02", b"CVAARP02",
}
ABSENT = {
    "archive_payload_pruning": (b"CVAPRND1",),
    "community_names": (b"CVACNAM1",),
    "dream_processing_epochs": (b"CVAPEP01",),
    "ego_publications": (b"CVAECR01", b"CVAEDL01", b"CVAETM01", b"CVAEIDA1"),
    "graph_rewrite_baseline": (b"CVAGBL01",),
    "rewrite_genesis": (b"CVARGN01",),
}
NO_PRESENT_CARRIER = frozenset(marker for markers in ABSENT.values() for marker in markers)


def canonical(value):
    return json.dumps(value, ensure_ascii=True, sort_keys=True, separators=(",", ":")).encode("ascii")


def field(data, *, required=False):
    size = int.from_bytes(read_exact(data, 4), "little")
    require(size <= 16 * 1024 * 1024 and (not required or size > 0), "invalid Archive text extent")
    raw = read_exact(data, size)
    raw.decode("utf-8", errors="strict")
    return raw


def hash_field(value):
    return len(value).to_bytes(8, "little") + value


@dataclass(frozen=True)
class Attachment:
    identifier: bytes
    content: bytes
    length: bytes
    filename: bytes
    mime: bytes

    def legacy_id(self):
        data = self.content + self.length + hash_field(self.filename) + hash_field(self.mime)
        return hashlib.sha256(b"CVAFILE1-ID\0" + data).digest()

    def project_id(self, path):
        fields = b"".join(hash_field(value.encode("utf-8")) for value in
                          (REPOSITORY_ID, PROJECT, REVISION, path))
        data = self.content + self.length + hash_field(self.filename) + hash_field(self.mime)
        return hashlib.sha256(b"WARLOCK-PROJECT-FILE1-ID\0\x02" + fields + data).digest()


@dataclass(frozen=True)
class Publication:
    local: int
    global_: int
    visible: int


def turn_references(body):
    with io.BytesIO(body[16:]) as data:
        read_exact(data, 8)  # Retained timestamp is irrelevant to the backing gate.
        content = read_exact(data, 32)
        node = field(data, required=True)
        conversation = field(data, required=True)
        field(data)  # Parent.
        field(data, required=True)  # Role.
        field(data)  # Principal.
        count = int.from_bytes(read_exact(data, 4), "little")
        require(count <= (len(body) - 16 - data.tell()) // 80, "invalid Archive attachment count")
        files = []
        for _ in range(count):
            files.append(Attachment(
                read_exact(data, 32), read_exact(data, 32), read_exact(data, 8),
                field(data, required=True), field(data),
            ))
        require(not data.read(1), "trailing Archive turn bytes")
    key = hashlib.sha256(hash_field(conversation) + hash_field(node)).hexdigest()
    return key, content, files


def add_reference(table, identity, descriptor, publication):
    previous = table.setdefault(identity, (descriptor, publication))
    require(previous[0] == descriptor, "conflicting retained Archive reference identity")


def missing_interval(publication, resolved, head):
    if publication > head or (resolved is not None and resolved <= publication):
        return None
    return publication, head if resolved is None else min(head, resolved - 1)


def mark_missing(deltas, domain, publication, resolved, head):
    interval = missing_interval(publication, resolved, head)
    if interval:
        first, last = interval
        deltas[first][domain] += 1
        deltas[last + 1][domain] -= 1
    return interval


def file_gate(attachment, publication, inline, binding, head, deltas):
    binding_cut = None
    legacy = attachment.identifier == attachment.legacy_id()
    if binding:
        binding_cut, content, path = binding
        require(attachment.content == content, "project binding content differs from Archive reference")
        require(legacy or attachment.identifier == attachment.project_id(path),
                "project attachment identity disagrees with the native hash rules")
    if not legacy:
        require(binding_cut is not None, "non-legacy file has no project identity binding")
    present = [cut for cut in (inline, binding_cut) if cut is not None]
    first_backing = min(present) if present else None
    require(first_backing is not None, "accepted Archive file lacks all retained backing")
    first_valid_id = 0 if legacy else binding_cut
    # The native file validator checks identity before backing. Keep distinct
    # intervals rather than labelling a non-legacy identity as missing content.
    invalid = mark_missing(deltas, "invalid_file_identity", publication, first_valid_id, head)
    missing = mark_missing(deltas, "missing_file_backing", max(publication, first_valid_id),
                           first_backing, head)
    return invalid, missing, binding_cut


def bounds(interval):
    return None if interval is None else [str(point) for point in interval]


def native_absences(owner, carriers):
    require(not any(carriers[marker] for marker in NO_PRESENT_CARRIER),
            "retained presence contradicts an absence claimed by this fixture-scoped checker")
    projections = {row["domain"]: row for row in owner["native"]["native_history_projections"]}
    require(all(projections[domain]["local_cut"] == projections[domain]["rows"] == "0"
                for domain in ("ego", "dream_processing"))
            and owner["native"]["history_generation"] == "0" * 32
            and owner["native"]["rewrite_genesis"] is None,
            "native head contradicts zero clock-neutral fixture state")
    return {family: {"status": "no_retained_carrier", "carriers": [tag.decode("ascii") for tag in markers]}
            for family, markers in sorted(ABSENT.items())}


def scan(output, artifacts, owner, schema, path, kind, head):
    before = regular(path)
    require(before.st_size == int(owner["source_byte_length"]), "source size differs from custody")
    clock = LegacyFrameClock()
    frontiers = {}
    carriers = Counter()
    nodes = {}
    files = {}
    contents = {}
    bindings = {}
    archive_head = 0
    with path.open("rb") as source:
        source_sha = open_header(source, kind, owner)
        frontiers[0] = (source.tell(), 0, source_sha.hexdigest())
        for ordinal, frame in take_stream(output, artifacts, owner, "frames", owner["frame_stream"], schema_id=schema):
            payload = read_frame(source, frame, source_sha, before.st_size)
            visible = clock.consume(payload)
            require(visible == int(frame["latest_global"]), "independent physical frontier clock mismatch")
            frontiers[visible] = (source.tell(), ordinal + 1, source_sha.hexdigest())
            marker = payload[:8]
            carriers[marker] += 1
            if marker == b"CVACONT1":
                require(kind == 1 and integer(payload, 40, 4) == len(payload) - 44,
                        "invalid retained Archive content extent")
                content = payload[8:40]
                require(hashlib.sha256(payload[44:]).digest() == content, "Archive content SHA differs from identity")
                contents.setdefault(content, visible)
            elif marker == b"PRJFILE1":
                require(kind == 1, "project file binding in a PHY")
                identity, (content, repository_path) = file_binding(payload)
                key = bytes.fromhex(identity)
                binding = (visible, bytes.fromhex(content), repository_path)
                previous = bindings.setdefault(key, binding)
                require(previous[1:] == binding[1:], "conflicting project binding")
            elif marker == b"CVAPUB01":
                body = payload[25 if payload[16] == 1 else 17:]
                if body[:8] not in ARCHIVE_PUBLICATIONS:
                    continue
                require(kind == 1, "Archive publication in a PHY")
                local = integer(body, 8)
                global_ = integer(payload, 8)
                require(local == archive_head + 1 and global_ <= visible,
                        "Archive publication is outside its physical frontier")
                archive_head = local
                if body[:8] == b"CVATUR03":
                    key, content, attachments = turn_references(body)
                    publication = Publication(local, global_, visible)
                    add_reference(nodes, key, content, publication)
                    for attachment in attachments:
                        add_reference(files, attachment.identifier, attachment, publication)
                else:
                    require(body[:8] in ARCHIVE_WITHOUT_DIRECT_BACKING, "unhandled Archive publication backing rule")
        require(clock.head == head and source.tell() == before.st_size and source_sha.hexdigest() == owner["source_sha256"]
                and snapshot(before) == snapshot(regular(path)), "source prefix scan differs from its pinned owner")
    facts = native_absences(owner, carriers)
    expected = next((row["local_cut"] for row in owner["native"]["native_history_projections"]
                     if row["domain"] == "archive"), "0")
    require(str(archive_head) == expected and bindings.keys() <= files.keys(), "Archive reference cohort differs from head")
    return frontiers, nodes, files, contents, bindings, facts


def reference_intervals(nodes, files, contents, bindings, head):
    deltas = defaultdict(Counter)
    rows = []
    for key, (content, publication) in sorted(nodes.items()):
        inline = contents.get(content)
        require(inline is not None, "accepted Archive node lacks unpruned retained content")
        missing = mark_missing(deltas, "missing_node_backing", publication.visible, inline, head)
        rows.append({
            "kind": "node_content", "stable_key_sha256": key, "content_sha256": content.hex(),
            "archive_version": str(publication.local), "publication_global": str(publication.global_),
            "first_visible_global": str(publication.visible), "first_inline_global": str(inline),
            "missing_backing_cuts_inclusive": bounds(missing),
        })
    for key, (attachment, publication) in sorted(files.items()):
        inline = contents.get(attachment.content)
        invalid, missing, binding_cut = file_gate(attachment, publication.visible, inline,
                                                  bindings.get(key), head, deltas)
        rows.append({
            "kind": "file_content", "stable_key_sha256": hashlib.sha256(key).hexdigest(),
            "content_sha256": attachment.content.hex(), "archive_version": str(publication.local),
            "publication_global": str(publication.global_), "first_visible_global": str(publication.visible),
            "first_inline_global": None if inline is None else str(inline),
            "first_project_binding_global": None if binding_cut is None else str(binding_cut),
            "invalid_file_identity_cuts_inclusive": bounds(invalid),
            "missing_backing_cuts_inclusive": bounds(missing),
        })
    return rows, deltas


def cut_rows(frontiers, deltas, head, kind):
    active = Counter()
    frontier = frontiers[0]
    for global_ in range(head + 1):
        frontier = frontiers.get(global_, frontier)
        active.update(deltas[global_])
        require(all(count >= 0 for count in active.values()), "invalid backing interval sweep")
        failed = any(active.values())
        if kind == 2:
            require(not failed, "Archive backing rule applied to PHY")
        yield {
            "global_version": str(global_),
            "physical_frontier_end": str(frontier[0]),
            "physical_frame_count": str(frontier[1]),
            "physical_prefix_sha256": frontier[2],
            "archive_backing_gate": "not_applicable" if kind == 2 else "blocked" if failed else "satisfied",
            "missing_node_backing": str(active["missing_node_backing"]),
            "missing_file_backing": str(active["missing_file_backing"]),
            "invalid_file_identity": str(active["invalid_file_identity"]),
            "whole_owner_availability": "archive_backing_rejects_prefix" if failed else "not_evaluated",
        }
    require(not any(active.values()), "validated source head failed the retained Archive backing gate")


def write_jsonl(path, records):
    digest = hashlib.sha256()
    with path.open("xb") as file:
        for row in records:
            data = canonical(row) + b"\n"
            file.write(data)
            digest.update(data)
    return digest.hexdigest()


def verify(copies, coverage_path, archived_path, destination):
    output = coverage_path.parent
    coverage, coverage_sha = read_document(coverage_path, 2 * 1024 * 1024)
    manifest, manifest_sha = read_document(output / MANIFEST, 2 * 1024 * 1024)
    archive, archive_sha = read_document(archived_path, 128 * 1024 * 1024)
    schema = coverage["schema_id"]
    require(schema in SCHEMAS and schema == manifest["schema_id"] and archive_sha == ARCHIVED_SHA256
            and coverage_sha == manifest["source_coverage_sha256"] == manifest["artifact_sha256"][COVERAGE],
            "unrecognized source or archived evidence provenance")
    archived = rows(archive)
    owners = {owner["source_sha256"]: owner for owner in coverage["sources"]}
    require(len(coverage["sources"]) == 6 and owners.keys() == set(PINS.values()),
            "expected exact pinned owner coverage")
    regular(destination.parent, directory=True)
    resolved = destination.resolve()
    require(not any(resolved == parent or resolved.is_relative_to(parent)
                    for parent in (copies.resolve(), output.resolve())),
            "evidence output must be separate from source copies and original witness")
    destination.mkdir()
    artifacts = dict(manifest["artifact_sha256"])
    results = []
    for ((relative, pinned), head) in zip(sorted(PINS.items()), HEADS):
        owner = owners[pinned]
        kind = 1 if relative.endswith(".rel") else 2
        frontiers, nodes, files, contents, bindings, facts = scan(
            output, artifacts, owner, schema, copies / relative, kind, head)
        refs, deltas = reference_intervals(nodes, files, contents, bindings, head)
        cuts = list(cut_rows(frontiers, deltas, head, kind))
        samples = []
        if kind == 1:
            for sample in archived[relative]["oracle"]["global_projection"]:
                cut = sample["cut"]
                expected_blocked = sample.get("rejected") == "missing Archive reference"
                row = cuts[cut]
                blocked = row["archive_backing_gate"] == "blocked"
                require(("rejected" not in sample or expected_blocked)
                        and blocked == expected_blocked
                        and (not blocked or row["invalid_file_identity"] == "0"),
                        "archived Archive backing sample differs from the current retained prefix")
                samples.append({"global_version": str(cut), "archive_backing_gate": row["archive_backing_gate"]})
        copy = owner["copy_id"]
        results.append({
            "copy_id": copy, "source_sha256": pinned, "global_cuts": len(cuts),
            "archive_backing_blocked_cuts": sum(row["archive_backing_gate"] == "blocked" for row in cuts),
            "archive_backing_satisfied_cuts": sum(row["archive_backing_gate"] == "satisfied" for row in cuts),
            "archive_node_references": len(nodes), "archive_file_references": len(files),
            "physical_project_bindings": len(bindings), "archived_samples_matched": samples,
            "constant_no_presence": facts,
            "cut_rows_sha256": write_jsonl(destination / f"{copy}-prefix-availability.jsonl", cuts),
            "reference_intervals_sha256": write_jsonl(destination / f"{copy}-archive-reference-intervals.jsonl", refs),
        })
    return {
        "status": "PASS", "authentic_admission": False,
        "scope": "exact physical frontiers and Archive backing gate at each global cut",
        "whole_owner_positive_availability_assessed": False,
        "archived_oracle_sha256": archive_sha, "source_manifest_sha256": manifest_sha,
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
    result = verify(args.source_copies, args.source_coverage, args.archived_oracle, args.private_output)
    print(json.dumps(result, indent=2))


if __name__ == "__main__":
    main()
