"""Read-only F03 custody check for six pinned 2.0 owner files and validation copies.

Never opens an owner through Reliquary and never writes source or copy bytes.
A passing result is a physical custody check, NOT semantic-history acceptance.
"""
from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path

EXPECTED = {
    "chatgpt-first14d/project.prj.rel": "a02c8696db754c53a73c896133b06df9828a32923aee610c9f918a16698f76d0",
    "chatgpt-first14d/user.phy": "7e690b4853afe0936702dc023f511dd7c78ffaae3e18e8e66f6ab1c6891a6c63",
    "chatgpt-first28d/project.prj.rel": "76c15780bd7cfe7e3083dbff5684e236a8fd2090fb0a82ec3d66529d14307d87",
    "chatgpt-first28d/user.phy": "32970f3975bfcd6de0fcdb78df9c80c78b76787832f528c1b09fd75dfa4f501d",
    "rhelm-david-r-ellis-pre-relationship/project.prj.rel": "cbc38d055936fc5702a17c4d569557d8c35e8887d1946c5562ba419038de6ee3",
    "rhelm-david-r-ellis-pre-relationship/user.phy": "58ac7872215e126749f024fc742828abc9a8a8f84775b39c0706d638392ce9e0",
}


def digest(path: Path) -> str:
    sha = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            sha.update(chunk)
    return sha.hexdigest()


def verify(source_root: Path, copy_root: Path) -> dict:
    rows = []
    for relative, expected in EXPECTED.items():
        source, copy = source_root / relative, copy_root / relative
        before = (source.stat(), copy.stat())
        source_sha, copy_sha = digest(source), digest(copy)
        after = (source.stat(), copy.stat())
        status = (
            source_sha == expected == copy_sha
            and before[0].st_size == before[1].st_size
            and before[0].st_size == after[0].st_size
            and before[1].st_size == after[1].st_size
            and before[0].st_mtime_ns == after[0].st_mtime_ns
            and before[1].st_mtime_ns == after[1].st_mtime_ns
        )
        rows.append({
            "fixture": relative,
            "bytes": before[0].st_size,
            "source_sha256": source_sha,
            "copy_sha256": copy_sha,
            "matches_pinned_and_unchanged": status,
        })
    return {
        "status": "PASS" if all(row["matches_pinned_and_unchanged"] for row in rows) else "FAIL",
        "owners": len(rows),
        "total_bytes": sum(row["bytes"] for row in rows),
        "rows": rows,
        "scope": "physical-custody-only",
    }


def main() -> int:
    workspace = Path(__file__).resolve().parents[2]
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source-root", type=Path, default=workspace / "reliquary-fixtures/local/authoritative")
    parser.add_argument("--copy-root", type=Path, default=workspace / "Reliquary-frs07-validation/corpus-copies")
    args = parser.parse_args()
    try:
        report = verify(args.source_root, args.copy_root)
    except (OSError, ValueError) as error:
        report = {"status": "ERROR", "reason": str(error), "scope": "physical-custody-only"}
    print(json.dumps(report, indent=2))
    return 0 if report["status"] == "PASS" else 1


if __name__ == "__main__":
    raise SystemExit(main())
