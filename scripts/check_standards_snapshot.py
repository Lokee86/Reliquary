#!/usr/bin/env python3
"""Verify every synchronized standards byte without a sibling checkout."""
from __future__ import annotations

import hashlib
import json
from pathlib import Path


def main() -> int:
    repo = Path(__file__).resolve().parents[1]
    root = repo / ".standards"
    manifest_bytes = (root / "manifest.json").read_bytes()
    provenance = json.loads((repo / "standards-provenance.json").read_text(encoding="utf-8"))
    if provenance.get("schema") != "reliquary.standards-provenance.v1":
        raise ValueError("unsupported standards provenance schema")
    if hashlib.sha256(manifest_bytes).hexdigest() != provenance.get("manifest_sha256"):
        raise ValueError("standards manifest does not match recorded provenance")
    manifest = json.loads(manifest_bytes)
    if manifest.get("schema") != "laughing-skull.engineering-standards-snapshot.v1":
        raise ValueError("unsupported standards snapshot schema")
    files = manifest["files"]
    if not files:
        raise ValueError("empty standards snapshot")
    errors = []
    for name, expected in sorted(files.items()):
        path = (root / name).resolve()
        if not path.is_relative_to(root.resolve()):
            raise ValueError(f"snapshot path escapes .standards: {name}")
        if not path.is_file() or hashlib.sha256(path.read_bytes()).hexdigest() != expected:
            errors.append(name)
    if errors:
        print("Standards snapshot mismatch: " + ", ".join(errors))
        return 1
    print(f"Standards snapshot: {len(files)} file hashes verified")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
