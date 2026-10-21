# Reliquary Memory v2 Documentation Procedure

Parent index: [Documentation index](INDEX.md)

## Purpose

This document defines the repository-local process for keeping documentation synchronized with implementation and architectural decisions.

## Overview

Use this procedure whenever public API, persistent format, ownership, lifecycle, failure behavior, tests, planning, or known limitations change.

## Procedure

1. Identify the changed responsibility, state, public contract, or invariant.
2. Use [the maintainer map](maintainer-map.md) when the canonical owner is unclear.
3. Classify each fact as current architecture/reference/development, planning, limitation, ADR rationale, or agent guidance.
4. Update the existing canonical owner before creating another document.
5. Keep implementation status explicit: shipped facts graduate into current docs; remaining work stays in planning.
6. Update [documentation coverage](documentation-coverage.md) when implementation boundaries change.
7. Update [behavioral contracts](behavioral-contracts.md) when a critical invariant or protecting test changes.
8. Add or remove entries in [the documentation index](INDEX.md) with every documentation-file or subfolder change.
9. Run the shared policy checker, Archivist integrity gate, Pitlord documentation-ownership guard, and normal Rust verification.
10. Report documentation and architecture impact using the format in `AGENTS.md`.

## Verification

```text
cargo fmt --check
cargo check
cargo test
python ../engineering-standards/tools/docs_policy/check.py --repo .
python ../engineering-standards/tools/docs_policy/check.py --repo . --changed-from origin/main
ddocs check --root docs --indexes --links
python scripts/check_architecture.py --refresh
pitlord docs --repo . --ddocs-root docs --changed-from origin/main --timeout 2m
```

When `cli/` changes or a public library change can affect the detachable CLI, also run:

```text
cargo fmt --manifest-path cli/Cargo.toml -- --check
cargo check --manifest-path cli/Cargo.toml --locked
cargo test --manifest-path cli/Cargo.toml --locked
```

When Archive import, fragment reconstruction, or reopen behavior changes, also run:

```text
cargo run --example archive_roundtrip -- <graph.jsonl> <output.cva>
```

## Failure and recovery

- Missing index entries are fixed in `docs/INDEX.md`; do not suppress them with broad exemptions.
- A stale current claim is corrected in its canonical current owner, not patched only in a plan.
- A future claim found in current documentation is moved or clearly reclassified.
- A structural checker pass with known semantic drift is reported as structural only.
- A format change without an updated format/reference owner is incomplete.

## Related docs

- [Documentation policy](documentation-policy.md)
- [Documentation coverage](documentation-coverage.md)
- [Behavioral contracts](behavioral-contracts.md)
- [Development](development.md)

## Notes

Documentation-only changes can still break repository navigation, examples, and checker policy and require verification.