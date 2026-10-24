# Roadmap

Parent index: [Documentation index](INDEX.md)

## Phase 6 public boundary acceptance

The public facade and mounted owner-execution cuts have passed the bounded Phase 6 source/API/behavior gates; final evidence is in the [Phase 6 execution receipt](dependency-convergence-phase6-execution-2026-10-23.md). Phase 7 retains the inherited full-graph proof obligation, not unfinished Phase 6 source cuts.

## Purpose

Own future Reliquary implementation work. Completed behavior belongs in current subsystem, architecture, API, storage, CLI, and behavioral-contract documentation.

## Overview

The dependency-convergence architecture refactor has completed Phases 0–3 and is accepted locally. Phase 4 Project Environment is integrated, and Phase 5 runtime convergence is tracked by its [execution receipt](dependency-convergence-phase5-execution-2026-10-23.md); Phases 6–7 retain public-facade and final graph closure. The campaign remains limited to dependency convergence, existing architectural corrections/retirements, and documentation catch-up for touched surfaces; it does not absorb new feature work from this roadmap. See [Dependency Convergence Architecture Refactor](dependency-convergence-architecture-refactor-2026-10-21.md) and its [permanent, explicitly allowlisted target component DAG](dependency-convergence-target-dag-2026-10-21.md). No cycle, upward/ancestral backreach (even if acyclic), unauthorized sibling coupling or unlisted direct dependency is permitted after convergence.

Phase 1's physical storage boundary is implemented and accepted locally on
Windows; its verification record documents platform limits. Its [execution specification](dependency-convergence-phase1-storage-foundation-spec-2026-10-21.md)
and [file lease contract](dependency-convergence-phase1-lock-contract-2026-10-21.md)
record the cursor, explicit repairs and writer/source publication rules. Later
owner and domain convergence remains separate work. Phase 2 owner-local convergence is tracked in the [91-task Phase 2 specification](dependency-convergence-phase2-owner-local-boundaries-spec-2026-10-22.md) and locally accepted [execution record](dependency-convergence-phase2-execution-2026-10-22.md). Phase 3 History/Maintenance/Import convergence is accepted at 119/119 tasks and 12/12 gates, with the CVAFRS07 settlement cut complete; the [execution ledger](dependency-convergence-phase3-execution-2026-10-22.md) and [final acceptance receipt](../benchmarks/dependency-convergence-phase3-final-acceptance-2026-10-23.json) record the evidence. It does not implement new Perception, Ego, Context or Warlock features.

Later work is now concentrated in a small number of tracks:

1. complete the remaining public-facade and final source-graph campaign gates under their separate acceptance plans;
2. complete Perception Observations, then Ego semantic synthesis and Context Engine final assembly;
3. finish explicit Insomnia commit semantics;
4. finish Warlock/Hermes consumer/release work around the implemented Gateway runtime;
5. expand historical/version recovery deliberately; and
6. continue product/security/interoperability hardening when those capabilities are actually needed; and
7. refine Context Engine compaction beneath the existing ownership contract.

Completed Freshness, Dream, decision-routing, tombstone/import/rewrite, external-source-core, and Gateway implementation sequences are not roadmap items.

## Completed storage/history track

The earlier CVAFRS06 settlement measured 395,533 bytes before normalization and 7,415,741 bytes across six canonical fixtures afterward. Those figures are historical measurements of the prior self-contained proof. The selected current-only CVAFRS07 design retains exact semantic scope and accepted-event journal references, stores a chronological causal numeric basis and sparse irreducible score corrections, and omits copied per-event recipient sets and complete derived output postimages. Its expanded UTF-8 budget is 64 MiB.

The six-file CVAFRS07 conversion, semantic/clock/source-byte/mtime oracles, promotion, D3 source enforcement and A3 integration/documentation gates are accepted. The final result is recorded in the [Phase 3 execution ledger](dependency-convergence-phase3-execution-2026-10-22.md), [source-enforcement receipt](../benchmarks/dependency-convergence-phase3-source-enforcement-2026-10-23.json) and [final acceptance receipt](../benchmarks/dependency-convergence-phase3-final-acceptance-2026-10-23.json). The prior 5.235% whole-file result remains historical measurement of the completed earlier hard cut, not a result for this settlement format.

Authoritative planning:

- [Storage/history specification](minimal-lossless-history-storage-spec-2026-10-19.md)
- [Linear execution plan](minimal-lossless-history-storage-linear-plan-2026-10-19.md)
- [Development-format hard-cut interruption](development-format-hard-cut-interruption-spec-2026-10-19.md)

Do not duplicate that plan here. When it completes, graduate its final current contracts into Storage Format/Architecture/subsystem docs and retire the execution documents.

## Semantic sequence

### 1. Cross-scope Entity identity completion

The durable Entity substrate and ordinary mention-resolution foundation exist. Remaining work is relational-evidence-driven cross-scope candidate comparison/reconciliation, exact dirty triggering/receipts/repair, and measured calibration on retained corpora.

This work should stabilize Entity identity before Observation reasoning depends on it broadly.

### 2. Perception Observations

Implement the future work in [Perception future implementation plan](perception-subsystem-plan.md):

- Observation persistence/lifecycle;
- whole-Web bootstrap;
- Exploitation/Exploration routing;
- reasoning-directed evidence expansion;
- reasoning-specific verification;
- relational Observations;
- routing receptors;
- reconsideration/currentness; and
- ambiguity/clarification.

Relationship should converge toward the relational-Observation model rather than remain a duplicated long-term authority.

### 3. Ego synthesis and Context Engine assembly

After the semantic substrate is stable, implement Ego's future semantic output generation and the Context Engine's future final-assembly responsibilities described in these existing plans:

- [Memory-Web synthesis](ego-web-synthesis-plan.md) — Ego creates and stores summaries; Context Engine selects/budgets/injects their content.
- [Cross-chat context](ego-cross-chat-context-plan.md) — Context Engine selects from existing Archive conversation compactions; no new Ego summarizer.
- [Cross-REL summary inclusion](ego-cross-rel-summary-inclusion-plan.md) — Context Engine includes eligible Ego-produced summaries from authorized scopes.

**Accepted target boundary (2026-10-21):** Context Engine owns all final model-facing context selection, budgeting, compaction and assembly. Ego owns semantic state and synthesis (Identity, Personality, Anchors, Memory-Web Summaries), not final injection. Runtime/Gateway executes and authorizes, but does not assemble context content. Maintain the one-way dependency Runtime → Context Engine → context producers, with no Ego → Context Engine imports. These are future capabilities, not additions to the correction-only dependency-convergence campaign.

## Context Engine compaction refinement

Refine current-session compaction under the existing Context Engine/host ownership contract rather than redesigning that boundary. The detailed unfinished work is tracked in [Context Engine compaction refinement plan](context-engine-compaction-refinement-plan-2026-10-21.md).

The required direction is:

- deterministically strip durable/retrievable pre-tail reasoning and completed tool traces before semantic summarization, with an immediate stop when that alone reaches target;
- preserve a completed conversational continuity tail bounded by both turn count (initially approximately 10 turns) and a percentage token cap;
- replace free-form/unvalidated retrieval references with structured output validated against current conversation/branch turn and fragment identities, then assemble the retrieval map deterministically;
- reject and model-reduce oversized summary output rather than truncating it;
- retain exceptional compaction-input capacity limits with hierarchical consolidation, while excluding generation-contract text from the conversation-content allocation;
- make repeated same-turn compaction monotonic so accepted summarized history and the protected tail are not repeatedly reprocessed;
- escalate to current-turn pressure only when older history cannot reach target, and automatically remove reasoning from a pressure-affected turn once it completes and authoritative/Echo storage succeeds; and
- recalibrate the current 40% target and 20% raw-tail defaults from measurements rather than preserving them by compatibility.

This work must first verify that Echo persistence/retrieval is sufficiently immediate and durable to back deterministic execution-trace eviction.

## Explicit Memory commit

Implement [Insomnia explicit Memory commit](insomnia-explicit-memory-commit-plan.md).

The operation must retain Insomnia as semantic author while giving an explicit user/model retention request a hard completion contract: the initiating turn is included in provenance and at least one accepted canonical Memory survives.

Do not expose a caller-authored direct Memory write API.

## Runtime consumer releases

Reliquary's Gateway/Instance multiplexing architecture is current behavior. Remaining release work is tracked in [the multiplexing consumer/release plan](multiplexing-implementation-plan.md).

Near-term:

- finish real Warlock provider/UI/multi-context acceptance;
- converge exact dependency pins and remaining legacy-shaped surface;
- run release-level recovery/privacy/stream/provider gates; and
- implement Hermes over the existing external-source core.

Hermes external transcript integration is detailed in [the external host integration plan](conversation-index-integration-plan.md).

## Agentic Memory retrieval qualification

The agentic controller is implemented. Remaining work is evaluation/product qualification rather than implementation of the basic controller.

The retained evidence-recall benchmark remains weak. Investigate candidate eligibility/discovery/presentation with measured changes, then rerun the frozen comparison and document a production disposition.

Do not weaken the successful-handoff Freshness boundary merely to improve benchmark recall.

## Chronos follow-on

Current Chronos is implemented and shared. Remaining work is in [Chronos future work](chronos-subsystem-plan.md):

- Perception/Observation integration;
- measured grammar/detector calibration;
- multi-evidence temporal validity composition; and
- optional disposable caching only if profiling justifies it.

## Historical Freshness follow-on

Explicit offline/exclusive REL historical reconstruction is complete.

Possible future work, each requiring a separate justification:

- mounted historical management;
- relevance-labelled/default-activation policy;
- wider scale/SLO evidence;
- a separately designed PHY activity/Freshness source; and
- source-identity conversion only with retained proof.

Do not reopen the completed REL historical reconstruction implementation to pursue these independent extensions.

## Versioning and historical recovery

Future whole-CVA historical views, restore-and-continue, retention policy, and rollback remain in [Versioning, historical cuts, and rollback](version-history-plan.md).

This work must compose owner-local history explicitly rather than inventing one fake global semantic ancestry chain.

## Project environment and VCS boundary

Continue reducing Reliquary responsibilities that duplicate authoritative project repository history.

Future work may simplify:

- duplicated project/blob ancestry;
- filesystem-recovery history better owned by Lore/Git;
- project-file revision resolution where an exact repository reference already suffices; and
- synchronization assumptions that conflate semantic-database history with project VCS history.

Preserve REL/PHY semantic history, provenance, Graph/Memory/Entity/Observation history, vector generations, and crash recovery independently of the project VCS.

## REL/PHY synchronization

Reassess whole-file reconciliation around divergent semantic owners only.

Future synchronization must:

- keep REL/PHY transport separate from project-repository transport;
- validate each semantic owner explicitly;
- define cross-device conflicts without assuming Git/Lore ancestry solves them;
- preserve recovery/promotion safety; and
- avoid growing reconciliation into another VCS.

## External/import interoperability

**Deferred External Sources redesign (2026-10-21):** Warlock should duplicate/copy source material into Warlock by default to simplify Reliquary-side operation and integration. External Sources remains a distinct capability (not Project Environment and not subsumed by Import), but its current reference-only, host-canonical transcript model is **not assumed to be the final design** for Warlock. A substantial redesign is anticipated. Before implementing it, specify the exact source/copy authority, provenance and identities, edits/deletes/rewinds, retention/privacy, replay and recovery, and how the duplicate feeds semantic processing; do not conflate a Warlock copy with REL/PHY semantic ownership. Preserve existing behavior until a separate design is accepted. This is **new deferred product/integration work, outside the correction-only dependency-convergence campaign**. See [External canonical-transcript host integration plan](conversation-index-integration-plan.md).

The framework-neutral external-source core is implemented under its **current** reference-only contract. Remaining interoperability work is adapter/product work, subject to the deferred Warlock redesign above:

- real Hermes adapter and conformance;
- production ChatGPT/Claude/provider import adapters where useful;
- deterministic re-import/edit/delete/branch semantics;
- Codex/structured-session adapters;
- optional ACP integration over normalized interaction seams; and
- artifact provenance.

ACP or provider export formats must not become canonical semantic schemas.

## File usability

Product-facing file work may include:

- add/import/export;
- path/folder/tree organization;
- rename/move with stable historical meaning;
- content extraction/indexing;
- retrieval separated from filename metadata; and
- generated-artifact provenance/lifecycle.

Do not reintroduce REL-owned project VCS history to implement these conveniences.

## Runtime and security hardening

Future production hardening includes:

- OAuth refresh where providers require it;
- OS credential-store-backed master key handling;
- standard application/config locations;
- provider retry/backoff/rate-limit behavior;
- observability without content/secret leakage;
- crash-safe background work recovery; and
- authentication/authorization only if a future IPC/server boundary is introduced.

A standalone service/IPC layer requires its own architecture decision.

## Scope evolution

Current REL/PHY ownership is documented in [Scope ownership](subsystems/scope-ownership.md).

Future scope work may include:

- explicit UI/API management of REL dependency topology;
- richer learned-state routing beyond conservative User/Project classification;
- governed organization/team state with explicit authority;
- cross-file source/Memory export lineage; and
- eventual public-facing terminology cleanup around legacy `Cva` naming.

Do not weaken ownership/privacy merely to make scope composition easier.

## Configurable VCS management — deferred

A future Project Environment may separate repository backend from management policy:

```text
backend: lore | git
management: manual | assisted | automatic
```

Managed Git must preserve unrelated working-tree/index state, bind owned operations to exact base revisions, keep push separately authorized, and make mutation retry/recovery idempotent.

This remains deferred product work.

## Release discipline

For every active track:

- current behavior belongs in current docs;
- plans contain only unfinished work plus enough baseline context to execute it;
- completed execution steps are removed rather than accumulating status diaries;
- verification evidence may remain as dated evidence when it is materially useful;
- new persistent formats/owners require explicit architecture/storage documentation; and
- Archivist/Pitlord gates must be green before release.

## Related docs

- [Subsystem documentation](subsystems/INDEX.md)
- [Current limitations](current-limitations.md)
- [Architecture](architecture.md)
- [Storage format](storage-format.md)
- [Behavioral contracts](behavioral-contracts.md)

## Notes

The roadmap intentionally avoids repeating detailed active execution plans. It should remain a future-work map, not a historical project log.
