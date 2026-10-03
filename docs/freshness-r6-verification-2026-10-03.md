# Freshness R6 verification — 2026-10-03

Parent: [Documentation index](INDEX.md).

## Purpose

Record bounded due delivery, worker lock ownership and consumer adapter contracts.

## Overview

The dispatcher remains derived, coalescing and at-least-once across reopen. Runtime-host Dream now prepares frozen inputs under the owner mutex, releases it for propagation/worker joins and reacquires it for proof/idempotency-validated commit.

## Collected evidence

Final root invocation `cargo test --locked -j 2` in job `job_KEDYwixNQjbm3N-JxjBYGT8u` passed 814 unit tests and all 54 integration tests, zero failed/ignored. Test-profile build used Rust 1.97.1, `CARGO_PROFILE_TEST_DEBUG=0`, two build jobs and isolated target `C:\Users\archa\AppData\Local\Temp\reliquary-r3-verification`. Earlier focused owner/recovery jobs also passed; the full final suite is acceptance evidence.

## Verified contracts

`freshness_due` and owner tests exercise exact 1,000/1,510-turn boundaries, custom pinned policy, 1,024-Memory backlog drained at most 64 transitions per step, both crossed boundaries, coalescing, reopen rebuild, saturation/reinforcement reversals and a later second crossing. Lazy score reads remain correct independently of notice backlog and do not append.

`frozen_dream_settlement_can_evaluate_without_owner_and_preserves_accepted_graph` moves frozen work to a detached thread, adds a later live graph edge, verifies the later edge receives no historical credit, commits once and checks retry/reopen. The accepted pass/proof remains authoritative after lock reacquisition; current graph mutations do not retime or rerun that historical event. The direct synchronous Dream processor wrapper remains valid for callers holding exclusive Cva access.

Owner tests cover accepted-use multi-source atomicity, durable retry after cache turnover, passive search/hydration, duplicate appearances and no credit from score/vector reads. Ego adapter tests retain a full small Web, apply graded preference within remaining budget, preserve hard keeps even over budget, and keep Dormant Memories searchable/protectable. Perception is not a Freshness prerequisite.

## Integration boundary

This repository exposes `AcceptedMemoryUseReceipt`, `record_accepted_memory_use`, `ego_memory_freshness` and `select_ego_routine_context`. It does not expose a universal observable accepted-context delivery producer or a running Ego summarizer selection consumer. Those integrations remain external/pending. Adapter tests do not prove live host acceptance, preview/cancellation UI behavior or end-to-end Ego synthesis. No producer was fabricated from passive retrieval.

## Measurement boundary

Frozen propagation topology/latency is measured by the calibration harness. Graph pinning time is now emitted for real disposable copies. Worker joins are outside the runtime mutation mutex by source review and detached-work test. Full inference-provider latency and real concurrent-host mutex wait percentiles are not measured by this deterministic test harness.

## Related docs

- [Completion execution plan](freshness-completion-execution-plan-2026-10-03.md)
- [Core verification](freshness-core-release-verification-2026-10-03.md)
- [Development](development.md)

## Notes

F8/S1–S6 are excluded by the user's execution scope. Measurements are diagnostic, not release-build latency guarantees. Exact frozen source identity and final repository gates are recorded in the core report.
