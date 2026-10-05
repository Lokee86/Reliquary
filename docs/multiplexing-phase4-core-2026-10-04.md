# Multiplexing Phase 4 — P4.1 REL sequencer core

Parent index: [Documentation index](INDEX.md)
Parent plan: [Phase 4 distribution plan](multiplexing-phase4-distribution-implementation-plan-2026-10-04.md)

## Purpose

Record the implemented notification authority and its P4.1 acceptance on the independent distribution branch.

## Overview

P4.0 is committed as `efe8087`, directly above shared foundation `388bd8f`.
One concrete `RelSequencer` now belongs to each mounted `RelExecution`.
The sequencer orders notifications, holds typed source watermarks and owns bounded
recipient registrations. It does not own Archive, semantic state, queue/branch
policy, checkpoint persistence, provider work or the conversation coordinator.

The current internal core is not a public subscription API. Atomic authoritative
snapshot/subscription is P4.2; resync acknowledgement is P4.3; complete mutation
publication is P4.4/P4.7. No production feed completeness is claimed at P4.1.

## Source and ownership

| Source | Responsibility |
| --- | --- |
| `src/runtime_inference/runtime_host_gateway_events.rs` | REL envelope/class/typed invalidation IDs, checked cursor allocation, epoch-bound publication/admission gate, source watermarks, close/reset |
| `runtime_host_gateway_subscriptions.rs` | Token-bound opaque subscriber IDs, bounded per-recipient queue, overflow latch, instance removal; owned inside sequencer state |
| `runtime_host_gateway_events_tests.rs` | Deterministic core, concurrent publishers and real gateway lifecycle acceptance |
| `runtime_host_gateway.rs` | Real module declarations, one sequencer per REL, trusted lifecycle and attachment teardown wiring |
| `runtime_host_gateway_ambient_extended.rs` | Matching instance-authorized quiesce/reopen wiring |

Paths without a prefix in this table share `src/runtime_inference/`.
P3 conversation/journal/observation shapes are unchanged. Root declarations and
lifecycle additions are P4-only integration changes to reconcile at convergence.

## Sequencing and bounds

A mutex guard is the sole notification publish/register gate. The initial cursor
is zero; successful publication uses checked increment. Source versions do not
advance the notification cursor implicitly, and unavailable clocks remain None.
No provider/network I/O or backpressure wait occurs inside this core.

Envelopes carry REL owner, gateway mount epoch, cursor, canonical event class,
durable source watermarks, typed identifiers and REL-viewer visibility. No
submission body, prompt, checkpoint content, arbitrary error text, file path or
PHY payload field is present. Branch/generation detail remains the frozen P3
observation contract, to be adapted at later transitions.

Each registration captures the exact gateway-issued ViewRel token, including
attachment ID/incarnation, attachment grant generation, owner grant generation,
mount epoch and fence generation. Subscriber IDs cannot transfer between
attachments. Callers must hold current gateway leases and revalidate authority
before outbound delivery; token equality alone is not a live authorization check.
These internal operations are not exported to clients before P4.2.

Per-subscriber queues are limited to 1–4096 entries, with at most 1024 registrations
per REL. Envelopes are shared with Arc across recipient queues; invalidation lists
are limited to 128 identifiers. Overflow replaces queued ordinary events with one
latched latest-cursor resync state and prevents further ordinary delivery.
P4.3 will add fresh snapshot acknowledgement; the core cannot clear a latch by
reading it. Oversized summaries fail closed rather than imply continuous delivery.

Cursor exhaustion closes distribution and clears registrations without wrapping
or inventing an epoch. Recovery uses normal gateway quiesce/reopen: the existing
gateway allocator advances the mount epoch, resets notification cursor/watermarks
and requires fresh registration/snapshot. P4 creates no independent epoch owner.

## Lifecycle

Quiesce drains owner leases then closes distribution. Both trusted and
instance-authorized reopen paths consume the same gateway-allocated new epoch.
Unmount and shutdown close distribution and clear registrations, including handles
retained by internal observers after removal. Shutdown remains idempotent.

Detach and grant refresh clear this attachment's recipient state after its leases
drain; shared REL work and other attachments are unaffected. Regrant does not
revive a prior subscription. Private PHY lifetime remains separately owned.

## Verification

- Focused `gateway::events::` acceptance: 10 passed.
- `cargo test --locked --offline --lib --tests -j 2 -- --test-threads=1`: 918 library and 93 integration tests passed.
- Library `cargo check`, `cargo fmt --check`, Phase 2 source ownership enforcement and `git diff --check`: passed.
- Structural documentation and change-impact checks against `origin/main`: passed; repeated after this final record update.
- CLI formatting and locked/offline compilation: passed. The initial compile found a stale missing `syn` cache artifact; cleaning that package resolved compilation. CLI tests then timed out during dependency compilation at the 600-second runner limit, before tests ran; no CLI test success is claimed.
- Unrestricted `cargo test`: blocked by the existing `archive_open_profile` example's duplicate global allocator with `lore_base`. The selected library/integration run above is green.
- Required `check_architecture.py --refresh`: attempted, but timed out at 60 seconds during Lexicon initialization (`analyzing rust`, `analyzing python`), before Arcana/Pitlord enforcement. Direct source ownership enforcement passed; refreshed Pitlord verification remains a gap.

P4.1 core behavior is implemented and verified by its targeted and library/integration suites. Public snapshot/subscription, live delivery adapters and complete production event coverage remain later steps.

## Documentation impact

- Inspected: required current ownership docs, shared contract, P4 plan and G3/G4 contract.
- Updated: this lane implementation record, P4 plan/index.
- Not affected: semantic storage bytes, P3 policy/coordinator/journal, CLI signatures and provider policy.
- Compliance check: recorded in acceptance below.
- Known documentation gaps: production snapshots, full mutation coverage and stream behavior remain later phases; canonical convergence docs are reserved for convergence.

## Architecture impact

- Standards added or changed: none.
- Ownership or boundary impact: one gateway REL distribution authority; recipient state stays outside semantic stores and P3.
- Pitlord enforcement impact: required refresh attempted and result recorded below; direct source ownership guard also run.
- Other verification impact: core and existing lifecycle/authorization regression.
- Known architectural gaps: P4.2/P4.3/P4.4/P4.7 adapters and acceptance remain unimplemented.

## Related docs

- [P4.0 acceptance](multiplexing-phase4-baseline-2026-10-04.md)
- [Shared observation contract](multiplexing-p34-observation-contract-2026-10-04.md)
- [Architecture](architecture.md)
- [G3/G4 contract](multiplexing-phase0b-events-authorization.md)

## Notes

The scoped dead-code allowances on the new internal modules cover the declared
P4 migration window until production snapshot/publication adapters consume them.
The modules implement real state and lifecycle behavior rather than a placeholder bus.
