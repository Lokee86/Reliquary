"""Freeze representative native reads before conversion and compare their values."""
from collections import Counter, defaultdict
import hashlib
import json
import subprocess
from uuid import UUID

from authentic_witness import require
from compare_clocked_prefix import pinned_rows


def freeze(evidence, source_copies, owners, artifacts):
    from f03_verify_archive_values import scan

    coverage = evidence / "witness"
    pubs, contents, _ = scan(coverage, artifacts, owners["REL"],
                             "reliquary-f03-historical-state-v2", source_copies / "chatgpt-first28d/project.prj.rel")
    turns = {}
    episodes = {}
    for row in pubs:
        record = row["record"]
        if "turn" in record:
            node = record["turn"]["node"]
            turns[node["conversation"], node["id"]] = node
        if "episode" in record:
            episode = record["episode"]
            episodes[episode["id"]] = episode

    rel_owner = owners["REL"]["owner_uuid"]
    phy_owner = owners["PHY"]["owner_uuid"]
    rel_label = f"proj-{UUID(rel_owner)}"
    phy_label = f"phy-{UUID(phy_owner)}"
    transfer_dir = evidence / "transfers/memory-transfer-v1"
    memory_receipt = json.loads((transfer_dir / "memory-source-transfer.json").read_bytes())
    receipts = {row["source_sha256"]: row for row in memory_receipt["sources"]}
    histories = {}
    references = {}
    linked = {}
    groups = []
    for kind, owner in owners.items():
        digest = receipts[owner["source_sha256"]]["transfer_sha256"]
        by_memory = defaultdict(list)
        stats = Counter()
        for row in pinned_rows(transfer_dir / f"{owner['copy_id']}-memory-transfer.jsonl", digest):
            revisions = row["revisions"]
            if group := row["group"]:
                groups.extend(group["external_memory_refs"])
            for item in revisions:
                record = item["record"]
                record_id = record["id"]
                cut = revisions[-1]["record"]["memory_version"]
                body = item["body"]
                revision = {
                    "memory_id": record_id, "revision": record["revision"], "body_id": record["body_id"],
                    "cut": cut, "title": body["title"],
                    "body_sha256": hashlib.sha256(body["content"].encode()).hexdigest(),
                }
                by_memory[record_id].append(revision)
                ref = record["source_ref"]
                if kind == "REL" and ref is None and record["source_episode_id"] and record["source_node_id"]:
                    ref = {**record, "owner_id": rel_owner}
                    stats["typed_local_sources_derived"] += 1
                if ref is None:
                    stats["absent_provenance_revisions"] += 1
                    continue
                if ref["owner_id"] not in (rel_owner, rel_label):
                    stats["outside_paired_rel_revisions"] += 1
                    continue
                episode = episodes.get(ref["source_episode_id"])
                node = turns.get((episode["conversation"], ref["source_node_id"])) if episode else None
                if node is None:
                    stats["unavailable_source_revisions"] += 1
                    continue
                stats["paired_archive_source_revisions"] += 1
                for field in ("content_source", "grounding_source"):
                    conversation = ref[f"{field}_conversation_id"]
                    source_node = ref[f"{field}_node_id"]
                    if conversation and source_node:
                        key = "resolved_distinct_sources" if (conversation, source_node) in turns else "unavailable_distinct_sources"
                        stats[key] += 1
                if kind not in linked and len(contents[node["content_id"]]) <= 12_000:
                    linked[kind] = {
                        **revision, "conversation": node["conversation"], "node": node["id"],
                        "turn_sha256": node["content_id"], "episode": ref["source_episode_id"],
                    }
        histories[kind] = by_memory
        references[kind] = dict(stats)

    for group in groups:
        require(group["owner_id"] in (phy_owner, phy_label) and group["memory_id"] in histories["PHY"],
                "grouped source reference outside the paired private history")
    samples = {}
    for kind, histories_by_id in histories.items():
        for revisions in histories_by_id.values():
            if len(revisions) > 1:
                samples[kind] = {**revisions[0], "head_revisions": len(revisions)}
                break
    require(set(samples) == set(linked) == {"REL", "PHY"}, "missing source-backed or earlier revision case")
    return {"history": samples, "provenance": linked, "reference_counts": references,
            "grouped_private_references": len(groups)}


def cli_read(cli, *args):
    result = subprocess.run([str(cli), *map(str, args)], capture_output=True, timeout=300)
    require(result.returncode == 0, f"native read failed during {' '.join(map(str, args[:2]))}")
    report = json.loads(result.stdout)
    if "page" in report:
        items = report["page"]["items"]
        require(len(items) == 1, "native read did not resolve exactly one object")
        return items[0]["value"]
    return report


def verify_samples(cli, candidates, samples):
    rel, phy = candidates / "project.rel", candidates / "user.phy"
    result = {"source_reference_counts": samples["reference_counts"],
              "grouped_private_references": samples["grouped_private_references"]}
    for kind, path in (("REL", rel), ("PHY", phy)):
        row = samples["history"][kind]
        id_, cut, revision = row["memory_id"], str(row["cut"]), str(row["revision"])
        cmd = (kind.lower(), "memory")
        at_cut = cli_read(cli, *cmd, "history", path, id_, "--cut", cut)
        require(at_cut["result"]["status"] == "available"
                and at_cut["result"]["revisions"] == at_cut["result"]["payloads_available"] == row["revision"],
                f"earlier {kind} Memory cut differs from source")
        result[f"{kind.lower()}_earlier_memory"] = {"cut": row["cut"], "revision": row["revision"]}
        if kind == "PHY":
            earlier = cli_read(cli, *cmd, "get", path, id_, "--revision", revision)
            require(earlier["revision"] == row["revision"] and earlier["body_id"] == row["body_id"]
                    and earlier["title"] == row["title"]
                    and hashlib.sha256(earlier["content"].encode()).hexdigest() == row["body_sha256"],
                    "earlier private Memory body differs from source")
            result["phy_earlier_memory"]["body_sha256"] = row["body_sha256"]

    for kind, row in samples["provenance"].items():
        path = rel if kind == "REL" else phy
        command = [kind.lower(), "memory", "provenance", path, row["memory_id"], "--revision", str(row["revision"])]
        if kind == "PHY":
            restricted = cli_read(cli, *command)
            require(restricted["availability"] == "restricted" and restricted["source_turn"] is None,
                    "private provenance is readable without the source REL grant")
            command += ["--rel", rel]
        proof = cli_read(cli, *command)
        require(proof["availability"] == "available" and proof["source_episode_id"] == row["episode"],
                "native Memory provenance differs from the source Episode")
        turn = proof["source_turn"]
        require(turn["conversation_id"] == row["conversation"] and turn["node_id"] == row["node"]
                and hashlib.sha256(turn["content"].encode()).hexdigest() == row["turn_sha256"],
                "native provenance transcript differs from the original turn")
        result[f"{kind.lower()}_provenance_turn_sha256"] = row["turn_sha256"]
    return result
