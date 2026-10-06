# Multiplexing Phase 4 — P4.9 local and convergence acceptance

Parent index: [Documentation index](INDEX.md)  
Parent plan: [Phase 4 distribution plan](multiplexing-phase4-distribution-implementation-plan-2026-10-04.md)  
Audited source: `244d23f`, `feat/multiplexing-p34-convergence`  
Status: **P4.9 validation complete; local/convergence functional acceptance passed with the repository-wide verification exceptions below.**

## Purpose

Record the final Phase 4 in-process acceptance evidence against the converged P3/P4 source. This step verifies existing implementation; it adds no runtime code, public exports, storage encoding, or subscriber authority.

## Overview

The locked library/integration matrix passed 1,026 library tests and 95 integration tests with zero failed or ignored tests. It includes all 12 production convergence regressions, generic deterministic-provider stream tests, gateway authorization/lifecycle, snapshot/resync/reconnect, mutation publication, and private-PHY tests. All 19 detachable CLI tests passed. Runtime/CLI locked checks and formatting passed.

Local acceptance does not assert that every repository gate is green. Unrestricted example compilation and refreshed architecture enforcement remain incomplete as detailed below. P3.8/P3.9 acceptance and Phase 5 public API/host-consumer migration are separate work.

## Acceptance matrix

| Requirement | Collected evidence |
| --- | --- |
| Phase 0B events/auth policy oracle | All 10 events/auth integration fixtures passed. The fixture is a specification model; production evidence is separately listed below. |
| Durable-success preservation | The production 129-branch cut test preserves synchronized mutation success after distribution capacity rejection, invalidates stale delivery, and recovers every branch after epoch reopen. |
| Exact checkpoint text and UTF-8 | Production checkpoint, late-join, archive-ahead/journal-authority, and multibyte suffix regressions passed; broad feeds contain safe summaries only. |
| Passive/inactive viewers and automatic branch | Production captured-fork/passive-viewer and inactive-conversation snapshot regressions passed; the submitter alone changes local branch selection. |
| Atomic commit/subscribe | Production broad Archive commit and exact checkpoint join races passed, as did generic snapshot/register and mutation races. |
| Overflow and reconnect | Count/byte overflow, exact-watermark acknowledgement, fresh-prefix replacement, remount, maintenance quiescence, and grant-generation recovery passed. |
| Revocation, dependency scope, privacy | Current grant/attachment delivery checks, REL/dependency isolation, and private-PHY snapshot/event exclusions passed within the library suite. |
| Restart/interruption | Production interrupted-prefix recovery, explicit terminal status, and no duplicate checkpoint replay passed. |
| Mutator classification | Direct source guard passed: 112 public APIs, all five REL worker files, and 28 isolated positive/negative fixtures. This structural heuristic does not prove arbitrary indirect Rust calls. |

## Commands and results

| Command | Result |
| --- | --- |
| `cargo test --tests --locked` | Passed: 1,026 library + 95 integration tests; zero failed/ignored. |
| `cargo check --locked` | Passed. |
| `cargo fmt --check` | Passed. |
| `cargo fmt --manifest-path cli/Cargo.toml -- --check` | Passed. |
| `cargo check --manifest-path cli/Cargo.toml --locked` | Passed. |
| `cargo test --manifest-path cli/Cargo.toml --locked` | Passed: 19/19. |
| `python scripts/check_multiplexing_phase4_coverage.py --self-test` | Passed: 28 fixtures / 112 APIs / five REL worker files. |
| Both documentation-policy modes | Passed: structural and changed-from origin/main. |
| `cargo test --locked` | Failed during example compilation: E0463 missing `reliquary_memory` and missing Lore rlib artifacts. No unrestricted pass is claimed. |
| `python scripts/check_architecture.py --refresh` | Timed out after 240 seconds during Lexicon Rust/Python analysis; no refreshed enforcement verdict. |

## Repository-wide verification exceptions

The passing `--tests` gate deliberately targets the production library and integration binaries. The separate unrestricted invocation failed in example builds with missing dependency artifacts after runtime check. Historical records also retain the independent `archive_open_profile`/`lore_base` allocator conflict. The newly collected failure does not prove that the allocator conflict was reached or resolved.

The interruption erased retained records for an earlier serial unrestricted retry and longer architecture attempt before their outcomes were collected. Neither is counted as a result. A later serial retry began rebuilding dependencies and was cancelled; it has no full-test verdict. The final file-backed architecture attempt is the outcome in the table above.

The final 240-second architecture attempt passed the direct guard and stopped during Lexicon Rust/Python indexing before Arcana/Pitlord. No refreshed architecture pass is claimed.

Existing product bounds and internal wrappers are unchanged: no network protocol, universal durable event log, large-prefix chunked transfer, or external consumer migration is accepted by this record. The P4.8 audit remains authoritative for lock/durability/source review.

## Documentation impact

- Inspected: Phase 4 plan/audit/convergence, parent multiplexing plan, architecture, invariants, storage format, maintainer map, behavioral contracts, coverage, limitations, development, roadmap, and shared documentation standards.
- Updated: this acceptance record, documentation index, Phase 4/parent plans, architecture status, limitations, roadmap, and development acceptance note.
- Not affected: runtime behavior, public contracts, persistent formats, semantic/distribution ownership, or test implementations.
- Compliance check: structural and changed-from origin/main checks passed.
- Known documentation gaps: repository-wide verification exceptions are explicit; no overall all-gates-green claim.

## Architecture impact

- Standards added or changed: none.
- Ownership or boundary impact: none; P3 owns durable conversation source, P4 owns distribution/subscribers/cursors.
- Pitlord enforcement impact: Lexicon refresh timed out before Arcana/Pitlord; no refreshed enforcement pass.
- Other verification impact: library/integration/CLI and direct coverage/privacy checks passed as listed above.
- Known architectural gaps: refreshed repository enforcement remains subject to the outcome above; Phase 5 consumer/public-export work remains separate.

## Related docs

- [P4.8 audit](multiplexing-phase4-audit-2026-10-05.md)
- [P4.7 production convergence](multiplexing-phase4-convergence-2026-10-05.md)
- [Current limitations](current-limitations.md)
- [Development](development.md)

## Notes

This is scoped local/convergence functional acceptance with disclosed verification exceptions. It does not claim unrestricted Cargo or refreshed Pitlord success.
