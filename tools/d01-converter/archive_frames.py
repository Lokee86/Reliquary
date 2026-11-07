"""Translate the pinned RELs' Archive records to ordered current frame bodies."""
import argparse
from collections import Counter
import hashlib
import io
import json
from pathlib import Path
import stat
from struct import pack, unpack


HEADER = b"CVA\0\r\n\x1a\n\x02\0\0\0\x28\0\0\0"
ARCHIVE_PUBLICATIONS = {b"CVATUR03", b"CVABRC02", b"CVACON04", b"CVAFRA02", b"CVAEPI02"}
SCOPES = {0: "unspecified", 1: "organization", 2: "project", 3: "connection"}


def require(condition, message):
    if not condition:
        raise ValueError(message)


def regular(path):
    info = path.lstat()
    require(stat.S_ISREG(info.st_mode) and info.st_nlink == 1
            and not getattr(info, "st_file_attributes", 0) & 0x400,
            "source is not a regular single-link file")
    return info.st_dev, info.st_ino, info.st_size, info.st_mtime_ns


def take(source, count):
    data = source.read(count)
    require(len(data) == count, "truncated source Archive field")
    return data


def field(source):
    size, = unpack("<I", take(source, 4))
    value = take(source, size)
    value.decode("utf-8", errors="strict")
    return value


def wire_field(data):
    return pack("<I", len(data)) + data


def optional(data):
    return b"\1" + wire_field(data) if data else b"\0"


def compact(value):
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode("utf-8")


def consume_frames(path, owner):
    original = regular(path)
    require(original[2] == owner["byte_length"], "source Archive extent changed")
    with path.open("rb") as source:
        header = take(source, 40)
        require(header[:16] == HEADER and header[16] == {"REL": 1, "PHY": 2}[owner["kind"]] and header[17] in SCOPES
                and SCOPES[header[17]] == owner["scope"] and not any(header[18:24])
                and header[24:].hex() == owner["owner_uuid"], "source Archive owner header changed")
        digest = hashlib.sha256(header)
        ordinal = 0
        while source.tell() < original[2]:
            offset = source.tell()
            prefix = take(source, 8)
            length, = unpack("<Q", prefix)
            require(8 <= length <= 16 * 1024 * 1024 and length <= original[2] - source.tell(),
                    "invalid source Archive frame length")
            payload = take(source, length)
            digest.update(prefix)
            digest.update(payload)
            yield ordinal, payload, offset
            ordinal += 1
    require(digest.hexdigest() == owner["source_sha256"]
            and regular(path) == original, "source Archive bytes changed or differ from pinned custody")


def project_binding(payload):
    with io.BytesIO(payload[8:]) as data:
        identity, tag, hashed = take(data, 32), take(data, 1), take(data, 1)
        require(tag in (b"\1", b"\2") and hashed in (b"\0", b"\1"), "unknown project binding type")
        content = take(data, 32) if hashed == b"\1" else b""
        fields = [field(data) for _ in range(4)]
        require(all(fields) and not data.read(1), "invalid retained project binding")
        target = b"\1" + tag + b"".join(map(wire_field, fields)) + hashed + content
        return identity, (content, target)


def repository_correlation(payload):
    with io.BytesIO(payload[8:]) as data:
        sequence, global_, archive, memory = unpack("<QQQQ", take(data, 32))
        repository = {b"\1": "lore", b"\2": "git"}.get(take(data, 1))
        management = {b"\1": "warlock_managed", b"\2": "external"}.get(take(data, 1))
        repository_id, project_path, revision = [field(data).decode("utf-8") for _ in range(3)]
        require(sequence and repository and management and repository_id and revision
                and not data.read(1), "invalid retained project correlation")
    return b"RELPRC01" + compact({
        "sequence": sequence, "global_version": global_, "archive_version": archive,
        "memory_version": memory, "repository_management": management,
        "project_revision": {"repository": {"kind": repository, "repository_id": repository_id,
                                            "project_path": project_path}, "revision": revision},
    })


def backing(path, owner):
    contents = {}
    projects = {}
    for _, payload, _ in consume_frames(path, owner):
        if payload[:8] == b"CVACONT1":
            require(len(payload) >= 44 and unpack("<I", payload[40:44])[0] == len(payload) - 44,
                    "invalid Archive content length")
            identity, value = payload[8:40], payload[44:]
            require(hashlib.sha256(value).digest() == identity
                    and contents.setdefault(identity, value) == value, "invalid Archive content identity")
        elif payload[:8] == b"PRJFILE1":
            identity, value = project_binding(payload)
            require(projects.setdefault(identity, value) == value, "conflicting project binding")
    return contents, projects


def turn(data, contents, projects, used_contents, used_projects, stored_blobs):
    fixed = take(data, 40)
    identity = fixed[8:]
    require(identity in contents, "Archive turn lacks body backing")
    body = contents[identity]
    body.decode("utf-8", errors="strict")
    used_contents.add(identity)
    node, conversation, parent, role, principal = [field(data) for _ in range(5)]
    require(node and conversation and role, "incomplete Archive turn identity")
    count, = unpack("<I", take(data, 4))
    require(count <= (data.getbuffer().nbytes - data.tell()) // 80, "invalid Archive attachment count")
    output = bytearray(b"RELTURN2" + fixed + wire_field(node) + wire_field(conversation)
                       + optional(parent) + wire_field(role) + optional(principal)
                       + wire_field(body) + pack("<I", count))
    blobs = []
    for _ in range(count):
        attachment = take(data, 72)
        file_id, content_id = attachment[:32], attachment[32:64]
        length, = unpack("<Q", attachment[64:])
        name, mime = field(data), field(data)
        require(name, "empty Archive attachment filename")
        project = projects.get(file_id)
        if project is None:
            require(content_id in contents, "Archive attachment lacks backing")
            reference = b"\0"
        else:
            digest, reference = project
            require(not digest or digest == content_id, "project binding content differs")
            used_projects.add(file_id)
        if content_id in contents:
            content = contents[content_id]
            require(len(content) == length, "Archive attachment backing extent differs")
            used_contents.add(content_id)
            if content_id not in stored_blobs:
                blobs.append(b"RELFILE1" + content_id + wire_field(content))
                stored_blobs.add(content_id)
        output.extend(attachment + wire_field(name) + optional(mime) + reference)
    return blobs, bytes(output)


def publication_body(body, contents, projects, used_contents, used_projects, stored_blobs):
    marker = body[:8]
    with io.BytesIO(body[16:]) as data:
        if marker == b"CVATUR03":
            blobs, encoded = turn(data, contents, projects, used_contents, used_projects, stored_blobs)
        elif marker == b"CVABRC02":
            canonical = take(data, 1)
            names = [field(data) for _ in range(3)]
            require(canonical in (b"\0", b"\1") and all(names), "invalid Archive branch")
            blobs, encoded = [], b"RELBRCH1" + canonical + b"".join(map(wire_field, names))
        elif marker == b"CVAEPI02":
            fixed = take(data, 50)
            names = [field(data) for _ in range(3)]
            require(fixed[32] in (1, 2) and fixed[33] in range(1, 6) and all(names), "invalid Archive Episode")
            blobs, encoded = [], b"RELEPIS1" + fixed + b"".join(map(wire_field, names))
        elif marker == b"CVAFRA02":
            identity = list(take(data, 32))
            conversation, start, end = [field(data).decode("utf-8") for _ in range(3)]
            require(conversation and start and end, "invalid Archive fragment")
            blobs, encoded = [], b"RELART01" + compact({
                "type": "fragment", "fragment": {"id": identity, "conversation_id": conversation,
                                                "start_node_id": start, "end_node_id": end},
            })
        else:
            conversation, title = field(data).decode("utf-8"), field(data).decode("utf-8")
            active = take(data, 1)
            policy = {b"\1": "sequential_chronological", b"\2": "visible_auto_branch"}.get(take(data, 1))
            revision, = unpack("<Q", take(data, 8))
            require(conversation and active in (b"\0", b"\1") and policy and revision,
                    "invalid Archive conversation metadata")
            blobs, encoded = [], b"RELART01" + compact({
                "type": "conversation_metadata", "conversation_id": conversation, "title": title or None,
                "active": active == b"\1", "policy": policy, "policy_revision": revision,
            })
        require(not data.read(1), "trailing Archive publication bytes")
        return blobs, encoded


def emit(out, source_ordinal, kind, body, **coordinate):
    row = {"source_ordinal": str(source_ordinal), "kind": kind, **coordinate}
    if body is not None:
        row["body_hex"] = body.hex()
    out.write(compact(row) + b"\n")


def translate(path, owner, output, handled):
    contents, projects = backing(path, owner)
    used_contents, used_projects, stored_blobs = set(), set(), set()
    reservations = set()
    counts = Counter()
    local = 0
    for ordinal, payload, _ in consume_frames(path, owner):
        marker = payload[:8]
        if marker in (b"CVAVERS1", b"CVAVERS2"):
            version, = unpack("<Q", payload[8:16])
            require(version not in reservations and version, "duplicate or invalid legacy reservation")
            reservations.add(version)
            continue
        if marker == b"PRJCOR02":
            emit(output, ordinal, "frame", repository_correlation(payload))
            disposition = "current_project_correlation"
        elif marker in (b"CVACONT1", b"PRJFILE1", b"CVAAFMT2"):
            disposition = {b"CVACONT1": "retained_in_current_archive_body",
                           b"PRJFILE1": "retained_in_current_project_attachment",
                           b"CVAAFMT2": "source_archive_format_replaced"}[marker]
        elif marker == b"CVAPUB01":
            require(len(payload) >= 25 and payload[16] in (0, 1, 2), "invalid legacy publication envelope")
            mode = payload[16]
            start = 25 if mode == 1 else 17
            body = payload[start:]
            marker = body[:8]
            if marker not in ARCHIVE_PUBLICATIONS:
                continue
            version, = unpack("<Q", payload[8:16])
            next_local, = unpack("<Q", body[8:16])
            require(next_local == local + 1 and version > 0 and (version in reservations) == (mode == 2),
                    "Archive publication coordinates do not bind accepted ordering")
            local = next_local
            blobs, record = publication_body(body, contents, projects, used_contents, used_projects, stored_blobs)
            for blob in blobs:
                emit(output, ordinal, "frame", blob)
            record = b"RELAPB01" + pack("<Q", local) + b"\0" + record
            coordinate = {"global_version": str(version)}
            if mode != 2:
                coordinate["transaction_time_ns"] = str(unpack("<q", payload[17:25])[0]) if mode == 1 else None
            emit(output, ordinal, "bind_reserved" if mode == 2 else "allocate", record, **coordinate)
            disposition = "current_archive_publication"
        else:
            continue
        counts[marker.decode("ascii")] += 1
        handled.write(compact({"source_ordinal": str(ordinal), "source_carrier": marker.decode("ascii"),
                               "disposition": disposition}) + b"\n")
    require(used_contents == contents.keys() and used_projects == projects.keys()
            and local > 0 and counts["CVAAFMT2"] == 1, "Archive backing or publications were not fully accounted for")
    return {"archive_head": local, "owned_carrier_counts": dict(counts), "project_bindings": len(used_projects),
            "inline_contents": len(used_contents), "target_file_blobs": len(stored_blobs)}


def sha256(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--inputs", type=Path, required=True)
    parser.add_argument("--expected-inputs-sha256", required=True)
    parser.add_argument("--copies", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    require(regular(args.inputs)[2] <= 64 * 1024, "custody inventory is too large")
    inputs = args.inputs.read_bytes()
    require(hashlib.sha256(inputs).hexdigest() == args.expected_inputs_sha256, "custody inventory SHA-256 differs")
    manifest = json.loads(inputs)
    require(manifest["status"] == "VERIFIED_DISPOSABLE_COPY_INPUTS" and manifest["preserves_original_sources"]
            and len(manifest["sources"]) == 6, "custody inventory did not verify the six originals and copies")
    owners = [owner for owner in manifest["sources"] if owner["kind"] == "REL"]
    require(len(owners) == 3 and len({owner["source_sha256"] for owner in owners}) == 3,
            "custody inventory does not identify three distinct REL files")
    require(not args.output.resolve().is_relative_to(args.copies.resolve()), "Archive output overlaps source copies")
    args.output.mkdir()
    results = []
    for owner in owners:
        path = Path(owner["fixture"])
        require(not path.is_absolute() and ".." not in path.parts and path.suffix == ".rel"
                and owner["source_sha256"] == owner["verified_disposable_sha256"], "invalid REL custody row")
        label = path.parent.name
        output, handled = args.output / f"{label}.archive.jsonl", args.output / f"{label}.archive-handled.jsonl"
        with output.open("xb") as out, handled.open("xb") as dispositions:
            row = translate(args.copies / path, owner, out, dispositions)
        results.append({**row, "fixture": owner["fixture"], "source_sha256": owner["source_sha256"],
                        "archive_frames": output.name, "archive_frames_sha256": sha256(output),
                        "archive_handled": handled.name, "archive_handled_sha256": sha256(handled)})
    receipt = {"status": "PASS", "scope": "Archive frame translation only",
               "input_custody_sha256": args.expected_inputs_sha256, "sources": results}
    with (args.output / "archive-transfer-receipt.json").open("xb") as report:
        report.write(compact(receipt) + b"\n")
    print(compact(receipt).decode("utf-8"))


if __name__ == "__main__":
    main()
