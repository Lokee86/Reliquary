# Multiplexing P5-A integrated acceptance — 2026-10-07

Branch: `feat/multiplexing-p5c-api-integration`  
Scope: Reliquary integration worktree only. P5-A source is integrated with the already-merged P5-B lane; Warlock and the legacy host remain outside this worktree's scope.

## Integrated API result

The canonical `GatewayRuntime` owner-qualified search, semantic mutation, provenance, project, status, and reconciliation APIs from P5-A remain the implementation surface. The shared result types used by the new external calls are now defined in canonical gateway semantic modules and re-exported through the existing public root:

- `VisibleArchiveSearchOwner` and `VisibleArchiveSearchResult` are owned by gateway semantic search.
- `RuntimeKnowledgeProvenanceResolution` is owned by gateway semantic provenance.
- Legacy host files and methods remain present for P5-D and continue to use the same public types; no forwarding runtime facade, second worker, journal, or adapter was added.

No new project-creation/adoption convenience API was invented. Those trusted pre-mount lifecycle seams remain an explicit integration decision for the embedding application.

## Verification record

| Gate | Result |
| --- | --- |
| `cargo test --locked -j4 --lib -- --test-threads=4` | **PASS** — all 1,099 tests, including P5-A search/privacy/mutation and current P5-C regressions |
| `cargo check --locked -j4` | **PASS** |
| `cargo fmt --all -- --check` | **PASS** |
| Integration tests | **PASS** — all 128 tests |
| CLI fmt / locked check / locked test | **PASS** — all 19 CLI tests |
| P4 coverage guard | **PASS** — 42 positive/negative fixtures; 148 public APIs classified |
| Documentation structural / change-impact checks | **PASS** |
| Warlock upgraded all-target check | **PASS** — local Reliquary wrapper; Warlock continuation commit `1782e72` |
| Warlock conversation / provider / admission fixtures | **PASS** — 19 + 2 + 2 tests; frontend build and 56-rule policy also pass |
| Reliquary architecture refresh | **TIMED OUT** during Rust indexing; full architecture gate remains open |

The previously missing P5-A runtime evidence is supplied on this integration revision. Final consumer convergence, dependency publication/pins, E/F integration and P5-D deletion remain open.

The earlier focused test/check attempts timed out before assertions or lost dependency metadata. They are historical failures and are superseded by the executed results above. The initial full-library run exposed a metadata test trying to rename a nonexistent conversation; the fixture now seeds a real admitted turn and retains the rejection assertion.

## Documentation impact

- Inspected: P5-A acceptance, P5-A consumer matrix, P5-B acceptance, current gateway implementation, root exports, semantic tests, and source guard.
- Updated: this record, API, architecture, invariants, maintainer map, behavioral contracts and documentation coverage.
- Not affected: persisted formats, CLI behavior, legacy host deletion, worker/journal ownership and unrelated Warlock main.
- Compliance check: formatting, library/integration/CLI checks, P4 coverage and both documentation policy checks pass. Full architecture refresh timed out.
- Known documentation gaps: final E/F/consumer convergence and P5-D current-state cleanup remain pending.

## Architecture impact

- Standards added or changed: none.
- Ownership or boundary impact: canonical gateway result ownership is retained. Exact submission matching uses the existing coordinator reservation; transcript reads join only acknowledged native checkpoints at the existing owner/coordinator/journal cut. No second queue or mutable owner is introduced.
- Pitlord enforcement impact: none; the existing P4 coverage classifications remain passing.
- Other verification impact: complete library/integration/CLI tests and upgraded Warlock compilation pass. Verification builds use per-task directories outside the cargo-reclaim daemon's C:/!bin scan root.
- Known architectural gaps: multi-context consumer UX, actual provider conformance, public P4 delivery/reconnect, E/F convergence and production dependency alignment remain open.

## Current continuation evidence

The 2026-10-07 P5-C continuation repaired missing canonical result re-exports, the legacy provenance consumer import, and project-lifecycle reconciliation/identity visibility. `cargo check --locked` now passes in this integration worktree. Warlock `cargo-local-reliquary.ps1 check --all-targets` also passes against it. Earlier timeout entries above are historical. The current integration passes all 1,099 library tests, including P5-A owner-qualified search and semantic mutation regressions, exact submission dispatch and acknowledged checkpoint recovery. This supplies the previously missing P5-A runtime evidence; final P5 convergence and release acceptance remain open.

## Purpose

Record the source contracts and verification for this isolated multiplexing lane.

## Overview

This record is branch-scoped. Source/API progress and executed runtime acceptance are separate gates; the parent parallel plan governs convergence.

## Related docs

- [Parallel execution plan](multiplexing-phase5-parallel-execution-plan-2026-10-06.md)
- [External consumer lane](multiplexing-phase5c-external-consumers-2026-10-06.md)

## Notes

Historical test results do not certify later integration revisions. Final release acceptance remains subject to the parent plan's consumer, deletion and release gates.
