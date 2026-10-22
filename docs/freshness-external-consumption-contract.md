# External Memory consumption and Freshness

Parent index: [Documentation index](INDEX.md)

## Purpose

Define the passive retrieval boundary, successful-handoff reinforcement contract, and the remaining host-specific deployment boundary.

## Overview

All internal Reliquary retrieval is non-reinforcing. Content/revision fetches, lexical/semantic search, graph traversal, provenance and Freshness metadata reads share the existing read primitives. Internal Ego, Dream, Insomnia and Perception processing must not generate access receipts. Reads neither advance REL activity nor append access effects; lazy decay at an established REL turn is permitted. Explicit publication, lifecycle and Dream events retain their own established semantics.

`Cva::record_accepted_memory_use` is the sole access-reinforcement write boundary. It reuses the existing Freshness engine and durable event path. It is a trusted in-process embedding-host API, not an authenticated public client endpoint. Its owner, activity-cut and replay validation does not prove consumption. Do not expose arbitrary receipt submission or a public `reinforce=false` switch.

Gateway Memory search and management APIs return candidates or inspection data. Even hydrated content and successful authorization/delivery do not establish accepted use. The Warlock `cf41c66` integration candidate now acknowledges exact content transfer at the requesting-context receiver and restores/replays the original manifest after restart; this is integration-candidate evidence, not a deployed production release. `deliver_rel`, stream acknowledgements, transcript admission, generic ConversationExecution/provider success, and conversation completion remain insufficient proof.

## Accepted handoff contract and consumer-release status

The implemented agent-facing retrieval contract is summarized in [Agentic Memory retrieval](subsystems/agentic-memory-retrieval.md). It uses a host-successful tool-result handoff boundary: after the host successfully receives newly selected Memory content, those IDs enter turn-scoped `SelectedIDs` and qualify for one idempotent accepted-use receipt. Candidate evaluation and failed handoff never reinforce. There is no `SentMemoryIDs` alongside `SelectedIDs`.

Reliquary's non-reinforcing retrieval and accepted-use boundary are implemented. The Warlock `cf41c66` integration candidate demonstrates receiver-owned Echo acknowledgement and exact-manifest replay, but that consumer integration is not a deployed release. The remaining requirements below describe the host obligations that any production consumer must satisfy.

## Required production host integration

The host owns one acceptance coordinator at the point where it can verify which exact Memory content entered an external consumer's usable context. It must:

1. Track owner-qualified source Memory IDs through hydration and context budgeting. Omitted candidates, references/metadata only, internal processing, failed sends and cancelled/unaccepted delivery generate no receipt.
2. At proven acceptance, freeze the exact included IDs, their durable REL owner UUID, the accepted REL activity turn, and one stable host-controlled `use_id`. Group each REL separately. Never infer the REL turn from wall time or accept arbitrary client IDs as evidence.
3. Durably retain the accepted manifest and pending reinforcement handoff using the host's existing acceptance transaction/outbox. Acceptance proof must survive a crash before Reliquary recording; an attempted send cannot become proof. If the host has no durable acknowledgement/reconstruction semantics, leave integration disabled until it does.
4. Under the existing authorized owner mutation coordination, construct `AcceptedMemoryUseReceipt` and call `record_accepted_memory_use`. Do not hold owner locks across provider I/O. Revalidate owner authority according to the host's existing accepted-work policy; never retarget a receipt to a selected owner after acceptance.
5. Mark the handoff complete only after success. An uncertain failure is retried with exactly the same owner, turn, identity and ID set. The existing event path synchronizes before success, de-duplicates Memory IDs and returns zero on identical replay, including after reopen; changed payload reuse is an error. No additional propagation, score cache, event store or retry identity belongs in Reliquary retrieval.

This is a production host requirement, not a claim that every consumer has implemented the required durable acknowledgement/outbox semantics. PHY has no justified activity-clock reinforcement path in this baseline; do not fabricate one. Client-facing acknowledgement endpoints require their own host-controlled delivery ledger and authorization, rather than exposing the library receipt directly.

## Notes

This contract does not add an authenticated client receipt endpoint, a second retrieval engine or a new score/propagation policy.

## Related docs

- [Architecture](architecture.md)
- [Agentic Memory retrieval](subsystems/agentic-memory-retrieval.md#freshness-boundary)

## Verification

The [execution record](freshness-retrieval-boundary-verification-2026-10-15.md) maps concrete production callers and records test results. [Agentic Memory retrieval](subsystems/agentic-memory-retrieval.md#freshness-boundary) owns the current handoff/reinforcement contract. Ego audit-selection changes are a separate workstream.
