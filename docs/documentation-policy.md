# Reliquary Memory v2 Documentation Policy

Parent index: [Documentation index](INDEX.md)

## Purpose

This document defines how this repository applies the shared Laughing Skull engineering documentation standard.

## Overview

The repository uses the `library-engine` profile with the `stateful` capability. Current implementation, accepted architecture, future work, limitations, and agent guidance have separate canonical owners. Documentation changes with the implementation that changes its facts.

## Canonical ownership

- `README.md` is the repository entry point and current status summary.
- [Operator and integration manual](manual/INDEX.md) is task-oriented workflow guidance. It derives current facts from canonical reference owners and must not become a competing architecture/API/storage authority.
- [Architecture](architecture.md) owns implemented responsibilities, state, lifecycle, boundaries, and code map.
- [Architectural invariants](invariants.md) owns governing constraints.
- [Storage format](storage-format.md) owns exact durable encoding and compatibility behavior.
- [Rust API](api.md) owns the current public library surface.
- [Development](development.md) owns build, test, and smoke workflows.
- [Maintainer map](maintainer-map.md) routes change areas to canonical owners and implementation boundaries.
- [Behavioral contracts](behavioral-contracts.md) maps critical behavior to protecting tests.
- [Roadmap](roadmap.md) owns future implementation sequence.
- [Version history and rollback](version-history-plan.md) owns future whole-CVA historical-view, restore, retention, and reclamation design.
- [Current limitations](current-limitations.md) owns present defects and incomplete behavior.
- [Documentation coverage](documentation-coverage.md) maps production code and examples to current documentation.
- [Architectural decisions](decisions/INDEX.md) record why consequential choices were made; they do not replace current architecture or planning owners.

## Required rules

1. Implemented behavior must not exist only in a plan or ADR.
2. Unimplemented behavior must be labeled as such and must not be described as current capability.
3. Exact persistent-format facts belong in `storage-format.md`.
4. Every direct Markdown file or documentation subfolder under `docs/` is listed in `docs/INDEX.md`.
5. Storage ownership, mutation, recovery, compatibility, and testing changes update their canonical owners in the same change.
6. A documentation checker pass proves structural compliance only; known semantic gaps remain explicit.
7. Planning documents are future-only. When planned behavior becomes current, remove it from the plan and document the resulting behavior in the appropriate current-state owner.
8. Multiplexing has one active execution plan, `multiplexing-implementation-plan.md`. Update it in place. Do not create dated acceptance, handoff, checkpoint or next-wave documents; keep verification artifacts in CI/external logs and commit history. Technical facts belong in the canonical architecture/API/behavioral contracts or an ADR.

## Enforcement

The canonical standard is the sibling `engineering-standards` repository. Reliquary composes three deterministic shared-tool gates rather than implementing repository-local documentation validation:

- the shared documentation-policy checker enforces the declared repository profile and compatibility rules;
- Archivist (`ddocs`) owns Markdown indexes, local links, navigation integrity, and codemap extraction/resolution; and
- Pitlord consumes Archivist codemaps plus the current Arcana file inventory and fails when code lacks codemap ownership or changed code has no owning-document change.

Local verification, with the shared tools available on `PATH`, is:

```text
python ../engineering-standards/tools/docs_policy/check.py --repo .
python ../engineering-standards/tools/docs_policy/check.py --repo . --changed-from origin/main
ddocs check --root docs --indexes --links
python scripts/check_architecture.py --refresh
pitlord docs --repo . --ddocs-root docs --changed-from origin/main --timeout 2m
```

All of these are blocking gates. The repository must not replace them with ad hoc source-layout scripts. Normal Rust verification remains required in addition to documentation checks.

## Related docs

- [Documentation procedure](documentation-procedure.md)
- [Maintainer map](maintainer-map.md)
- [Documentation coverage](documentation-coverage.md)
- [Development](development.md)

## Notes

Repository-local architecture policy lives under `tools/pitlord/` and composes the shared architecture-core policy from the sibling `engineering-standards` repository. `python scripts/check_architecture.py --refresh` rebuilds Lexicon/Arcana evidence and validates architecture policy. The separate `pitlord docs` guard uses that Arcana evidence with Archivist's codemap export to enforce documentation ownership. CI runs Archivist integrity and Pitlord ownership as independent blocking surfaces so one failure cannot make the other disappear.