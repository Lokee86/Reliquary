# ADR 0049: Rebuild the architecture independently; verify before cutover

Parent index: [Architectural decisions](INDEX.md)

**Status: Selected execution strategy — 2026-11-03. Implementation has not begun.** This supersedes any previous description of Step 6 as an in-place incremental migration or a deliberately red migration window. Full [rebuild plan](../architecture-rebuild-implementation-plan-2026-11-03.md); [capability and acceptance plan](../architecture/implementation-rebuild-capabilities-2026-11-03.json). Stages 1–5 and [ADRs 0043–0048](0048-rel-current-format-and-stable-identity-domains.md) remain architecture and data-format authorities.

## Purpose

Clarify that Reliquary is undergoing a **clean architectural rebuild using the existing mature Rust implementation**, not progressive extraction of a working product through temporary interfaces.

## Context

The prior crate-first refactor extracted many lower libraries without removing Root's ownership knot. Its partial Gateway/owner-lifecycle crates and deeply exposed `Cva` fields demonstrate why a code-move sequence that preserves a globally compiling old Root on every step incentivizes temporary forwarding layers. Previous Stage 6 planning initially described a “red migration window,” which still assumes incremental surgery on the old system and misstates the user's objective.

The old implementation is a valuable **verified reference**, not a topology to keep operational while each extracted owner changes underneath it. The final dependency graph, canonical per-owner storage, semantic services, named workflows, Gateway and REL naming/format are already selected.

## Decision

1. **Freeze an untouched, usable source reference** with golden tests, known failures, performance measurements and immutable corpus hashes. Perform old-format FRS07 replay investigation only in a disposable validation worktree/copy.
2. **Initialize a separate clean rebuild worktree** with the selected architecture. Build required product capabilities in their actual owner, semantic, application and runtime modules. Examine existing code selectively: reuse what is suitable, adapt it where helpful, reimplement what conflicts with the design, and omit unnecessary legacy internals. No old file or function has a mandatory destination.
3. **No intermediate live architecture.** The rebuilt product may be incomplete during development, but the stable reference remains the usable program. There is no deliberate period in which a shipping product or the stable implementation is made red. Do not preserve the old Root as a linked compatibility library or ship old/new systems side by side.
4. **Build the final 2.1 current format from the outset.** New `RelSession`/`PhySession` and domain owners have no runtime 2.0 readers or old `Cva` aliases. Semantic/host APIs use the approved final target contracts from the start.
5. **Use meaningful target integration gates**, not artificial every-step green builds: separately validate the completed owner-session graph, semantic/Application graph, Gateway graph, full rebuilt product with synthetic 2.1 inputs, converted owner corpus and consumers. Any unfinished new source is ordinary development work, not an intentional broken migration phase.
6. **Only genuine migration is persisted data:** after the new runtime passes synthetic-format/product tests and the reference old-format FRS07 cohort is verified, run one isolated typed **2.0 → 2.1 offline converter**, compare full identities/history/proofs and activate a complete verified generation. Retire converter/old codec machinery; reference data is unchanged.
7. **Cut over once and retain rollback reference.** The rebuilt crate graph, public API, CLI, host adapters, converted corpus, privacy and measured behavioral/performance parity must pass before activation. No premature traffic/producers may write the new current owner files while converter results are unverified.
8. **Capability-driven construction, not source transfer.** Work increments describe required new-system behavior and acceptance tests. Before implementing each one, choose only the reference algorithms or types that demonstrably help; old internal methods and files need no individual transfer, classification or replacement. Small coding slices are chosen from the new interface/tests when that capability is built. Do not introduce generic scaffolding, duplicate mutable stores, placeholders or generated full-repository structural ratchets.

## Alternatives considered

- **Progressively mutate the working Root architecture until all 30 crates are extracted:** rejected; was already tried and preserved the central ownership knot.
- **Create a deliberate red migration window in the existing live working tree:** rejected; unnecessary when a clean target can be built independently.
- **Run old and new products concurrently behind compatibility readers/shims:** rejected; creates two semantic authorities and violates current-only development-format policy.
- **Automatically rewrite every working algorithm:** rejected; reuse mature correct implementation when it fits. **Reimplementing specific incompatible internals** is permitted and often preferable to carrying their old dependencies.
- **Separate stable reference + direct clean rebuild + one verified cutover:** selected; only offline persisted data requires an actual migration.

## Consequences and verification

- The existing crate-first physical targets are historical evidence, not temporary required milestones. The [Stage 4 target DAG](../architecture/target-crate-dag-2026-11-03.json) is the only approved physical destination.
- The [Stage 5 format migration](../cva-to-rel-format-and-identity-migration-2026-11-03.md) remains valid as **data conversion**, including identical ID hashes and source-corpus proof.
- [Current-format FRS07](../../tests/current_format_boundary.rs) verification is required **before old-data conversion**, not a reason to inject old readers into the new system. Any proof failure stops promotion and may block dependent source semantics.
- No major feature redesign, retrieval tuning, new Ego synthesis or Observation synthesis is implied by rebuilding existing supported capabilities. Obsolete internals may be omitted; required external behavior and data invariants still need proof.
- A complete rebuild requires full Rust/CLI integration, frozen behavioral fixtures, stream/queue/host fault tests, authorization negatives, exact owner-store provenance and measured latency/memory comparison with reference.
- An old implementation still being present in the **reference** tree does not count as stale code in the rebuilt **target**. The target must have no old `Cva` paths/aliases or empty extraction placeholders when accepted.

## Related docs

- [Clean architecture rebuild plan](../architecture-rebuild-implementation-plan-2026-11-03.md)
- [Capability increments and acceptance gates](../architecture/implementation-rebuild-capabilities-2026-11-03.json)
- [Stage 4 crate layout](../dependency-architecture-and-crate-layout-2026-11-03.md)
- [Stage 5 data-format conversion](../cva-to-rel-format-and-identity-migration-2026-11-03.md)
- [ADR 0047: physical package direction](0047-physical-dependency-architecture.md)
- [ADR 0048: current REL 2.1 and stable IDs](0048-rel-current-format-and-stable-identity-domains.md)

## Notes

This decision alters **how the design is built**, not the selected architectural boundaries or the need for one verified format conversion. It provides a clear stopping condition: until the new complete implementation, consumers and converted files pass the final acceptance gates, the reference remains the working system.
