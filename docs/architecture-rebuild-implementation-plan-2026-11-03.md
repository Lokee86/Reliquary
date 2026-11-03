# Reliquary clean-architecture rebuild — executable implementation plan (2026-11-03)

Parent index: [Documentation index](INDEX.md)

**Status: proposed executable Step 6 plan; no rebuild implementation started.** The target architecture is fixed by [Stages 1–5](architecture-redesign-foundation-2026-11-03.md), [ADR 0044](decisions/0044-canonical-rel-phy-owner-session-architecture.md) through [ADR 0048](decisions/0048-rel-current-format-and-stable-identity-domains.md), and the [physical target DAG](architecture/target-crate-dag-2026-11-03.json). This plan supersedes incremental crate extraction/repeated in-place migration. An explicit statement of its execution model is [ADR 0049](decisions/0049-parallel-architecture-rebuild-and-verified-cutover.md).

## Purpose

**Rebuild Reliquary's architecture around existing implementation**, rather than progressively refactoring a live codebase through temporary internal states. Preserve the algorithms, correctness, ownership, source provenance, tests and measured hot-path behavior. Remove the old Root knot and establish one direct final architecture that can support future capabilities.

## Overview

There are **two separate workspaces/roles**, not two competing production implementations:

1. **Stable reference worktree**: freeze a known source/fixture/consumer baseline, including version 2.0 REL/PHY files, algorithms, executable tests and expected outputs. It stays usable and is not progressively rewritten. Any FRS07 old-format inspection/repair happens in an **isolated validation copy**, not the working reference itself.
2. **Clean rebuild worktree**: initialize the **final 22-supporting-crate-plus-public-Root structure** from the approved architecture. Reuse verified source modules/functions/algorithms as material, placing them **directly in their final owning package**. New implementation is compiled and tested by final owner/service/workflow boundaries. There is no production two-path runtime, old `Cva` aggregate or incremental chain of temporary adapters.
3. **Cutover**, after all target code is implemented and has passed full integration, runs one **separate data-format conversion** on copies of existing 2.0 data into canonical 2.1 files, independently reopens and compares them, and activates one verified corpus generation.
4. The stable reference remains available for comparison and rollback until the new system's acceptance. Incomplete rebuilt components do not replace or disrupt the reference program. The target workspace may have unimplemented modules while being constructed; **that is not a planned “red migration window” in the working product**. No claim of green full workspace is made before it actually exists.

**Naming and format from the start of the rebuild:** final Rust public API is `Rel`/`RelSession` and `PhySession`/`Phylactery`; the new runtime emits and opens **only current REL 2.1**. The isolated one-time data converter alone reads verified 2.0 copies. Six identified stable SHA-derived IDs keep their original binary hash inputs under versioned opaque names so semantic references do not rekey.

## 1. Concrete execution contract

- **Source is reused, not blindly copied:** Insomnia, Dream, Retrieval, Freshness, Graph, Archive, Ego stores, Gateway and other mature algorithms are imported into **their final modules**. Split mixed Root files by responsibility *while reading them*; do not first move a whole `Cva` or `InteractionRuntime` into another crate and plan to fix it later.
- **One canonical implementation in the rebuild:** the reference tree is a *separate baseline*, not a live compatibility library. The new worktree cannot link it, call its Root as a dependency, spawn its Gateway as a hidden fallback, or duplicate persistent semantics to keep both paths working.
- **Direct package map:** Container and eight independent durable domain owners → one `reliquary-owner-session` → named `reliquary-application` workflows → one actual `reliquary-runtime-gateway` → thin public `reliquary-memory`. Semantic services consume bounded immutable facts; external providers remain at Switchboard/host seams.
- **Where existing helper crates go:** Echo's durable module inside Archive; Dream/Import/Maintenance/external-index codecs inside owner-session; History/Import/Maintenance application helpers inside Application. Retire owner-lifecycle forwarding crate and empty Ego service placeholder. None survives as a forwarding package.
- **Test the final destinations:** each rebuilt domain crate has its own unit/replay tests; owner-session has grouped write/current-only open/history tests; Application has semantic/provenance integration tests; Gateway has host/queue/stream/auth/recovery tests. Full product tests become meaningful after those packages compose.
- **Avoid speculative architectural machinery:** no general `Store` trait, universal workflow registry, event bus, all-service context, new architecture-check database, broad mutable accessor, or generated source wrappers. Use existing Rust/Cargo tests, the target JSON DAG and targeted code search.
- **Retain ordinary developer workflow:** stable reference and rebuild worktree have separate `target` build roots (fix or route around the observed deep Windows target-path failure at bootstrap). Never wipe unrelated working trees, target directories or scratch without inspection.
- **Keep work units small:** a ticket owns one concrete module/subsystem boundary and its acceptance; within it, split changes into coherent functions/types and focused tests. Avoid files over 300 lines and prefer under 200 when reasonable; small coordinators, deep components.

## 2. The task DAG and its verification gates

**Authoritative tasks:** [58-task rebuild DAG](architecture/implementation-rebuild-task-dag-2026-11-03.json). Every task has a unique ID, outcome, concrete source scope, dependencies, acceptance evidence and initial `not-started` state. The DAG is a **build order**, not a sequence of changes to the stable implementation. Overlapping targets cannot be edited concurrently.

| Phase | Tickets | Deliverable / gate |
| --- | --- | --- |
| **P0 — Reference evidence** | T01–T07 | Two independent worktrees; frozen semantic/performance golden outputs, real CLI/host consumers, complete tag/identity inventory and old-data FRS07 replay in isolated validation copies. Stable reference untouched |
| **P1 — Final storage from the outset** | T49–T51, T08–T17 | Current 2.1 Container and owner codecs, preserved opaque ID domains, final durable domain owners, `RelSession`/`PhySession`, grouped publication, history and owner-wide method mapping |
| **G1** | T18 | Build/test coherent *new* Container + eight stores + owner-session against synthetic 2.1 REL/PHY data; old corpus remains reference input only |
| **P2 — Semantic and Application** | T19–T34 | Import verified algorithm implementation directly into deep semantic services; build named `reliquary-application` workflows for ingest, Insomnia, Dream, Perception, Freshness, Retrieval/agentic, Vectors, Context, Import, History, Maintenance |
| **G2** | T35 | Compile/test completed new Application + semantic dependency subgraph; frozen algorithm outcomes and exact session publication |
| **P3 — Complete Gateway** | T36–T43 | Actual Gateway/Instance and per-owner worker sets, operational RQMUX/RQOP journals, host queue contract, branch cancel/finalize, stream checkpoints/reconnect, live adapters |
| **G3** | T44 | Gateway compiled and tested end-to-end on new-format synthetic data: real granted principal, queue races, replay, stale owners, privacy, unknown host result |
| **P4 — Public product** | T45–T47 | Clean public Root, canonical REL API, standalone CLI/known consumers, all 10 retired/merged packages absent, exact target 23-package graph |
| **G4** | T48 | Complete rebuilt product passes full synthetic **2.1** workspace/CLI/host integration checks and owner/workflow/stream correctness; **no 2.0 runtime reader** |
| **P5 — Only true data migration** | T52–T55 | One-time isolated converter reads verified 2.0 copies and outputs 2.1; rehearse complete six-file owner corpus, exact owner/clock/history/ID comparisons, verified generation activation; delete converter |
| **G5** | T56 | Full rebuilt product and available consumers pass on **converted real 2.1 corpus**; old/mixed headers reject without mutation |
| **G6 — Final acceptance** | T57–T58 | Measured performance/privacy/host fault parity, target DAG, zero old Rust names, closed source ledger, accepted cutover and retained baseline for audit |

**Gates are meaningful completeness boundaries**, not permission to ship partially built architecture. A subsystem can be incomplete in the new worktree while being authored; don't create a shim or keep an old implementation linked just to make premature whole-project checks pass.

## 3. Detailed build order and source-of-truth rules

### P0: establish evidence before constructing the replacement

- **T01:** capture reference HEAD, pre-existing unstaged edits and mounted data/fixtures, pin repository/submodule/git pins and vendor overrides; create **separate rebuild worktree** with isolated Rust build output. Do not modify old reference to establish a new package graph.
- **T02–T04:** record native conversation, Memory-Web, Dream/Entity/Insomnia, Freshness, retrieval/agentic, Ego persistent, vector and Gateway/host baseline behavior. Prove FRS07 old current-format corpus in an isolated validation copy first; this is a **data-baseline blocker**, not an excuse for a fallback reader inside the new architecture.
- **T05–T06:** classify all 173 lexical marker candidates as live current writer/reader vs invalid/retired/test and inventory every deterministic ID/hash input, CLI, external host and repo consumer. A named external consumer that cannot be compiled is recorded as **unverified**, never silently labeled passing.
- **T07:** no fresh-build files touched until source/identity/corpus evidence baseline is explicit.

### P1/G1: complete the final data substrate directly

- Build **canonical 2.1** physical writer/reader in Container first, then accepted REL-tagged durable codecs and the six stable ID derivations. In the new codebase, there is **never** a transient normal 2.0 reader.
- Import reusable Archive/Echo, Memory, Graph, Entity, Ego, Freshness, Vector, Insomnia store code directly into the **final** durable owner packages; record codecs for Dream/Import/Maintenance/external-index into private owner-session modules. Do not hide unrelated service logic inside a shared owner.
- Implement `RelSession`/`PhySession` once with private typed stores. Prove ordered open/replay; exact global/local clocks, source owner UUID and kind, wrong-kind rejection, histories, grouped Insomnia/Memory/Freshness and independent no-mutation failure behavior. Use **synthetic 2.1 fixtures** generated through new current writer for G1.
- The [519-inherent-method crosswalk](rel-phy-method-ownership-crosswalk-2026-11-03.md) is a **responsibility audit**, not permission to put all 519 on the session; each Root function is consumed at its actual target when that module is authored.

### P2/G2: deep evaluators and real named workflows

- Reuse already implemented Insomnia/Dream/Entity Resolution/Freshness/Retrieval/Chronos/Vectors/Switchboard/Context logic; split Root adjuncts into **bounded evidence/evaluation** versus **typed owner commits**. Keep future Ego synthesis and Observations unimplemented rather than building skeleton services.
- Application has **named** ingestion, episode, Dream, Perception, accepted-use, retrieval/agentic, vector, context, external source, history, import and maintenance operations. Cross-owner REL→PHY stays independent/idempotent; no simultaneous broad locks, no provider under owner mutex.
- G2 tests exact candidate/decision parity and committed owner proof, zero-link Dream settlement, accepted-use-only Freshness, PHY privacy, native vs reference-only external source, historical cut/rewrite eligibility and failure replay.

### P3/G3: mount the real Gateway, not another placeholder

- Reuse existing Gateway/Instance/worker/journal/host/stream code directly in `reliquary-runtime-gateway`, replacing dependencies on `InteractionRuntime { cva: Cva }` with new owner-session and Application contracts. Keep privileged instance grant mutation **inside Gateway**.
- Reproduce one owner-runtime per REL/PHY, native pending queue versus host queue arbitration, exact RQMUX/RQOP journal formats, idempotent cancel/finalize, Held ambiguous handoff, durable checkpoint-before-stream, snapshot+watermark and reconnect grant verification.
- G3 must test fault/interleaving behavior and already accepted durable owner history, not just a struct/constructor compile. Actual host conformance is reported separately if not available.

### P4/G4: complete product before converting old files

- Final public `Rel`, `RelError`, `Phylactery`, Host/CLI entrypoints and curated reexports. No `Cva` compatibility alias, broad Root facade, dual owner path or runtime stubs.
- Remove old packages in the **rebuild worktree** rather than attempting to keep old code operational there. Preserve the reference worktree for comparison. All target Cargo dependencies must match [the 23-package DAG](architecture/target-crate-dag-2026-11-03.json).
- **G4** requires new current-only 2.1 whole-product compilation and synthetic current-data behavioral checks; first-party CLI rebuilt against it, with applicable external contracts. The unconverted old corpus is not expected to open in the new product yet.

### P5/G5/G6: separate one-time DATA conversion and release cutover

- Only after G4, compile the isolated old-2.0-to-new-2.1 converter, using old reference codecs **outside** rebuilt runtime. Converter writes new files, re-encodes accepted frames, maps physical offsets, recomputes transport proofs, preserves all semantic IDs/domain clocks and refuses unknown records.
- Rehearse all 3 REL + 3 PHY fixtures, independent current 2.1 reopens, exact history/generation/identity comparison and host journal/owner operation reconciliation. Convert additional recognized deployments/fixtures only with explicit inventory and source/target digests.
- Promote only a complete verified owner generation; six file renames alone are not atomic. Keep backup/reference and rollback selector, and do **not** ship converter/old normal readers in the final product.
- Compare measured baseline versus rebuilt hot paths, memory allocation, concurrency and host workflows; fix material regressions in the **new target code**, never by importing old Root implementation as a fallback. Accept cutover only after tests, provenance and security/authorization all pass.

## 4. Every source has an assignment, without a giant generated inventory

Use the existing [Stage 4 physical source disposition](physical-crate-source-disposition-2026-11-03.md), [Stage 1 method crosswalk](rel-phy-method-ownership-crosswalk-2026-11-03.md), [Stage 2 source crosswalk](semantic-service-boundary-crosswalk-2026-11-03.md) and [Stage 3 runtime crosswalk](runtime-orchestration-source-crosswalk-2026-11-03.md). At each task, inspect exact original functions and *their consumers*, then mark each `keep/copied`, `split`, `replace` or `omit (proven dead/obsolete)` in **that task's evidence**, not a speculative 1,415-row static ledger. Tests and examples follow their behavior into the final test seam.

**Never infer ownership by filename alone.** For a mixed Root file, implementation moves by semantic authority (domain owner, evaluator, workflow, runtime, public API); it may split across modules. Before closing each ticket, search reverse call sites and tests in the reference, confirm final APIs in the rebuilt code, and leave no second active implementation. An obsolete old source file in the *reference worktree* is expected; the **rebuilt** tracked Rust has no active old API or old path at G6.

## 5. Verification commands and evidence protocol

Commands are **instructions for implementation**, not checks run as part of this design document. Execute in the **rebuild worktree**, with separate `CARGO_TARGET_DIR` and preflight Windows path limits:

```powershell
cargo fmt --all --check
cargo metadata --format-version 1 --no-deps
cargo check --workspace --locked --offline
cargo test --workspace --locked --offline
cargo check --manifest-path cli/Cargo.toml --locked --offline
cargo test --manifest-path cli/Cargo.toml --locked --offline
ddocs check --indexes --links
```

Where the repository has pinned git dependencies not yet locally cached, `--offline` may be unavailable; record the precise reason and use a verified pinned online resolution **without changing revisions**. Capture exit status and relevant test/benchmark outputs for each gate. Run existing specialized staging/host and oracle suites when their authentic fixtures/endpoints are present; mark absent facilities **not verified** rather than claiming green.

**Structural check:** parse target JSON, compare actual package members/dependency edges from `cargo metadata`; assert no cycles, unlisted direct dependency, lower→higher backedge, duplicate members, build-only placeholders or unexpected Root implementation. Targeted `rg`/source checks catch mutable store leaks and historical aliases. Arcana's older snapshot is not current structural proof; refresh only if useful and cheap. Avoid rebuilding the old 60k-node custom architecture ratchet machinery.

**Source/name check:** scan tracked **active Rust source and paths** case-insensitively for the old `cva` spelling; check compiled CLI/host symbols and examples. Keep source-of-truth historical docs and archived binary identity fixtures separate. Golden SHA input and output comparisons establish unchanged ID values with opaque names.

**Evidence:** each task records predecessor SHA, exact copied/adapted original modules, new canonical code locations, tests and measured results, any exception with owner identity and format, and next dependent tickets. Evidence belongs beside the plan or in existing docs; do not generate a new issue database or high-volume structural snapshots.

## 6. Failure, rollback and completion criteria

- If FRS07 current corpus cannot be verified, **stop data conversion** and resolve that source proof. Build design work unrelated to that evidence may continue only when it cannot conceal source corruption; no new model-generated replacement data.
- If an existing algorithm changes semantic output during reuse, stop that owner task and compare frozen reference tests; an architectural rebuild **does not authorize behavioral tuning**.
- If a target dependency requires a reverse edge, fix ownership in the **new code**, not through a temporary generic shared context or Root callback.
- If a host operation has uncertain status, keep Held and reconcile rather than replaying; if cross-owner file effects partially commit, use existing deterministic idempotent receipts, not global rollback fiction.
- If a new 2.1 corpus differs in any stable ID, retained historical cut, graph/Memory state, source/provenance proof, host operation identity or PHY authorization, **abort activation**. The reference corpus remains untouched.
- **Final acceptance:** exact target crate DAG, canonical REL code/2.1 current format, owner-session and Application real behavior, working complete Gateway, all supported tests/CLI/consumer checks, exhaustive source responsibility closure, unchanged semantic identity and accepted history, no abandoned Root pathway, validated copy-based corpus promotion, measured performance/permissions parity. **Only then cut over.**

## Related docs

- [Task DAG, 58 individually verifiable work items](architecture/implementation-rebuild-task-dag-2026-11-03.json)
- [ADR 0049: parallel rebuild and cutover](decisions/0049-parallel-architecture-rebuild-and-verified-cutover.md)
- [Stages 1–4 target structure](dependency-architecture-and-crate-layout-2026-11-03.md)
- [Stage 5 REL format/identity conversion](cva-to-rel-format-and-identity-migration-2026-11-03.md)
- [Current implementation inventory](current-system-inventory-2026-11-03.md)
- [Current source maps](physical-crate-source-disposition-2026-11-03.md)
- [Current-format FRS07 boundary test](../tests/current_format_boundary.rs)

## Notes

The plan is **not an incremental refactor schedule**. The existing program is the working reference; the new final architecture is constructed independently from its implementation. The word “migration” is reserved here for the real persistent **file-format/data conversion**. The rebuild will still involve copying/adapting source code and changing the eventual public API, but those changes are delivered at one verified cutover rather than through an intentionally broken live state.
