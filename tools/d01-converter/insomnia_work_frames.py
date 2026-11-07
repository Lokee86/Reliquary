"""Attach exact current owner and Episode order to the retained Insomnia work rows."""
import argparse
from collections import Counter
import hashlib
import io
import json
from pathlib import Path
from struct import pack, unpack

from archive_frames import compact, consume_frames, emit, field, require, sha256, take, wire_field


def episode_order(payload):
    if payload[:8] != b"CVAPUB01":
        return None
    body = payload[25 if payload[16] == 1 else 17:]
    if body[:8] != b"CVAEPI02":
        return None
    with io.BytesIO(body[16:]) as source:
        identity = take(source, 32)
        source.seek(34)
        through = take(source, 8)
        source.seek(50)
        conversation, start, end = [field(source) for _ in range(3)]
        require(conversation and start and end and not source.read(1), "invalid accepted Episode order")
    return identity, through + wire_field(conversation) + wire_field(start)


def validate_work(data):
    with io.BytesIO(data) as source:
        priority, state, _, _ = unpack("<BBIq", take(source, 14))
        require(1 <= priority <= 3 and 1 <= state <= 5, "invalid retained work status")
        flags = []
        for read in (field, lambda item: take(item, 32), lambda item: take(item, 8),
                     lambda item: take(item, 8), field):
            option = take(source, 1)
            require(option in (b"\0", b"\1"), "invalid retained work optional field")
            flags.append(option == b"\1")
            if option == b"\1":
                read(source)
        require(flags[:3] == [state == 2] * 3 and not source.read(1), "invalid retained work lease")


def translate(path, owner, output, handled):
    episodes = {}
    revisions = Counter()
    counts = Counter()
    uuid = bytes.fromhex(owner["owner_uuid"])
    for ordinal, payload, _ in consume_frames(path, owner):
        order = episode_order(payload)
        if order is not None:
            identity, value = order
            require(identity not in episodes, "duplicate accepted Episode order")
            episodes[identity] = value
        marker = payload[:8]
        if marker == b"CVAINSF1":
            require(len(payload) == 8 and not counts["CVAINSF1"], "invalid source Insomnia format marker")
            disposition = "source_insomnia_format_replaced"
        elif marker == b"CVAINSW1":
            require(len(payload) >= 54, "truncated retained Insomnia work")
            identity = payload[8:40]
            require(identity in episodes, "Insomnia work has no prior accepted Episode order")
            state = payload[40:]
            validate_work(state)
            revisions[identity] += 1
            body = b"RELINSW1" + uuid + pack("<Q", revisions[identity]) + identity + episodes[identity] + state
            emit(output, ordinal, "frame", body)
            disposition = "current_insomnia_work"
        else:
            continue
        counts[marker.decode("ascii")] += 1
        handled.write(compact({"source_ordinal": str(ordinal), "source_carrier": marker.decode("ascii"),
                               "disposition": disposition}) + b"\n")
    require(counts["CVAINSF1"] == 1, "source Insomnia format absent")
    return {"work_revisions": counts["CVAINSW1"], "work_episodes": len(revisions)}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--inputs", type=Path, required=True)
    parser.add_argument("--expected-inputs-sha256", required=True)
    parser.add_argument("--copies", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    raw = args.inputs.read_bytes()
    require(hashlib.sha256(raw).hexdigest() == args.expected_inputs_sha256, "source custody receipt differs")
    manifest = json.loads(raw)
    require(manifest["status"] == "VERIFIED_DISPOSABLE_COPY_INPUTS" and manifest["preserves_original_sources"],
            "source custody did not pass")
    owners = [owner for owner in manifest["sources"] if owner["kind"] == "REL"]
    require(len(owners) == 3 and not args.output.resolve().is_relative_to(args.copies.resolve()),
            "invalid Insomnia work cohort or output")
    args.output.mkdir()
    results = []
    for owner in owners:
        source = Path(owner["fixture"])
        require(not source.is_absolute() and ".." not in source.parts, "invalid source path")
        stem = source.parent.name
        frames, handled = args.output / f"{stem}.insomnia-work.jsonl", args.output / f"{stem}.insomnia-work-handled.jsonl"
        with frames.open("xb") as out, handled.open("xb") as dispositions:
            stats = translate(args.copies / source, owner, out, dispositions)
        results.append({**stats, "fixture": owner["fixture"], "source_sha256": owner["source_sha256"],
                        "frames": frames.name, "frames_sha256": sha256(frames),
                        "handled": handled.name, "handled_sha256": sha256(handled)})
    receipt = {"status": "PASS", "scope": "Insomnia work translation only",
               "input_custody_sha256": args.expected_inputs_sha256, "sources": results}
    with (args.output / "insomnia-work-transfer-receipt.json").open("xb") as report:
        report.write(compact(receipt) + b"\n")
    print(compact(receipt).decode("utf-8"))


if __name__ == "__main__":
    main()
