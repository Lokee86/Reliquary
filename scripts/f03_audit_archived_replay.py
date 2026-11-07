"""Audit a completed optimized archival oracle replay and its inputs."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import tarfile

from f03_verify_archived_oracle import AFTER, PINS, READER, git, require
from f03_verify_independent_repeat import read_document, regular, snapshot

ARCHIVE_DIGEST = "d3c1cae685932927d30dd4a0b4189280b4acd0487397e89632022fff0c2b7d34"
ARGUMENTS = [
    "test", "--release", "--lib", "--features", "history-oracle-fixtures",
    "--locked", "--offline", "-j", "1",
    "canonical_current_corpus_matches_frozen_history_oracle",
    "--", "--ignored", "--nocapture", "--test-threads=1",
]
INSTRUMENTATION = {
    "src/facade/historical_current_corpus_tests.rs": {
        "Cva::open(&path)": "Cva::open_read_only(&path)",
        "Phylactery::open(&path)": "Phylactery::open_read_only(&path)",
    },
    "src/facade/historical_whole_owner_fingerprint.rs": {
        "Cva::open(&path)": "Cva::open_read_only(&path)",
    },
    "src/facade/historical_whole_phy_fingerprint.rs": {
        "Phylactery::open(&path)": "Phylactery::open_read_only(&path)",
    },
}


def file_digest(path):
    before = regular(path)
    with path.open("rb") as source:
        digest = hashlib.file_digest(source, "sha256").hexdigest()
    require(snapshot(before) == snapshot(regular(path)), "archive replay artifact changed")
    return digest


def verify_reader(run, historical_repo, inputs):
    require(inputs["reader_commit"] == READER and inputs["arguments"] == ARGUMENTS
            and inputs["rustc"] and inputs["cargo"], "archival source, command or toolchain mismatch")
    require(file_digest(historical_repo / "scripts/run_f03_archived_oracle.ps1") == inputs["runner_sha256"],
            "archival replay runner differs from the captured script")
    archive_path = run / "historical-source.tar"
    archive = file_digest(archive_path)
    require(archive == inputs["historical_archive_sha256"]
            and archive == hashlib.sha256(git(historical_repo, "archive", "--format=tar", READER)).hexdigest(),
            "archival reader source differs from the pinned commit")
    reader = run / "reader"
    lock = file_digest(reader / "Cargo.lock")
    require(lock == inputs["cargo_lock_sha256"]
            and lock == hashlib.sha256(git(historical_repo, "cat-file", "blob", f"{READER}:Cargo.lock")).hexdigest(),
            "archival dependency lock differs")
    changes = {row["relative_path"]: row for row in inputs["instrumentation"]}
    require(len(inputs["instrumentation"]) == len(INSTRUMENTATION) and changes.keys() == INSTRUMENTATION.keys(),
            "unexpected archival instrumentation files")
    for relative, replacements in INSTRUMENTATION.items():
        row = changes[relative]
        before = git(historical_repo, "cat-file", "blob", f"{READER}:{relative}")
        after = before.decode("utf-8")
        for old, new in replacements.items():
            require(old in after, "expected archived open call is missing")
            after = after.replace(old, new)
        require(row["replacements"] == replacements
                and row["before_sha256"] == hashlib.sha256(before).hexdigest()
                and row["after_sha256"] == hashlib.sha256(after.encode("utf-8")).hexdigest()
                and row["after_sha256"] == file_digest(reader / relative),
                "archival replay used unreviewed instrumentation")
    member_count = 0
    with tarfile.open(archive_path, "r:") as pinned_source:
        for member in pinned_source:
            if member.isdir():
                continue
            relative = Path(member.name)
            require(member.isfile() and not relative.is_absolute() and ".." not in relative.parts,
                    "unsupported archived source member")
            if member.name not in INSTRUMENTATION:
                content = pinned_source.extractfile(member)
                require(content is not None, "missing archived source member")
                with content:
                    expected = hashlib.file_digest(content, "sha256").hexdigest()
                require(file_digest(reader / relative) == expected,
                        f"archival reader extraction differs at {member.name}")
            member_count += 1
    require(member_count >= len(INSTRUMENTATION) + 1, "archival reader source inventory is empty")
    frozen_oracle = reader / "benchmarks/canonical-history-equivalence-2026-10-21.json"
    require(file_digest(frozen_oracle) == ARCHIVE_DIGEST, "archived oracle differs from frozen reference")


def verify_custody(run, bridge):
    before, before_sha = read_document(run / "custody-before.json", 64 * 1024)
    after, after_sha = read_document(run / "custody-after.json", 64 * 1024)
    require(before_sha == after_sha and before == after and after["status"] == "PASS",
            "source or copy custody changed")
    observed = {row["fixture"]: row for row in after["rows"]}
    require(after["owners"] == len(PINS) and len(after["rows"]) == len(PINS)
            and observed.keys() == PINS.keys()
            and all(row["matches_pinned_and_unchanged"] is True
                    and row["source_sha256"] == row["copy_sha256"] == PINS[key]
                    for key, row in observed.items()), "copy custody does not bind the six pinned originals")
    require(bridge["status"] == "PASS" and bridge["fixture_postimage_commit"] == AFTER
            and bridge["archived_oracle_sha256"] == ARCHIVE_DIGEST,
            "archive promotion bridge does not bind the frozen oracle")
    require(file_digest(run / "archived-migration-receipt.json") == bridge["migration_receipt_sha256"],
            "archived migration receipt changed")
    fixtures = {row["fixture"]: row["frs07_sha256"] for row in bridge["fixtures"]}
    require(len(bridge["fixtures"]) == len(PINS) and fixtures == PINS,
            "archive bridge covers a different fixture population")
    return after_sha


def verify(run, historical_repo, release_deps):
    receipt, receipt_sha = read_document(run / "archived-oracle-repeat.json", 64 * 1024)
    require(receipt["status"] == "PASS" and receipt["oracle_exit"] == 0
            and receipt["custody_exit"] == 0 and receipt["reported_all_six"] is True,
            "fresh archive replay did not complete successfully")
    inputs, inputs_sha = read_document(run / "inputs.json", 64 * 1024)
    require(receipt["input_sha256"] == inputs_sha, "archive replay input receipt changed")
    verify_reader(run, historical_repo, inputs)
    bridge, bridge_sha = read_document(run / "archive-custody-bridge.json", 64 * 1024)
    custody_sha = verify_custody(run, bridge)
    stdout = run / "oracle.stdout.log"
    stderr = run / "oracle.stderr.log"
    require(file_digest(stdout) == receipt["stdout_sha256"] and file_digest(stderr) == receipt["stderr_sha256"],
            "archive replay logs changed")
    log = stdout.read_text(encoding="utf-8-sig")
    names = re.findall(r"^current-format frozen oracle verified (.+)$", log, re.MULTILINE)
    require(len(names) == len(PINS) and set(names) == set(PINS) and names == receipt["reported_owners"]
            and re.search(r"^test result: ok\. 1 passed; 0 failed; 0 ignored;", log, re.MULTILINE),
            "archive replay logs lack the complete six-file test result")
    build = stderr.read_text(encoding="utf-8-sig")
    require(re.search(r"Finished .release. profile \[optimized", build), "archive runner used an unoptimized build")
    binaries = re.findall(r"Running unittests .* \(([^()\r\n]+)\)", build)
    require(len(binaries) == 1, "archive build log does not identify one test executable")
    launched = Path(binaries[0])
    if not launched.is_absolute():
        launched = run / "reader" / launched
    require(re.fullmatch(r"reliquary_memory-[a-f0-9]+\.exe", launched.name)
            and launched.resolve().parent == release_deps.resolve(),
            "archive test executable is outside the isolated release cache")
    return {
        "status": "PASS", "scope": "fresh optimized six-file archived oracle replay",
        "archived_oracle_sha256": ARCHIVE_DIGEST,
        "archive_replay_receipt_sha256": receipt_sha, "archive_inputs_sha256": inputs_sha,
        "archive_bridge_sha256": bridge_sha, "source_custody_sha256": custody_sha,
        "archive_test_executable_sha256": file_digest(launched),
        "source_sha256": PINS, "fixture_count": len(names),
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--private-run-root", type=Path, required=True)
    parser.add_argument("--historical-repo", type=Path, required=True)
    parser.add_argument("--release-deps", type=Path, required=True)
    args = parser.parse_args()
    print(json.dumps(verify(args.private_run_root, args.historical_repo, args.release_deps), indent=2))


if __name__ == "__main__":
    main()
