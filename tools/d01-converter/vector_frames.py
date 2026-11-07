"""Translate the pinned owners' exact vector values and activation references."""
import argparse
from collections import Counter
import hashlib
import io
import json
from pathlib import Path
from struct import pack, unpack

from archive_frames import compact, consume_frames, emit, regular, require, sha256, take

FORMATS = {
    b"CVAPVFM1": b"RELPVFM1", b"CVACPFM1": b"RELCPFM1", b"CVAMVFM1": b"RELMVFM1",
    b"CVAAVFM1": b"RELAVFM1", b"CVAVGFM2": b"RELVGFM1",
}
SCALAR_WIDTH = {1: 1, 2: 1, 3: 2, 4: 2, 5: 4, 6: 4, 7: 8, 8: 8, 9: 2, 10: 2, 11: 4, 12: 8}
OBJECTS = {
    b"CVAPVEC1", b"CVACPRO1", b"CVACPRT1", b"CVAMVEC1",
    b"CVAAVEC1", b"CVAVGEN2", b"CVAVGRC2",
}


def stable(domain, payload, expected):
    require(hashlib.sha256(domain + payload).digest() == expected,
            "vector stable identity disagrees with retained value")


def matrix(payload):
    require(len(payload) >= 64 and not any(payload[45:48]), "invalid packed-vector header")
    dimensions, scalar, rows, length = unpack("<IB3xQQ", payload[40:64])
    require(dimensions and scalar in SCALAR_WIDTH
            and dimensions * SCALAR_WIDTH[scalar] * rows == length == len(payload) - 64,
            "invalid packed-vector shape")
    stable(b"CVA-PACKED-VECTOR-V1\0", payload[40:45] + payload[64:], payload[8:40])
    return dimensions, scalar, rows


def profile(payload):
    require(len(payload) >= 60 and not any(payload[45:48]), "invalid compatibility profile header")
    dimensions, norm, suite, policy, count = unpack("<IB3xIII", payload[40:60])
    require(dimensions and norm in (0, 1) and suite == 1 and policy in (2, 3) and count == 4,
            "unsupported compatibility profile")
    preimage = bytearray(payload[40:45] + payload[48:56])
    with io.BytesIO(payload[60:]) as data:
        for mode in (1, 1, 2, 2):
            observed, size = unpack("<B3xI", head := take(data, 8))
            require(observed == mode and not any(head[1:4]) and size == dimensions,
                    "invalid ordered compatibility reference")
            preimage.append(mode)
            preimage.extend(take(data, size * 4))
        require(not data.read(1), "trailing compatibility probe bytes")
    stable(b"CVA-COMPATIBILITY-PROFILE-V1\0", bytes(preimage), payload[8:40])
    return dimensions


def mapping(payload, header):
    require(len(payload) >= header, "truncated ordered vector mapping")
    count, = unpack("<Q", payload[header - 8:header])
    require(len(payload) == header + count * 32, "vector mapping extent differs")
    rows = [payload[index:index + 32] for index in range(header, len(payload), 32)]
    require(len(set(rows)) == count, "duplicate ordered vector row")
    return rows


class VectorTransfer:
    def __init__(self):
        self.matrices = {}
        self.profiles = {}
        self.retired = set()
        self.fragments = {}
        self.archives = {}
        self.generations = {}
        self.reservations = set()
        self.next_vector_version = 1

    def observe_reference(self, payload):
        if payload[:8] in (b"CVAVERS1", b"CVAVERS2"):
            self.reservations.add(unpack("<Q", payload[8:16])[0])
        if payload[:8] == b"CVAPUB01":
            start = 25 if payload[16] == 1 else 17
            body = payload[start:]
            if body[:8] == b"CVAFRA02":
                version, identity = unpack("<Q32s", body[8:48])
                require(identity not in self.fragments, "duplicate retained Archive fragment")
                self.fragments[identity] = version

    def object(self, marker, payload, ordinal, source_offset):
        identity = payload[8:40]
        if marker == b"CVAPVEC1":
            require(identity not in self.matrices, "duplicate packed matrix")
            self.matrices[identity] = matrix(payload)
            return "frame", b"RELPVEC1" + payload[8:], {}
        if marker == b"CVACPRO1":
            require(identity not in self.profiles, "duplicate compatibility profile")
            self.profiles[identity] = profile(payload)
            return "frame", b"RELCPRO1" + payload[8:], {}
        if marker == b"CVACPRT1":
            require(len(payload) == 40 and identity in self.profiles and identity not in self.retired,
                    "invalid compatibility retirement")
            self.retired.add(identity)
            return "frame", b"RELCPRT1" + payload[8:], {}
        if marker == b"CVAMVEC1":
            rows = mapping(payload, 112)
            dimensions = self.profiles.get(payload[40:72])
            require(dimensions and self.matrices.get(payload[72:104]) == (dimensions, 11, len(rows)),
                    "Memory-vector mapping has missing or wrong-shaped backing")
            stable(b"CVA-MEMORY-VECTORS-V1\0", payload[40:104] + payload[112:], identity)
            return "frame", b"RELMVEC1" + payload[8:], {}
        if marker == b"CVAAVEC1":
            rows = mapping(payload, 80)
            matrix_info = self.matrices.get(payload[40:72])
            require(matrix_info and matrix_info[2] == len(rows) and all(row in self.fragments for row in rows)
                    and identity not in self.archives, "Archive-vector mapping lacks accepted fragments or backing")
            maximum = max((self.fragments[row] for row in rows), default=0)
            self.archives[identity] = maximum
            stable(b"CVA-ARCHIVE-VECTORS-V1\0", payload[40:72] + payload[80:], identity)
            return "frame", b"RELAVEC1" + payload[8:80] + pack("<Q", maximum) + payload[80:], {}
        if marker == b"CVAVGEN2":
            require(len(payload) == 112 and payload[40:72] in self.profiles and payload[72:104] in self.archives,
                    "invalid retained Vector Generation closure")
            archive_version, = unpack("<Q", payload[104:])
            require(archive_version >= self.archives[payload[72:104]], "generation is earlier than its fragments")
            stable(b"CVA-VECTOR-GENERATION-V2\0", payload[40:], identity)
            self.generations[(source_offset, len(payload))] = ordinal
            return "frame", b"RELVGEN1" + payload[8:], {}

        require(marker == b"CVAVGRC2" and len(payload) == 40, "invalid vector activation receipt")
        global_version, local, old_offset, old_length = unpack("<QQQQ", payload[8:])
        generation_ordinal = self.generations.pop((old_offset, old_length), None)
        require(local == self.next_vector_version and global_version in self.reservations
                and generation_ordinal is not None, "invalid vector activation clocks or source ObjectRef")
        self.next_vector_version += 1
        return "vector_receipt", None, {
            "vector_version": str(local), "global_version": str(global_version),
            "generation_source_ordinal": str(generation_ordinal),
        }


def translate(path, owner, frames, handled):
    transfer = VectorTransfer()
    formats = set()
    counts = Counter()
    for ordinal, payload, offset in consume_frames(path, owner):
        marker = payload[:8]
        transfer.observe_reference(payload)
        if marker in FORMATS:
            require(marker not in formats and payload[8:] == pack("<I", 1),
                    "duplicate or unsupported vector format")
            formats.add(marker)
            kind, body, details = "frame", FORMATS[marker] + payload[8:], {}
        elif marker in OBJECTS:
            kind, body, details = transfer.object(marker, payload, ordinal, offset)
        else:
            require(not marker.startswith((b"CVAPVE", b"CVAPVR", b"CVAMVE", b"CVAAVE", b"CVACPR", b"CVAVGEN", b"CVAVGRC")),
                    "unmapped vector carrier")
            continue
        counts[marker.decode("ascii")] += 1
        emit(frames, ordinal, kind, body, **details)
        handled.write(compact({"source_ordinal": str(ordinal), "source_carrier": marker.decode("ascii"),
                               "disposition": "current_vector_value"}) + b"\n")
    expected = FORMATS.keys() if owner["kind"] == "REL" else FORMATS.keys() - {b"CVAAVFM1", b"CVAVGFM2"}
    require(formats == expected and not transfer.generations, "vector format or activation coverage differs")
    return dict(counts)


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
    require(not args.output.resolve().is_relative_to(args.copies.resolve()), "vector output overlaps source copies")
    args.output.mkdir()
    results = []
    for owner in manifest["sources"]:
        source = Path(owner["fixture"])
        require(not source.is_absolute() and ".." not in source.parts
                and source.suffix == "." + owner["kind"].lower()
                and owner["source_sha256"] == owner["verified_disposable_sha256"], "invalid vector custody row")
        stem = f"{source.parent.name}-{owner['kind'].lower()}"
        output, accounted = args.output / f"{stem}.vectors.jsonl", args.output / f"{stem}.vectors-handled.jsonl"
        with output.open("xb") as frames, accounted.open("xb") as handled:
            counts = translate(args.copies / source, owner, frames, handled)
        results.append({"fixture": owner["fixture"], "source_sha256": owner["source_sha256"],
                        "owned_carrier_counts": counts, "frames": output.name, "frames_sha256": sha256(output),
                        "handled": accounted.name, "handled_sha256": sha256(accounted)})
    receipt = {"status": "PASS", "scope": "current vector frame translation and relocated activation plan",
               "input_custody_sha256": args.expected_inputs_sha256, "sources": results}
    with (args.output / "vector-transfer-receipt.json").open("xb") as report:
        report.write(compact(receipt) + b"\n")
    print(compact(receipt).decode("utf-8"))


if __name__ == "__main__":
    main()
