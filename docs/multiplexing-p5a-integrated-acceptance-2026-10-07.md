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
| `cargo test -j1 --lib p5a_ -- --test-threads=4` | **NOT RUN / FAIL TO START** — timed out after ~124 seconds with no test output; no assertion result claimed |
| `cargo check -j1` | **NOT COMPLETE** — timed out after ~124 seconds while rebuilding dependencies; ended on missing `cfg_if` rmeta in the temperamental target cache |
| `cargo fmt --all -- --check` | **PASS** |
| `python scripts/check_multiplexing_phase4_coverage.py --self-test` | **PASS** — 39 positive/negative fixtures; 128 public APIs classified |
| `git diff --check` | **PASS** |
| Existing gateway semantic/auth/project test groups | **NOT RUN** — dependency cache prevented reaching test execution |
| Integration tests / CLI | **NOT RUN** — no CLI or Warlock changes are in scope |

The focused runtime acceptance gate is therefore **open**. Formatting and source classification are passing evidence only; this document does not claim P5-A runtime acceptance or external-consumer compilation.

## Documentation impact

- Inspected: P5-A acceptance, P5-A consumer matrix, P5-B acceptance, current gateway implementation, root exports, semantic tests, and source guard.
- Updated: this integrated acceptance/API stabilization record.
- Not affected: architecture, storage format, CLI behavior, Warlock, legacy host deletion, worker ownership, journal ownership.
- Compliance check: formatting, P4 source guard, and diff check passed; documentation policy checks were not run because the Rust dependency cache blocked the broader acceptance cycle.
- Known documentation gaps: final P5-C consumer proof and P5-D legacy deletion/root cleanup remain pending; runtime tests need an intact dependency/target cache.

## Architecture impact

- Standards added or changed: none.
- Ownership or boundary impact: common external result shapes moved to their canonical gateway semantic owners; public names and legacy compatibility remain unchanged.
- Pitlord enforcement impact: none; the existing P4 coverage classifications remain passing.
- Other verification impact: source-backed public export stability is improved, but compile/test evidence remains blocked by the dependency cache.
- Known architectural gaps: trusted project creation/adoption convergence and final external consumer migration remain separate gates.

## Current continuation evidence

The 2026-10-07 P5-C continuation repaired missing canonical result re-exports, the legacy provenance consumer import, and project-lifecycle reconciliation/identity visibility. `cargo check --locked` now passes in this integration worktree. Warlock `cargo-local-reliquary.ps1 check --all-targets` also passes against it. Earlier timeout entries above are historical; focused P5-A runtime tests remain unverified. This is not final P5 acceptance.

## Purpose

Record the source contracts and verification for this isolated multiplexing lane.

## Overview

This record is branch-scoped. Source/API progress and executed runtime acceptance are separate gates; the parent parallel plan governs convergence.

## Related docs

- [Parallel execution plan](multiplexing-phase5-parallel-execution-plan-2026-10-06.md)
- [External consumer lane](multiplexing-phase5c-external-consumers-2026-10-06.md)

## Notes

Historical test results do not certify later integration revisions. Final release acceptance remains subject to the parent plan's consumer, deletion and release gates.
