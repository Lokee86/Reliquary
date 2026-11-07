"""Independently verify retained vector objects and profile retirement frontiers."""
import argparse
from collections import Counter
import hashlib
import json
import math
from pathlib import Path
from struct import iter_unpack, unpack_from

from f03_legacy_frame_clock import LegacyFrameClock
from f03_verify_archived_oracle import PINS, require
from f03_verify_independent_repeat import (
    MANIFEST, SCHEMAS, open_header, read_document, read_frame, regular, snapshot, take_stream,
)


WIDTH = {1: 1, 2: 1, 3: 2, 4: 2, 5: 4, 6: 4, 7: 8, 8: 8, 9: 2, 10: 2, 11: 4, 12: 8}
PREFIX = b"reliquary:f03:clock-neutral-vector-objects:v1\0"
FORMATS = {b"CVAPVFM1", b"CVAMVFM1", b"CVACPFM1", b"CVAAVFM1"}


def encode(value):
    return json.dumps(value, sort_keys=True, separators=(",", ":")).encode("ascii")


def digest(value):
    return hashlib.sha256(value).hexdigest()


def check_identity(payload, domain, preimage):
    require(hashlib.sha256(domain + preimage).digest() == payload[8:40],
            "vector identity differs from its exact source preimage")


def matrix(payload):
    require(len(payload) >= 64 and not any(payload[45:48]), "invalid packed vector header")
    dimensions, scalar, rows, length = unpack_from("<IB3xQQ", payload, 40)
    bits = payload[64:]
    require(dimensions > 0 and scalar in WIDTH and dimensions * WIDTH[scalar] * rows == length == len(bits),
            "packed vector shape mismatch")
    check_identity(payload, b"CVA-PACKED-VECTOR-V1\0", payload[40:45] + bits)
    return {
        "dimensions": str(dimensions), "scalar": scalar, "rows": str(rows), "byte_length": str(length),
        "payload_sha256": digest(bits), "pruned": False, "prune_manifest_sha256": None,
    }


def compatibility_profile(payload):
    require(len(payload) >= 60 and not any(payload[45:48]), "invalid compatibility profile header")
    dimensions, norm, suite, policy, count = unpack_from("<IB3xIII", payload, 40)
    require(dimensions > 0 and norm in (0, 1) and suite == 1 and policy in (2, 3) and count == 4,
            "invalid compatibility profile contract")
    preimage = bytearray(payload[40:45] + payload[48:56])
    references = []
    offset = 60
    for mode in (1, 1, 2, 2):
        require(offset + 8 <= len(payload) and payload[offset] == mode
                and not any(payload[offset + 1:offset + 4]), "invalid compatibility probe header")
        size, = unpack_from("<I", payload, offset + 4)
        start = offset + 8
        offset = start + 4 * size
        require(size == dimensions and offset <= len(payload), "invalid compatibility probe length")
        bits = payload[start:offset]
        require(all(math.isfinite(value) for (value,) in iter_unpack("<f", bits)), "nonfinite probe")
        preimage.append(mode)
        preimage.extend(bits)
        references.append({"mode": mode, "float32_le": [item.hex() for (item,) in iter_unpack("4s", bits)]})
    require(offset == len(payload), "compatibility profile trailing bytes")
    check_identity(payload, b"CVA-COMPATIBILITY-PROFILE-V1\0", preimage)
    return {
        "dimensions": str(dimensions), "normalization": norm, "probe_suite_version": str(suite),
        "compatibility_policy_version": str(policy), "retired": False, "references": references,
    }


def ordered_mapping(payload, start):
    require(len(payload) >= start, "truncated vector row mapping")
    rows, = unpack_from("<Q", payload, start - 8)
    require(len(payload) == start + 32 * rows, "invalid ordered vector row mapping length")
    result = [identity for (identity,) in iter_unpack("32s", payload[start:])]
    require(len(set(result)) == rows, "duplicate vector row mapping")
    return result


def decode_object(payload, state, fragments):
    marker = payload[:8]
    if marker == b"CVAPVEC1":
        return "packed_matrix", matrix(payload)
    if marker == b"CVACPRO1":
        return "compatibility_profile", compatibility_profile(payload)
    if marker == b"CVAMVEC1":
        members = ordered_mapping(payload, 112)
        profile_id, packed_id = payload[40:72], payload[72:104]
        profile = state.get(("compatibility_profile", profile_id))
        packed = state.get(("packed_matrix", packed_id))
        require(profile is not None and packed is not None and packed["scalar"] == 11
                and packed["dimensions"] == profile["dimensions"] and packed["rows"] == str(len(members)),
                "Memory vector reference closure or shape differs")
        check_identity(payload, b"CVA-MEMORY-VECTORS-V1\0", payload[40:104] + payload[112:])
        return "memory_vector_set", {
            "compatibility_profile_id": profile_id.hex(), "packed_vector_id": packed_id.hex(),
            "memory_body_ids": [item.hex() for item in members],
        }
    members = ordered_mapping(payload, 80)
    packed_id = payload[40:72]
    packed = state.get(("packed_matrix", packed_id))
    require(packed is not None and packed["rows"] == str(len(members))
            and all(item in fragments for item in members), "Archive vector reference closure or shape differs")
    check_identity(payload, b"CVA-ARCHIVE-VECTORS-V1\0", packed_id + payload[80:])
    return "archive_vector_set", {
        "packed_vector_id": packed_id.hex(), "fragment_ids": [item.hex() for item in members],
        "max_fragment_archive_version": str(max((fragments[item] for item in members), default=0)),
    }


def observe(path, output, artifacts, owner, schema, kind):
    expected = {}
    for _, row in take_stream(output, artifacts, owner, "records", owner["native"]["record_stream"],
                              schema_id=schema):
        if row["projection_domain"] == "vector_objects":
            key = row["family"], row["stable_key_sha256"]
            require(key not in expected and row["availability"] == "retained", "invalid native vector commitment")
            expected[key] = row["canonical_value_sha256"], row["canonical_byte_length"]
    state = {}
    transitions = []
    formats = set()
    fragments = {}
    clock = LegacyFrameClock()
    before = regular(path)
    with path.open("rb") as source:
        source_hash = open_header(source, kind, owner)
        for ordinal, frame in take_stream(output, artifacts, owner, "frames", owner["frame_stream"],
                                          schema_id=schema):
            payload = read_frame(source, frame, source_hash, before.st_size)
            global_ = clock.consume(payload)
            require(str(global_) == frame["latest_global"], "vector physical clock mismatch")
            marker = payload[:8]
            if frame["carrier"] == "CVAFRA02":
                body_start = 25 if payload[16] == 1 else 17
                local, identity = unpack_from("<Q32s", payload, body_start + 8)
                fragments.setdefault(identity, local)
            if marker in FORMATS:
                require(marker not in formats and payload[8:] == b"\1\0\0\0", "invalid vector format")
                formats.add(marker)
                continue
            identity = payload[8:40]
            if marker == b"CVACPRT1":
                family = "compatibility_profile"
                previous = state.get((family, identity))
                require(len(payload) == 40 and previous is not None and not previous["retired"],
                        "invalid compatibility profile retirement")
                value = {**previous, "retired": True}
                event = "retire"
            elif marker in {b"CVAPVEC1", b"CVACPRO1", b"CVAMVEC1", b"CVAAVEC1"}:
                family, value = decode_object(payload, state, fragments)
                require((family, identity) not in state, "duplicate vector object")
                event = "publish"
            else:
                require(not marker.startswith((b"CVAPVE", b"CVAMVE", b"CVAAVE", b"CVACPR", b"CVAPVR")),
                        "vector carrier outside the verified schema")
                continue
            state[(family, identity)] = value
            transitions.append({
                "first_global": str(global_), "physical_ordinal": str(ordinal), "family": family,
                "stable_key_sha256": digest(identity), "canonical_value_sha256": digest(encode(value)),
                "event": event,
            })
        require(source.tell() == before.st_size and source_hash.hexdigest() == owner["source_sha256"]
                and snapshot(before) == snapshot(regular(path)), "vector source differs from its pins")
    require(formats == (FORMATS if kind == 1 else FORMATS - {b"CVAAVFM1"}), "vector format inventory differs")
    got = {}
    for (family, identity), value in state.items():
        canonical = encode(value)
        got[(family, digest(identity))] = digest(canonical), str(len(canonical))
    require(got == expected, "stored vector values differ from typed native commitments")

    root = hashlib.sha256(PREFIX + bytes.fromhex(owner["owner_uuid"]))
    cut_roots = [{"first_global": "0", "sha256": root.hexdigest()}]
    for transition in transitions:
        encoded = encode(transition)
        root.update(len(encoded).to_bytes(8, "little") + encoded)
        cut = {"first_global": transition["first_global"], "sha256": root.hexdigest()}
        if cut["first_global"] == cut_roots[-1]["first_global"]:
            cut_roots[-1] = cut
        else:
            cut_roots.append(cut)
    for cut, successor in zip(cut_roots, cut_roots[1:]):
        cut["last_global"] = str(int(successor["first_global"]) - 1)
    cut_roots[-1]["last_global"] = str(clock.head)
    return {
        "copy_id": owner["copy_id"], "source_sha256": owner["source_sha256"], "global_cuts": clock.head + 1,
        "native_objects": len(state), "family_counts": dict(sorted(Counter(family for family, _ in state).items())),
        "profile_retirements": sum(row["event"] == "retire" for row in transitions),
        "physical_transitions": transitions, "retained_history_cut_intervals": cut_roots,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source-copies", type=Path, required=True)
    parser.add_argument("--source-coverage", type=Path, required=True)
    args = parser.parse_args()
    output = args.source_coverage.parent
    coverage, coverage_sha = read_document(args.source_coverage, 2 * 1024 * 1024)
    manifest, manifest_sha = read_document(output / MANIFEST, 2 * 1024 * 1024)
    schema = coverage["schema_id"]
    require(schema in SCHEMAS and manifest["schema_id"] == schema
            and manifest["source_coverage_sha256"] == coverage_sha, "invalid source coverage provenance")
    owners = {row["source_sha256"]: row for row in coverage["sources"]}
    require(len(coverage["sources"]) == len(PINS) and owners.keys() == set(PINS.values()),
            "vector cohort differs from reviewed pins")
    artifacts = manifest["artifact_sha256"].copy()
    results = [observe(args.source_copies / fixture, output, artifacts, owners[pin], schema,
                       1 if fixture.endswith(".rel") else 2)
               for fixture, pin in PINS.items()]
    print(json.dumps({
        "status": "PASS", "scope": "stored vector objects and profile retirement physical frontiers",
        "source_coverage_sha256": coverage_sha, "source_manifest_sha256": manifest_sha,
        "native_objects": sum(row["native_objects"] for row in results),
        "profile_retirements": sum(row["profile_retirements"] for row in results),
        "global_cuts": sum(row["global_cuts"] for row in results),
        "owners": results,
    }, indent=2))


if __name__ == "__main__":
    main()
