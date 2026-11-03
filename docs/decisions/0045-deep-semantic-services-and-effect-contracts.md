# ADR 0045: Deep semantic evaluators over bounded evidence and typed effects

Parent index: [Architectural decisions](INDEX.md)

**Status: Selected semantic architecture — 2026-11-03; no implementation authorized until complete architecture and migration-plan approval.**

Specification: [Stage 2 semantic service architecture](../semantic-service-architecture-2026-11-03.md). Source mapping: [semantic service boundary crosswalk](../semantic-service-boundary-crosswalk-2026-11-03.md). Storage authority: [ADR 0044](0044-canonical-rel-phy-owner-session-architecture.md).

## Purpose

Define who owns semantic decisions, where inference and data access happen, how semantic services depend on one another, and how substantial future functionality can be added without widening the Root/REL owner aggregate.

## Context

Current Rust already contains substantial independently compiled Insomnia, Dream, Freshness, Retrieval, Entity Resolution, Chronos, Vectors, Switchboard and Context Engine implementations. However, Root's `Cva`/`Phylactery` and runtime modules still mix owner reads, semantic inference, multi-domain publication, worker scheduling and public API. `reliquary-ego` remains a placeholder even though Ego persistence exists, and Observation synthesis remains future work. The service/store pairs are not uniformly shallow or uniformly valid: durable reopening/history requires semantic records to outlive the evaluator that produced them.

Reliquary prioritizes maintainability and extensibility while preserving present behavior and performance. The old physical crate boundaries are evidence, not an architectural goal.

## Decision

1. **Deep evaluator/service ownership:** Each semantic capability owns its algorithm, policy, prompts, input validation and interpretation of endpoint outputs. It accepts bounded, typed, owner-qualified **facts**, not a mutable REL/PHY aggregate. It returns evaluated decisions, staged evidence requests or typed proposal/effect values.
2. **Explicit coordination:** A named application workflow authenticates source and destination, hydrates evidence, invokes evaluation without broad owner locks, rechecks revision/authorization, commits using the [canonical REL/PHY owner-session commands](0044-canonical-rel-phy-owner-session-architecture.md), and reports exact receipts. Generic cross-service registries, event buses and all-service contexts are not introduced.
3. **Distinct authorship:** Insomnia authors source-grounded Memories. Dream decides Memory-Web links, duplicate/canonical components and lifecycle intent. Perception resolves Entities and later authors higher-order Observations; relational Observations share one Observation truth/lifecycle. Ego later synthesizes user-relative/context-oriented projections. Storage owners persist records and validate intrinsic semantics; their types are not second reasoning engines.
4. **Shared primitives:** Chronos supplies temporal detection/resolution; Vectors supplies embedding validation/compatibility; Retrieval supplies bounded candidate policy; Switchboard supplies configured inference/decision/embedding endpoints. Reuse their justified pure interfaces, not a mandatory service-to-service dependency chain. Inference routes do not own domain policy.
5. **Independent relevance and consumption:** Freshness is REL-local relevance, not truth or a Perception gate. Retrieval, internal service hydration, model candidate selection and Ego audits are non-reinforcing. Only an authenticated successful handoff permits a separate idempotent accepted-use event.
6. **Context output ownership:** Context Engine owns deterministic **final model-facing context assembly** and current-session compaction. Ego synthesis is a distinct, not-yet-implemented producer. Ego has no right to rewrite native conversations or become the final context assembler.
7. **Real service/store boundaries:** Persisted domain state must reopen, replay, validate, retain history and support maintenance **without** constructing its inference service. Keep independent stores where this boundary is deep; allow merging shallow packages where durable authority remains protected. No automatic crate per feature.
8. **Growth without reconstruction:** A future semantic computation needs its evaluator, bounded evidence workflow and invocation; it needs a new stored domain only if it creates durable knowledge. Future Observations and Ego must not force changes in Insomnia/Dream's core algorithms or a universal owner API.
9. **No false completion:** Entity Resolution is implemented, but general Perception Observation synthesis is not. Ego Store is implemented, but Ego synthesis is not. History, Maintenance and Import have implemented helpers but substantial Root coordination still exists.

## Alternatives considered

- **One semantic orchestrator object importing every algorithm and mutable store:** rejected; recreates the Root god-object and couples unrelated extensions.
- **Each service owns its entire persistence and invokes the next service directly:** rejected as a universal rule; History/open/recovery would depend on inference and ownership cycles would be hard to avoid.
- **Service-specific evaluators, canonical independent state, and named workflows:** selected; preserves current behavior, clear authority and extensibility.
- **Generic service-plugin framework and universal effect bus:** rejected without a concrete substitution/deployment need; hides transaction and retry boundaries.

## Consequences and verification

- Root `DreamProcessor`, Insomnia processor, Perception integration, Retrieval adapters and Freshness hooks must be dissected by policy/evidence/commit/runtime authority; moving these entire classes into one service crate would be incorrect.
- Existing Cargo dependencies on immutable model types or pure helpers may remain; dependencies that let evaluators open or directly mutate authoritative stores may not. The complete compiled package DAG is Stage 4.
- Owner locks must not span model/provider calls; retries must not fabricate semantic events. Existing work receipts, source identity, privacy, performance and deterministic tests remain authoritative.
- [ADR 0043](0043-rel-terminology-and-wire-identity.md) still requires canonical REL naming and a separately proven wire-format/identifier migration.
- Stage 3 must design workflow sequencing, runtime worker/host scheduling, and cross-owner recovery. Stage 6 must assign every remaining Root symbol and prove integration/benchmarks; no code has been changed by this ADR.

## Related docs

- [Stage 2 semantic architecture](../semantic-service-architecture-2026-11-03.md)
- [Source-to-target crosswalk](../semantic-service-boundary-crosswalk-2026-11-03.md)
- [ADR 0033](0033-perception-entities-observations-and-ambiguity.md)
- [ADR 0034](0034-cross-owner-relationship-graph-and-active-phy-privacy.md)
- [ADR 0035](0035-chronos-shared-temporal-semantics.md)
- [ADR 0039](0039-unified-deterministic-memory-freshness.md)

## Notes

This is a logical semantic ownership decision, not a crate count, a compiled interface or approval to implement planned cognitive functionality. In later stages, specific API and package naming can be refined, but authority and owner-lock restrictions must not be reversed silently.
