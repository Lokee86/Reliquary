"""Compose one authenticated historical copy into the isolated current writer transfer."""
import argparse
from collections import defaultdict
import hashlib
import json
import os
from pathlib import Path
import sys
import tempfile


def require(valid, message):
    if not valid:
        raise ValueError(message)


def line(value):
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode("utf-8") + b"\n"


def event(ordinal, kind, **value):
    return {"source_ordinal": str(ordinal), "kind": kind, **value}


def stream(path, expected):
    with path.open("rb") as source:
        before = os.fstat(source.fileno())
        digest = hashlib.sha256()
        for data in source:
            require(0 < len(data) <= 64 * 1024 * 1024 and data.endswith(b"\n"),
                    "invalid source-transfer row length")
            digest.update(data)
            yield json.loads(data)
        after = os.fstat(source.fileno())
    require((before.st_dev, before.st_ino, before.st_size, before.st_mtime_ns)
            == (after.st_dev, after.st_ino, after.st_size, after.st_mtime_ns)
            and digest.hexdigest() == expected, "source transfer custody or SHA-256 differs")


def receipt(root, name, source_sha, *, coverage_sha=None, manifest_sha=None):
    file = root / name
    raw = file.read_bytes()
    require(0 < len(raw) < 2 * 1024 * 1024, "invalid transfer receipt size")
    value = json.loads(raw)
    if coverage_sha is not None:
        require(value["source_coverage_sha256"] == coverage_sha and value["source_manifest_sha256"] == manifest_sha,
                "transfer receipt differs from source witness")
    found = [row for row in value["sources"] if row["source_sha256"] == source_sha]
    require(len(found) == 1, "missing or ambiguous per-owner transfer receipt")
    return found[0], hashlib.sha256(raw).hexdigest()


def index(rows, field):
    result = defaultdict(list)
    previous = -1
    for row in rows:
        ordinal = int(row[field])
        require(ordinal >= 0 and ordinal >= previous, "source transfer rows out of order")
        result[ordinal].append(row)
        previous = ordinal
    return result


def memory_events(ordinal, frame, rows):
    require(len(rows) == 1, "ambiguous Memory source transfer")
    row = rows[0]
    require(row["carrier"] == frame["carrier"] and row["source_payload_sha256"] == frame["payload_sha256"],
            "Memory source transfer differs from held physical frame")
    if row["group"] is not None:
        revisions = [{"record": item["record"], "body": {"body_id": item["record"]["body_id"], **item["body"]}}
                     for item in row["revisions"]]
        yield event(ordinal, "group", group=row["group"], bodies=row["bodies"], revisions=revisions,
                    routing=row["routing"])
        return
    for body in row["bodies"]:
        yield event(ordinal, "memory_body", body=body)
    for revision in row["revisions"]:
        binding = row["clock_binding"]
        require(binding is not None and binding["global_version"] == revision["record"]["global_version"]
                and binding["clock_mode"] in ("bind_reserved", "allocate"), "invalid Memory source binding")
        body = {"body_id": revision["record"]["body_id"], **revision["body"]}
        yield event(ordinal, "memory_revision", record=revision["record"], body=body,
                    bound_reservation=binding["clock_mode"] == "bind_reserved")
    for routing in row["routing"]:
        yield event(ordinal, "memory_routing", routing=routing)


def entity_events(ordinal, frame, rows):
    require(len(rows) == 1, "ambiguous Entity source transfer")
    row = rows[0]
    if row["kind"] == "embedded":
        require(row["carrier"] == frame["carrier"] == "CVAENTF1", "invalid Entity format disposition")
        yield event(ordinal, "embedded", carrier=row["carrier"])
        return
    require(row["kind"] == "entity" and row["carrier"] == frame["carrier"]
            and row["source_payload_sha256"] == frame["payload_sha256"], "Entity transfer differs from held frame")
    yield event(ordinal, "entity", bound_reservation=row["bound_reservation"],
                publication={"family": row["family"], "value": row["value"]})


def scalar_events(ordinal, marker, payload):
    if marker in (b"CVAVERS1", b"CVAVERS2"):
        timed = marker == b"CVAVERS2"
        require(len(payload) == (24 if timed else 16), "invalid source reservation size")
        version = int.from_bytes(payload[8:16], "little")
        timestamp = str(int.from_bytes(payload[16:24], "little", signed=True)) if timed else None
        yield event(ordinal, "reservation", global_version=str(version), transaction_time_ns=timestamp)
    elif marker == b"CVAMEMF2":
        require(payload == marker, "invalid source Memory format")
        yield event(ordinal, "frame", body_hex=b"RELMFMT1".hex())
    elif marker == b"CVARELM1":
        yield event(ordinal, "frame", body_hex=(b"RELRELM1" + payload[8:]).hex())
    elif marker in (b"CVADRM01", b"CVADRP02"):
        require(len(payload) == (44 if marker == b"CVADRM01" else 16), "invalid source Dream pair framing")
        yield event(ordinal, "frame", body_hex=(b"REL" + payload[3:]).hex())
    elif marker in (b"CVDREAM1", b"CVDREAM2"):
        yield event(ordinal, "frame", body_hex=payload.hex())
    else:
        raise ValueError(f"retained source carrier requires typed translation: {marker.decode('ascii')}")


def adapter(root, name, owner_sha, field):
    owner, _ = receipt(root, f"{name}-transfer-receipt.json", owner_sha)
    prefix = "archive_" if name == "archive" else ""
    frame_path = owner[f"{prefix}frames"]
    handled_path = owner[f"{prefix}handled"]
    require(Path(frame_path).name == frame_path and Path(handled_path).name == handled_path,
            "invalid carrier transfer path")
    frames = index(stream(root / frame_path, owner[f"{prefix}frames_sha256"]), field)
    handled = index(stream(root / handled_path, owner[f"{prefix}handled_sha256"]), field)
    require(frames.keys() <= handled.keys(), "target body not bound by handled source ordinals")
    return frames, handled


def compose(args, witness):
    verify, pins = witness
    coverage, coverage_sha = verify.read_document(args.source_coverage, 2 * 1024 * 1024)
    output_dir = args.source_coverage.parent
    manifest, manifest_sha = verify.read_document(output_dir / verify.MANIFEST, 2 * 1024 * 1024)
    require(coverage["schema_id"] == manifest["schema_id"] == verify.SCHEMA_V2
            and manifest["source_coverage_sha256"] == coverage_sha, "invalid source coverage provenance")
    source_sha = pins[args.fixture]
    matches = [row for row in coverage["sources"] if row["source_sha256"] == source_sha]
    require(len(matches) == 1, "source custody differs from the reviewed witness")
    owner = matches[0]
    kind = 1 if args.fixture.endswith(".rel") else 2
    copy_id = owner["copy_id"]
    memory_dir = args.transfer_root / "memory-transfer-v1"
    graph_dir = args.transfer_root / "graph-transfer-v1"
    memory_receipt, memory_sha = receipt(memory_dir, "memory-source-transfer.json", source_sha,
                                        coverage_sha=coverage_sha, manifest_sha=manifest_sha)
    graph_receipt, graph_sha = receipt(graph_dir, "graph-source-transfer.json", source_sha,
                                      coverage_sha=coverage_sha, manifest_sha=manifest_sha)
    memory = index(stream(memory_dir / f"{copy_id}-memory-transfer.jsonl", memory_receipt["transfer_sha256"]),
                   "physical_ordinal")
    graph = index(stream(graph_dir / f"{copy_id}-graph-transfer.jsonl", graph_receipt["transfer_sha256"]),
                  "source_ordinal")
    entity_dir = args.transfer_root / "entity-resolution-transfer-v1"
    entity_receipt, _ = receipt(entity_dir, "entity-source-transfer.json", source_sha,
                                coverage_sha=coverage_sha, manifest_sha=manifest_sha)
    entity_path = entity_receipt["transfer"]
    require(Path(entity_path).name == entity_path, "invalid Entity transfer path")
    entity = index(stream(entity_dir / entity_path, entity_receipt["transfer_sha256"]), "source_ordinal")
    vector, vector_handled = adapter(args.transfer_root / "vector-transfer-v1", "vector", source_sha, "source_ordinal")
    archive, archive_handled = adapter(args.transfer_root / "archive-transfer-v1", "archive", source_sha, "source_ordinal") if kind == 1 else ({}, {})
    community, community_handled = adapter(args.transfer_root / "community-transfer-v1", "community",
                                           source_sha, "source_ordinal")
    native_adapters = [(archive, archive_handled), (vector, vector_handled), (community, community_handled)]
    native_adapters.append(adapter(args.transfer_root / "entity-resolution-transfer-v1", "resolution",
                                   source_sha, "source_ordinal"))
    if kind == 1:
        native_adapters.extend([
            adapter(args.transfer_root / "insomnia-work-transfer-v1", "insomnia-work", source_sha, "source_ordinal"),
            adapter(args.freshness_target_transfer or args.transfer_root / "freshness-target-transfer-v2",
                    "freshness-target", source_sha, "source_ordinal"),
        ])
    artifacts = manifest["artifact_sha256"].copy()
    native = owner["native"]
    header = {
        "schema_id": "reliquary-d01-authentic-native-frames-v1", "source_sha256": source_sha,
        "source_frames": owner["frame_stream"]["rows"], "file_kind": "rel" if kind == 1 else "phy",
        "owner_uuid": owner["owner_uuid"],
        "scope": owner["header_observation"]["native_scope_observed_from_header"] or "unspecified",
        "global_head": str(int(native["global_stream"]["rows"]) - 1),
    }
    path = args.source_copies / args.fixture
    before = verify.regular(path)
    digest = hashlib.sha256()
    with tempfile.TemporaryDirectory(prefix=".d01-transfer-", dir=args.output.parent) as stage:
        temp = Path(stage) / "transfer.jsonl"
        with path.open("rb") as source, temp.open("xb") as destination:
            source_hash = verify.open_header(source, kind, owner)
            first = line(header)
            digest.update(first)
            destination.write(first)
            for ordinal, frame in verify.take_stream(output_dir, artifacts, owner, "frames", owner["frame_stream"],
                                                     schema_id=coverage["schema_id"]):
                payload = verify.read_frame(source, frame, source_hash, before.st_size)
                matched_adapters = [(values, handled) for values, handled in native_adapters if ordinal in handled]
                selected = sum(ordinal in lane for lane in (memory, graph, entity)) + len(matched_adapters)
                require(selected <= 1, "source frame has ambiguous target mapping")
                if ordinal in memory:
                    records = memory_events(ordinal, frame, memory.pop(ordinal))
                elif ordinal in graph:
                    records = graph.pop(ordinal)
                elif ordinal in entity:
                    records = entity_events(ordinal, frame, entity.pop(ordinal))
                elif matched_adapters:
                    values, handled = matched_adapters[0]
                    disposition = handled.pop(ordinal)
                    require(len(disposition) == 1 and disposition[0]["source_carrier"] == frame["carrier"],
                            "source disposition differs from held frame")
                    records = values.pop(ordinal, [])
                    if not records:
                        require(disposition[0]["disposition"] in ("source_archive_format_replaced",
                                 "retained_in_current_archive_body", "retained_in_current_project_attachment",
                                "source_insomnia_format_replaced"),
                                "source disposition lacks its target body")
                        records = [event(ordinal, "embedded", carrier=frame["carrier"])]
                else:
                    records = scalar_events(ordinal, frame["carrier"].encode("ascii"), payload)
                count = 0
                for record in records:
                    require(record["source_ordinal"] == str(ordinal), "target event is out of source order")
                    data = line(record)
                    digest.update(data)
                    destination.write(data)
                    count += 1
                require(count > 0, "unclassified retained source frame")
            require(source.tell() == before.st_size and source_hash.hexdigest() == source_sha
                    and verify.snapshot(before) == verify.snapshot(verify.regular(path)), "source copy changed during transfer")
            require(not memory and not graph and not entity and not any(values or handled for values, handled in native_adapters),
                    "target transfer contains unbound source events")
            destination.flush()
            os.fsync(destination.fileno())
        os.link(temp, args.output)
    return {"status": "PASS", "scope": "source_custody_and_ordered_native_transfer_only",
            "source_sha256": source_sha, "source_coverage_sha256": coverage_sha,
            "memory_receipt_sha256": memory_sha, "graph_receipt_sha256": graph_sha,
            "transfer_sha256": digest.hexdigest(), "source_frames": header["source_frames"]}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source-tools", type=Path, required=True)
    parser.add_argument("--source-coverage", type=Path, required=True)
    parser.add_argument("--source-copies", type=Path, required=True)
    parser.add_argument("--transfer-root", type=Path, required=True)
    parser.add_argument("--freshness-target-transfer", type=Path,
                        help="current Freshness adapter directory for a separate conversion stage")
    parser.add_argument("--fixture", required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    sys.path.insert(0, str(args.source_tools))
    import f03_verify_independent_repeat as verify
    from f03_verify_archived_oracle import PINS
    try:
        require(args.fixture in PINS, "source fixture outside reviewed custody")
        print(json.dumps(compose(args, (verify, PINS)), sort_keys=True))
    except (KeyError, OSError, ValueError) as error:
        print(json.dumps({"status": "FAIL", "reason": str(error)}, sort_keys=True), file=sys.stderr)
        raise SystemExit(1) from None


if __name__ == "__main__":
    main()
