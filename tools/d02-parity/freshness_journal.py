"""Decode current Freshness policy, admission and event values independently."""
from __future__ import annotations

import hashlib
import json
from struct import unpack

from authentic_witness import require
from compare_authentic_graph import Clock, current_frames
from evidence_io import reject_json_number, unique_object


class Wire:
    def __init__(self, data):
        self.data = data
        self.position = 0

    def take(self, count):
        end = self.position + count
        require(end <= len(self.data), "truncated current Freshness value")
        value = self.data[self.position:end]
        self.position = end
        return value

    def read(self, shape, count):
        return unpack(shape, self.take(count))

    def text(self):
        size, = self.read("<I", 4)
        require(0 < size <= 1024 * 1024, "invalid current Freshness text size")
        return self.take(size).decode("utf-8")

    def option(self):
        flag, = self.take(1)
        require(flag in (0, 1), "invalid current Freshness option")
        return self.read("<Q", 8)[0] if flag else None


def read_entry(body, uuid):
    require(body[8:24] == uuid and body[24:26] == b"\1\0", "invalid current Freshness owner or version")
    reader = Wire(body[26:])
    if body[:8] == b"RELFRP01":
        family = "policy"
        decay, access, linkage, local, outside = reader.read("<Qhhhh", 16)
        value = {
            "policy_version": 1, "decay_turns_per_point": decay,
            "access_principal": access, "linkage_principal": linkage,
            "local_hop_cost": local, "outside_hop_cost": outside,
        }
    else:
        tag, = reader.take(1)
        if tag == 1:
            family = "admit"
            value = {"memory_id": list(reader.take(32)), "source_turn": reader.read("<Q", 8)[0]}
        else:
            require(tag == 2, "current Freshness journal entry outside the pinned population")
            family = "event"
            identity = reader.text()
            turn, producer = reader.read("<QB", 9)
            provenance = {
                "producer_kind": producer, "source_id": reader.text(), "origin_owner": list(reader.take(16)),
            }
            provenance["accepted_turn"], provenance["policy_version"], graph = reader.read("<QQQ", 24)
            proof = {
                "graph_version": graph, "community_generation": reader.option(),
                "community_graph_version": reader.option(),
            }
            count, = reader.read("<I", 4)
            require(0 < count <= (len(reader.data) - reader.position) // 34, "invalid current Freshness effect count")
            effects = [[list(reader.take(32)), reader.read("<h", 2)[0]] for _ in range(count)]
            value = {"event_id": identity, "source_turn": turn, "provenance": provenance,
                     "proof": proof, "effects": effects, "semantic": None}
    require(reader.position == len(reader.data), "trailing current Freshness journal bytes")
    return family, value


def source_entries(path, expected_sha):
    raw = path.read_bytes()
    require(0 < len(raw) <= 16 * 1024 * 1024 and hashlib.sha256(raw).hexdigest() == expected_sha,
            "Freshness source journal size or SHA-256 differs")
    for line in raw.splitlines():
        value = json.loads(line, object_pairs_hook=unique_object,
                           parse_float=reject_json_number, parse_constant=reject_json_number)
        yield value["physical_global_frontier"], value["family"], value["value"]


def compare_journal(candidate, owner, candidate_digest, expected_projection, journal_path, journal_digest):
    expected = list(source_entries(journal_path, journal_digest))
    uuid = bytes.fromhex(owner["owner_uuid"])
    clock = Clock(1_000_000)
    actual = []
    settlements = 0
    for payload in current_frames(candidate, owner, candidate_digest):
        _, body = clock.read(payload)
        if body[:8] in (b"RELFRP01", b"RELFRS02"):
            require(body is payload and settlements == 0, "current Freshness journal appears outside source order")
            family, value = read_entry(body, uuid)
            actual.append((len(clock.times) - 1, family, value))
        elif body.startswith(b"RELHFA01"):
            require(body is payload and settlements == 0
                    and str(len(clock.times) - 1) == expected_projection["global_version"],
                    "current Freshness settlement physical global frontier differs")
            settlements += 1
        else:
            require(not body.startswith((b"RELHFR", b"RELHFA", b"RELFRS", b"RELFRP")),
                    "unreviewed current Freshness carrier")
    require(settlements == 1 and actual == expected,
            "current Freshness policy, admission or event values differ from source")
    return len(actual)
