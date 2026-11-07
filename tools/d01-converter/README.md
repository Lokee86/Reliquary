# D01-T01 provisional tooling lane

Status: **provisional synthetic-only scaffold**. This is not an accepted converter and does not implement the FRS07 reader, 2.1 target writer, source traversal, candidate publication, or any authentic conversion.

This lane is rooted at product integration commit `26e84f45b54985d5aa1ceb3ab8e7fd7f183b8b0a` in a dedicated worktree/branch. The root `Cargo.toml` has an explicit workspace member list; this tooling is under `tools/d01-converter/`, outside that list and the 23-package production DAG. No Cargo manifest is added here.

## Isolation contract

The eventual disposable converter must use separate processes and separate Rust workspaces:

- `source-reader/` is pinned to the verified FRS07 source snapshot and has no target-codec dependency.
- `target-writer/` links only accepted current 2.1 owner APIs and has no old-reader/reference dependency.
- a small coordinator owns custody, invocation and receipt assembly, not semantic policy.

Keep the original source read-only. Use synthetic fixtures only until source admission is independently frozen. Do not use a raw `Container`/store append as a restore API, add an old-format runtime fallback, or move migration semantics into the production DAG. If exact old values cannot be represented by current 2.1, prove the gap with a synthetic owner case and request the narrow value-based operation at its owning boundary.

## What this scaffold contains

- `synthetic.py` creates/parses only the deliberately separate `D01SYN01` test envelope. It never reads files, recognizes no FRS07 marker, and emits no conversion receipt.
- `tests/test_synthetic.py` covers deterministic synthetic building and malformed/unknown refusal.
- `codec-coverage.provisional.json` records inspected target seams and explicit blockers. It has **zero admitted source carriers** and is not the contractual `codec-coverage.json`.

The planning repository's lexical marker inventory is a search hint only. The pinned provisional PG-C01 physical-carrier registry is empty and unauthenticated; neither artifact is an observed-carrier allowlist. Full family coverage must wait for an authenticated source-frame inventory and a reviewed PG-C01 contract.

## Local test

Run the standard-library-only fixture tests from the product repo root:

```powershell
python -m unittest discover -s tools/d01-converter/tests -p "test_*.py" -v
```

This is a synthetic harness check only. No Cargo command, private corpus, source owner, or target candidate is involved. T01 remains incomplete until the required source inventory, shared contract, old/new process boundary, target gap review, and coverage evidence are reviewed.
