"""Recheck the six authoritative originals and disposable conversion inputs without writes."""
import argparse
import hashlib
from pathlib import Path

from f03_verify_archived_oracle import PINS, require
from f03_verify_corpus_scope import GIT_ROOT, verified_metadata
from f03_verify_dream_journal_frontiers import encode
from f03_verify_independent_repeat import read_document, regular, snapshot


def digest(path):
    before = regular(path)
    with path.open("rb") as file:
        value = hashlib.file_digest(file, "sha256").hexdigest()
    require(snapshot(before) == snapshot(regular(path)), "custody file changed during its read")
    return value, before


def verify(fixture_repo, copies, receipt, scope_path):
    originals = fixture_repo / GIT_ROOT
    custody, custody_sha = read_document(receipt, 64 * 1024)
    scope, scope_sha = read_document(scope_path, 64 * 1024)
    require(custody["status"] == "VERIFIED_DISPOSABLE_COPY_INPUTS" and scope["status"] == "PASS"
            and scope_sha == custody["fixture_scope_sha256"]
            and scope["fixture_operational_sidecars"] == "absent",
            "conversion input custody or fixture scope differs")
    owners = {row["fixture"]: row for row in custody["sources"]}
    scope_owners = {row["fixture"]: row for row in scope["authenticated_owner_headers"]}
    require(len(custody["sources"]) == len(scope["authenticated_owner_headers"]) == len(PINS)
            and owners.keys() == scope_owners.keys() == PINS.keys(), "expected the exact six reviewed source owners")
    roots = originals.resolve(), copies.resolve()
    require(not any(root.is_relative_to(other) for root, other in (roots, roots[::-1])),
            "authoritative and disposable roots overlap")
    metadata = {row["cohort"]: row for row in scope["metadata"]}
    cohorts = {name.split("/")[0] for name in PINS}
    require(len(metadata) == len(scope["metadata"]) and metadata.keys() == cohorts,
            "unexpected authoritative fixture cohort")
    for root in (originals, copies):
        regular(root, directory=True)
        require({item.name for item in root.iterdir()} == cohorts, "unexpected source directory member")
        for cohort in cohorts:
            path = root / cohort
            regular(path, directory=True)
            expected = {name.split("/")[1] for name in PINS if name.startswith(cohort + "/")}
            if root == originals:
                expected.update(("AUTHORITATIVE.txt", "SHA256SUMS.txt"))
            require({item.name for item in path.iterdir()} == expected, "unbound source or sidecar file")

    checked = []
    for relative, pin in sorted(PINS.items()):
        row = owners[relative]
        original_sha, original_stat = digest(originals / relative)
        copy_sha, copy_stat = digest(copies / relative)
        require(pin == original_sha == copy_sha == row["source_sha256"] == row["verified_disposable_sha256"]
                == scope_owners[relative]["source_sha256"]
                and original_stat.st_size == copy_stat.st_size == row["byte_length"] == scope_owners[relative]["byte_length"]
                and original_stat.st_mtime_ns == row["original_mtime_ns"],
                f"original or disposable custody differs for {relative}")
        checked.append({"fixture": relative, "source_sha256": pin, "byte_length": original_stat.st_size,
                        "original_mtime_ns": original_stat.st_mtime_ns})
    for cohort, row in sorted(metadata.items()):
        for name, field in (("AUTHORITATIVE.txt", "authority_sha256"), ("SHA256SUMS.txt", "checksum_list_sha256")):
            _, pinned_sha = verified_metadata(fixture_repo, originals / cohort / name, f"{cohort}/{name}")
            require(pinned_sha == row[field], "authoritative fixture metadata differs")

    return {
        "status": "PASS", "scope": "original and disposable input bytes, original mtimes, and authority metadata",
        "verified_conversion_inputs_sha256": custody_sha, "fixture_scope_sha256": scope_sha,
        "fixture_promotion_commit": scope["fixture_promotion_commit"], "original_files": len(checked),
        "disposable_files": len(checked), "authoritative_metadata_files": 2 * len(metadata),
        "sources": checked,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("fixture-repo", "source-copies", "verified-conversion-inputs", "fixture-scope", "output"):
        parser.add_argument(f"--{name}", type=Path, required=True)
    args = parser.parse_args()
    originals = args.fixture_repo / GIT_ROOT
    output = args.output.resolve()
    regular(output.parent, directory=True)
    require(not any(output.is_relative_to(root.resolve()) for root in (originals, args.source_copies)),
            "custody receipt overlaps reviewed source files")
    result = verify(args.fixture_repo, args.source_copies, args.verified_conversion_inputs, args.fixture_scope)
    with output.open("xb") as file:
        file.write(encode(result) + b"\n")


if __name__ == "__main__":
    main()
