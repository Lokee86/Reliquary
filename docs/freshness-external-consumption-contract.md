# External Memory consumption and Freshness

Parent index: [Documentation index](INDEX.md)

## Purpose

Define the existing passive retrieval boundary and the required, not-yet-implemented external host acceptance integration.

## Overview

All internal Reliquary retrieval is non-reinforcing. Content/revision fetches, lexical/semantic search, graph traversal, provenance and Freshness metadata reads share the existing read primitives. Internal Ego, Dream, Insomnia and Perception processing must not generate access receipts. Reads neither advance REL activity nor append access effects; lazy decay at an established REL turn is permitted. Explicit publication, lifecycle and Dream events retain their own established semantics.

`Cva::record_accepted_memory_use` is the sole access-reinforcement write boundary. It reuses the existing Freshness engine and durable event path. It is a trusted in-process embedding-host API, not an authenticated public client endpoint. Its owner, activity-cut and replay validation does not prove consumption. Do not expose arbitrary receipt submission or a public `reinforce=false` switch.

Gateway Memory search and management APIs return candidates or inspection data. Even hydrated content and successful authorization/delivery do not establish accepted use. No production external accepted-context producer is implemented in this repository; therefore automatic end-to-end consumption reinforcement remains pending in the embedding host. `deliver_rel`, stream acknowledgements, transcript admission and conversation completion are not substitutes.

## Subsequent design decision (not yet implemented)

The accepted future agent-facing retrieval architecture is specified in [Agentic Memory retrieval design](agentic-memory-retrieval-design-2026-10-15.md). It replaces the earlier *design requirement* to prove eventual language-model context admission with a host-successful tool-result handoff boundary: after the host successfully receives newly selected Memory content, those IDs enter turn-scoped `SelectedIDs` and qualify for one idempotent accepted-use receipt. Candidate evaluation and failed handoff never reinforce. There is no `SentMemoryIDs` alongside `SelectedIDs`.

The existing implementation and F1/F2/F4 evidence below remain valid; the older unimplemented host acceptance/outbox requirements below are historical F3 assumptions to be reconciled when implementing this newly accepted handoff contract, **not** additional decisions that override the agentic retrieval design. The host confirmation mechanism remains to be identified in code.

## Required host integration (original F3 contract; superseded where inconsistent)

The host owns one acceptance coordinator at the point where it can verify which exact Memory content entered an external consumer's usable context. It must:

1. Track owner-qualified source Memory IDs through hydration and context budgeting. Omitted candidates, references/metadata only, internal processing, failed sends and cancelled/unaccepted delivery generate no receipt.
2. At proven acceptance, freeze the exact included IDs, their durable REL owner UUID, the accepted REL activity turn, and one stable host-controlled `use_id`. Group each REL separately. Never infer the REL turn from wall time or accept arbitrary client IDs as evidence.
3. Durably retain the accepted manifest and pending reinforcement handoff using the host's existing acceptance transaction/outbox. Acceptance proof must survive a crash before Reliquary recording; an attempted send cannot become proof. If the host has no durable acknowledgement/reconstruction semantics, leave integration disabled until it does.
4. Under the existing authorized owner mutation coordination, construct `AcceptedMemoryUseReceipt` and call `record_accepted_memory_use`. Do not hold owner locks across provider I/O. Revalidate owner authority according to the host's existing accepted-work policy; never retarget a receipt to a selected owner after acceptance.
5. Mark the handoff complete only after success. An uncertain failure is retried with exactly the same owner, turn, identity and ID set. The existing event path synchronizes before success, de-duplicates Memory IDs and returns zero on identical replay, including after reopen; changed payload reuse is an error. No additional propagation, score cache, event store or retry identity belongs in Reliquary retrieval.

This is an integration requirement, not a claim that the absent host outbox is implemented. PHY has no justified activity-clock reinforcement path in this baseline; do not fabricate one. Client-facing acknowledgement endpoints require their own host-controlled delivery ledger and authorization, rather than exposing the library receipt directly.

## Notes

This contract does not add an authenticated client receipt endpoint, a second retrieval engine or a new score/propagation policy.

## Related docs

- [Architecture](architecture.md)
- [Freshness implementation plan](freshness-internal-retrieval-boundary-implementation-plan-2026-10-15.md)

## Verification

The [execution record](freshness-retrieval-boundary-verification-2026-10-15.md) maps concrete production callers and records test results. The [implementation plan](freshness-internal-retrieval-boundary-implementation-plan-2026-10-15.md) owns execution status. Ego audit-selection changes are a separate workstream.
