# Freshness internal retrieval boundary — implementation specification

Status: F1/F2/F4 complete; full Rust verification passed. F3 remains blocked on external host acceptance integration. F5 closeout records four unrelated Ego documentation findings and a timed-out architecture graph refresh. See the [execution record](freshness-retrieval-boundary-verification-2026-10-15.md).
Date: 2026-10-15
Repository: Reliquary
Scope: retrieval/access-reinforcement boundary only; Ego synthesis selection is separate.

## Overview

Execute F1–F5 in order while preserving the engine. A missing external acceptance coordinator leaves F3 pending and permits independent regression verification to continue.

## Purpose

Internal Reliquary reads must never reinforce Memory Freshness simply because the system is inspecting its own data. Retrieval and accepted consumption remain distinct. Access-based reinforcement occurs only when an authoritative external consumption boundary records proven Memory use.

This is an ownership extraction, not a redesign of Freshness, retrieval, or graph propagation.

## Inspected baseline (main)

- `src/retrieval/memory_search.rs`: `Cva::search_memories` and `Phylactery::search_memories` fetch hits and Memory content; they do not explicitly call accepted-use reinforcement.
- `src/runtime_inference/freshness_access.rs`: `Cva::ego_memory_freshness` supplies current/historical scores without an accepted-use write.
- The same file: `Cva::record_accepted_memory_use(AcceptedMemoryUseReceipt, workers)` validates ownership and activity turn, de-duplicates receipts, calculates propagation, appends and syncs the event, ingests it, and refreshes due state.
- `AcceptedMemoryUseReceipt` includes `owner_uuid`, `accepted_turn`, `use_id`, and `memories`.
- Source search found test call sites for `record_accepted_memory_use`, but no identified production invocation. This is not evidence that every external integration path was exhausted; trace all adapters before wiring.
- `Cva::select_ego_routine_context` still takes `hard_keep` and ranks by Freshness; that policy is not the agreed Ego audit-preparation pipeline.

## Required invariants

**R1. Internal read invariance.** Internal retrieval of Memory content, graph edges, provenance, lifecycle state, and current/historical Freshness never creates an accepted-use receipt or applies an access-principal reinforcement event.

**R2. Non-reinforcing primitive.** Normal low-level retrieval stays non-reinforcing. Do not duplicate search algorithms or create parallel copies of storage readers.

**R3. Explicit external consumption.** Only an authoritative boundary that knows actual Memory content was admitted into an external user/agent context may submit an accepted-use receipt. Search results, candidate listings, metadata inspection, and failed/cancelled delivery are not evidence of acceptance.

**R4. Controller-owned receipts.** The internal controller/host constructs receipts from verified accepted Memory IDs, not untrusted client assertions that arbitrary IDs were used. Preserve the existing ownership, activity-turn, and idempotence validation. No externally exposed `reinforce=false` selector or arbitrary receipt-submission endpoint.

**R5. No incidental counter changes.** Internal reads do not advance the REL activity clock, append Freshness effects, trigger access propagation, or schedule work *as an effect of access reinforcement*. Independently authorized lifecycle/mutation/Dream events retain their established effects.

**R6. Lazy decay is permitted.** Computing a current score at a specified REL turn is a non-reinforcing read. Do not confuse derived decay with an access boost.

**R7. Deterministic retries.** Replaying the same accepted consumption with the same identity and payload does not reinforce twice; conflicting reuse of an identity follows the existing validation rules. Multiple memories in one accepted context are de-duplicated according to the existing event semantics.

**R8. Internal-only contract.** Non-reinforcing primitives are accessible to trusted Reliquary code but must not become a public user-selectable bypass. Retain truly public search functionality as needed, but separate its invocation from proof of content consumption.

## Ownership and interface design

```text
                Internal Reliquary components
        (Ego / Dream / Insomnia / search / graph / storage)
                             |
                             v
                  Existing read primitives
                    [non-reinforcing]
                             |
               +-------------+-------------+
               |                           |
       internal processing          external delivery host
                                           |
                                  verified accepted content
                                           |
                                  AcceptedMemoryUseReceipt
                                           |
                              record_accepted_memory_use()
                                           |
                                existing event + propagation
```

Do not put access reinforcement inside `Cva::memory`, `search_memories`, graph traversal, or `ego_memory_freshness`. The preferred implementation is a single trusted delivery/consumption coordinator at the existing external integration boundary, calling the existing record method once confirmed. Do not invent a new public `MemoryReader` trait or duplicate Cva/Phylactery retrieval unless a concrete ownership constraint requires it.

### Consumption definition

An accepted-use event means identifiable Memory *content* was successfully incorporated into the external consumer's usable context, with a stable `use_id`. Merely querying, prefetching, returning references, reading IDs, or inspecting metadata does not qualify. If the integration cannot prove acceptance, it must not speculate or reinforce. Where consumption is acknowledged asynchronously, define durable, retryable receipt handoff based on existing delivery semantics before wiring; do not treat an attempted send as accepted use.

### Freshness read behavior

For metadata-only selection use `ego_memory_freshness(ids, as_of_turn)` or existing equivalent low-level reads. Score and lifecycle checks must be free of access reinforcement. Maintain the existing accepted-turn bound and source-owner checks. Avoid a new maintained/cached mutable score as the authority.

## Execution steps

### F1 — Map and classify the actual call graph (Sol)
- Enumerate `Cva::memory`, `Phylactery::memory`, search/retrieval facade calls, Ego/Dream/Insomnia readers, integration adapters, and any explicit freshness-use APIs.
- Classify each caller as internal processing, external candidate delivery, verified accepted-context delivery, or explicit mutation.
- Identify the single authoritative acceptance event, if one exists, including delivery failure/retry semantics.
- Record concrete paths, caller/callee evidence, and a before-change behavior matrix.
- **Gate:** do not infer acceptance from search or response generation; if no acceptance boundary exists, explicitly report the missing host integration and do not add artificial reinforcement.

### F2 — Make the non-reinforcing internal contract explicit (Sol)
- Reuse existing low-level reads; rename/restrict or extract the smallest appropriate internal ownership seam only where required by F1.
- Guarantee no internal read invokes `record_accepted_memory_use` or equivalents, including indirect helper paths.
- Do not expose a public bypass flag, construct receipts during read, or modify Memory search semantics.
- Add documentation on the authoritative API boundary and its access control.
- **Gate:** internal reads preserve existing return values/errors and create no access events.

### F3 — Bind verified external consumption to the existing receipt method (Sol)
- At the authoritative host-owned accepted-context event, gather exactly the included source Memory IDs and a stable delivery/use identity.
- Submit one `AcceptedMemoryUseReceipt` using the existing Cva method, with correct REL owner and accepted REL turn.
- Persist/replay safely using existing idempotence and integration transaction semantics; do not mark failed/cancelled/unaccepted deliveries as consumed.
- If the authoritative acceptance event is owned outside this repository, specify its integration contract and leave wiring pending there rather than incorrectly inferring acceptance inside Reliquary.
- **Gate:** accepted content reinforces once; mere retrieval, metadata delivery, or failed delivery reinforces zero times.

### F4 — Regression tests (Luna)
- Internal Memory content fetch, lexical search, provenance lookup, graph traversal, Freshness metadata/score computation, and representative Ego/Dream/Insomnia paths produce no accepted-use event or access-originated score boost.
- Capture observable event ledger, REL turn, and affected-node Freshness across each operation, including reopen where appropriate.
- Externally accepted use: exact included IDs, one event on first receipt, zero on identical replay, validation failure on invalid owner/future turn/conflicting identity as defined by current code, correct propagation and persistence.
- Verify lazy historical/current score reads without reinforcement, across several turns.
- Failure/retry paths do not create phantom reinforcement.
- **Gate:** tests pass without relying on private mocks that bypass the real Cva event path.

### F5 — Verification and closeout (Sol)
- Run formatting, targeted retrieval/Freshness unit and integration tests, then relevant broader regression suites.
- Compare behavior matrix from F1 with actual results and inspect persisted Freshness event behavior after reopen.
- Report source changes, test counts, any external-host integration remaining, and the exact commit.
- Do not mark F3 complete if the acceptance boundary is absent or cannot reliably prove consumption.

## Separate Ego workstream — not part of F1–F5

Replace or retire the outdated `select_ego_routine_context` `hard_keep`/score-ranking policy only when implementing Ego audit preparation. Its required order is:

1. Whole Web: if the complete Web fits, audit it unchanged.
2. If not, exclude Archived; stop if it fits.
3. Exclude Historical; stop if it fits.
4. Collapse Duplicates; stop if it fits.
5. Exclude Superseded; stop if it fits.
6. Exclude Dormant; stop if it fits.
7. Exclude Stale; stop if it fits.
8. Batch all remaining Memories; every Fresh Memory gets mandatory audit coverage, even if multiple batches are required.

This sequence produces audit inputs, not `SummaryCandidates` directly. Do not mix Ego's candidate/summary machinery into this Freshness-boundary patch.

## Notes

Existing low-level reads already satisfy the passive implementation contract. Do not manufacture a runtime change or host acceptance event merely to close a step.

## Related docs

- [Current host consumption contract](freshness-external-consumption-contract.md)
- [Execution and verification](freshness-retrieval-boundary-verification-2026-10-15.md)

## Non-goals

- Changing numeric boost/decay/propagation policies or Freshness storage format.
- Introducing a new retrieval engine or duplicate reinforcement implementations.
- Making every external search hit a consumed Memory.
- Reworking summary generation, provenance averaging, or PHY storage.
- Adding a user-facing no-reinforcement toggle.
- Altering explicit write/Dream relationship reinforcement rules.

## Acceptance criteria

1. Every classified internal read remains non-reinforcing, including metadata-only requests.
2. Exactly one ownership location authorizes external accepted-use reinforcement, or an explicitly documented external integration blocker remains.
3. Existing accepted-use idempotence, owner/turn validation, graph propagation, persistence, and replay are preserved.
4. Regression tests cover the two boundaries and failure/retry conditions.
5. Ego selection is explicitly deferred to its own implementation while its finalized audit order is recorded.
6. No unrelated working-tree changes are overwritten.
