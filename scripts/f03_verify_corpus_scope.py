"""Authenticate the exact committed F03 fixture scope and historical owner headers."""
import argparse
import hashlib
import json
from pathlib import Path
import re

from f03_verify_archived_oracle import AFTER, PINS, git, require
from f03_verify_independent_repeat import read_exact, regular, snapshot

GIT_ROOT = "local/authoritative"
HEADER = b"CVA\0\r\n\x1a\n\x02\0\0\0\x28\0\0\0"
SCOPES = {0: "unspecified", 1: "organization", 2: "project", 3: "connection"}


def inventory(repo, expected):
    observed = set()
    for row in git(repo, "ls-tree", "-rz", AFTER, "--", GIT_ROOT).split(b"\0"):
        if not row:
            continue
        descriptor, name = row.split(b"\t", 1)
        require(descriptor.startswith(b"100644 blob "), "fixture tree contains a non-regular entry")
        name = name.decode("utf-8")
        require(name.startswith(GIT_ROOT + "/"), "fixture path escapes the committed scope")
        observed.add(name[len(GIT_ROOT) + 1:])
    require(observed == expected, "committed fixture inventory differs from reviewed population")


def verified_metadata(repo, path, relative):
    pinned = git(repo, "cat-file", "blob", f"{AFTER}:{GIT_ROOT}/{relative}")
    info = regular(path)
    require(0 < info.st_size <= 64 * 1024, "fixture metadata is outside its expected bounds")
    text = path.read_text(encoding="utf-8")
    require(text == pinned.decode("utf-8").replace("\r\n", "\n")
            and snapshot(info) == snapshot(regular(path)), "fixture metadata differs from the pinned promotion")
    return text, hashlib.sha256(pinned).hexdigest()


def read_owner(path, expected_uuid, expected_kind, pinned):
    before = regular(path)
    with path.open("rb") as source:
        header = read_exact(source, 40)
        digest = hashlib.sha256(header)
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    require(header[:16] == HEADER and header[16] == expected_kind and header[17] in SCOPES
            and (expected_kind == 1 or header[17] == 0) and not any(header[18:24]),
            "invalid historical owner header")
    require(digest.hexdigest() == pinned and snapshot(before) == snapshot(regular(path)),
            "owner file differs from pinned custody")
    require(header[24:].hex() == expected_uuid, "owner UUID differs from committed fixture metadata")
    return {"owner_uuid": expected_uuid, "kind": "REL" if expected_kind == 1 else "PHY",
            "scope": SCOPES[header[17]], "source_sha256": pinned, "byte_length": before.st_size}


def verify(repo):
    root = repo / GIT_ROOT
    cohorts = sorted({relative.split("/")[0] for relative in PINS})
    expected_files = set(PINS)
    for cohort in cohorts:
        expected_files.update(f"{cohort}/{name}" for name in ("AUTHORITATIVE.txt", "SHA256SUMS.txt"))
    inventory(repo, expected_files)
    regular(root, directory=True)
    require({entry.name for entry in root.iterdir()} == set(cohorts), "unexpected authoritative cohort entry")
    scopes = []
    metadata = []
    for cohort in cohorts:
        directory = root / cohort
        regular(directory, directory=True)
        expected_names = {name.split("/", 1)[1] for name in expected_files if name.startswith(cohort + "/")}
        require({entry.name for entry in directory.iterdir()} == expected_names,
                "unbound file or sidecar in authoritative cohort")
        authority, authority_digest = verified_metadata(
            repo, directory / "AUTHORITATIVE.txt", f"{cohort}/AUTHORITATIVE.txt")
        sums, sums_digest = verified_metadata(repo, directory / "SHA256SUMS.txt", f"{cohort}/SHA256SUMS.txt")
        checksum_rows = [line.split() for line in sums.splitlines() if line.strip()]
        require(len(checksum_rows) == 2 and all(len(row) == 2 for row in checksum_rows),
                "ambiguous fixture checksum metadata")
        checksums = {name: digest for digest, name in checksum_rows}
        for filename, kind, label in (("project.prj.rel", 1, "REL"), ("user.phy", 2, "PHY")):
            relative = f"{cohort}/{filename}"
            uuid = re.findall(rf"^{label} owner: ([a-f0-9]{{32}})$", authority, re.MULTILINE)
            require(len(uuid) == 1 and checksums.get(filename) == PINS[relative],
                    "fixture identity or checksum authority is ambiguous")
            scopes.append({"fixture": relative, **read_owner(directory / filename, uuid[0], kind, PINS[relative])})
        metadata.append({"cohort": cohort, "authority_sha256": authority_digest,
                         "checksum_list_sha256": sums_digest})
    for cohort in cohorts:
        directory = root / cohort
        expected_names = {name.split("/", 1)[1] for name in expected_files if name.startswith(cohort + "/")}
        require({entry.name for entry in directory.iterdir()} == expected_names,
                "authoritative fixture inventory changed")
    require({entry.name for entry in root.iterdir()} == set(cohorts), "authoritative cohort inventory changed")
    return {
        "status": "PASS",
        "scope": "committed F03 authoritative fixture tree only",
        "fixture_promotion_commit": AFTER,
        "authenticated_owner_headers": scopes,
        "metadata": metadata,
        "fixture_operational_sidecars": "absent",
        "external_host_state": "outside_scope_unattested",
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--fixture-repo", type=Path, required=True)
    args = parser.parse_args()
    print(json.dumps(verify(args.fixture_repo), indent=2))


if __name__ == "__main__":
    main()
