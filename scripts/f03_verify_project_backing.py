"""Verify external Git attachment bytes referenced by the pinned F03 REL copies."""
import argparse
import csv
import hashlib
import io
import json
from pathlib import Path

from f03_verify_archived_oracle import PINS, git, require
from f03_verify_independent_repeat import read_exact, regular, snapshot

REVISION = "ec1a547f81294bb540b86bc54da875e1d9945ea3"
PROJECT = "local/projects/chatgpt-history"
REPOSITORY_ID = "1f5a2319-e3e4-4c1a-9792-8191ca5d1cd2"
EXPECTED_COUNTS = (95, 99, 0)


def read_bindings(path, pinned_hash):
    before = regular(path)
    bindings = {}
    with path.open("rb") as file:
        header = read_exact(file, 40)
        require(header[:16] == b"CVA\0\r\n\x1a\n\x02\0\0\0\x28\0\0\0" and header[16] == 1,
                "expected a typed historical REL")
        digest = hashlib.sha256(header)
        while file.tell() < before.st_size:
            length = read_exact(file, 8)
            size = int.from_bytes(length, "little")
            require(8 <= size <= 16 * 1024 * 1024 and file.tell() + size <= before.st_size,
                    "invalid historical frame length")
            payload = read_exact(file, size)
            digest.update(length)
            digest.update(payload)
            if payload.startswith(b"PRJFILE1"):
                key, value = file_binding(payload)
                require(key not in bindings, "duplicate external attachment identity")
                bindings[key] = value
    require(digest.hexdigest() == pinned_hash and snapshot(before) == snapshot(regular(path)),
            "attachment witness source changed or is not pinned")
    return bindings


def file_binding(payload):
    require(len(payload) >= 90 and payload[40:42] == b"\x02\x01",
            "expected hashed Git attachment binding")
    with io.BytesIO(payload[74:]) as fields:
        names = []
        for _ in range(4):
            size = int.from_bytes(read_exact(fields, 4), "little")
            require(0 < size <= 32 * 1024, "invalid attachment field length")
            names.append(read_exact(fields, size).decode("utf-8"))
        require(not fields.read(1), "trailing attachment binding bytes")
    repo_id, project, revision, path = names
    require((repo_id, project, revision) == (REPOSITORY_ID, PROJECT, REVISION),
            "unexpected repository identity, project path or revision")
    require(path.startswith(PROJECT + "/") and "\\" not in path
            and all(part not in {"", ".", ".."} for part in path.split("/")),
            "attachment path escapes pinned project")
    return payload[8:40].hex(), (payload[42:74].hex(), path)


def verify(copies, repo):
    index = git(repo, "cat-file", "blob", f"{REVISION}:{PROJECT}/ATTACHMENTS.tsv")
    entries = list(csv.DictReader(io.StringIO(index.decode("utf-8")), delimiter="\t"))
    table = {row["file_id"]: row for row in entries}
    require(len(entries) == len(table) == 99, "unexpected attachment index membership")
    tree = {}
    for record in git(repo, "ls-tree", "-rz", REVISION, "--", PROJECT).split(b"\0"):
        if record:
            header, name = record.split(b"\t", 1)
            mode, kind, blob_id = header.split(b" ")
            if mode in {b"100644", b"100755"} and kind == b"blob":
                tree[name.decode("utf-8")] = blob_id.decode("ascii")
    rel_files = sorted(relative for relative in PINS if relative.endswith(".rel"))
    counts = []
    verified = {}
    memberships = []
    for relative, count in zip(rel_files, EXPECTED_COUNTS):
        bindings = read_bindings(copies / relative, PINS[relative])
        require(len(bindings) == count, "unexpected attachment count for pinned REL")
        counts.append(len(bindings))
        memberships.append(set(bindings))
        for file_id, (digest, path) in bindings.items():
            row = table[file_id]
            require(row["content_id"] == digest and f'{PROJECT}/{row["repository_path"]}' == path,
                    "attachment index disagrees with retained binding")
            if path not in verified:
                body = git(repo, "cat-file", "blob", tree[path])
                verified[path] = (hashlib.sha256(body).hexdigest(), len(body))
            require(verified[path] == (digest, int(row["bytes"])),
                    "pinned Git attachment digest or length mismatch")
    require(memberships[0] < memberships[1] == table.keys() and not memberships[2],
            "attachment membership differs from the archived fixture cohorts")
    return {
        "status": "PASS",
        "scope": "external attachment custody for the three pinned REL copies",
        "repository_revision": REVISION,
        "attachment_index_sha256": hashlib.sha256(index).hexdigest(),
        "binding_counts_in_fixture_order": counts,
        "verified_distinct_attachment_blobs": len(verified),
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source-copies", type=Path, required=True)
    parser.add_argument("--fixture-repo", type=Path, required=True)
    args = parser.parse_args()
    print(json.dumps(verify(args.source_copies, args.fixture_repo), indent=2))


if __name__ == "__main__":
    main()
