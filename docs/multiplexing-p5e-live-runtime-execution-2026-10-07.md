# Multiplexing P5-E live runtime execution — 2026-10-07

Parent: [P5-E specification](multiplexing-phase5e-live-runtime-surface-2026-10-06.md)
Wave: [Next parallel execution plan](multiplexing-phase5-next-parallel-execution-plan-2026-10-07.md)
Branch: `feat/multiplexing-p5-live-runtime`
Base: `999fbf7d85c9f892a1bf979293588033bcc1484b`

Current continuation: [E5 acceptance record](multiplexing-p5e-e5-acceptance-2026-10-08.md) supersedes the native receipt, original-token cancellation and E scenario blockers below. This document preserves the earlier draft and its verification history.

## Purpose

Record the E0 inventory, implemented live adapter draft, owner requests and remaining acceptance gates. This is a continuation record, not full E0–E5 acceptance.

## Overview

`interaction_live.rs` adds a recipient adapter backed by one shared `Arc<GatewayRuntime>` and a verified `Arc<InstanceRuntime>`. It owns a viewer token and at most one exact stream recipient per live session. The gateway module adds only the module declaration. Root exports remain with convergence.

The lower `InteractionRuntime` remains the mounted semantic owner's Cva/session machinery. Its standalone constructor, `cva`, `into_cva` and legacy background processing are unsuitable as the final host seam. The new adapter neither constructs that owner nor exposes it.

## E0 inventory and contract

| Host operation | Existing authority | Adapter responsibility |
| --- | --- | --- |
| Open existing durable conversation branch | Gateway selection and conversation viewer registry | Bind explicit owner/conversation, retain original ViewRel token |
| Submit user turn / status | REL journal and ConversationCoordinator | Scope check, retain mount admission, forward durable receipt |
| Cancel undispatched Pending turn | Gateway original-principal cancellation | Retain admission; never locally remove a queue item |
| Interrupt native generation | P3 generation journal / branch coordinator | Confirm native operation under original admission, forward interruption |
| Exact text delivery | P4 authoritative stream hub | Normalize synchronized prefix/delta/status, own one recipient |
| Reconnect | P4 snapshot/register replacement | Full authoritative prefix before future events; no replay |
| Recipient close | Existing viewer / stream registries | Disconnect delivery; accepted work survives |
| Wake / quiesce / reconcile / reopen | B/P3/Gateway lifecycle | Forward current instance-authorized management operation |
| Capability / worker diagnostics | CapabilityRouter and owner worker state | Fresh reads; remove provider strings and content |
| Tool invocation/result and generated artifacts | Missing durable owner receipts | Request owner support; no E-local semantic log |

Normalized values carry stable operation/message/branch identity, generation phase, durable checkpoint sequence, delivery cursor and mount epoch. Prefix/delta shape preserves checkpoint continuity while hashes remain in the lower validation owner. Protocol envelopes and transport callbacks stay outside.

A moved `LiveSession` can migrate between transport handlers without replacing its recipient or semantic writer. A replacement adapter opens a new authorized recipient over the same durable work. A different authenticated instance requires its own session. Original mount/grant generations remain fenced; a caller cannot manufacture or retarget the opaque stream token.

## Implemented draft

Existing-branch open/close, durable submission/status, own Pending cancellation, native Running interruption, exact stream join/poll/overflow/reconnect, privacy-safe capability/worker diagnostics and instance-authorized owner maintenance are implemented. Capability facts are read from the router each time; this draft caches none, so reconnect or downgrade cannot leave cached authority.

The adapter factory is classified as a read in the P4 source coverage guard, which now discovers this E module explicitly. It also rejects raw owner imports/handles, owner-access escape functions and construction of another gateway; masked comments and strings do not trigger the check. Existing production mutation and worker checks remain intact.

## Required owner requests and acceptance blockers

1. **P3 / conversation operation owner:** add cancellation admitted with the original opaque owner token or equivalent epoch/grant-bound capture. Revalidate that original authority around durable cancellation stages while releasing leases before external host calls. The current instance-based API obtains fresh authority and cannot by itself guarantee an E session's old epoch across a remount race. E exposes only native interruption under retained admission; external cancellation fails closed with `UnsupportedDurableEvent`.
2. **A / source attribution and file owners:** provide durable typed invocation/result and generated-artifact receipt operations tied to an accepted generation and exact source identity. Preserve stable invocation/result/message IDs, result ordering, idempotence, artifact owner/file identity and provenance. E must not turn transport events into accepted Archive facts or infer origin from ancestry.
3. **Convergence / D export owner:** review and re-export the E types once the contract is frozen. Candidate names are `LiveRuntimeAdapter`, `LiveSession`, `LiveRuntimeError`, `LiveGenerationStatus`, `LiveGeneration`, `LiveStreamCursor`, `LiveCheckpointShape`, `LiveCheckpoint`, `LiveStreamSnapshot`, `LiveStreamEvent`, `LiveCapability` and `LiveDiagnostics`. Re-export through gateway, runtime_host and lib in one convergence cut.
4. **E remaining acceptance:** durable new-session creation, external Pending/Running cooperative cancellation and Unknown/downgrade scenarios, tool/artifact ordering, complete held-work/resync diagnostics and the final consumer-facing contract remain open. Existing-branch attachment is not durable session creation.

E0 inventory is complete; E1/E2/E3/E4 have the listed implemented subset; E5 is incomplete until all owner seams and scenario gates pass. C must not migrate to this draft as the final E contract. N1 contract freeze and N3 integration remain blocked.

## Verification

Collected on final Rust source at accepted N0 plus this draft:

| Gate | Result |
| --- | --- |
| Root formatting and locked check | PASS |
| Focused live adapter tests | 7 passed |
| Public adapter consumer test | 1 passed |
| Full root tests | 1,106 unit and 129 integration tests passed; no failures |
| CLI formatting, locked check and tests | PASS; 19 tests passed |
| Documentation structural / changed-from origin/main | PASS |
| Final P4 source coverage and adapter ownership fixtures | PASS; 62 fixtures, 149 public APIs |
| P2 / P3 enforced ownership scans | PASS; no gateway/owner violations |
| Pitlord policy validation | PASS; 15 areas, 6 rules |

The protected external Cargo target was used to avoid active-build reclamation. Existing compiler and MSVC linker warnings remain. The final guard-only Python addition does not change the Rust source tested by the full matrix.

A bounded `python scripts/check_architecture.py --refresh` attempt passed the existing P4 source guard, then timed out after four minutes during Lexicon Rust analysis. No fresh Arcana/Pitlord acceptance is claimed. The inherited indexing gap remains open.

## Documentation impact

- Inspected: architecture, invariants, storage format, maintainer map, limitations, roadmap, API, ADR 0016 and both execution plans.
- Updated: this record the index, E specification status, next execution plan, and isolated-draft scope notes in architecture, invariants, API, maintainer map, limitations and roadmap.
- Not affected: REL/PHY/container/journal formats, rollback/history, configuration and CLI command behavior.
- Compliance check: structural and change-impact checks passed; repeated after final evidence update.
- Known documentation gaps: final tool/artifact and external cancellation contracts depend on the owner requests above.

## Architecture impact

- Standards added or changed: none.
- Ownership or boundary impact: E owns recipient adaptation only; Gateway/P3/P4/B retain semantic, journal, stream and worker authority.
- Pitlord enforcement impact: existing runtime-inference owner remains; P4 lexical coverage includes the E factory and rejects adapter owner escapes or a second gateway construction.
- Other verification impact: focused live runtime and public consumer scenarios plus existing root/CLI/source gates.
- Known architectural gaps: fresh Rust indexing, missing durable event owner seams, token-bound external cancellation and convergence exports.

## Related docs

- [Architecture](architecture.md)
- [Architectural invariants](invariants.md)
- [Rust API](api.md)
- [Current limitations](current-limitations.md)
- [Maintainer map](maintainer-map.md)

## Notes

This branch is an isolated implementation draft. Partial verification is not full E0-E5 acceptance or authorization for consumer migration.
