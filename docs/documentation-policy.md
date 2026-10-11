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

The canonical standard is the sibling `engineering-standards` repository. This repository currently runs the shared checker directly from that source rather than carrying a generated `.standards/` snapshot.

The local structural commands are:

```text
python ../engineering-standards/tools/docs_policy/check.py --repo .
python ../engineering-standards/tools/docs_policy/check.py --repo . --changed-from origin/main
```

The first checks repository structure. The changed-from form additionally enforces configured documentation impact for implementation paths changed since the comparison revision. Normal Rust verification remains required in addition to documentation checks.

## Related docs

- [Documentation procedure](documentation-procedure.md)
- [Maintainer map](maintainer-map.md)
- [Documentation coverage](documentation-coverage.md)
- [Development](development.md)

## Notes

Repository-local architecture enforcement lives under `tools/pitlord/` and composes the shared architecture-core policy from the sibling `engineering-standards` repository. `python scripts/check_architecture.py --refresh` rebuilds Lexicon/Arcana evidence and then validates the current Pitlord policy; the documentation checker remains a separate structural/change-impact gate.