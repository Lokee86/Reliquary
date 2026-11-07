"""Compare retained Insomnia work, grouped receipts, and Dream history after migration."""
import argparse
from collections import Counter
import io
import json
from pathlib import Path
from struct import unpack

from verify_candidate_vectors import CURRENT, LEGACY, open_frames, require


class Fields(io.BytesIO):
    def exact(self, size):
        require(size >= 0 and size <= len(self.getbuffer()) - self.tell(), "invalid retained field extent")
        return self.read(size)

    def u32(self):
        return unpack("<I", self.exact(4))[0]

    def field(self):
        return self.exact(self.u32())

    def ids(self):
        return self.exact(32 * self.u32())

    def done(self):
        require(not self.read(1), "trailing retained journal bytes")


def publication_body(payload):
    if payload[:8] not in (b"CVAPUB01", b"RELPUB01"):
        return payload
    require(len(payload) >= 25 and payload[16] in (0, 1, 2), "invalid journal publication envelope")
    body = payload[25 if payload[16] == 1 else 17:]
    if body[:8] == b"RELAPB01":
        if body[16] == 1:
            return b""
        require(body[16] == 0, "invalid Archive publication application flag")
        return body[17:]
    return body


def episode(payload, legacy):
    start = 16 if legacy else 8
    with Fields(payload[start:]) as file:
        identifier = file.exact(32)
        file.exact(2)
        source_through = file.exact(8)
        file.exact(8)
        conversation, start_node, end_node = file.field(), file.field(), file.field()
        require(conversation and start_node and end_node, "empty accepted Episode order")
        file.done()
        return identifier, (source_through, conversation, start_node)


def work(payload, owner, episodes, revisions, legacy):
    with Fields(payload[8:]) as file:
        if legacy:
            episode_id = file.exact(32)
        else:
            require(file.exact(16) == owner, "Insomnia work names a different owner")
            revision, = unpack("<Q", file.exact(8))
            episode_id = file.exact(32)
            order = (file.exact(8), file.field(), file.field())
            require(revision == revisions[episode_id] + 1 and episodes.get(episode_id) == order,
                    "Insomnia work revision or accepted Episode order differs")
        require(episode_id in episodes, "Insomnia work precedes an accepted Episode")
        revisions[episode_id] += 1
        tail = file.read()
        return ("work", episode_id, revisions[episode_id], episodes[episode_id], tail)


def memory_link(body, global_, legacy):
    if legacy:
        require(body[:8] == b"CVAMC001" and len(body) >= 88, "invalid legacy grouped Memory link")
        require(unpack("<Q", body[72:80])[0] == global_, "legacy grouped Memory global differs")
        return body[8:40], body[40:72], 1, unpack("<Q", body[80:88])[0], global_
    require(body[:8] == b"RELMCRT1" and len(body) >= 88, "invalid current grouped Memory link")
    return body[8:40], body[48:80], unpack("<Q", body[40:48])[0], unpack("<Q", body[80:88])[0], global_


def group(payload, legacy):
    with Fields(payload[8:]) as file:
        layout = file.exact(1)[0]
        require(layout in (3, 5, 6), "unsupported retained grouped receipt layout")
        header = file.exact(68)
        global_start, count = unpack("<QI", header[-12:])
        if layout >= 5:
            file.exact(8)
        for _ in range(2):
            file.field()
        file.ids()
        for _ in range(file.u32()):
            file.field()
            file.exact(32)
        for _ in range(file.u32()):
            file.exact(32)
            file.field()
        prefix = payload[8:8 + file.tell()]
        require(file.u32() == count, "grouped receipt publication count differs")
        links = [memory_link(file.field(), global_start + index, legacy) for index in range(count)]
        routing = []
        if layout >= 5:
            for _ in range(file.u32()):
                row = file.field()
                require(row[:8] == (b"CVAMRTE2" if legacy else b"RELMRTE2"),
                        "retained group has an unsupported routing record")
                routing.append(row[8:])
        births = b""
        if layout == 6:
            births = file.exact(16) + file.ids()
        file.done()
        return ("completion", prefix, links, routing, births)


def journal(frames, owner, legacy):
    episodes = {}
    revisions = Counter()
    for _, payload in frames:
        body = publication_body(payload)
        if body[:8] == (b"CVAEPI02" if legacy else b"RELEPIS1"):
            identifier, order = episode(body, legacy)
            require(identifier not in episodes, "repeated Episode changes journal order")
            episodes[identifier] = order
        marker = payload[:8]
        if marker == (b"CVAINSW1" if legacy else b"RELINSW1"):
            yield work(payload, owner, episodes, revisions, legacy)
        elif marker == (b"CVAINSC8" if legacy else b"RELINSC8"):
            require(payload[9:41] in episodes, "grouped receipt precedes accepted Episode")
            yield group(payload, legacy)
        elif marker == (b"CVARELM1" if legacy else b"RELRELM1"):
            yield ("owner_metadata", payload[8:])
        elif marker in (b"CVDREAM1", b"CVDREAM2"):
            yield ("dream_journal", payload)
        elif marker in ((b"CVADRM01", b"CVADRP02") if legacy else (b"RELDRM01", b"RELDRP02")):
            yield ("dream_pair", payload[3:])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--candidate", type=Path, required=True)
    parser.add_argument("--candidate-sha256", required=True)
    parser.add_argument("--source-copy", type=Path, required=True)
    parser.add_argument("--source-sha256", required=True)
    args = parser.parse_args()
    with args.source_copy.open("rb") as file:
        header = file.read(40)
    source = list(journal(open_frames(args.source_copy, LEGACY, args.source_sha256, header[16:]), header[24:], True))
    target = list(journal(open_frames(args.candidate, CURRENT, args.candidate_sha256, header[16:]), header[24:], False))
    require(source == target, "retained Insomnia, Dream or owner metadata values/order differ")
    print(json.dumps({"status": "PASS", "scope": "retained Insomnia and Dream journal history and owner metadata",
                      "source_sha256": args.source_sha256, "candidate_sha256": args.candidate_sha256,
                      "rows": dict(Counter(row[0] for row in target))}))


if __name__ == "__main__":
    main()
