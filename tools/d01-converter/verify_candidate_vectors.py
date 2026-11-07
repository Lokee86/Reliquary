"""Compare current retained vector values against the independent pinned source witness."""
import argparse
import hashlib
import json
from pathlib import Path
import stat
from struct import iter_unpack, unpack


CURRENT = b"REL\0\r\n\x1a\n\x02\0\x01\0\x28\0\0\0"
LEGACY = b"CVA\0\r\n\x1a\n\x02\0\0\0\x28\0\0\0"
SCALAR_WIDTH = {1: 1, 2: 1, 3: 2, 4: 2, 5: 4, 6: 4, 7: 8, 8: 8, 9: 2, 10: 2, 11: 4, 12: 8}


def require(condition, reason):
    if not condition:
        raise ValueError(reason)


def file_info(path):
    value = path.lstat()
    require(stat.S_ISREG(value.st_mode) and value.st_nlink == 1
            and not getattr(value, "st_file_attributes", 0) & 0x400,
            "proof input is not a regular single-link file")
    return value.st_dev, value.st_ino, value.st_size, value.st_mtime_ns


def open_frames(path, format_, expected, owner):
    before = file_info(path)
    with path.open("rb") as file:
        header = file.read(40)
        require(len(header) == 40 and header[:16] == format_ and header[16:] == owner,
                "file header does not match the verified source owner")
        digest = hashlib.sha256(header)
        while (offset := file.tell()) < before[2]:
            prefix = file.read(8)
            require(len(prefix) == 8, "truncated frame length")
            size, = unpack("<Q", prefix)
            require(8 <= size <= min(128 * 1024 * 1024, before[2] - offset - 8),
                    "invalid frame extent")
            body = file.read(size)
            require(len(body) == size, "truncated frame")
            digest.update(prefix)
            digest.update(body)
            yield offset, body
    require(digest.hexdigest() == expected and file_info(path) == before,
            "observed file is different from its pinned bytes")


def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(",", ":")).encode("ascii")


def digest(data):
    return hashlib.sha256(data).hexdigest()


def matrix(body):
    require(len(body) >= 64 and not any(body[45:48]), "invalid current packed vector")
    dimensions, scalar, rows, extent = unpack("<IB3xQQ", body[40:64])
    require(scalar in SCALAR_WIDTH and dimensions > 0
            and dimensions * SCALAR_WIDTH[scalar] * rows == extent == len(body) - 64,
            "invalid current packed vector shape")
    return {"dimensions": str(dimensions), "scalar": scalar, "rows": str(rows),
            "byte_length": str(extent), "payload_sha256": digest(body[64:]),
            "pruned": False, "prune_manifest_sha256": None}


def profile(body):
    require(len(body) >= 60 and not any(body[45:48]), "invalid current compatibility profile")
    dimensions, norm, suite, policy, count = unpack("<IB3xIII", body[40:60])
    references = []
    offset = 60
    for _ in range(count):
        mode, size = unpack("<B3xI", body[offset:offset + 8])
        require(not any(body[offset + 1:offset + 4]) and mode in (1, 2) and size == dimensions,
                "invalid current probe reference")
        end = offset + 8 + size * 4
        require(end <= len(body), "truncated current probe")
        references.append({"mode": mode, "float32_le": [value.hex() for (value,) in
                                                        iter_unpack("4s", body[offset + 8:end])]})
        offset = end
    require(offset == len(body) and [row["mode"] for row in references] == [1, 1, 2, 2],
            "current compatibility probes differ in order")
    return {"dimensions": str(dimensions), "normalization": norm, "probe_suite_version": str(suite),
            "compatibility_policy_version": str(policy), "retired": False, "references": references}


def ordered_ids(body, count, start):
    require(len(body) == start + 32 * count, "current vector mapping has the wrong extent")
    return [identity.hex() for (identity,) in iter_unpack("32s", body[start:])]


def vector_values(frames, expected):
    rows = []
    profiles = {}
    generations = {}
    generation_values = []
    for offset, body in frames:
        marker = body[:8]
        identity = body[8:40]
        event = "publish"
        if marker == b"RELPVEC1":
            family, value = "packed_matrix", matrix(body)
        elif marker == b"RELCPRO1":
            family, value = "compatibility_profile", profile(body)
            require(identity not in profiles, "duplicate current compatibility profile")
            profiles[identity] = value
        elif marker == b"RELCPRT1":
            family, event = "compatibility_profile", "retire"
            require(len(body) == 40 and identity in profiles and not profiles[identity]["retired"],
                    "invalid current compatibility retirement")
            value = {**profiles[identity], "retired": True}
            profiles[identity] = value
        elif marker == b"RELMVEC1":
            family = "memory_vector_set"
            count, = unpack("<Q", body[104:112])
            value = {"compatibility_profile_id": body[40:72].hex(), "packed_vector_id": body[72:104].hex(),
                     "memory_body_ids": ordered_ids(body, count, 112)}
        elif marker == b"RELAVEC1":
            family = "archive_vector_set"
            count, version = unpack("<QQ", body[72:88])
            value = {"packed_vector_id": body[40:72].hex(), "fragment_ids": ordered_ids(body, count, 88),
                     "max_fragment_archive_version": str(version)}
        elif marker == b"RELVGEN1":
            require(len(body) == 112, "invalid current Vector Generation length")
            generations[(offset, len(body))] = body[8:]
            continue
        elif marker == b"RELPUB01" and body[17:25] == b"RELVGRC1":
            require(len(body) == 49 and body[16] == 2, "current vector activation lacks a reserved global binding")
            global_, = unpack("<Q", body[8:16])
            local, target, length = unpack("<QQQ", body[25:])
            activation = generations.pop((target, length), None)
            require(activation is not None and local == len(generation_values) + 1,
                    "current vector activation target is unresolved")
            generation_values.append((global_, local, activation))
            continue
        else:
            require(marker != b"RELVGRC1", "current vector activation lost its global binding")
            continue
        rows.append((family, digest(identity), digest(canonical(value)), event))
    require(not generations, "current vector backing has no activation")
    wanted = [(row["family"], row["stable_key_sha256"], row["canonical_value_sha256"], row["event"])
              for row in expected]
    require(rows == wanted, "current retained vector values or publication order differ from the independent source")
    return len(rows), generation_values


def legacy_activations(frames):
    generations = {}
    activations = []
    for offset, body in frames:
        if body[:8] == b"CVAVGEN2":
            require(len(body) == 112, "invalid legacy Vector Generation length")
            generations[(offset, len(body))] = body[8:]
        elif body[:8] == b"CVAVGRC2":
            global_, local, target, length = unpack("<QQQQ", body[8:])
            generation = generations.pop((target, length), None)
            require(generation is not None and local == len(activations) + 1,
                    "unresolved legacy Vector Generation activation")
            activations.append((global_, local, generation))
    require(not generations, "legacy vector backing has no activation")
    return activations


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--candidate", type=Path, required=True)
    parser.add_argument("--candidate-sha256", required=True)
    parser.add_argument("--source-copy", type=Path, required=True)
    parser.add_argument("--source-sha256", required=True)
    parser.add_argument("--expectations", type=Path, required=True)
    parser.add_argument("--expectations-sha256", required=True)
    args = parser.parse_args()
    expect_before = file_info(args.expectations)
    require(expect_before[2] <= 1024 * 1024, "vector proof exceeds its evidence bound")
    expected = args.expectations.read_bytes()
    require(digest(expected) == args.expectations_sha256 and file_info(args.expectations) == expect_before,
            "independent source vector proof differs")
    receipt = json.loads(expected)
    owners = [row for row in receipt["owners"] if row["source_sha256"] == args.source_sha256]
    require(receipt["status"] == "PASS" and len(owners) == 1, "independent source witness lacks this owner")
    with args.source_copy.open("rb") as source:
        header = source.read(40)
    owner = header[16:]
    old = legacy_activations(open_frames(args.source_copy, LEGACY, args.source_sha256, owner))
    count, current = vector_values(open_frames(args.candidate, CURRENT, args.candidate_sha256, owner),
                                   owners[0]["physical_transitions"])
    require(current == old, "retained Vector Generation identity or activation differs")
    print(json.dumps({"status": "PASS", "scope": "retained vector values and activation coordinates",
                      "source_sha256": args.source_sha256, "candidate_sha256": args.candidate_sha256,
                      "vector_transitions": count, "generation_activations": len(current),
                      "source_vector_expectations_sha256": args.expectations_sha256}))


if __name__ == "__main__":
    main()
