# Agentic Memory retrieval implementation plan — 2026-10-16

Parent design: [Agentic Memory retrieval design](agentic-memory-retrieval-design-2026-10-15.md)

Linear execution plan: [Agentic Memory retrieval — linear implementation plan](agentic-memory-retrieval-linear-implementation-plan-2026-10-16.md)

Status: **Architecture specification; Reliquary Steps 1–36 implemented; final integration checks passed; frozen-corpus qualification blocked by CVAFRS03**

## Purpose

Translate the accepted agentic Memory retrieval design into a concrete, ordered implementation and verification plan while preserving the existing passive retrieval and Freshness ownership boundaries.

## Overview

The target is one turn-scoped controller with exactly three model-visible tools, a full-pool lexical/semantic candidate seam, non-reinforcing Freshness ordering, concurrent one-hop traversal, and a two-phase host delivery boundary. The implementation is a staged hard cut: existing passive search remains independent, but no compatibility adapter or alternate agentic path is introduced.

## Problem Statement

Reliquary's passive Memory search, owner-local vector retrieval, graph reads, non-reinforcing Freshness reads, and durable `Cva::record_accepted_memory_use()` boundary remain the underlying primitives. The accepted agent-facing controller, full lexical/exact-semantic candidate union, dedicated decision route, strict refinement, concurrent one-hop traversal, fallback semantics, and exact prepared-manifest completion seam are implemented through Step 36. Frozen-corpus fidelity and final disposition remain open.

The existing `GatewayRuntime::search_visible_memories()` is not the target implementation. It uses `MemoryRetrievalConfig` seed/community/traversal bounds and then truncates results. Reusing it underneath `memory_query()` would Freshness-rank an already bounded set and would import depth-based graph traversal into a design that explicitly requires an independent one-hop `traverse_graph()`.

The architecture originally required the following pieces, now implemented in the isolated integration candidates:

- one turn-scoped controller with `CurrentQuery`, `SearchedIDs`, and `SelectedIDs`;
- a lower-domain lexical + semantic Memory candidate collector that exposes the eligible pool before the ten-candidate presentation cut;
- a dedicated Memory-retrieval decision-model route;
- strict query-selection and refinement contracts;
- concurrent one-hop graph exploration with consolidated state mutation;
- `already_provided` fallback semantics; and
- a real successful-host-handoff integration that is allowed to call the existing accepted-use receipt path.

At the time this architecture specification was drafted, F3 remained incomplete until the final host-handoff item was connected. The Step 36 receiver/replay path is now evidenced in Warlock `cf41c66`; deployment and frozen-corpus acceptance remain separate gates.

## Target Architecture

```text
requesting agent
    |
    | exactly three model-visible tools
    v
AgenticMemoryController
    |
    +-- memory_query(query)
    |      lexical + semantic candidate pool
    |      -> exclude SelectedIDs
    |      -> non-reinforcing Freshness ordering
    |      -> top 10
    |      -> decision model
    |
    +-- refine_query(refinement)
    |      semantic rewrite of full CurrentQuery
    |      -> reconsider current declined SearchedIDs
    |      -> no ordinary retrieval rerun
    |
    +-- traverse_graph(memoryIDs)
           one-hop pool per anchor
           -> exclusions
           -> Freshness top 10 per anchor
           -> concurrent decisions
           -> gather
           -> global content collapse + per-anchor reports

prepared result
    |
    | actual successful tool-result handoff by embedding host
    v
handoff completion
    +-- add exact delivered MemoryRef values to SelectedIDs
    +-- group by REL owner
    +-- AcceptedMemoryUseReceipt
    +-- Cva::record_accepted_memory_use()
```

The controller is runtime state, not semantic persistence. Candidate indexes and Freshness reads remain derived/read state. Accepted-use events remain owned by the existing Freshness implementation.

### Canonical module ownership

The implementation should converge on these responsibilities; exact filenames may follow local naming conventions.

- `src/retrieval/memory_candidate_pool.rs`: REL-local lexical/semantic candidate collection only. No decision model, Freshness policy, graph traversal, reinforcement, or ten-result presentation cap.
- `src/runtime_inference/agentic_memory_retrieval_model.rs`: turn state, owner-qualified candidates, operation results, graph reports, prepared-delivery manifests, errors.
- `src/runtime_inference/agentic_memory_retrieval_decision.rs`: strict candidate-selection and query-refinement calls over `DecisionEndpoint`.
- `src/runtime_inference/agentic_memory_retrieval.rs`: `AgenticMemoryController` and the three model-visible operations.
- `src/runtime_inference/runtime_host_gateway_agentic_memory.rs`: gateway authorization, REL owner access, prepared delivery, and post-handoff completion.
- `src/runtime_inference/freshness_access.rs`: unchanged authority for accepted-use reinforcement.

The existing passive `search_visible_memories()` surface remains available to current callers. It is not an adapter or fallback implementation for the new controller.

## User / Developer Outcomes

After completion:

1. The model sees exactly `memory_query()`, `refine_query()`, and `traverse_graph()`.
2. Query presentation is at most ten new candidates after full candidate collection and Freshness ordering.
3. Traversal is exactly one hop and at most ten new candidates **per anchor**.
4. Refinement can reinterpret the active query and reconsider previously declined candidates without ordinary retrieval.
5. A Memory successfully handed off once during the requesting turn is not sent or reinforced again.
6. A new query that finds only previously delivered matching Memories returns `already_provided` identifiers without content, another decision call, or reinforcement.
7. Search, hydration, graph reads, query refinement, model examination, and Freshness reads remain non-reinforcing.
8. Only exact source Memory content confirmed as successfully handed to the requesting agent may reach `record_accepted_memory_use()`.
9. The 14-day, 28-day, and Ellis frozen fixtures validate retrieval behavior without fixture mutation.

## Implementation Decisions

### 1. New agentic retrieval is REL-source retrieval

Freshness and accepted-use reinforcement are currently REL-owned. `Cva::ego_memory_freshness()` and `Cva::record_accepted_memory_use()` have no PHY equivalent, and the external-consumption contract explicitly says PHY has no justified activity-clock reinforcement path.

Therefore the new Freshness-ranked agentic tools search the active authorized REL plus its authorized REL dependency closure. They do **not** include bound-PHY Memories.

This does not change the existing PHY-inclusive passive gateway search. Agentic PHY retrieval requires a separate design decision defining PHY ranking and consumption semantics.

### 2. Controller identity is owner-qualified

The repository already exposes `MemoryRef { owner_id, memory_id }`. All controller bookkeeping uses `MemoryRef`, because raw `MemoryId` is not a sufficient cross-owner key.

Canonical turn state:

```text
CurrentQuery: Option<String>
SearchedIDs: BTreeSet<MemoryRef>
SelectedIDs: BTreeSet<MemoryRef>
```

There is no `SentMemoryIDs`, persistent evaluation object, selected-content cache, or additional persistent state set.

### 3. `memory_query()` transition

A new query:

- validates and replaces `CurrentQuery`;
- clears `SearchedIDs`;
- preserves turn-wide `SelectedIDs`;
- collects lexical and semantic matches across authorized REL owners;
- retains the raw matched set long enough for fallback reporting;
- excludes `SelectedIDs` from new content;
- reads Freshness without mutation;
- sorts the full remaining eligible pool by descending Freshness;
- breaks equal Freshness deterministically by `owner_id`, then `MemoryId`;
- presents at most ten candidates to the decision model;
- validates that model-selected IDs are a subset of those ten;
- adds exactly the evaluated candidates to `SearchedIDs`;
- prepares newly approved content for host delivery.

If no new candidate remains, intersect the raw lexical/semantic match set with `SelectedIDs`. A non-empty intersection returns `already_provided` IDs only. An empty intersection returns `no_matches`. Neither path calls the decision model or reinforces Freshness.

### 4. `refine_query()` transition

Refinement requires an active query.

- Send the complete current query plus the refinement instruction to the dedicated refinement decision.
- Require one complete, non-empty revised query.
- Replace `CurrentQuery` only after valid output.
- Preserve both ID sets.
- Compute declined IDs as `SearchedIDs - SelectedIDs`.
- Hydrate and re-evaluate those exact Memories against the revised query.
- Do **not** run lexical/semantic retrieval again.
- Process every declined ID in deterministic batches of at most ten; there is no second total selection budget.
- Prepare newly approved content for host delivery.

Reconsideration does not remove IDs from `SearchedIDs`.

### 5. `traverse_graph()` transition

Traversal requires an active query and owner-qualified anchors in the current authorized REL scope.

For each anchor independently:

- read exactly one hop of active Memory-to-Memory neighbours from the same REL;
- collect the complete neighbour pool before presentation limits;
- report `unseen_neighbors` as neighbours absent from both state sets;
- report `declined_neighbors` as neighbours in `SearchedIDs` and absent from `SelectedIDs`;
- exclude both state sets from fresh candidate presentation;
- Freshness-sort the remaining pool;
- present at most ten candidates for that anchor.

Anchor workers run concurrently from the same pre-batch state. Gather every worker result before changing `SearchedIDs`. Then add the union of evaluated candidate refs.

Duplicate neighbour content is delivered once globally, while each applicable anchor report retains its own relation kind and anchor association. There is no depth parameter and no recursive traversal inside the tool.

### 6. Candidate collection is a lower-domain primitive

The new candidate collector reuses existing lexical and Memory-vector indexes/scoring but stops before the current community/seed/traversal presentation bounds.

For each REL owner:

- lexical lane: collect every Memory matching the existing Memory lexical semantics;
- semantic lane: exact-score the searchable Memory-vector population for the selected compatibility profile without graph traversal or ten-result truncation;
- union the owner-qualified Memory refs from both lanes;
- retain lexical/semantic scores only as retrieval evidence.

The controller does **not** create a new lexical/semantic fusion score. Retrieval determines eligibility; Freshness determines presentation order.

Where possible, extract/deepen existing exact-scoring helpers so the old and new paths share vector interpretation. Do not duplicate cosine or packed-vector decoding in the controller.

### 7. Freshness is ranking metadata, not an eligibility filter

For eligible REL Memories:

- read Freshness at the current accepted REL activity cut;
- do not advance activity;
- do not append Freshness events;
- do not exclude Stale or Dormant Memories.

A legacy Memory lacking a Freshness record remains eligible and receives neutral ordering score `0` for this read only. Ranking must not enroll or mutate it.

### 8. Dedicated decision-model route

Add `models.memory_retrieval_decision`. It must not reuse `models.entity_resolution_decision` and must not silently fall back to the general model.

Reuse the existing `DecisionEndpoint` transport. Generalize `ConfiguredDecisionEndpoint` construction internally so both decision capabilities share transport code without sharing configuration ownership.

Candidate-selection input contains:

- complete `CurrentQuery`;
- operation kind: query, refinement reconsideration, or graph anchor;
- at most ten candidate records;
- owner-qualified IDs;
- Memory title/content;
- traversal anchor/relation evidence when applicable.

Output contains approved candidate identities only. Unknown, duplicate, or malformed IDs fail the operation and cannot authorize delivery.

Refinement input contains the complete current query and refinement instruction. Output contains exactly one complete revised query string.

Prompts/schemas use versioned constants and strict parser tests.

### 9. Delivery is two-phase

Decision approval is not consumption.

The controller produces a prepared result containing:

- model-visible content/report payload;
- exact newly included `MemoryRef` values;
- the relevant owner activity cut for receipt construction;
- a stable tool invocation/delivery identity supplied by the trusted embedding host.

Preparation does not modify `SelectedIDs` and does not reinforce.

After the embedding host confirms that exact result was successfully handed to the requesting agent, the internal completion seam:

1. verifies the confirmation matches the prepared delivery identity and payload;
2. adds exact delivered refs to `SelectedIDs`;
3. groups them by REL owner;
4. constructs one stable `AcceptedMemoryUseReceipt` per owner;
5. calls existing `record_accepted_memory_use()` under normal owner mutation coordination.

This completion API is **not** a fourth model-visible tool.

Failed, cancelled, or unknown handoff does not invoke completion. Exact retry reuses the same manifest and use identity; the existing receipt path remains the durable idempotency authority after recording.

### 10. Real host success remains external authority

Reliquary must not treat a provider response, generic HTTP 200, stream event, transcript admission, or the current `ConversationExecution` handoff as proof that Memory tool content reached the requesting agent.

This specification's source trace correctly found no embedding-host callback in Reliquary itself; the receiver belongs to Warlock. Warlock candidate `cf41c66` now provides the real receiver acknowledgment and exact-manifest restart replay. Treat the following list as the acceptance contract that this candidate must satisfy, not as an assertion that the host is absent:

- identifies the actual successful Memory tool-result handoff;
- supplies stable delivery identity;
- can replay the exact successful manifest across its retry/crash boundary; and
- calls the AR8 completion seam only after success.

Do not create a second Reliquary delivery journal merely to manufacture proof. If the host cannot provide replayable handoff evidence, automatic reinforcement remains disabled.

### 11. Authorization is revalidated around model I/O

Use the existing gateway pattern:

- snapshot the active REL and authorized REL dependency closure;
- obtain exact owner tokens before embedding/decision I/O;
- do not hold owner semantic locks across external model calls;
- reacquire/revalidate the attachment and owner authority before exposing prepared content;
- reject stale/revoked results rather than retargeting them.

### 12. Hard-cut rule

The target has one controller, one candidate collector, one decision route, one turn-state model, and one post-handoff accepted-use path.

Do not add:

- an adapter that makes `search_visible_memories()` impersonate the agentic tools;
- a legacy agentic query mode;
- depth compatibility for `traverse_graph()`;
- a fourth model-visible tool;
- `SentMemoryIDs`;
- an alternate Freshness write path;
- duplicate ranking implementations;
- a persistent evaluation-state store.

## Migration Strategy

This is a **staged hard cut**. Existing passive search remains valid, but no compatibility layer is introduced between it and the new agentic protocol. Intermediate commits may expose incomplete target modules; the feature is not production-complete until all gates pass.

### Execution sequence

| Step | Assigned | Work | Completion gate |
| --- | --- | --- | --- |
| **AR1** | **Sol** | Add owner-qualified controller/result/error types, the three state fields, prepared-delivery manifest, and exact state transitions. | Unit tests prove new-query reset/preserve behavior, owner qualification, and prepared-vs-completed state. |
| **AR2** | **Luna** | Add `models.memory_retrieval_decision` through config codec, auth validation, switchboard, configured runtime, runtime routes, and CLI config; generalize decision-endpoint construction. | Config round-trip, auth, route-isolation, and CLI tests pass; no fallback to Entity/general routes. |
| **AR3** | **Sol** | Implement the REL Memory candidate-pool primitive by extracting/reusing lexical and exact vector scoring before current seed/community/traversal presentation limits. | A candidate outside raw rank 10 can still enter the Freshness top 10; existing `retrieve_memories()` behavior is unchanged. |
| **AR4** | **Sol** | Implement Freshness ordering and `memory_query()`, including raw-match tracking, exclusions, strict selection output, `SearchedIDs`, `already_provided`, and `no_matches`. | Full-pool-before-cut, Stale/Dormant eligibility, neutral missing grade, deterministic ties, ten max, passive fallback all pass. |
| **AR5** | **Sol** | Implement `refine_query()` with atomic query rewrite and full declined-set reconsideration in <=10-item batches without ordinary retrieval. | Correction/reinterpretation and >10 declined-ID tests pass; failed refinement leaves state unchanged. |
| **AR6** | **Sol** | Implement concurrent one-hop `traverse_graph()`, state counts, per-anchor top ten, gather-before-update, overlap collapse, and relation reports. | >10-neighbour, shared-neighbour, selected/declined-anchor, conflicting-overlap, and strict one-hop tests pass. |
| **AR7** | **Luna** | Add versioned selection/refinement contracts and simulator-driven malformed-output tests. | Invalid/unknown/duplicate model output cannot mutate state or authorize delivery. |
| **AR8** | **Sol** | Implement the two-phase gateway delivery completion seam and owner-grouped calls to existing `record_accepted_memory_use()`. | Failed/cancelled/unknown delivery gives zero reinforcement; success, multi-owner grouping, exact retry, and changed-manifest conflict pass. |
| **AR9** | **Sol** | Wire the real embedding host to AR8 at the actual successful Memory tool-result handoff, including replayable exact manifest identity. | Implemented in Warlock integration candidate `cf41c66`; focused receiver/restart tests pass. Not a deployed release; frozen-corpus/final disposition gates remain. |
| **AR10** | **Sol** | Verify 14-day, 28-day, and Ellis frozen REL fixtures, using copies/read-only inputs and a broader-context evaluation baseline. | Search order, repeat traversal, refinement, overlap, fallback, and handoff idempotence pass; fixture hashes are unchanged. |
| **AR11** | **Luna** | Update architecture, roadmap, current limitations, external-consumption contract, Freshness plan/record, docs index, and maintainer map to actual implementation status. | Docs checks pass and no document claims AR9 before real evidence exists. |
| **AR12** | **Sol** | Remove migration-only helpers, refresh Lexicon/Arcana, audit exports/call graph, and run the full repository integration gate. | Rust, CLI, docs, architecture checks pass; one canonical agentic path remains. |

### Dependencies and safe parallelism

- AR2 may run beside AR1/AR3 once names are frozen.
- AR4 requires AR1 + AR3.
- AR5 requires AR4.
- AR6 requires AR1 + AR3 and the controller state semantics from AR4.
- AR7 may start after AR1/AR2 stabilize the payload and route shapes.
- AR8 requires the final prepared-result shape from AR4–AR6.
- AR9 requires AR8 and cannot be replaced by simulator evidence.
- AR10 requires AR4–AR8; its real reinforcement portion requires AR9.
- AR11 follows actual implementation state.
- AR12 is the final convergence gate.

## Testing Decisions

Focused tests must cover:

- state reset/preservation;
- owner-qualified identity collisions;
- full candidate pool before Freshness cut;
- deterministic Freshness tie order;
- missing grade as neutral/read-only;
- exactly ten query candidates;
- refinement of more than ten declined IDs;
- malformed/unknown/duplicate decision output;
- one-hop graph traversal;
- anchors with more than ten neighbours;
- overlapping concurrent anchors and gather-before-update;
- `unseen_neighbors` and `declined_neighbors`;
- globally collapsed content with per-anchor relations retained;
- `already_provided` only after zero new query results;
- no model call/reinforcement on fallback;
- failed/cancelled/unknown handoff;
- successful handoff;
- identical retry and conflicting retry.

Use `SimulatedDecisionEndpoint` or another implementation of the existing production interface. Do not add a public test-only bypass.

Gateway integration tests must also prove authorization revalidation after external I/O, REL dependency closure behavior, explicit exclusion of bound PHY from the new agentic path, no internal Freshness mutation, and separate accepted-use receipts for multiple REL owners.

### Frozen fixture verification

Authoritative fixtures:

`C:\\!bin\\workspace\\reliquary-fixtures\\local\\authoritative\\`

For each 14-day, 28-day, and Ellis corpus verify:

1. a query where Freshness changes final presentation order relative to raw retrieval order;
2. an anchor with >10 active neighbours;
3. repeated traversal exposing later neighbours after exclusions;
4. refinement changing relevance without fresh retrieval;
5. shared-neighbour overlap across concurrent anchors;
6. new-query `already_provided`;
7. failed handoff with zero accepted-use effect;
8. successful handoff + identical retry with one accepted-use effect;
9. answer/evidence recall against a broader-context baseline.

The broader-context baseline is evaluation evidence, not a runtime mode.

### Repository verification

```text
cargo fmt --check
cargo check
cargo test

cargo fmt --manifest-path cli/Cargo.toml -- --check
cargo check --manifest-path cli/Cargo.toml --locked
cargo test --manifest-path cli/Cargo.toml --locked

python ../engineering-standards/tools/docs_policy/check.py --repo .
python ../engineering-standards/tools/docs_policy/check.py --repo . --changed-from origin/main
python scripts/check_architecture.py --refresh
```

After source convergence, refresh Lexicon/Arcana and verify the controller, candidate-pool seam, decision route, and sole accepted-use caller structurally. While this plan was authored, the current Arcana snapshot disagreed with direct source for `runtime_host_memory_search.rs`; direct source is authoritative until refresh.

## Out of Scope

This work does not:

- change Freshness values, decay, propagation, states, or event policy;
- add semantic-access reinforcement;
- implement PHY Freshness or PHY accepted-use reinforcement;
- include PHY Memories in the new agentic Freshness-ranked path;
- change Ego candidate selection, audit, summary generation, budgeting, or injection;
- add recursive/depth graph traversal;
- add a fourth model-visible tool;
- add a second numeric selection budget beyond ten-candidate presentation;
- add ANN or cross-profile score fusion;
- persist controller state as semantic authority;
- add a second Reliquary delivery/reinforcement journal;
- reinterpret provider/HTTP/conversation handoff as Memory-delivery proof;
- mutate frozen fixture inputs.

## Completion Gates

The feature is complete only when:

- [ ] exactly three model-visible Memory tools exist;
- [ ] controller ID state is owner-qualified `MemoryRef`;
- [ ] query eligibility is collected before the Freshness top-ten cut;
- [ ] the agentic path does not delegate to bounded/depth `search_visible_memories()` or `retrieve_memories()`;
- [ ] Freshness ranking remains non-reinforcing and Stale/Dormant are eligible;
- [ ] refinement reconsiders existing declined IDs without ordinary retrieval;
- [ ] traversal is one hop, ten candidates per anchor, concurrent, and consolidated;
- [ ] `SelectedIDs` alone suppress repeated delivery/reinforcement;
- [ ] `already_provided` is identifiers-only, query-only, and passive;
- [ ] prepared delivery is non-reinforcing;
- [ ] the real embedding host has a verified successful handoff producer;
- [ ] failed/cancelled/unknown delivery creates no accepted-use receipt;
- [ ] successful delivery records exact owner-grouped IDs through existing `record_accepted_memory_use()`;
- [ ] exact retry/restart creates no duplicate reinforcement;
- [ ] all three frozen fixture gates pass without mutation;
- [ ] Rust, CLI, docs, and architecture checks pass;
- [ ] superseded F3 assumptions are no longer presented as current implementation requirements.

AR9 receiver/replay behavior is evidenced by Warlock candidate `cf41c66` and eight passing host Memory tests. The truthful current status is: **Reliquary Steps 1–36 and host receiver/replay implementation are complete in their respective worktrees; frozen-corpus qualification is blocked by CVAFRS03 and final production disposition remains pending, and the host candidate is not deployed**.

## Notes

### Existing passive search

`search_visible_memories()` remains a valid bounded generic API. Its PHY participation, community-routed retrieval, and depth semantics are different from the accepted agentic protocol. Keeping the surfaces independent avoids hidden compatibility behavior.

### Ego

This plan does not implement Ego’s candidate-selection → audit → summary extraction → summary creation → storage → budgeting → injection pipeline. Both systems may use passive Memory/Freshness reads, but they own separate state and inference contracts.

### F3

The earlier F3 work remains correct that internal retrieval cannot create access reinforcement and that host-owned evidence is required. The accepted design narrows the qualifying event to successful Memory tool-result handoff and makes `SelectedIDs` the sole turn-scoped delivered-ID set.

AR8 creates the canonical Reliquary completion seam. AR9's real-host receiver acknowledgment and exact-manifest recovery are implemented in candidate `cf41c66`; the F3 implementation gate is evidenced, while deployment and frozen-corpus verification remain separate.

## Related docs

- [Agentic Memory retrieval design](agentic-memory-retrieval-design-2026-10-15.md)
- [External Memory consumption and Freshness](freshness-external-consumption-contract.md)
- [Freshness retrieval boundary implementation plan](freshness-internal-retrieval-boundary-implementation-plan-2026-10-15.md)
- [Freshness retrieval boundary execution record](freshness-retrieval-boundary-verification-2026-10-15.md)
- [Architecture](architecture.md)
- [Roadmap](roadmap.md)
