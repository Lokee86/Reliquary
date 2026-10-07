# Multiplexing Phase 5 — next parallel execution wave

Parent: [P5/P6 parallel execution and convergence](multiplexing-phase5-parallel-execution-plan-2026-10-06.md)
Current implementation line: Reliquary `feat/multiplexing-p5-convergence` — tested N0 base `f9dafba`
Current external consumer line: Warlock `feat/multiplexing-p5c-final-cut` @ `3a62470` (N0 regression repairs)
Accepted harness line: `feat/multiplexing-p6-harness` @ `81a64b6`
Status: **N0 accepted on the convergence branch; A/B are integrated, P6-H remains accepted separately, C is late-stage, E/F/G/D remain.**

## Purpose

Finish Phase 5 without serializing all remaining work behind Warlock or legacy-host deletion. Establish one common Reliquary convergence base containing the accepted A/B implementation and current C-side canonical API repairs, then run C closure, E, F, G core, D0 deletion inventory, and the remaining P6 diagnostics concurrently. Delete the legacy host only after the live consumer/runtime-management dependencies have converged.

## Overview

The planning-only main branch is not the implementation frontier. Current verified branch state is:

| Area | Current evidence | Disposition |
| --- | --- | --- |
| P5-A | Source commit `7350149`; integrated runtime acceptance on `4ce4911`: 1,099 library, 128 integration, 19 CLI; upgraded Warlock checks also pass | **done for implementation/acceptance; integrated into N0** |
| P5-B | `c45f50e`; 1,093 library, 128 integration, 19 CLI plus P2/P3/P4 guards | **done for implementation/acceptance; integrated into N0** |
| P5-C | Reliquary `4ce4911`; Warlock final-cut `1782e72`; old `ReliquaryRuntimeHost` consumer removed on the Warlock branch | **late-stage; remaining real-product gates below** |
| P5-E | specified only | **start now** |
| P5-F | specified only | **start now** |
| P5-G | specified only; external-derived R0–R7 design already exists | **start framework-neutral/core work now; host-specific adapters separately gated** |
| P5-D | deletion plan only | **start D0 inventory now; D1–D4 blocked** |
| P6-H | harness accepted on `81a64b6`; full H5 instrumentation incomplete | **continue diagnostic tail now** |
| P6-R | no final post-deletion run | **blocked on D** |

The architecture refresh remains a bounded tooling gap on several accepted branches; direct ownership/coverage guards and actual tests are still required and have passed where recorded. Do not promote a stale Arcana snapshot into acceptance evidence.

## N0 — establish one convergence base before more production branching

**Accepted on `feat/multiplexing-p5-convergence`.** Tested base: `f9dafba2a885c140b1aa879dc0494c9d2d44b1c9`; upgraded consumer: Warlock `3a62470`. The combined floor passes: 1,099 library, 128 integration, 19 CLI, 202 Warlock tests (one existing benchmark ignored), formatting/locked checks, P2/P3/P4 guards and documentation checks. Three consumer regressions were repaired in C before acceptance. Main planning tip `2407dbe` includes `dfa82ae` and this next-wave plan. See [N0 convergence acceptance](multiplexing-n0-convergence-acceptance-2026-10-07.md) for exact commands, provenance, timeout/retry history and remaining gaps. The acceptance commit adds documentation only and is the branch point for the next lanes.

Create `feat/multiplexing-p5-convergence` from the current Reliquary C integration head `4ce4911`, because that line already contains accepted A/B source plus the current C-side canonical API repairs. Bring the main planning/status documentation from `dfa82ae` forward without discarding implementation-branch acceptance records. Do not merge Warlock source into Reliquary.

Before branching E/F/G/D0 from it, rerun the existing combined acceptance floor:

- root formatting and locked check;
- library and integration matrix;
- CLI formatting/locked check/test;
- P2/P3/P4 ownership/coverage guards;
- documentation structural/change-impact checks;
- local upgraded Warlock compile/test wrapper against this exact Reliquary revision.

A regression here belongs to the owning A/B/C source before new tracks begin. Record the convergence-base commit and exact results in a new branch-scoped acceptance record.

## Immediate parallel tracks after N0

### Track C-close — real Warlock convergence

**Repo:** clean Warlock P5-C worktree, continuing from `3a62470` (including N0 consumer regression repairs).
**Reliquary dependency:** exact N0 convergence revision; no old-host fallback.

Remaining work:

1. **Multi-context acceptance:** two authenticated instances, independent REL/conversation/branch selections; same principal shares exactly one PHY worker; different principals remain private; detach/viewer churn does not cancel accepted work.
2. **Public P4 delivery/reconnect:** Tauri-visible durable text must come from the authorized exact stream/checkpoint path, including late join, slow consumer, reconnect, epoch change and resync. No provider delta may masquerade as durable transcript before synchronized checkpoint.
3. **Actual provider conformance:** bind the real Warlock provider lifecycle to the accepted ConversationExecution host contract. Prove stable ID/digest, retry, Unknown Held, reconnect/reconcile, Pending cancel versus dequeue, and truthful Running cooperative cancellation. Warlock still does not advertise a durable pending queue.
4. **Dependency hard cut:** repin Reliquary and direct Arcana to the accepted convergence revisions together, regenerate lockfile, and require one intended Arcana package identity.
5. **Final C matrix:** locked Warlock Rust check/tests, provider/admission/conversation fixtures, frontend production build, no legacy host imports, and source inventory updated to final interfaces.

**No Reliquary source edits in this track.** Missing runtime/management operations are requests to E/F/A/B.

### Track E — transport-neutral live runtime

Branch `feat/multiplexing-p5-live-runtime` from N0. Implement E0–E5 in [P5-E](multiplexing-phase5e-live-runtime-surface-2026-10-06.md).

Priority order:

1. inventory actual C call patterns and existing `InteractionRuntime`/Gateway generation/stream behavior;
2. freeze minimal normalized session/event/status types;
3. reconnect/resume token/cursor behavior using existing P3/P4 identities rather than new state;
4. cancellation/shutdown and bounded runtime controls;
5. capability cache invalidation and diagnostics;
6. acceptance through the public E surface.

**Constraint:** E extracts a reusable host-neutral seam from already-proven Gateway behavior. It must not recreate a host-global session authority or force C to maintain parallel old/new workflows.

### Track F-core — workspace management

Branch `feat/multiplexing-p5-management` from N0. Implement F0–F4 immediately.

Parallel-safe scope:

- conversation/session listing, selection and inspection composition;
- file/source/project inventory and reads;
- Memory/Graph/Entity/Community inspection and permitted existing lifecycle operations;
- health/verification/statistics using A primitives and E status types when available.

Keep F5 selective import/export behind the G1 normalized import contract. F may initially compile against an internal E DTO module if both branches freeze the type shape; final merge must have one owner and no duplicated DTO definitions.

**Constraint:** no generalized semantic CRUD and no mutable Cva/Phylactery escape.

### Track G-import — normalized importers

Branch from N0 or a G umbrella branch. Execute G0–G2 independently of the external-derived index:

1. re-audit available provider export formats and maintained fixtures;
2. freeze provider-neutral source/message/branch/artifact identity and deterministic re-import keys;
3. implement supported ChatGPT/Claude/structured-agent importers through one normalized native Archive/Insomnia path;
4. golden edit/replacement/deletion/branch/artifact/idempotency fixtures.

This subtrack feeds F5 but does not depend on E.

### Track G-external — framework-neutral external-derived core

In a separate non-overlapping G worktree, implement the existing external conversation index plan R0–R4:

- external source namespace/message/chunk refs and durable cursor;
- reference-only derived index and rebuild;
- authorized transient hydration and source-reference search;
- generalized external provenance and Insomnia processing;
- edit/rewind/delete lifecycle invalidation.

Do **not** implement Hermes or ACP transport in core. G3/G5 adapters occupy their own external seams after the framework-neutral contracts pass.

### Track D0 — deletion inventory only

D0 starts immediately against N0 and is read-only/source-audit plus documentation/scripts. Produce the definitive old-host deletion manifest:

- each `ReliquaryRuntimeHost` field/method/module;
- every current Reliquary caller;
- upgraded Warlock/CLI caller status;
- reusable types to relocate;
- persisted readers/format compatibility that must survive;
- exact final canonical owner;
- test replacement or deletion disposition.

D0 may add a negative no-legacy-caller guard, but **must not delete or rewire root modules yet**. D1 starts only after C/E/F convergence proves those consumers no longer need old authority.

Full G adapter completion is not a D blocker. D requires only that G's framework-neutral contracts that touch the public runtime are integrated or explicitly independent of the legacy host.

### Track P6-H5 — diagnostic tail

Keep P6 production-neutral. Complete the missing measurement visibility identified by the accepted harness:

- exact REL worker/thread ownership counts where an owning-lane diagnostic can expose them safely;
- wake/notification counts or equivalent bounded work signals;
- sequencer recipient queue occupancy and retained-byte diagnostics where the implementation actually tracks them;
- final reproducible contention/fan-out/memory baselines on N0 and later post-D.

Any production diagnostic field belongs to B/E/P4 owner code; P6 only consumes it. Do not add test-only mutable control seams.

## Merge and dependency gates

### N1 — E/F/G core contracts frozen

E's public runtime types, F's management DTOs, G1 normalized import types and G-external source identity must each have one owner. No root re-export churn in every branch; integrate exports once on convergence. C may continue app-local tests against N0 during this gate.

### N2 — independent lane acceptance

- **C-close:** multi-context + real provider + public P4 reconnect/stream + final pins.
- **E:** E0–E5 tests and no alternate session/queue authority.
- **F-core:** F0–F4 authorization/privacy/management acceptance.
- **G-import:** deterministic re-import/importer fixture acceptance.
- **G-external:** R0–R4 rebuild/privacy/lifecycle acceptance.
- **D0:** complete reverse-caller manifest and negative guard.
- **P6-H5:** diagnostics/measurements runnable without production authority changes.

A failure is repaired in its owner branch; do not solve integration failures with compatibility forwarding.

### N3 — Reliquary convergence and final C pin

Merge accepted E, F-core and G framework-neutral/core work onto the N0 convergence line. Resolve root exports once. Run the full Reliquary/CLI/guard matrix. Then pin Warlock C to **that exact revision**, align Arcana, and rerun the full C acceptance matrix.

F5 selective import/export may join here after G1/G-import is accepted.

Host-specific G3 ACP and G5 Hermes adapters may continue in parallel. If their source repositories/SDKs are unavailable, record the concrete blocker. They do not justify preserving `ReliquaryRuntimeHost`.

### N4 — D hard cut

Once C is accepted against N3 and E/F no longer require legacy runtime state:

1. relocate any surviving common result/error/report types out of legacy modules;
2. remove `ReliquaryRuntimeHost`, ambient REL/PHY/session selection, duplicate worker/router/mount state and old-only forwarding;
3. migrate/delete characterization tests;
4. enforce no-legacy-callers across Reliquary, Warlock and CLI;
5. update architecture/API/maintainer/contracts/limitations/ADR current state.

Run the complete functional matrix before calling D accepted.

### N5 — P6-R final release

Port/run the accepted P6-H fixtures against the post-D tree and upgraded Warlock. Execute the complete release commands, actual provider/Tauri/operator integration, performance comparisons and bounded architecture refresh. A timeout or unavailable external adapter remains explicit; no silent waiver.

## Branch cleanup after convergence

Do **not** prune current P5-A/B/C/P6 worktrees before their accepted commits are reachable from the final convergence branch and all unique acceptance records are preserved. After N3/N4:

- verify ancestry for A/B/C helper branches;
- preserve any unique acceptance docs/log summaries in the convergence history;
- remove redundant P5-C API/project/conversation helper worktrees;
- retain external Warlock dirty main separately until its unrelated graph/UI work is resolved;
- prune P6-H only after its fixtures/evidence are represented in P6-R or retained as an accepted historical branch.

## Completion definition

The multiplexing hard cut is complete when C/E/F and the relevant G core surfaces use one Gateway/Instance authority, D has removed the old host, upgraded Warlock has one accepted dependency set, and P6-R passes. G's named host-specific adapters remain visible work until implemented or concretely blocked by unavailable external sources; they are never treated as implicitly complete.

## Documentation impact

This plan supersedes the stale execution-status portions of the 2026-10-06 parent plan but does not replace the individual lane specifications. Current-state docs are updated only when their implementation reaches the convergence branch.

## Architecture impact

The next wave preserves existing deep owners: P3 owns durable conversation state, P4 distribution, A semantic/project owner-qualified operations, B REL/PHY background execution. E/F/G are adapters/composition surfaces over those owners, not additional authorities. D removes the shallow duplicate legacy runtime once callers are gone.

## Related docs

- [P5/P6 parent plan](multiplexing-phase5-parallel-execution-plan-2026-10-06.md)
- [P5-C](multiplexing-phase5c-external-consumers-2026-10-06.md)
- [P5-E](multiplexing-phase5e-live-runtime-surface-2026-10-06.md)
- [P5-F](multiplexing-phase5f-workspace-management-2026-10-06.md)
- [P5-G](multiplexing-phase5g-interoperability-2026-10-06.md)
- [P5-D](multiplexing-phase5d-legacy-deletion-2026-10-06.md)
- [P6](multiplexing-phase6-parallel-verification-plan-2026-10-06.md)

## Notes

This plan distinguishes the hard-cut critical path from adapter tails. Explicitly blocked work remains visible, but unavailable external adapter source does not force obsolete in-process authority to remain alive.
