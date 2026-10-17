# Agentic Memory retrieval architecture — decided design

Date: 2026-10-15
Status: **Design accepted; Reliquary implementation complete through Step 36; frozen-corpus verification blocked by CVAFRS03.** This is the behavioral authority for the REL-only agent-facing retrieval workflow; see the [implementation evidence](agentic-memory-retrieval-baseline-2026-10-16.md), [host handoff seam](agentic-memory-host-handoff-seam-2026-10-16.md), and [frozen-fixture verification](agentic-memory-retrieval-verification-2026-10-16.md).
Implementation sequence: [Agentic Memory retrieval implementation plan — 2026-10-16](agentic-memory-retrieval-implementation-plan-2026-10-16.md).
Parent index: [Documentation index](INDEX.md)

## Overview

This document is the accepted behavioral authority for the planned agent-facing Memory retrieval workflow. It defines the three-tool protocol, turn-scoped bookkeeping, Freshness ordering, graph exploration, fallback behavior, and successful-host-handoff reinforcement boundary. The linked implementation plan owns execution order and verification.

## Purpose

Provide high-fidelity, turn-scoped Memory retrieval through a dedicated decision model while keeping all internal search, graph inspection, refinement and Freshness reads non-reinforcing. Only source Memory content successfully handed to the requesting agent qualifies for existing access reinforcement.

## Model and tools

The requesting agent supplies its information need to the retrieval decision model. The controller provides exactly three conceptual operations:

- `memory_query(query)`: start a **new** query. Lexical + semantic candidate retrieval occurs first, then deterministic descending Freshness ordering of the retrieved candidate pool, then a **maximum of 10** new candidates are presented to the decision model. A new query resets `SearchedIDs` and replaces `CurrentQuery` but preserves turn-wide `SelectedIDs`.
- `refine_query(refinement)`: a small semantic call receives the full current query and the refinement and returns **one complete revised query** under a strict output contract (e.g. `kids basketball game` plus `this week` becomes `kids basketball game this week`). The controller re-evaluates previously declined `SearchedIDs` against the revised full query, without rerunning ordinary retrieval. Further graph traversal uses that revised query. The refinement may reinterpret/correct, not merely append.
- `traverse_graph(memoryIDs)`: each anchor performs an **independent one-hop search**, with **up to 10 new candidates per anchor** after exclusions and descending Freshness ordering. Anchors execute concurrently for latency savings. The independent worker results are collapsed and recorded together at the end. Content for a duplicate neighbor is returned once, but its relationship is retained in each applicable anchor report. Do not reinterpret the 10-per-anchor limit as 10 for the entire batch. No depth parameter or separate browse mode is required for the initial design.

Both search and graph traversal collect the underlying eligible candidate set **before** applying the ten-candidate Freshness cut; do not sort an already capped top ten. Freshness ranks qualifying candidates, it does not remove stale/dormant Memories by eligibility. Stable, deterministic ties must be defined by implementation. For each returned Memory include relationship information where applicable, an `unseen_neighbors` count, and a `declined_neighbors` count. Counts are based on the current query state; previously examined Memories remain valid graph anchors and bridges.

## Session state and bookkeeping

The controller carries three logical pieces of state:

- `CurrentQuery`: full active query, retained through refinements and traversal.
- `SearchedIDs`: Memory IDs already **presented/evaluated** by the decision model under this query. Retained across refinements and traversals; cleared only by the next `memory_query()`. Previously declined IDs may be reconsidered by `refine_query()`, but not re-presented as fresh search/traversal candidates.
- `SelectedIDs`: **only** IDs whose Memory content has successfully been handed to the requesting agent in the current requesting turn. Retained through all new queries/refinements/traversals in that turn; suppresses repeat delivery and repeat access reinforcement.

There is **no** separate `SentMemoryIDs` object, persistent evaluated-status object, selected-content cache, or additional numeric selection budget. Decision-model approvals awaiting handoff are ephemeral within the operation and must not enter `SelectedIDs` before success. An operation can return all ten if the decision model accepts all ten; subsequent calls are allowed.

When concurrent anchor workers overlap, gather all results before updating shared `SearchedIDs`; deduplicate content globally while preserving each anchor's relationship/report. Neighbors that were previously selected/declined can still serve as traversal anchors. The graph reports distinguish unseen neighbors from previously declined neighbors; `declined` means `SearchedIDs` minus `SelectedIDs` for the current query.

## Delivery, fallback reporting and Freshness

1. Low-level lexical/semantic retrieval, graph reads, semantic refinement, content hydration, Freshness lookups and decision-model examination **never** invoke access reinforcement.
2. Deliver only newly approved Memory content to the requesting host. After **successful host handoff** (not provider/model response or HTTP-200 speculation), record handed-off IDs in turn-wide `SelectedIDs` and use the existing `AcceptedMemoryUseReceipt` / `record_accepted_memory_use()` path once, with owner, accepted turn and stable idempotency identity. Failed/cancelled/undelivered attempts produce no accepted-use reinforcement.
3. Host integration must identify its actual successful handoff event and preserve idempotence across retries/crashes. This remains F3 implementation work; do not claim the host acknowledgment already exists or introduce an alternative reinforcement engine.
4. For **subsequent new queries**, if at least **one new Memory** is returned, suppress any previously-provided report entirely. If **zero new Memories** are returned but lexical/semantic retrieval matched IDs already in `SelectedIDs`, return `already_provided` referencing those IDs **without their content**, without additional decision-model judgment and without reinforcement. The requesting agent itself decides relevance of content it already received. If neither new results nor previously-provided matches exist, return `no_matches`. Actual retrieval errors remain errors. A raw search match in this fallback is not an assertion of decision-model relevance.

The returned content/handoff boundary, not an internal selection event, is what qualifies for reinforcement. Exactly one persistent turn-scoped selected set is required.

## Fidelity, performance and verification

The authoritative fixture repository is the sibling checkout `C:\!bin\workspace\reliquary-fixtures\local\authoritative\` (14-day, 28-day and Ellis REL/PHY pairs). Preserved `freshness-p5-breadth-2026-10-04.zip` evidence showed undirected active graph connection counts 7,029 / 22,464 / 4,901 over 1,084 / 2,810 / 674 graph nodes respectively; anchors can exceed 10 neighbors. These figures demonstrate the reason for repeatable one-hop exploration, **not** measured answer-fidelity parity. The 28-day and Ellis community-membership snapshots in that benchmark were stale or missing.

Verify against all three frozen RELs: full-candidate-pool Freshness ordering, 10-per-anchor limits, repeated exploration and exclusion, query refinement/reconsideration, shared-anchor overlap with independent reports, concurrent results collapsed once, `already_provided` fallback only with zero new content, failed-handoff absence of reinforcement, successful handoff idempotence and answer/evidence recall relative to broader-context baseline. Do not modify authoritative fixture bytes.

## Scope and non-goals

This design changes **agent-facing retrieval**, not Ego's Candidate Selection → Candidate Audit → Summary Extraction → Summary Creation → Storage → Budgeting → Injection pipeline. Ego's whole-Web-first audit policy remains separate. No new propagation/decay/scoring engine, no fourth browse tool, no model-facing depth option and no budget other than ten presented candidates per ordinary query or per graph anchor.

## Related docs

- [Passive internal retrieval and host consumption](freshness-external-consumption-contract.md)
- [F1–F5 Freshness boundary implementation status](freshness-internal-retrieval-boundary-implementation-plan-2026-10-15.md)
- [F1–F5 verification record](freshness-retrieval-boundary-verification-2026-10-15.md)
- [Ego Memory-Web summary design](ego-web-synthesis-plan.md)

## Notes

Reliquary's agentic controller and retrieval operations are implemented, and the Warlock `cf41c66` integration candidate implements receiver acknowledgment plus exact-manifest restart recovery. It is not a deployed release, and frozen 14-day, 28-day, and Ellis corpus fidelity verification is blocked by unsupported CVAFRS03 records. Preserve the distinction between passive internal reads, which never reinforce, and the confirmed successful-host-handoff boundary. Generic ConversationExecution/provider success remains insufficient consumption evidence.
