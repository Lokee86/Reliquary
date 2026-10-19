#!/usr/bin/env python3
"""Hash the six authoritative history fixtures without opening mutable owners."""
from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import struct

ROOT = Path(__file__).resolve().parents[1]
CORPORA = (
    "chatgpt-first14d",
    "chatgpt-first28d",
    "rhelm-david-r-ellis-pre-relationship",
)


def inspect(path: Path) -> dict:
    with path.open("rb") as stream:
        before = path.stat()
        header = stream.read(40)
        if len(header) != 40 or header[:8] != b"CVA\0\r\n\x1a\n":
            raise ValueError(f"invalid typed fixture header: {path}")
        major, minor, header_size = struct.unpack_from("<HHI", header, 8)
        if (major, minor, header_size) != (1, 0, 40):
            raise ValueError(f"expected typed V1.0 fixture: {path}")
        kind = {1: "rel", 2: "phy"}.get(header[16])
        if kind is None or kind != ("phy" if path.suffix == ".phy" else "rel"):
            raise ValueError(f"fixture kind mismatch: {path}")
        stream.seek(0)
        digest = hashlib.file_digest(stream, "sha256").hexdigest()
        after = path.stat()
        if (before.st_size, before.st_mtime_ns) != (after.st_size, after.st_mtime_ns):
            raise ValueError(f"fixture changed while hashing: {path}")
    return dict(bytes=after.st_size, sha256=digest, format=[major, minor],
                header_bytes=header_size, kind=kind, owner_uuid=header[24:40].hex())


def inventory(root: Path) -> dict:
    return {f"{corpus}/{name}": inspect(root / corpus / name)
            for corpus in CORPORA for name in ("project.prj.rel", "user.phy")}


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--fixture-root", type=Path,
                        default=ROOT.parent / "reliquary-fixtures/local/authoritative")
    parser.add_argument("--out", type=Path,
                        default=ROOT / "benchmarks/history-fixture-baseline-2026-10-19.json")
    args = parser.parse_args()
    out = args.out.resolve()
    if not out.is_relative_to(ROOT) or out.is_relative_to(args.fixture_root.resolve()):
        raise ValueError("output must be inside the active repository and outside fixtures")
    first = inventory(args.fixture_root)
    # Re-read every byte: a stat-only guard cannot prove unchanged fixture identity.
    if inventory(args.fixture_root) != first:
        raise ValueError("fixture identity changed between inventory passes")
    prior_path = ROOT / "benchmarks/authoritative-history-rewrite-step62-2026-10-18.json"
    prior = json.loads(prior_path.read_text(encoding="utf-8"))["authoritative_hashes"]
    for name, row in first.items():
        if row["sha256"] != prior[name]:
            raise ValueError(f"fixture differs from accepted rewrite baseline: {name}")
    report = dict(schema=1, fixture_root=str(args.fixture_root.resolve()),
                  unchanged_between_passes=True, matches_accepted_step62_hashes=True,
                  total_bytes=sum(row["bytes"] for row in first.values()), fixtures=first)
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(f"verified {len(first)} fixtures, {report['total_bytes']} bytes: {out}")


if __name__ == "__main__":
    main()
