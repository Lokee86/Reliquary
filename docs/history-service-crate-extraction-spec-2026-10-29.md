# History service crate extraction — implementation specification

**Status:** Specified; not implemented (2026-10-29).
**Target:** `crates/reliquary-history/` (`reliquary-history`), presently a four-line placeholder.
**Branch / baseline:** `refactor/crate-first-restart`; HEAD `b95f43cd` (Import acceptance).
**Scope:** correction-only physical boundary; no new product capability, data format, general time-travel engine or migration.

## 1. Audited production boundary

There is **no existing general production historical reconstruction engine** in Root that can be moved wholesale. The source currently falls into distinct authorities:

| Source | Disposition |
| --- | --- |
| `src/facade/cva.rs::historical_memory_revision`; `src/facade/phylactery.rs::historical_memory_revision` | Production REL/PHY Memory historical read paths. Both contain essentially the same logic classifying retained revision as Available/Tombstoned/Pruned. **Real History consumer.** |
| `src/perception/entity_owner.rs::historical_entity_revision`; `src/perception/relationship_owner.rs::historical_relationship_revision` | Production Entity and Relationship historical read paths with the same available/tombstoned/pruned classification. **Two more real consumers.** |
| `src/owner_lifecycle.rs::HistoricalPayload<T,S>` | Single current public result vocabulary. Rehome this *one exact definition* to History and reexport through its original `reliquary_memory::owner_lifecycle::HistoricalPayload` path. No duplicate type or API break. |
| `src/owner_lifecycle.rs::{LifecycleState,TombstoneCoordinates}` | Owner lifecycle, not History. If the shared History policy needs these types as inputs, relocate only their *canonical value definitions* to the existing lower `reliquary-owner-lifecycle` crate and reexport via Root; leave mutation/admission policy and public call sites unchanged. Do not duplicate lifecycle models. |
| Archive, Memory Core, Perception Store, Dream Store, Freshness Store, Ego Store, Graph, Vector Store | Already own current-format record and tombstone history, owner-specific replay, proof and retention. **Keep all store-owned history there.** |
| `src/facade/cva_historical_dream.rs`, `cva_historical_freshness.rs`, `cva_dream_journal_replay.rs`, `cva_dream_population.rs` | Owner-authenticated source hydration / physical prefix replay / Dream or Freshness decisions. Remain in Root and their current service/storage owners. No History dependency on Dream/Freshness services. |
| `src/facade/historical_reconstruction.rs`, `cva_historical_import.rs`, `historical_{epoch_input,events,manifest*,import_*}.rs` | Explicit **Import/Historical Freshness** freeze, proof derivation, model/embedding and settlement publication. Not read-only; stay with Import/Freshness/Root authority. |
| `src/facade/{cva_history_prune,phylactery_history_prune,history_prune_manifest,owner_maintenance,cva_reconcile*}.rs` and `src/rewrite/**` | **Maintenance** pruning/rewrite/reconciliation and owner-authorized mutation. Not History. |
| `src/facade/historical_{fingerprint,global_cuts,global_frontier,archive_fingerprint,memory_fingerprint,graph_fingerprint,whole_owner_fingerprint,whole_phy_fingerprint,...}.rs` | Historical fingerprint/projection fixtures compiled through `#[cfg(test)]` or `history-oracle-fixtures`. Not production APIs. **Do not promote them to production or count them as extraction.** |

**Minimum meaningful extraction:** replace the repeated production historical-result decision logic for **all four consumers** with a single compiled, genuinely invoked, read-only History evaluator, preserving the exact public generic result type. This is a narrow read-view service, **not** completion of every historical feature. If that cannot be achieved without upward dependencies or duplicate models, record the blocker and leave History unimplemented rather than producing a cosmetic crate.

## 2. Exact contract

1. **One canonical result type.** Preserve `HistoricalPayload<T,S>` variants `Available(T)`, `Tombstoned(T)`, and `Pruned { structural_history: S, reason: PayloadUnavailable }` with unchanged generic/trait behavior. Existing Root paths, pattern matches, and method signatures continue to refer to the *same actual type* by reexport.
2. **History is passive.** It can classify **already owner-authorized** source facts. Root/Cva/Phylactery exclusively perform live and historical revision lookup, retention descriptor reads, lifecycle checks and error translation. History cannot hold a `Cva`, `Phylactery`, `Container`, gateway/owner writer, mutable store, live stream or second source file. No clocks, scans, tokens, model calls or store writes.
3. **Preserve order and laziness.** The owner reads the requested revision first. On a successful full record, query lifecycle and return `Available` or `Tombstoned`. Only on its *specific, recognized* `PayloadUnavailable::Pruned` error does the owner look up the **requested revision** in its retained structural descriptor and return `Pruned`. Missing retained revision is the original domain-specific MissingMemory/MissingEntity/MissingRelationship error. Any unrelated read/error propagates untouched. Do not query lifecycle or hydrate body on the Pruned path.
4. **No invented generic history framework.** Implement one small typed policy function over provided facts (lazy inputs where needed) and the shared result type; Root keeps its domain-specific error branches and conversions. No new manager, trait-object registry, second revision cache, generic owner interface, owner-wide clone or redundant snapshot; no unchanged policy duplicates in Root.
5. **Lifecycle is not History-owned.** If `LifecycleState` must become a dependency-visible value type, use `reliquary-owner-lifecycle`, not a History copy. Move no `LifecycleTarget` mutation rights. Existing conversions and public Root reexports stay intact. This must be proven by an early compile gate, not presumed.
6. **Physical dependencies:** only actual allowed lower crates, expected `reliquary-archive` (existing payload-unavailable reason) and `reliquary-owner-lifecycle` if state sharing is needed. `reliquary-memory-core` and `reliquary-perception-store` only if concrete input types justify them. The crate-map list is a *maximum* DAG permission, not a dependency shopping list. **No History→Root, Container (direct), Gateway, Import, Maintenance, Dream, Freshness or Ego** dependency. Root depends downward on History.
7. **No format/API changes.** Preserve ADR 0042 current-only envelope and compact history, retained revision identity, timestamp/global/domain coordinates, pruned-body evidence, source provenance, import semantic settlements, and all owner errors. No compatibility readers, new durable history records, cross-owner global-cut synthesis or revised reference semantics.

## 3. Bounded implementation gates

| Gate | Task | Required proof |
| --- | --- | --- |
| **H0 — baseline and source crosswalk** | Capture HEAD/status, the four exact production consumers, Root public type paths, lifecycle/error types, relevant named tests and current direct dependencies. | Freeze current outcomes; unrelated dirty Graph evidence file not staged. |
| **H1 — canonical value boundary** | Move the one shared `HistoricalPayload` definition to History and retain `owner_lifecycle` reexport. If needed, relocate only shared `LifecycleState/TombstoneCoordinates` definitions to existing lower Owner Lifecycle. | No duplicate enums; Root/CLI compile, public-path and exhaustive variant-matching consumer test; Cargo DAG valid. |
| **H2 — historical view policy** | Extract the existing common available/tombstoned/pruned classification with exact lazy/error ordering; source stores remain below their proper owners. | Crate unit/external-consumer tests for active, tombstoned, verified pruned, missing revision, wrong error, and absence of extra source reads. |
| **H3 — four production consumers** | Wire both Memory owner paths and the Entity and Relationship historical methods to compiled History; remove duplicated classification. Keep source-specific retention, lifecycle, lookup and errors in Root. | REL/PHY/Entity/Relationship historical lifecycle, prune, reopen and rejected-query regression tests, with identical outputs. |
| **H4 — hard-boundary review** | Check the complete `rg` source/consumer crosswalk and Cargo tree; no History→Root, original methods/signatures unchanged, no moved `#[cfg(test)]` fingerprint or Import/Maintenance code, no extra persistent state. | Independent source review and nonmutating read tests. |
| **H5 — acceptance and commit** | Run complete tests, documentation and dependency checks; update actual source maps, write evidence, commit implementation then acceptance. | All required acceptance checks green. No partial migration, unintegrated placeholder or unrelated Graph edit committed. |

Do not split a single policy cut into dozens of artificial tasks. Prefer new files below 200 lines; avoid new 300+ line coordinator files. If an owner-type mismatch cannot be bridged without an upward dependency, **stop at H1** and amend the spec based on concrete compiler/source evidence rather than add a parallel owner projection.

## 4. Verification and completion criteria

The last verified baseline at `b95f43cd` is **1,457 passed / 5 ignored** in Root, **32/32** CLI, and a passing workspace all-target check. Those are *previous acceptance records*, not a new H0 run. Capture the fresh pre-cut test witnesses.

Required after implementation:

- `cargo check -p reliquary-history --locked --offline -j 1`; `cargo test -p reliquary-history --locked --offline -j 1`, including a real external consumer of the concrete evaluator and original public HistoricalPayload path.
- Focused Root tests for Memory historical revisions and tombstone/prune, Entity and Relationship historical revisions, rejected/missing historical records, both REL and PHY reopen and retained proof; name the real selected tests (zero selected is not success).
- `cargo check --workspace --all-targets --locked --offline -j 1`; `cargo test --lib --locked --offline -j 1 -- --test-threads=8`; `cargo test --manifest-path cli/Cargo.toml --locked --offline -j 1`. If the CLI's separate lockfile rejects a new local dependency, refresh it offline, then **rerun locked**. Use a short writable local Windows TEMP/TMP/TMPDIR.
- `cargo fmt --all -- --check`, `git diff --check`, `cargo tree -p reliquary-history --depth 1 --offline`, `ddocs check --indexes --links` and existing architecture/ownership checks where operational. The earlier Lexicon/Arcana refresh timed out and Pitlord source ownership evidence was stale; do not claim a fresh graph passes without one.
- Compare preserved result variants, revision/tombstone/prune structural evidence, transaction/owner version and output errors before/after. No durable format conversion or new historical fixture is needed.
- Record exact baseline/implementation commit, source moves, four Root consumer methods, commands/results, unresolved graph limitations and untouched preexisting Graph edit in `docs/history-service-crate-extraction-evidence-2026-10-29.md`.
- On success, update `docs/api.md`, `docs/architecture.md`, `docs/maintainer-map.md`, `docs/crate-first-layout-spec-2026-10-25.md` and `docs/INDEX.md`. Change `docs/architecture/crate-map.json` only for a real approved ownership/dependency change.

**Accept** a compiled History crate **only** when the four production historical reads use one canonical result view and a real shared evaluator, public APIs remain unchanged, all owner reads remain authorized/nonmutating, and the test matrix passes. **Do not claim** this creates full global historical reconstruction, whole-owner oracle fingerprints, Import rebuilding, Maintenance rewrite/pruning, or future cross-REL history queries. If only an enum move or toy wrapper is achievable, reject as an incomplete extraction.

## References

[Crate-first architecture](crate-first-architecture.md), [crate layout](crate-first-layout-spec-2026-10-25.md), [crate map](architecture/crate-map.json), [ADR 0042](decisions/0042-minimal-lossless-current-history.md), [ADR 0041](decisions/0041-semantic-import-boundaries-owner-pruning-history-rewrite.md), [maintainer map](maintainer-map.md), [Rust API](api.md), [behavioral contracts](behavioral-contracts.md).

**Specification only:** no Rust implementation, storage records, corpus files or owner APIs were changed.
