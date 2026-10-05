# Multiplexing Phase 4 — P4.6 conversation stream hub

Parent index: [Documentation index](INDEX.md)
Parent plan: [Phase 4 distribution plan](multiplexing-phase4-distribution-implementation-plan-2026-10-04.md)

## Purpose

Record the generic P4.6 stream machinery after P4.5 commit `9881bc4`. Production P3 adapter wiring remains P4.7.

## Overview

The mounted REL sequencer owns a separate exact stream registry under its existing publication gate. A stream key binds REL UUID, mount epoch, conversation, operation, generation and branch. Its checked delivery cursor is independent of REL notification cursor, Archive version, journal sequence and durable checkpoint sequence.

An internal `ConversationStreamProvider` supplies the frozen authoritative conversation observation and a reconstructable full durable checkpoint prefix. The provider is a short owner read under the mounted gate, not a network/model callback. This branch implements a deterministic test producer, not a production P3 producer. It reads no mutable InteractionRuntime session fields and changes no frozen contract shapes or P3 policy.

## Snapshot, delivery and recovery

Joining authorizes explicit conversation scope with current ViewRel. The same attachment/owner leases and publication gate cover provider snapshot, scope/content validation, final authority validation and registration. The requested generation must exist uniquely in the authoritative conversation/branch snapshot. A checkpoint must match REL, epoch, conversation, operation/generation/branch, source sequence and status, UTF-8 length and SHA-256 identity. Late join requires a full prefix; a generation with no durable checkpoint returns no invented text.

Only post-sync checkpoint/status facts enter `publish_stream`. Source persistence and publication must retain the same gate at P4.7 convergence. Revision and checkpoint predecessor checks reject stale facts, duplicate publication and delta gaps. Invalid publication closes both distribution namespaces; gateway reopen supplies a new epoch. This is fail-closed recovery, not rollback of authoritative P3 work.

A late join receives the durable prefix once and only later events. Prefix events replace content; delta events name their durable predecessor. Status events include terminal/interrupted P3 status without fabricating text. Subscriber disconnect grants no cancellation and affects no owner work.

Queues are bounded by both item count and pending text bytes. Overflow clears the queue and latches one latest-cursor ResyncRequired marker. Recovery replaces one recipient atomically after a fresh authoritative prefix, with no replay or acknowledgement shortcut. Failed reader, validation, bounds or authorization leaves the old recipient intact.

## Bounds and lifecycle

Per REL epoch: at most 256 exact stream identities and 1,024 stream recipients. Recipient capacity is 1–128 events, pending checkpoint text at most 1 MiB, and an individual checkpoint/prefix at most 1 MiB. Identity strings are bounded. No checkpoint content is retained outside pending recipient events; shared Arc envelopes avoid copying fan-out payloads.

Empty stream identity records remain until epoch teardown to preserve cursor identity. Exhausting that bounded table requires a new epoch rather than silently resetting a cursor. Oversized authoritative prefixes cannot join this initial in-process contract; chunked checkpoint transfer remains outside P4.6.

Grant refresh/detach remove stream recipients alongside broad-feed recipients. Quiesce, remount/reopen, unmount, shutdown and continuity invalidation clear stream state. Retained handles revalidate attachment incarnation, grant generation and owner fence before each delivery/recovery. Broad REL envelopes/snapshots still have no text field.

## Verification

- `cargo fmt --check` and `cargo check --locked --offline -j4`: passed.
- `cargo test --tests --locked --offline -j4 -- --test-threads=1`: 970 library tests and 93 integration tests passed, with no ignored tests. All 16 new exact-stream regressions passed, including checkpoint/publish and revoke/snapshot races, byte/count overflow, cursor exhaustion, authority/lifecycle fencing and resource limits.
- P4 coverage guard self-test: seven fixtures passed; 104 public APIs classified and REL worker writes admitted. P2 ownership enforcement: no gateway violations.
- Documentation structural and changed-from-origin/main checks, and `git diff --check`: passed after correcting documentation line endings. Final verification uses bounded result capture.
- `python scripts/check_architecture.py --refresh`: coverage guard passed; Lexicon stalled at Rust/Python analysis and timed out after 120 seconds. Refreshed architecture enforcement remains incomplete.
- Unrestricted `cargo test --locked --offline -j4`: blocked by the existing `archive_open_profile` example global allocator conflict with `lore_base` (exit 101). The library/integration run above passed independently.
- CLI checks were not required: no public library exports or detachable CLI behavior changed. The generic provider is intentionally internal; production P3 synchronization and adapter wiring remain P4.7.

## Documentation impact

- Inspected: architecture, invariants, storage format, maintainer map, limitations, roadmap, behavioral contracts, index, phase plan and frozen observation contract.
- Updated: this record, architecture, invariants, maintainer map, behavioral contracts, limitations, roadmap, index and P4 plan.
- Not affected: semantic storage/record format, public API, CLI configuration, P3 branch/generation policy and frozen shared source types.
- Compliance check: structural and changed-from-origin/main results recorded in Verification.
- Known documentation gaps: P3 production provider and post-sync integration are explicitly P4.7.

## Architecture impact

- Standards added or changed: none.
- Ownership or boundary impact: exact subscriber queues/cursors stay in the mounted sequencer; P3 retains all checkpoint reconstruction, policy and durable source authority.
- Pitlord enforcement impact: coverage/ownership guards retained; architecture refresh outcome recorded in Verification.
- Other verification impact: exact-scope, durability, late join, race, bounds, overflow, authority and lifecycle regressions against a deterministic provider.
- Known architectural gaps: production P3 wiring, convergence public exports and broader P4 audit/acceptance remain later steps.

## Related docs

- [Frozen observation contract](multiplexing-p34-observation-contract-2026-10-04.md)
- [P4 reconnect](multiplexing-phase4-reconnect-2026-10-05.md)
- [Architecture](architecture.md)
- [Behavioral contracts](behavioral-contracts.md)

## Notes

P4 consumes frozen source data without changing its semantics. The test provider demonstrates the required linearization protocol; it does not prove production P3 atomicity. The convergence adapter must preserve gate-before-coordinator/source lock order and publish only after synchronization.
