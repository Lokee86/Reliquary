# ADR 0047: Final physical crate boundaries and dependency direction

Parent index: [Architectural decisions](INDEX.md)

**Status: Selected physical architecture — 2026-11-03; implementation remains deferred until Steps 5–6 and complete architectural approval.**

Full specification: [Stage 4 dependency architecture](../dependency-architecture-and-crate-layout-2026-11-03.md). Direct dependency contract: [target-crate-dag-2026-11-03.json](../architecture/target-crate-dag-2026-11-03.json). Existing implementation disposition: [physical source crosswalk](../physical-crate-source-disposition-2026-11-03.md). Earlier decisions: [ADR 0044](0044-canonical-rel-phy-owner-session-architecture.md), [ADR 0045](0045-deep-semantic-services-and-effect-contracts.md), [ADR 0046](0046-gateway-runtime-and-named-workflow-ownership.md).

## Purpose

Select the enforceable Rust package structure for the completed storage, semantic and Gateway/workflow designs. Close the question of whether to preserve 30 extracted libraries, collapse everything into one package, or combine actual deep domains behind a controlled small number of seams.

## Context

Thirty distinct supporting crates currently coexist with the `reliquary-memory` Root, which still holds 55.4% of an earlier audited production-line snapshot and the actual mutable `Cva`/`Phylactery`, Gateway, and higher application logic. `runtime-gateway` and `ego` are placeholders; `owner-lifecycle`, `history` and `maintenance` do not own their full promised responsibility. Cargo's existing acyclic inter-crate DAG therefore does not establish architectural ownership. Reliquary's product requires substantial further extension, reliable current-only file recovery, deterministic provenance, low latency and owner-specific privacy.

## Decision

1. **Target package graph:** 22 supporting crates + `reliquary-memory` Root, with exact permitted direct edges in the design-only [JSON DAG](../architecture/target-crate-dag-2026-11-03.json). The graph must be acyclic; all unlisted direct edges are prohibited without an explicit decision update. This is a selected target **not** a current `cargo metadata` result.
2. **Canonical owner composition:** create `reliquary-owner-session` as the only concrete `RelSession`/`PhySession` builder, typed open/reader/command/publication/record-replay boundary over independent domain owners. Only Container handles physical file truth. No higher semantic policy, provider or Gateway dependency below this boundary.
3. **Named application workflows:** create `reliquary-application` for bounded ingestion, Insomnia, Dream, Perception, retrieval, accepted-use, Context Engine, vectors, history, import, external-source and exclusive maintenance workflows. It is stateless between operations except explicit operation-scoped values; no central mutable `ApplicationContext`, generalized workflow registry or arbitrary store callback.
4. **One real Gateway package:** populate `reliquary-runtime-gateway` with the *entire* canonical Gateway/Instance/host/journal/stream/worker/lifecycle authority, not only an extracted grant policy. `InstanceRuntime` remains in the Gateway package so its privileged mutators can stay Rust-private. A public attached Instance is a view, not a grant issuer.
5. **Preserve durable owner libraries:** Archive, Memory Core, Graph, Perception Store, Ego Store, Freshness Store, Vector Store, Insomnia Store and Container remain their own implemented crates. Their records/history/recovery authority cannot be folded into an inference service.
6. **Selective consolidation:** physically merge Echo into an independent Archive Echo module; merge small Dream/Import/Maintenance/external-index record codecs and partial owner-lifecycle into owner-session; merge helper History/Import/Maintenance libraries into their cohesive Application workflow modules. Remove the placeholder-only Ego semantic crate. **These are ten exact retirements/merges**, not blanket store/service unification.
7. **Retain real semantic boundaries:** Chronos, Switchboard, Vectors, Insomnia, Dream, Entity Resolution, Freshness, Retrieval, Context Engine and Project Environment remain separate substantive packages with only the approved value/pure-computation/service dependencies. No new independent Observation/Ego synthesis package without real implementation.
8. **Public Root contains no runtime internals:** `reliquary-memory` is the stable consumer-facing product API and curated reexports. Direct dependencies on lower owner/service crates are allowed *only* for public canonical type reexports or narrow explicit construction. No Root mutable aggregate, service algorithm, journal, worker, history/import processing or hidden orphan module remains.
9. **Pure service versus store access:** a semantic crate may import canonical immutable domain types from a durable owner package, but never live Container authority, a store writer handle or `owner-session`. Public store API must be shaped so a semantic service cannot persist without a typed write capability that only owner-session legitimately holds. The actual access restriction needs API/negative tests in addition to Cargo DAG verification.
10. **No compatibility layering:** this design describes the *final* layout. Retired crates cannot linger as pass-through facades, transitional aliases or duplicate mutable state. Stage 5 handles canonical REL naming/current-format wire migration. Stage 6 schedules move-once bounded hard cuts and final consumer/test gates.

## Alternatives

- **Keep every one of the 30 crates and add more orchestration wrappers:** rejected, because many current helpers do not own the responsibility their name implies and prolong call-site indirection.
- **One monolithic Root or physical storage crate with everything under a broad mutable API:** rejected, because this recreates the internal dependency knot despite an acyclic Cargo DAG.
- **A crate for every service, store, codec, adapter and worker:** rejected, because it generates shallow interfaces, duplicate conversions and premature abstraction; no real owner/deployment reason.
- **Deep durable owners plus typed session, named Application, full Gateway and selective consolidation:** selected. The exact 22+Root result comes from concrete source ownership and the deletion test, not a numeric quota.

## Consequences and verification

- Existing package names, `use` paths, public exports, tests, CLI consumers and Root logic must undergo a coordinated hard cut. Some previously private Rust methods need genuinely narrow cross-crate contract shapes rather than indiscriminate `pub` visibility.
- New core crates (`reliquary-owner-session`, `reliquary-application`) must contain real compiled production behavior; empty placeholder crates do not satisfy an extraction gate.
- Preserve semantic owner identity, grouped physical publication, runtime operational journal formats/queue authority, native/external source separation, exact PHY grants, host reconcile, stream checkpoint visibility and observed performance.
- The design-only graph has been mechanically checked for completeness and cycles against current `Cargo.toml` directories; it has **not** been compiled. Stage 6 must compare actual `cargo metadata` to this graph and inspect method-level restrictions/callers, plus execute full deterministic and performance tests.
- Existing six-file `CVAFRS07` Freshness recovery and the requested CVA→REL persisted-format/identity hard cut remain **separate** acceptance blockers.
- [ADR 0043](0043-rel-terminology-and-wire-identity.md) eliminates old code naming; old binary identities cannot be text-replaced without a verified offline format/source-domain migration.
- Stages 1–4 now give one selected architecture, but **do not authorize source migration** until Stage 5 format design and Stage 6 ordered implementation plan are complete.

## Related docs

- [Stage 4 physical architecture](../dependency-architecture-and-crate-layout-2026-11-03.md)
- [Canonical target package DAG](../architecture/target-crate-dag-2026-11-03.json)
- [Physical source disposition](../physical-crate-source-disposition-2026-11-03.md)
- [Stages 1–3 system foundation](../architecture-redesign-foundation-2026-11-03.md)
- [ADR 0043](0043-rel-terminology-and-wire-identity.md)
- [ADR 0044](0044-canonical-rel-phy-owner-session-architecture.md)
- [ADR 0045](0045-deep-semantic-services-and-effect-contracts.md)
- [ADR 0046](0046-gateway-runtime-and-named-workflow-ownership.md)

## Notes

The counts and Rust source families reflect the inspected design baseline; future package churn requires explicit update rather than silently widening allowed edges. Acyclicity proves no package-level cycle *in the proposed JSON only*, not that the actual Rust implementation has been migrated or passes tests.
