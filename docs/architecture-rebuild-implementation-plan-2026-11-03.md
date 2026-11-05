# Reliquary selective-reuse architecture rebuild plan (2026-11-03)

Parent index: [Documentation index](INDEX.md)

**Status (2026-11-04): implementation is active and architecture-correction checkpoint `95db1b41` is committed and tested; product and release gates G1–G5 remain incomplete.** The current execution/handoff record is [rebuild continuation (2026-11-04)](rebuild-continuation-2026-11-04.md). The implementation lives in `C:\!bin\workspace\Reliquary-rebuild`; its root `AGENTS.md` points back to this canonical plan and the capability tracker. The target boundaries are specified in [Stages 1–5](architecture-redesign-foundation-2026-11-03.md) and [ADR 0044–0048](decisions/0048-rel-current-format-and-stable-identity-domains.md). [ADR 0049](decisions/0049-parallel-architecture-rebuild-and-verified-cutover.md) governs the independent reference/rebuild setup. The [capability build plan](architecture/implementation-rebuild-capabilities-2026-11-03.json) replaces the earlier 58-item old-source relocation DAG.

## Purpose

**Build the new Reliquary, using only the existing code that earns a place in it.** The original implementation supplies working algorithms, known behaviors, existing persisted data and test oracles. It is **not** an exhaustive inventory to transport, a set of crates to peel away, or a requirement to reproduce every internal API. The user-facing behavior and safety guarantees that remain in scope must work in the replacement; unnecessary internals may be discarded.

## Overview

The work is an **independent clean implementation of the agreed target architecture**, not an in-place refactor:

- Freeze an unchanged reference source and representative corpus. Investigations into the old FRS07 corpus happen on copies; failures are recorded, not “fixed” by rewriting the source of truth.
- Create a new worktree with the **final** architectural seams from [Stage 4](dependency-architecture-and-crate-layout-2026-11-03.md): physical Container, purpose-built durable owners, one REL/PHY owner-session, deep semantic evaluators, named application workflows, one Gateway and thin public Root. Target is currently **22 supporting crates plus Root**; an unnecessary package must earn its existence or be removed through an explicit architecture revision, never left as a placeholder.
- Implement **observable capabilities** and tests directly against that architecture. For each, study whatever part of the reference is useful, choose minimal code to reuse, adapt or rewrite it, then test the new behavior. No source function, old filename, old crate or forwarding layer is mandatory just because it exists.
- Use a *working vertical capability* as an integration checkpoint (accepted turn → grounded Memory → Dream/Entity organization → authorized retrieval and confirmed use), not a parade of old source-file relocations.
- Build the **current-only REL 2.1** writer/reader from the beginning; old 2.0 records are read only by a later **isolated offline data converter**, and only if the old corpus can be verified. No production compatibility reader, parallel owner aggregate, fake gateway or all-service registry.
- Keep the original as the functional comparison and rollback reference until the replacement and converted corpus are independently validated. Do not intentionally break the stable program to move implementation.

## 1. Select behavior before selecting code

The [capability plan](architecture/implementation-rebuild-capabilities-2026-11-03.json) records **49 capability increments and gates**. These are not 49 mandatory source transfers or claims that a subsystem can be implemented in one small code edit. As each increment becomes ready, inspect its **target interface and observable tests** and break it into just the few implementation slices actually necessary. Do not pre-generate one ticket per reference function or a speculative hundreds-step file map.

For every prospective behavior, make one of four decisions:

| Disposition | Criterion | Consequence |
| --- | --- | --- |
| **Required behavior** | Existing supported user/API/storage behavior, durable invariant, security boundary, or designated conformance case | Implement and verify in the new architecture; internal implementation may differ |
| **Selected existing implementation** | Correct, simple enough and compatible with the target owner/interface | Reuse the actual algorithm/record implementation, with appropriate tests |
| **Replace implementation** | Required behavior but existing internals depend on Root-wide mutable state, complex scaffolding, or incorrect boundaries | Implement a simpler correct owner-local solution using reference outputs as an oracle |
| **Omit/defer** | Unused compatibility shim, superseded code, diagnostic-only machinery, phantom placeholder, or unimplemented feature | Do not copy it into the new repository; explicitly keep any *supported* external behavior or defer a planned feature without pretending it ships |

**Acceptance is behavior-based, not function-count-based.** The 519 inherited `Cva`/`Phylactery` methods from the old source crosswalk are useful for looking up existing behavior; **there is no requirement to re-create or classify the fate of all 519**. Similarly, the 30 old crates are a reference catalog, not 30 things that must be migrated.

A capability is complete when its desired externally observable behavior, durable evidence, privacy and negative cases pass through the **new** interfaces. It does not need byte-identical internal traces unless those bytes are part of a retained protocol, identity or file-format contract.

## 2. Build increments and proof points

The JSON lists each outcome, prerequisites and acceptance. Implementation proceeds by dependency readiness, **not by linearly completing legacy modules**. A dependency blocks **declaring acceptance**, not provisional exploration or code construction. Some Gateway and application modules already have partial implementations ahead of prerequisite G1/G2 gates; do not claim those increments accepted until the prerequisites and their own behavioral evidence pass.

| Group | Desired new behavior | Evidence required |
| --- | --- | --- |
| **Reference and scaffold** | Stable old comparison, scoped feature inventory, FRS07 data proof, isolated target workspace | Reference not modified; target graph valid; known failures surfaced |
| **Physical and durable owners** | REL 2.1 Container; Archive/Echo; Memory; Graph/Community; Entity; Ego records; vectors; Freshness; durable work/import/maintenance evidence | Independent read/write/replay/identity and exact owner history tests per chosen owner; no inference required for open |
| **Owner sessions** | One `RelSession` and one `PhySession` with typed reads, grouped commits and historical proof | Owner-kind/privacy tests, synchronized publication/restart recovery and short lock boundaries |
| **Semantic capabilities** | Switchboard/Chronos; Insomnia; Dream; Entity Resolution; Freshness; Retrieval/agentic; Vectors/Context; Project Environment | Pure/evaluator tests from frozen behavior; bounded inputs, typed effects and no owner-store mutation by service |
| **Product verticals** | Native turn→Memory; Dream Memory-Web; Entity resolution; retrieval→host accepted-use; vectors/context; history/import/maintenance; external reference sources | Whole workflow tests for provenance, idempotence, source authorization, privacy and negative/crash cases |
| **Gateway** | One mounted owner runtime, authenticated Instances, conversation queue, host reconciliation, streams, workers and restart | Owner/branch concurrency, cancellation, withheld ambiguous outcomes, checkpoint-before-visibility and grant revocation tests |
| **Post-G3 maintenance parity** | Restore supported fresh-history rewriting, multi-domain pruning/reclamation/repair, divergent REL semantic reconciliation and historical reads before exposing final public interfaces | [M01–M04 / GM specification](architecture/maintenance-history-parity-phase-2026-11-05.md): independently verified REL/PHY behavior, typed failure paths and reference-based parity before P01 |
| **Public product** | Thin canonical `Rel`/PHY API, REL-only CLI, supported host integration | Consumer conformance, no old `Cva` surface, no Root mutable implementation |
| **Existing data conversion** | Offline verified 2.0→2.1 files only after complete rebuilt product tests | Source/corpus proof, exact IDs/history/clocks, offset relocation, independent reopen and safe activation |
| **Final acceptance** | Complete tested replacement | Actual Cargo DAG, no obsolete paths, measured performance/host/privacy parity and retained rollback reference |

**Integration gates** in the plan are storage, semantic/application, Gateway, **mandatory post-G3 maintenance/history parity (GM)**, complete synthetic-2.1 product, and converted-real-data cutover. Gate failure is a defect in **the new worktree**; the reference stays usable. Individual modules are allowed to be under construction, but no gate is reported green because a placeholder compiles.

## 3. How to implement one capability without importing the old architecture

A practical increment has a **thin implementation record**, not a migration manifest:

1. Identify its behavior contract and required input/output/evidence. Example: `native turn → grounded Memory` means accepted Archive turn, source-authenticated Episode, Insomnia candidate, correct REL/PHY owner routing and idempotent publication.
2. Read the relevant reference algorithms and existing tests. Keep what is correct and fits. Discard any old Root `Cva` field access, forwarding types, duplicate caches or compatibility wrapper rather than reproducing it.
3. Implement its minimal new types and operations **in final owner/service/workflow modules**. Rust call-site shape derives from the new architecture, not the old constructor signature.
4. Verify with one or more **small focused tests**, then run the capability-level integration test. Expand the unit only when a concrete compile, behavior, persistence or dependency failure requires additional work.
5. Record the new source locations, reused algorithm (if any), tests and remaining limitations. No inventory of old functions not selected for reuse is required.

The initial slices should be small and cohesive, with entry-point coordinators kept small; prefer source files under 200 lines, split before 300 when feasible. Avoid creating large “implementation ticket” structures or an architecture-evidence framework to prove things Cargo/tests already demonstrate.

## 4. Requirements that cannot be discarded casually

These are **supported behavior and data contracts**, not demands to reuse their old code:

- One canonical REL/PHY owner; exact identity, accepted history, source proofs, typed authorizations and PHY-private information.
- Native Archive records distinct from reference-only host transcripts; no fabricated native turns.
- Insomnia source-grounded extraction and user/project routing; Dream duplicate/canonical/first-cycle behavior; Entity canonical identity; Freshness accepted-event semantics; current Retrieval/agentic behavior.
- Model/embedding/host operations outside broad owner mutation locks; bounded work and owner-local concurrency.
- Gateway durable cancellation/finalization, queue ownership, ambiguous-host `Held` handling, stream/checkpoint sequencing, reconnect and revocation.
- Real current-only file-format validation, consistent grouped publication, recovered owner clocks and supported import/rewrite/history semantics.
- **Stable externally referenced deterministic IDs** and original semantic data. A format-signature rename alone does not authorize rekeying existing records.
- CLI/public API supported operations that consumers actually use; deliberate hard-cut renames are allowed where documented. Obsolete internal APIs are not automatically retained.

Future Observation synthesis and Ego cognition remain **future features**. Existing Entity Resolution and Ego **storage** stay in scope, but creating fake new inference services is not.

## 5. Implementation verification and release cutover

Run the following checks **from `Reliquary-rebuild`** when accepting an increment, not after every small edit. The executable package/import guard is mandatory: compilation alone does not enforce the approved dependency boundaries.

```powershell
python scripts/check_architecture.py
cargo fmt --all --check
cargo metadata --format-version 1 --no-deps --locked --offline
cargo check --workspace --locked --offline -j 1
cargo test --workspace --locked --offline -j 1 -- --test-threads=1
# Only after the replacement CLI exists (P01):
cargo check --manifest-path cli/Cargo.toml --locked --offline
cargo test --manifest-path cli/Cargo.toml --locked --offline
# Only after replacement documentation tooling/content exists:
ddocs check --indexes --links
```

Avoid proliferating Windows Cargo target directories: low C: disk space previously caused an unrelated PDB linker failure. Use isolated output only when needed, and never delete user files, corpus, reference data, or another worktree's cache. CLI/docs checks against the old source do **not** prove the replacement is ready. Commit every verified capability as a recoverable checkpoint. A missing pinned dependency or model/host endpoint is **unverified**, not a passing test; record the reason. Only start benchmarking comparable workloads after both the old reference and rebuilt equivalent can run.

**Crate verification:** Run `python scripts/check_architecture.py` as a mandatory acceptance check; compare the rebuilt `cargo metadata` dependency graph against [the selected DAG](architecture/target-crate-dag-2026-11-03.json); no reverse edge into Root/Gateway and no unwanted all-owner state. A package with no substantive implementation fails. A proposed graph change needs a brief explicit architecture decision, not an ad hoc compatibility crate.

**Data verification:** Only the separate converter uses old-format records. First prove the existing FRS07 old corpus with authentic semantics; then convert copies, independently reopen all supported outputs using **2.1-only** runtime and compare durable owner IDs, body/history/revisions, exact provenance, cross-owner refs and SHA-derived IDs. Do not treat independent renames of six files as atomic; activate one validated generation or use an explicit stopped-release rollback protocol. Retire converter and old readers after acceptance.

**Cutover:** Release only when the rebuilt product, applicable consumers, converted corpus, host/queue/privacy tests and actual measured performance acceptance pass. The old code and corpus remain as **reference and rollback assets**, not linked fallback libraries.

## Related docs

- [Capability increments and acceptance gates](architecture/implementation-rebuild-capabilities-2026-11-03.json)
- [ADR 0049 — Independent clean rebuild](decisions/0049-parallel-architecture-rebuild-and-verified-cutover.md)
- [Physical target DAG](architecture/target-crate-dag-2026-11-03.json)
- [Storage and owner sessions](rel-phy-storage-architecture-2026-11-03.md)
- [Semantic services](semantic-service-architecture-2026-11-03.md)
- [Runtime and orchestration](runtime-orchestration-architecture-2026-11-03.md)
- [REL 2.1 format/data conversion](cva-to-rel-format-and-identity-migration-2026-11-03.md)
- [Reference source inventory](current-system-inventory-2026-11-03.md)

## Notes

This plan intentionally does **not** assign every legacy Rust function, CLI flag, file or crate a new address. The new system implements chosen capabilities under selected boundaries, and unused old implementation stays behind. The capability plan is a coordination/checkpoint guide; minimal coding slices are derived from each next capability **when it is actually implemented**, based on its new interface and tests. The independent rebuild workspace is under construction; no conversion or cutover has occurred. See the capability statuses and workspace setup evidence for current progress.
