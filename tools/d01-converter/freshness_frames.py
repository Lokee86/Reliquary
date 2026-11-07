"""Translate verified Freshness journal values and retain the accepted FRS07 authority."""
import argparse
import hashlib
import json
from pathlib import Path
from struct import pack

from archive_frames import compact, consume_frames, emit, require, sha256, wire_field

SOURCE_CHECKSUM_DOMAIN = b"reliquary-semantic-import-settlement-v7"


def json_receipt(path, expected):
    raw = path.read_bytes()
    require(hashlib.sha256(raw).hexdigest() == expected, "verified transfer receipt digest changed")
    return json.loads(raw)


def identity(value, length=32):
    raw = bytes.fromhex(value) if isinstance(value, str) else bytes(value)
    require(len(raw) == length, "wrong Freshness identity width")
    return raw


def text(value):
    return wire_field(value.encode("utf-8"))


def optional(value):
    return b"\0" if value is None else b"\1" + pack("<Q", value)


def journal_frame(owner, row):
    value = row["value"]
    if row["family"] == "policy":
        require(value["policy_version"] == 1, "unsupported source Freshness policy")
        return b"RELFRP01" + owner + pack(
            "<HQhhhh", 1, value["decay_turns_per_point"], value["access_principal"],
            value["linkage_principal"], value["local_hop_cost"], value["outside_hop_cost"],
        )
    prefix = b"RELFRS02" + owner + pack("<H", 1)
    if row["family"] == "admit":
        return prefix + b"\1" + identity(value["memory_id"]) + pack("<Q", value["source_turn"])
    require(row["family"] == "event" and value["semantic"] is None, "unsupported source Freshness journal value")
    provenance = value["provenance"]
    proof = value["proof"]
    effects = [(identity(row_id), amount) for row_id, amount in value["effects"]]
    require(effects and [row_id for row_id, _ in effects] == sorted({row_id for row_id, _ in effects}),
            "Freshness effect ordering changed")
    return (prefix + b"\2" + text(value["event_id"]) + pack("<QB", value["source_turn"], provenance["producer_kind"])
            + text(provenance["source_id"]) + identity(provenance["origin_owner"], 16)
            + pack("<QQQ", provenance["accepted_turn"], provenance["policy_version"], proof["graph_version"])
            + optional(proof["community_generation"]) + optional(proof["community_graph_version"])
            + pack("<I", len(effects)) + b"".join(row_id + pack("<h", amount) for row_id, amount in effects))


def settlement_frame(owner, raw, proof, births, entries):
    value = proof["settlement"]
    require(329 <= len(raw) <= 16 * 1024 * 1024 and raw[:8] == b"CVAFRS07"
            and raw[8:24] == owner and raw[24:56] == identity(value["import_id"])
            and hashlib.sha256(SOURCE_CHECKSUM_DOMAIN + raw[:-32]).digest() == raw[-32:],
            "retained FRS07 checksum or owner changed")
    require(raw[248:296] == pack("<QQQQQQ", *(value[key] for key in (
                "base_cut", "settlement_cut", "global_version", "archive_version",
                "graph_version", "memory_graph_version",
            ))), "retained FRS07 settlement coordinates changed")
    records = [(identity(row_id), score) for row_id, score in value["records"]]
    require([row_id for row_id, _ in records] == sorted(births)
            and value["born_memories"] and all(identity(item) in births for item in value["born_memories"]),
            "retained FRS07 snapshot does not cover the accepted owner births")
    materialized = bytearray(b"RELHFRS1" + owner + text(value["import_id"])
                             + pack("<QQQ", value["base_cut"], value["settlement_cut"], entries)
                             + hashlib.sha256(raw).digest() + pack("<I", len(records)))
    for row_id, record in records:
        materialized.extend(row_id + pack("<h", record["score"])
                            + optional(record["admitted_at_turn"]) + optional(record["accounted_turn"]))
    clock = pack("<QQQQ", *(value[key] for key in (
        "global_version", "archive_version", "graph_version", "memory_graph_version",
    )))
    return b"RELHFA01" + clock + pack("<I", len(materialized)) + materialized + raw


def decode_source_lines(path, expected):
    raw = path.read_bytes()
    require(hashlib.sha256(raw).hexdigest() == expected, "verified typed source rows changed")
    return [json.loads(line) for line in raw.splitlines()]


def translate(copies, owner, summary, journal_path, proof_path, memory_summary, memory_path, frames, handled):
    journal = decode_source_lines(journal_path, summary["journal_sha256"])
    proof = json_receipt(proof_path, summary["settlement_sha256"])
    memory = decode_source_lines(memory_path, memory_summary["transfer_sha256"])
    births = {}
    for row in memory:
        group = row.get("group") or {}
        birth = group.get("freshness_birth")
        if birth is not None:
            require(birth["policy_version"] == 1, "unsupported grouped Freshness policy")
            for row_id in birth["memory_ids"]:
                row_id = identity(row_id)
                require(row_id not in births, "duplicate grouped Freshness birth")
                births[row_id] = birth["activity"]
    require(len(births) == summary["grouped_births_bound_in_memory_transfer"]
            and len(journal) == summary["accepted_admissions"] + summary["accepted_events"] + 1,
            "verified Freshness source counts changed")
    uuid = identity(owner["owner_uuid"], 16)
    expected = {row["physical_ordinal"]: row for row in journal + [proof]}
    require(len(expected) == len(journal) + 1, "duplicate Freshness carrier position")
    for ordinal, raw, _ in consume_frames(copies / owner["fixture"], owner):
        observed = expected.pop(ordinal, None)
        if observed is None:
            require(not raw.startswith((b"CVAFRP", b"CVAFRS")), "unreviewed Freshness authority carrier")
            continue
        require(raw[:8].decode("ascii") == observed["carrier"] and raw[8:24] == uuid
                and hashlib.sha256(raw).hexdigest() == observed["source_payload_sha256"],
                "verified Freshness source frame changed")
        if observed["carrier"] == "CVAFRS07":
            body = settlement_frame(uuid, raw, proof, births, len(births) + len(journal) - 1)
        else:
            body = journal_frame(uuid, observed)
        emit(frames, ordinal, "frame", body)
        handled.write(compact({"source_ordinal": str(ordinal), "source_carrier": observed["carrier"],
                               "disposition": "current_freshness_authority"}) + b"\n")
    require(not expected, "verified Freshness source omitted a carrier")
    return {"journal_entries": len(journal) - 1, "settlement_records": len(births)}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--inputs", type=Path, required=True)
    parser.add_argument("--expected-inputs-sha256", required=True)
    parser.add_argument("--copies", type=Path, required=True)
    parser.add_argument("--freshness-transfer", type=Path, required=True)
    parser.add_argument("--expected-freshness-sha256", required=True)
    parser.add_argument("--memory-transfer", type=Path, required=True)
    parser.add_argument("--expected-memory-sha256", required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    inputs = json_receipt(args.inputs, args.expected_inputs_sha256)
    require(inputs["status"] == "VERIFIED_DISPOSABLE_COPY_INPUTS" and inputs["preserves_original_sources"],
            "unverified source input custody")
    owners = {row["fixture"]: row for row in inputs["sources"] if row["kind"] == "REL"}
    freshness = json_receipt(args.freshness_transfer / "freshness-source-transfer.json", args.expected_freshness_sha256)
    memory = json_receipt(args.memory_transfer / "memory-source-transfer.json", args.expected_memory_sha256)
    mem_by_id = {row["copy_id"]: row for row in memory["sources"]}
    require(len(owners) == 3 and len(freshness["sources"]) == 3
            and not args.output.resolve().is_relative_to(args.copies.resolve()), "invalid Freshness source cohort or output")
    args.output.mkdir()
    sources = []
    for row in freshness["sources"]:
        owner = owners.pop(row["fixture"])
        copy_id = row["copy_id"]
        mem = mem_by_id[copy_id]
        require(row["source_sha256"] == mem["source_sha256"] == owner["source_sha256"]
                and owner["verified_disposable_sha256"] == owner["source_sha256"]
                and mem["fixture"] == owner["fixture"] and row["owner_uuid"] == owner["owner_uuid"],
                "Freshness and Memory receipts bind different source files")
        label = Path(owner["fixture"]).parent.name
        frames, handled = args.output / f"{label}.freshness.jsonl", args.output / f"{label}.freshness-handled.jsonl"
        with frames.open("xb") as out, handled.open("xb") as dispositions:
            stats = translate(
                args.copies, owner, row, args.freshness_transfer / f"{copy_id}-freshness-journal.jsonl",
                args.freshness_transfer / f"{copy_id}-freshness-frs07-authority.json",
                mem, args.memory_transfer / f"{copy_id}-memory-transfer.jsonl", out, dispositions,
            )
        sources.append({**stats, "fixture": owner["fixture"], "source_sha256": owner["source_sha256"],
                        "frames": frames.name, "frames_sha256": sha256(frames),
                        "handled": handled.name, "handled_sha256": sha256(handled)})
    require(not owners, "omitted Freshness source")
    receipt = {"status": "PASS", "scope": "Freshness frame translation only",
               "input_custody_sha256": args.expected_inputs_sha256,
               "source_freshness_transfer_sha256": args.expected_freshness_sha256,
               "source_memory_transfer_sha256": args.expected_memory_sha256, "sources": sources}
    with (args.output / "freshness-target-transfer-receipt.json").open("xb") as output:
        output.write(compact(receipt) + b"\n")
    print(compact(receipt).decode("utf-8"))


if __name__ == "__main__":
    main()
