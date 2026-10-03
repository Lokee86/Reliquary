# Multiplexing Phase 0B: design contracts and implementation handoff

Parent index: [Documentation index](INDEX.md)

## Purpose

Collect the source-backed designs and executable reference contracts required before the GatewayRuntime/InstanceRuntime migration. This document owns cross-lane integration and the readiness decision; focused documents own their detailed mechanisms.

## Overview

This is future architecture, prepared separately on `design/multiplexing-phase0b` from baseline `a6f580c`. Production multiplexing is not implemented. Reference-model tests establish the consistency of selected rules; they do not demonstrate that the current runtime enforces them. Existing production characterization and eventual tests against the canonical gateway interface are separate gates.

## Design packages

| Gate | Canonical contract | Executable reference fixture |
| --- | --- | --- |
| G1 | [Conversation coordination and REL-wide journal](multiplexing-phase0b-conversation.md) | `tests/multiplexing_phase0b_conversation_contract.rs`, `tests/multiplexing_phase0b_journal_contract.rs` |
| G2 | [Personal publication](multiplexing-phase0b-personal-publication.md) | `tests/multiplexing_phase0b_personal_contract.rs` |
| G3/G4 | [Events and authorization](multiplexing-phase0b-events-authorization.md) | `tests/multiplexing_phase0b_events_auth_contract.rs` |
| G5 | [Lifecycle and owner fencing](multiplexing-phase0b-lifecycle-consumers.md) | `tests/multiplexing_phase0b_lifecycle_contract.rs` |
| G6 | [Source-backed consumer verification and migration gates](multiplexing-phase0b-consumer-verification.md) | `scripts/verify_multiplexing_phase0b_consumers.py`, `tests/test_multiplexing_phase0b_consumer_audit.py` plus pinned Warlock Cargo/TypeScript baseline |
| Host recovery | Stage handoff contract below | `tests/multiplexing_phase0b_host_contract.rs` |

## Cross-contract rules

1. Durable REL/PHY owner identity, verified principal, transient instance, durable conversation/branch, submission, generation, and view attachment are different identities. A principal binding cannot be supplied or changed by an untrusted request.
2. Every request carries an authenticated instance context; the authoritative owner revalidates grants and owner mount epoch at admission, dispatch and publication. A captured context preserves attribution, but never freezes permission through revocation.
3. Each mounted owner has one execution; each active continuation has one writer/coordinator. Views neither duplicate writable InteractionRuntime sessions nor own shutdown or cancellation.
4. The native provisional journal is operational durability outside Archive history. Archive alone commits conversation turns/activity. **One journal owner per REL serializes transitions and whole-REL compaction across all its independent conversation coordinators; entries and tombstones use `(conversation_id, submission_id)` identity.** No per-conversation lock can append to or compact that shared file. Stable submission-to-turn identities reconcile a committed Archive turn when a later journal receipt is absent.
5. Cancellation acknowledgment must describe the transition it won. Cancelled pending work is never dispatched; already-running work has an honest cooperative-interruption outcome. Unknown external completion blocks replay.
6. All accepted native pending work remains durable through gateway restart and owner quiescence. Host-owned upstream work stays under that host's persistence contract until explicit stage handoff. Cancellation is reconciled before either stage resumes.
7. Source authorship, generation initiator and personal-publication destination are independent. Destination PHY follows verified provenance and mapping; neither selected PHY nor parent turn grants another person's identity.
8. Cross-owner publication uses stable receipts and staged idempotent commits without nested REL/PHY locks. No cross-file atomicity is implied.
9. Every authoritative mutation publishes its relevant compact owner update after durability, or marks delivery discontinuity and requires authoritative resynchronization. Stream checkpoints and semantic clocks are separate.
10. Disconnect/reconnect always reauthorizes and reconciles. Mount epoch plus feed cursor establishes continuity; an atomic snapshot/watermark replaces unverifiable continuity. Private PHY bodies never enter REL-wide updates.
11. Quiescence fences admission and old operation tokens before replacement; reopened owners advance epoch. Unrelated REL work remains independent. Initial guarantees cover in-process handed-over resources, not arbitrary external writers. Physical file identity is local to one OS; equal UUIDs in copies on separate devices do not create distributed locking or exactly-once execution. Quiesce local gateways and use offline reconcile/promote before reopening synchronized conflicts.

## Host startup and stage handoff

Start host connection attempts immediately at process startup, concurrently with local recovery. Native work independent of the host may proceed; host-dependent dispatch waits for authenticated reconnect, status and cancellation reconciliation. Neither an unknown remote outcome nor an interrupted connection permits blind redispatch.

Each pending stage has one authority. Prefer a host-owned upstream queue only when its adapter proves durable accepted work, stable IDs, effective pending cancellation serialized with dequeue, and authoritative status lookup. Otherwise use the native durable queue. A durable native handoff intent precedes host acceptance; stable-ID acceptance precedes sender acknowledgment and transfer of stage ownership. On restart, resolve the intent against host status before retrying. Unknown status holds work for explicit reconciliation. A capability downgrade fences new handoffs until outstanding accepted work is reconciled.

A cancellation received before sender acknowledgment may already concern host-owned work. Persist the cancellation request and propagate it to the receiving stage's authority; do not mark it cancelled just because it was pending locally. A confirmed pending cancellation prevents dispatch and adds no Archive turn. Dequeue winning the race produces an honest running cancellation request. Completion and semantic publication deduplicate by stable operation receipt.

The fixture tests local recovery, handoff crash windows, receiving-owner cancellation, cancel/dequeue outcomes, downgrade fencing and exact-once history publication in a reference model. Real adapters must prove the same guarantees; Warlock currently has no durable upstream pending queue.

## Canonical migration sequence

- **0B integration:** review all contracts together, run reference fixtures and existing production characterization, record honest blockers.
- **1A authority foundation:** introduce keyed gateway owner/instance identities, grants, epoch tokens and quiescence at their final owners. No legacy/new mutable authority synchronization.
- **1B coordinator:** establish the single REL journal owner and complete REL-wide compaction/recovery, independent conversation continuations, writer admission, durable cancellation/finalization and view attachments before moving managed selectors. Verify no nested journal/semantic-owner locks or lost events across replacement.
- **1C instance routing:** move active REL and per-REL managed selection into InstanceRuntime and enforce context on every user-facing ambient/explicit route.
- **2 personal publication:** route multi-principal work through source-supported destinations and cross-owner receipts; deduplicate per-PHY processing.
- **3/4 expansion:** complete visible branching and subscriptions at the authoritative mutation boundaries; no global version poll substitute.
- **5 consumers:** migrate verified library/CLI/Warlock callers directly, remove obsolete host authority and update current-state docs only when behavior ships.
- **6 release:** execute real multi-instance adversarial scenarios, full Rust/CLI integration checks, documentation and architecture enforcement, then assess measured contention.

## Verification and readiness

Executed on 2026-10-03 in the isolated worktree at baseline `a6f580c`:

| Check | Outcome |
| --- | --- |
| Six std-only fixtures, `rustc --edition=2024 --deny warnings --test` | 62 passed: conversation 13, REL journal/compaction 6, events/auth 10, host 18, lifecycle 7, personal 8 (G1 journal extension verified independently) |
| `cargo check --locked` | Passed |
| `cargo test --locked --test multiplexing_baseline_contract` | Passed, 1 test |
| `cargo test --locked` | Failed in existing examples: competing global allocator in archive_open_profile; insomnia_stress could not locate reliquary_memory |
| `cargo test --locked --lib --tests` | Library 767/768; Lore historical-read test failed with Windows file-sharing error 32; integration tests not reached |
| `cargo test --locked --lib runtime_host_` | 43/44; provider-backpressure first-wave timing assertion failed (passed in broader library run) |
| Combined Cargo reference/activity test attempt | Initial compile timed out at 120s; retry cancelled during Lore dependency rebuild before tests; no Cargo test pass claimed |
| Warlock `cargo check --locked` at existing pinned revisions | Missing dependency/rmeta artifacts; single-job retry cancelled after 6m16s while compiling dependencies, no source/API diagnostic |
| Warlock `cargo tree --locked --offline -i arcana` | Passed, one Arcana revision shared by Warlock and its currently pinned Reliquary |
| Warlock `npm run build` | Passed, TypeScript/Vite production bundle |
| G6 pinned source-drift script and negative tests | Passed, source inventory/pins match and four Python assertions passed; cannot replace a Rust compiler or target-interface conformance test |
| `cargo fmt --check` and both documentation policy checks | Passed |

The six reference fixtures passed standalone without dependency linkage; Cargo integration remains incomplete. Full production and downstream gates are not green. No production concurrency guarantee is inferred from a passing reference model. A source-backed design gate can be settled while runtime implementation and its release tests remain pending.

**Readiness:** design packages and reference oracles are available for review; production migration is pending, and the broad hard-cut/release gate remains closed by the red or incomplete baselines above. No grant, journal, filesystem durability or consumer conformance guarantee is promoted to shipped behavior.

Before the broad hard cut, each design must have concrete transition and failure outcomes, compatibility boundaries and an executable oracle. Any unresolved mechanism is listed explicitly rather than hidden by marking every gate passed.

## Related docs

- [Implementation plan](multiplexing-implementation-plan.md)
- [Readiness review](multiplexing-readiness-review.md)
- [Phase 0 baseline](multiplexing-phase0-baseline.md)
- [ADR 0038](decisions/0038-gateway-instance-runtime-multiplexing.md)

## Notes

No production source ownership, public API or REL/PHY encoding changes in this design package. Implementation must graduate shipped facts into architecture, API, storage and behavioral-contract documentation.
