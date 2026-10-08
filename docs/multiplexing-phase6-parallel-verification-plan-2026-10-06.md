# Multiplexing Phase 6 — concurrent harness and final verification

Parent: [P5/6 parallel execution plan](multiplexing-phase5-parallel-execution-plan-2026-10-06.md)  
Baseline: [P3/P4 main acceptance](multiplexing-p34-main-integration-2026-10-06.md)  
Status: **P6-H infrastructure is accepted historically on `81a64b6`. The N0 H5 diagnostic tail is runnable and verified on `feat/multiplexing-p6-h5`; see [P6-H5 N0 diagnostic acceptance](multiplexing-p6-h5-acceptance-2026-10-08.md). Post-D measurements and P6-R still wait for D and upgraded Warlock.**

## Purpose

Build reproducible adversarial, stress and measurement infrastructure while P5 consumer lanes are executing. Then use it to verify the final single-authority runtime after deletion, rather than treating P6 as a wholly serial last phase. Do not turn tests into a second runtime implementation or rewrite production interfaces merely to make a fixture pass.

## Overview

Accepted P3/P4 integration reports 1,085 library + 128 integration + 19 CLI passes with serial all-target compilation, while a refreshed architecture graph did not complete. Existing Phase 0B G1/G3/G4/G5/G6 contract models and production gateway, journal, worker, personal publication and stream tests provide prior-art seams. Reference oracles do not prove runtime conformance; run scenarios through real GatewayRuntime, in-memory controlled GatewayHostAdapter and actual REL/PHY files.

P6-H is independent of P5-A/B/C/E/F/G and writes its own harness/tests only. P6-R is the final release verdict after P5-D and upgraded Warlock; no premature release claim.

## Track ownership and interfaces

The accepted harness already exists on `feat/multiplexing-p6-harness` @ `81a64b6`. Preserve that historical harness branch. For H5 diagnostic completion and later P6-R, port or rerun the test-only harness against the N0 convergence/post-D revisions defined by the [2026-10-07 next-wave plan](multiplexing-phase5-next-parallel-execution-plan-2026-10-07.md). New production-facing tests call public/frozen gateway/instance APIs, real journal and provider adapter seams, with deterministic barriers/fault injection. Do not edit root runtime ownership or Warlock application code merely to facilitate a test; request a narrow owning-lane diagnostic when necessary.

## P6-H implementation sequence (parallel with P5)

### H0 — frozen baseline and runner
Capture commit/build profile, platform, feature flags, isolated Cargo targets, REL/PHY fixture seeds, existing known failures, repeatable seed/random ties, test-thread policy and command manifest. Record a small deterministic topology generator and stable operation IDs. Reuse existing fixtures instead of duplicating every unit test.

### H1 — multi-instance topology matrix

Exercise combinations:
- 2 instances, same principal/PHY and one REL;
- 2 principals, distinct PHYs and one shared REL;
- one principal across multiple RELs/one PHY;
- different principals across different RELs with separately granted dependencies;
- viewer attach/detach without generation ownership;
- one REL quiesced, another REL and PHY continue;
- wrong owner, guessed PHY and dependency permission denied.

Assert single real worker/owner/journal identity, independent selected conversation, exact personal delivery, correct Archive ancestry and no leakage. Use assertions on source receipts and persisted state rather than only observed UI updates.

### H2 — queue/branch/generation failure matrix

Test verified chronological pending order, deterministic equal-timestamp ties, sequential vs visible branch policy, multiple branch generations, cancel-before-finalize, cancel-vs-finalize one winner, retries with stable payload fingerprints, post-commit receipt loss, Archive sync crash, checkpoint sync crash, reconnect with Interrupted/Unknown generation, and restart while no viewers remain.

Inject failure at durable transition boundaries. For each crash, reopen from actual files, reconcile once, and check no duplicate Archive turn, Episode/activity increment, branch or owner publication.

### H3 — subscriptions/reconnect/privacy

Real P4 REL feed: atomic snapshot vs mutation, inactive conversation change, two authorized instances, remount epoch invalidation, old cursor/grant rejection, bounded queue and byte overflow ResyncRequired, slow subscriber, reconnect/resnapshot, revocation during queued delivery and exact stream late join. Broad events contain no generation text or private PHY content; stream checkpoints were durable before visible delta.

### H4 — real host boundary and cross-stage cancellation

Deterministic fake GatewayHostAdapter must model: connect/unavailable/downgrade; stable handoff receipt; failed/corrupt digest; crash after handoff but before local receipt; Pending cancel vs host dequeue; running cooperative interruption; cancellation before downstream handoff ack; Unknown status Held; reconnect with divergent native/host committed state; cancel reconciliation **before** replay; unrelated native stage remains operational. Never assert exactly-once external provider effects unless host contract actually proves it.

Use Warlock's actual migrated adapter in release tests later; fake adapter alone is not product conformance.

### H5 — reproducible profiling and load

Measure (not estimate):
- REL coordinator/owner lock contention and hold durations;
- subscriber fan-out queue/byte footprint at increasing instance counts;
- per-REL/PHY worker count and wake frequency;
- accepted-queue latency and restart/reconciliation time;
- generation checkpoint throughput/replay size;
- memory peak/steady state and startup under bounded topologies.

Report workload, hardware, before/after revisions, warm/cold cache, repetitions and variation. Compare to accepted P3/P4 base on identical scenarios. Define thresholds only from measured acceptance criteria or externally required limits, not invented targets. Optimize only demonstrable bottlenecks in their owning source lane and rerun correctness.

## P6-R final release (serialized after P5-D)

1. Integrate accepted A+B+E+F and applicable G core work with D hard deletion; build upgraded C Warlock pinned to the exact accepted revision and one Arcana. External G adapters that require unavailable repositories remain explicit blockers/deferred adapters, never silent omissions.
2. Run H1–H5 matrices on the **post-deletion** code, not only synthetic oracle.
3. Reliquary: cargo fmt --check, cargo check --locked, cargo test --tests --locked, all-target/example compilation and unrestricted cargo test --locked; report actual differences/known baseline exceptions independently. Tests should include library/integration and doc-specific contracts.
4. Detachable CLI: locked fmt/check/tests if public API affected; otherwise still run confirmed baseline.
5. Warlock: cargo tree --locked for single intended Arcana, cargo check/test --locked, npm run build and actual Tauri/provider multi-instance integration where available. Pin exact source revs; no source-only success claims.
6. Run both docs-policy modes, P2/P3/P4 ownership/coverage guards plus P5 no-legacy callers guard, and current-state docs/ADR conformance.
7. Run a **bounded** architecture refresh if Arcana/Lexicon is available/current; previous indexing timed out. Do not poll indefinitely or mark Pitlord green from a stale snapshot. Independently perform direct structural/caller tests even when indexing is blocked.
8. Inspect final Git branch/worktree cleanliness, regression inventory, remaining external integration gaps and reproducible logs.

## Acceptance and blockers

P6-H acceptance: runnable, deterministic real-runtime topology/fault/stream/host tests; snapshot of baseline and metrics; no semantic behavior changed just for harness. P6-R acceptance: all intended real integration, privacy, cancellation/recovery, deletion, Warlock and operator tests pass; no new unresolved regressions; measured contention/fan-out costs within explicitly justified bounds; any unreached architecture or external-platform gate is called out as incomplete rather than silently waived.

Previously recorded Cargo example dependency-artifact/allocator and indexing interruptions are **historical**. Re-evaluate on the final integrated tree; never assume they persist or are fixed without running the corresponding gate.

## Out of scope

Implementing new providers, distributed multi-writer storage, global persistent event log, broader semantic redesign, new Warlock UI and performance tuning without measured regression.

## Documentation and architecture impact

P6-H records test fixture and diagnostic docs. P6-R owns a single final verification report with each command, pass/fail/timeout, environment, limitation, version and source of the claim; canonical current-state architecture docs are updated during P5-D after actual deletion.

## Related docs

- [Parallel execution](multiplexing-phase5-parallel-execution-plan-2026-10-06.md)
- [Phase 0B conversation](multiplexing-phase0b-conversation.md)
- [Phase 0B events/auth](multiplexing-phase0b-events-authorization.md)
- [P3/P4 main integration](multiplexing-p34-main-integration-2026-10-06.md)

## Notes

Do not claim release acceptance from reference or fake-host tests. P6-R requires real post-deletion integration and upgraded Warlock evidence.

