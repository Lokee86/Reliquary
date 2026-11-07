"""Compare every retained current Graph publication to independently decoded source history."""
import argparse
import json
from pathlib import Path
from struct import unpack

from verify_candidate_vectors import CURRENT, LEGACY, digest, file_info, open_frames, require


def read_checked(path, expected, limit):
    before = file_info(path)
    require(before[2] <= limit, "Graph expectation exceeds its evidence bound")
    data = path.read_bytes()
    require(digest(data) == expected and file_info(path) == before, "Graph expectation custody differs")
    return data


def source_graph(transfer, expected):
    for line in read_checked(transfer, expected, 32 * 1024 * 1024).splitlines():
        row = json.loads(line)
        if row["kind"] == "graph":
            time = row["transaction_time_ns"]
            yield (int(row["global_version"]), int(row["graph_version"]), row["bound_reservation"],
                   None if time is None else int(time), row["changes"])


def current_graph(frames, owner):
    for _, payload in frames:
        require(payload[:8] not in {b"RELGPB01", b"RELGRF01"},
                "converted Graph publication lost its source global envelope")
        if payload[:8] != b"RELPUB01":
            continue
        mode = payload[16]
        require(mode in (0, 1, 2), "unknown current publication clock mode")
        start = 25 if mode == 1 else 17
        body = payload[start:]
        if body[:8] != b"RELGPB01":
            continue
        require(body[8:24] == owner, "Graph publication names a different owner")
        global_, = unpack("<Q", payload[8:16])
        time = unpack("<q", payload[17:25])[0] if mode == 1 else None
        local, count = unpack("<QI", body[24:36])
        require(count > 0 and len(body) == 36 + 70 * count, "invalid current atomic Graph extent")
        changes = []
        for offset in range(36, len(body), 70):
            change = body[offset:offset + 70]
            kind, active, origin = unpack("<HBB", change[66:])
            require(change[0] in (1, 2) and change[33] in (1, 2) and active in (0, 1),
                    "invalid retained Graph relation fields")
            changes.append([[change[0], change[1:33].hex()],
                            [change[33], change[34:66].hex()], kind, bool(active), origin])
        yield (global_, local, mode == 2, time, changes)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--candidate", type=Path, required=True)
    parser.add_argument("--candidate-sha256", required=True)
    parser.add_argument("--source-copy", type=Path, required=True)
    parser.add_argument("--source-sha256", required=True)
    parser.add_argument("--expectations", type=Path, required=True)
    parser.add_argument("--expectations-sha256", required=True)
    args = parser.parse_args()
    report = json.loads(read_checked(args.expectations, args.expectations_sha256, 64 * 1024))
    matches = [row for row in report["sources"] if row["source_sha256"] == args.source_sha256]
    require(len(matches) == 1, "independent Graph evidence lacks the exact source specimen")
    source = matches[0]
    with args.source_copy.open("rb") as file:
        header = file.read(40)
    require(header[24:].hex() == source["owner_uuid"], "independent Graph owner identity differs")
    for _ in open_frames(args.source_copy, LEGACY, args.source_sha256, header[16:]):
        pass
    path = args.expectations.with_name(f'{source["copy_id"]}-graph-transfer.jsonl')
    wanted = list(source_graph(path, source["transfer_sha256"]))
    observed = list(current_graph(open_frames(args.candidate, CURRENT, args.candidate_sha256, header[16:]),
                                  header[24:]))
    require(wanted == observed, "retained Graph publication values or coordinates differ")
    changes = sum(len(row[-1]) for row in observed)
    require(len(observed) == source["graph_publications"] and changes == source["relation_mutations"],
            "retained Graph publication count differs")
    print(json.dumps({"status": "PASS", "scope": "retained Graph transactions and relation mutations",
                      "source_sha256": args.source_sha256, "candidate_sha256": args.candidate_sha256,
                      "publications": len(observed), "relation_mutations": changes,
                      "source_graph_expectations_sha256": args.expectations_sha256}))


if __name__ == "__main__":
    main()
