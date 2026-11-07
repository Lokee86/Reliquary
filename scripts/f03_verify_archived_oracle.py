"""Verify the immutable canonical V2 to FRS07 archive custody bridge.

This check authenticates bytes and the archived receipt's scope. Semantic
reconciliation still requires a fresh independent run of the historical oracle.
"""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess

READER = "c6010fd30dd52acb81c667f7aed99b831307e49f"
RECEIPT = "3c7e8fe7da6f71c5e929a2b8069aa504d2156b4d"
BEFORE = "2b170044d3fd6b693baf0dec326c6429c66d8a9f"
AFTER = "1ec88cc896531db99c17b672bfdedc22193a5474"
ORACLE_PATH = "benchmarks/canonical-history-equivalence-2026-10-21.json"
RECEIPT_PATH = "benchmarks/dependency-convergence-phase3-corpus-conversion-2026-10-23.json"
PINS = {
    "chatgpt-first14d/project.prj.rel": "a02c8696db754c53a73c896133b06df9828a32923aee610c9f918a16698f76d0",
    "chatgpt-first14d/user.phy": "7e690b4853afe0936702dc023f511dd7c78ffaae3e18e8e66f6ab1c6891a6c63",
    "chatgpt-first28d/project.prj.rel": "76c15780bd7cfe7e3083dbff5684e236a8fd2090fb0a82ec3d66529d14307d87",
    "chatgpt-first28d/user.phy": "32970f3975bfcd6de0fcdb78df9c80c78b76787832f528c1b09fd75dfa4f501d",
    "rhelm-david-r-ellis-pre-relationship/project.prj.rel": "cbc38d055936fc5702a17c4d569557d8c35e8887d1946c5562ba419038de6ee3",
    "rhelm-david-r-ellis-pre-relationship/user.phy": "58ac7872215e126749f024fc742828abc9a8a8f84775b39c0706d638392ce9e0",
}


def git(repo, *args):
    return subprocess.check_output(["git", "-C", str(repo), *args])


def require(condition, message):
    if not condition:
        raise ValueError(message)


def rows(document):
    result = {row["fixture"]: row for row in document["fixtures"]}
    require(len(document["fixtures"]) == len(PINS) and result.keys() == PINS.keys(),
            "expected the exact six fixture rows")
    return result


def blob(repo, commit, relative):
    return git(repo, "cat-file", "blob", f"{commit}:local/authoritative/{relative}")


def verify(source, fixtures, run_root):
    oracle_bytes = git(source, "cat-file", "blob", f"{READER}:{ORACLE_PATH}")
    receipt_bytes = git(source, "cat-file", "blob", f"{RECEIPT}:{RECEIPT_PATH}")
    require(oracle_bytes == git(source, "cat-file", "blob", f"{RECEIPT}:{ORACLE_PATH}"),
            "the receipt must use the unchanged archived oracle")
    require(git(fixtures, "rev-parse", f"{AFTER}^").strip().decode() == BEFORE,
            "fixture promotion does not directly descend from the oracle preimage")
    receipt = json.loads(receipt_bytes)
    oracle = json.loads(oracle_bytes)
    require(oracle["all_owner_domain_mismatches"] == 0, "oracle records semantic mismatches")
    require(receipt["format"] == "CVAFRS07"
            and git(source, "rev-parse", receipt["runtime_source_commit"]).strip().decode() == READER
            and git(fixtures, "rev-parse", receipt["fixture_before_commit"]).strip().decode() == BEFORE
            and receipt["fixture_promotion_commit"] == AFTER,
            "receipt revision or format mismatch")
    observed = receipt["semantic_oracle"]
    require(observed["passed"] is True and observed["fixture_count"] == len(PINS)
            and observed["semantic_mismatches"] == 0
            and observed["inspection_preserved_bytes_and_mtime"] is True,
            "archived receipt does not report a six-file passing comparison")
    expected = rows(oracle)
    mapping = rows(receipt)
    output = []
    if run_root is not None:
        require(run_root.is_dir(), "run root must already be private and present")
        (run_root / "corpus-copies").mkdir()
    for relative, pinned in PINS.items():
        original = blob(fixtures, BEFORE, relative)
        current = blob(fixtures, AFTER, relative)
        original_hash = hashlib.sha256(original).hexdigest()
        current_hash = hashlib.sha256(current).hexdigest()
        row = mapping[relative]
        unchanged = original_hash == current_hash
        require(original_hash == expected[relative]["canonical_sha256"] == row["original_sha256"]
                and current_hash == pinned == row["converted_sha256"]
                and row["original_bytes"] == len(original)
                and row["converted_bytes"] == len(current)
                and row["source_unchanged"] is True
                and row.get("physical_bytes_unchanged", False) is unchanged
                and unchanged == relative.endswith(".phy"),
                f"fixture custody bridge differs for {relative}")
        if run_root is not None:
            destination = run_root / "corpus-copies" / relative
            destination.parent.mkdir(exist_ok=True)
            with destination.open("xb") as handle:
                handle.write(current)
        output.append({
            "fixture": relative,
            "canonical_v2_sha256": original_hash,
            "frs07_sha256": current_hash,
            "frs07_bytes": len(current),
        })
    if run_root is not None:
        with (run_root / "archived-migration-receipt.json").open("xb") as handle:
            handle.write(receipt_bytes)
    return {
        "status": "PASS",
        "scope": "archived-oracle-physical-custody-bridge",
        "reader_commit": READER,
        "receipt_commit": RECEIPT,
        "fixture_preimage_commit": BEFORE,
        "fixture_postimage_commit": AFTER,
        "archived_oracle_sha256": hashlib.sha256(oracle_bytes).hexdigest(),
        "migration_receipt_sha256": hashlib.sha256(receipt_bytes).hexdigest(),
        "fixtures": output,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--historical-repo", type=Path, required=True)
    parser.add_argument("--fixture-repo", type=Path, required=True)
    parser.add_argument("--private-run-root", type=Path)
    args = parser.parse_args()
    report = verify(args.historical_repo, args.fixture_repo, args.private_run_root)
    print(json.dumps(report, indent=2))


if __name__ == "__main__":
    main()
