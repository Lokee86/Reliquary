# Memory Core crate extraction — implementation specification

**Status:** Implemented; Memory Core and its consumers are extracted and locally verified.
**Branch:** `refactor/crate-first-restart`, pre-extraction HEAD `5318e2a0` (Container and Archive already extracted).  
**Destination:** `crates/reliquary-memory-core/` (`reliquary-memory` is the existing public product package).  
**Policy:** correction-only staged hard cut, one semantic Memory owner, no feature or format redesign.

## Purpose

Extract the existing Memory semantic owner across the accepted Cargo boundary.

## Overview

The staged cut below preserves current bytes and behavior while moving one owner
and its tests into a compiled library.

## 1. Problem and developer outcome

The durable Memory owner is still in the root `reliquary-memory` package: 42 Rust files under `src/memory/` (22 implementation, 20 test files), plus publication, historical read, validation and composition logic in `src/facade/**`. File location is not domain ownership. `MemoryStore` and `MemoryOpenState` currently depend upward on Archive, Insomnia, External Source, Owner Lifecycle, Maintenance, Chronos and Perception-shaped values. Moving those files unchanged would create forbidden dependencies instead of an independently compiled owner.

**Target:** `reliquary-memory-core` owns Memory identity, immutable semantic body identity/content, accepted creation/revision/terminal publications, per-owner dense Memory versions, mutation-id idempotency, expected-revision and immutable-body checks, compact current-format codecs, validated replay, retention facts and immutable historical read views. Both REL and PHY use the *same compiled semantic Memory owner*, with separate authorized physical owner sessions. The root `Cva`/`Phylactery` consumes it directly.

**Not the target:** a service for generating Memories, model inference, scoped access policy, Graph/Entity state, Freshness events, embeddings, imported-epoch coordination, or multi-owner rewriting. Cargo must enforce the direction that the monolith did not.

## Related docs

The governing documents and recovery evidence are listed below.

## 2. Governing plans, decisions and recovery evidence

| Existing authority | Consequence for Memory Core |
| --- | --- |
| [Crate-first layout](crate-first-layout-spec-2026-10-25.md), [target DAG](architecture/component-dag.json), [physical map](architecture/crate-map.json) | `memory-store` is rank 2 and maps to `reliquary-memory-core`. **Its only permitted direct Reliquary Cargo dependency is `reliquary-container`.** Archive is a distinct rank-2 peer, not a dependency; all higher services depend downward on Memory. |
| [Container extraction](container-crate-extraction-spec-2026-10-25.md), [Archive extraction](archive-crate-extraction-spec-2026-10-25.md) and [Archive verification](archive-crate-extraction-results-2026-10-25.md) | Reuse compiled physical cursor, publication envelope, clocks, review/codec primitives. Archive supplies source Episodes; Memory must never import the Archive crate. Archive already persists source link target IDs without validating Memory existence. |
| [ADR 0001](decisions/0001-purpose-built-database-ownership.md), [ADR 0012](decisions/0012-deterministic-episodes-and-insomnia-memory-authority.md), [Insomnia](subsystems/insomnia.md) | Insomnia is the *semantic author* of extracted Memories and decides accepted candidates; Memory Core validates and durably publishes those results. Source Episodes remain Archive truth. Memory revision identity and mutation admission do not depend on embeddings or queue leases. |
| [Scope ownership](subsystems/scope-ownership.md), [ADR 0040](decisions/0040-terminal-tombstones-and-import-epochs.md), [ADR 0041](decisions/0041-semantic-import-boundaries-owner-pruning-history-rewrite.md), [ADR 0042](decisions/0042-minimal-lossless-current-history.md) | REL and PHY do not share a mutable semantic owner. Tombstones and read-only history preserve exact owner coordinates. Cross-owner authorization, import settlement, prune manifests and new-history rewrite are handled above the store. |
| [Storage format](storage-format.md), [compact-history specification](minimal-lossless-history-storage-spec-2026-10-19.md), [publication envelope contract](publication-envelope-contract-2026-10-20.md) | Preserve literal `CVAMC002`, `CVAMD002`, `CVAMT002` Memory bodies inside `CVAPUB01`, current compact field-mask/delta semantics, separate grouped Insomnia/External Source carriers, Memory-local and CVA-global clocks, every retained revision. No new wire schema. |
| [Freshness](subsystems/freshness.md), [Review clocks](subsystems/review-clocks.md), [Retrieval](subsystems/agentic-memory-retrieval.md) | Memory revisions and lifecycle are not Freshness scores, accepted-use reinforcement or query selection. Reads of Memory do not themselves reinforce Freshness. |
| [Behavioral contracts](behavioral-contracts.md), [API](api.md), [maintainer map](maintainer-map.md), [documentation index](INDEX.md) | Maintain current supported public Memory semantics and error ordering; update path maps after the real move, not before. |

**Recovery review:** The abandoned refactor at the preserved original `main` has Memory-related changes in `86145476` (physical boundary), `1380d0b4` (owner cut), `408d7751` (read-only History), `8103711f`/`d685e0cc` (lifecycle), `957733a4` (Maintenance/reconciliation), `50101068` (qualified retrieval), and `e5d62b7f`/`7c74b511`/`f431401e` (upper caller cuts). Review *specific source diff and associated tests* for genuine errors fixed or useful owner seams; do **not** cherry-pick old Phase 0–7 plans, ratchets, tool-calling proofs, giant fixture ledgers, or compiler/graph evidence. Recovery inventory and untouched authoritative corpus are preserved in `C:\!bin\backups\Reliquary-crate-restart-2026-10-25\`.

**Format caveat:** The preserved promoted corpus uses post-baseline `CVAFRS07` semantic import-settlement evidence; this is not a Memory publication format. Reconcile it during the later Freshness/Import recovery. Do not add retired-format readers to Memory or claim corpus acceptance simply because these Memory tests pass.

## 3. Target source and module layout

Keep current descriptive modules where useful; no generated empty module skeleton or per-module trait. Illustrative organization:

```text
crates/reliquary-memory-core/
├── Cargo.toml                  # only reliquary-container among Reliquary dependencies
└── src/
    ├── lib.rs                  # compact Memory-owner API and durable values
    ├── memory_model.rs         # Memory, MemoryId, MemoryBodyId, revisions, draft, provenance
    ├── memory_error.rs         # owner error, not product LifecycleError
    ├── memory_store.rs         # owner state, publish, current/historical, tombstone
    ├── memory_rebuild.rs       # replay of recognized Memory facts, read-only finish
    ├── memory_codec*.rs        # body, routing and scalar/current value codecs
    ├── memory_compact_*.rs     # exact 20-field creation/delta/apply/publication
    ├── memory_temporal_*.rs    # persisted inference snapshot *codec*, not inference
    └── memory_store/
        ├── grouped_publish.rs # Memory-owned batch preparation/admission/read facts
        └── history_prune.rs   # Memory marker/retention facts; no Maintenance policy
```

**Move directly:** the owner-specific 22 implementation files under `src/memory/` **after the exceptions below**, plus Memory-owned low-level helpers currently in `src/facade/cva_memory_publish.rs`, `src/facade/phylactery_memory_source.rs` and `src/facade/historical_memory_fingerprint.rs` when those methods are truly Memory-local. Do not copy facade coordination, Archive inspections or historical multi-owner fingerprint aggregation into Memory just because a file name mentions it.

**Keep above Memory (current root until their own extraction):** `cva_memory_publish`'s Archive provenance admission; `cva_reconcile_memory` and history/rewrite normalization; REL/PHY routing and capability checks; `cva_memory_retrieval`, `memory_search`, vector store and retrieval indexes; Insomnia model extraction/leases/receipt formation, Dream mutation orchestration, and provider/External Source transport/processing; Freshness score/accepted-use receipts; multi-owner pruning and import settlement. `memory_source_time.rs` performs Archive Episode/Node lookup and remains an upper-level source-time resolver (its pure “already persisted timestamp” projection can move).

**Processing epochs:** `src/memory/processing_epoch_{model,store}.rs` (`CVAPEP01`) track processing-lane/checkpoint state and only validate a referenced Memory ID; they are *not* Memory semantic revisions. Leave them at root during this cut, and move their single durable implementation to planned `reliquary-owner-lifecycle` (cross-lane operational coordination) at that later extraction. The Memory crate must expose a historical-ID existence read for their validation. Avoid a transient duplicate processing-epoch store.

## 4. Exact upward dependency cuts

These are observable in current source, so **completion requires cutting them**, not hiding them under an interface facade.

| Existing coupling | Target responsibility and call direction |
| --- | --- |
| `memory_model.rs` imports `EpisodeId` and `MemoryId` exported by Archive. Archive's `file_memory_link_model.rs` carries its own opaque `MemoryId` target reference. | **Memory Core defines canonical semantic `MemoryId`** as the unchanged 32-byte identity and stable body/revision identifiers. Archive continues to own `EpisodeId` and its source-side opaque target-ID evidence, without an Archive→Memory dependency. Store source Episode identity in Memory as a fixed 32-byte **foreign-source reference** with unchanged binary representation, and convert/validate it at the product/Owner Lifecycle source boundary. Archive's foreign target ID becomes an explicitly named source-link value if necessary, with a narrow, byte-exact conversion at the higher composition seam; do not create two competing Memory owners, change hash derivation, or introduce Archive→Memory/Memory→Archive Cargo edges. Preserve supported product-facing `MemoryId`/`EpisodeId` API semantics. |
| `memory_rebuild.rs` decodes `crate::insomnia::completion` and `crate::external_source::publication`; `grouped_publish.rs` uses `InsomniaCompletionBody` and mutates `InsomniaCompletion`. | **Memory Core owns** body, routing, revision and grouped batch value/replay validation. **Insomnia/External Source own** complete grouped carrier decoding, source proof, work receipt and authorization. The root owner-open coordinator pulls Container frames, validates the full concrete carrier through its owning decoder, and passes immutable, typed Memory body/record/routing batches to Memory Core in original physical order. Batch preparation returns concrete Memory-owned facts that the upper carrier encodes; no generic decoder/owner callback registry and no buffered all-file replay. Preserve `CVAXMEM2` and Insomnia completion bytes, transaction ranges, atomicity and error precedence. |
| `memory_store.rs` stores `external_source::ExternalMemoryEvidence`, external publication windows and an `impl MemoryStore` in `src/external_source/publication.rs`. | External provenance/processing windows remain **External Source** owner evidence at the upper boundary; Memory Core owns accepted Memory records/bodies. The upper coordinator validates source binding/evidence and manages window idempotence before/with the Memory batch, using exact accepted Memory IDs and receipt proofs. Remove external-source-specific fields and implementation from Memory Core; do not re-encode or lose window retry protection. |
| `MemoryStore::validate_provenance(&Archive)`, `cva_memory_publish::validate_memory_provenance`, and `memory_source_time` dereference Archive source nodes/Episodes. | Memory Core checks intrinsic required-field pairings and stored provenance format. **Root/Owner Lifecycle validates source existence, Episode membership and authority** against Archive, before publication and after both owners replay. Keep the same `InvalidProvenance` and missing-source fail-closed behavior. Do not import Archive into Memory Core. |
| `memory_model::MemoryTemporalInference` embeds `TemporalInference` and `memory_temporal_codec_kind.rs` imports Chronos resolution/kind shapes. | Persisted temporal result is a Memory-owned immutable **value snapshot** (model, contract version, spans, evidence, canonical expression and kind tags), with identical encoding. Chronos owns inference, detection and verification; an upper caller converts its result into the stored Memory DTO only after validation. Do not move Chronos inference engine or add Memory→Chronos dependency. |
| `MemoryRoutingMetadata` and Memory store's mention index use `MemoryEntityMentionKey` from Perception. | Memory owns stored mention spans and its own private key/index representation. Perception owns mention resolution/admission policy and can consume immutable Memory mention facts. Use one canonical stored mention codec (`CVAMRTE2`), unchanged text boundary constraints, without Memory→Perception dependency. |
| `MemoryError::PayloadUnavailable`, `MemoryStore::tombstones`, `tombstone()` outcome and `history_prune.rs` import `owner_lifecycle`/`owner_maintenance`. | Memory Core owns Memory-specific terminal coordinates (the physical fields now carried by `TombstoneCoordinates`), idempotent delete, body-unavailable reason, marker bytes (`CVAMPRN1`) and retained revision facts. Root maps these to public `LifecycleState`/`TombstoneOutcome`/`HistoricalPayload`. Maintenance owns cross-owner manifests, descriptor comparison, protected-source decisions and destructive rewrites; read-only Memory views expose needed marker/revision facts without importing Maintenance. An authorized explicit Memory write may manage its own local bytes; ordinary opening must not mutate or “fix” tombstones or pruned bodies. |
| `processing_epoch_store` is collocated with Memory. | Do not conflate cross-lane scheduling receipt state with a semantic Memory revision. Move at Owner Lifecycle stage as stated above; root holds the sole existing store until then. |
| Freshness receipts and Graph/Dream/Insomnia use Memory state. | They depend downward on a narrow versioned Memory read/admission surface. **Memory Core never imports them** and never reinforces on a read. |

No new crate, generic `contracts` library or permanent alias layer is authorized to shortcut these cuts. Where an existing externally supported public type path is in use, preserve it *at the root product boundary* without routing internal cross-crate calls back through root.

## 5. Memory Core's deep public contract

The exported surface should be **much smaller than** all former `pub(crate)` helpers.

- `MemoryStore` and `MemoryOpenState`: create/replay without hidden writes, accept validated Memory creation/delta/tombstone, preserve dense local versions, exact global coordinates, immutable body ID/hash and durable mutation identity; reject conflicting/reordered events before changing committed state.
- Typed `MemoryId`, `MemoryBodyId`, `MemoryRevisionId`, `MemoryDraft`, `Memory`, `MemorySourceRef`/source evidence snapshot, `MemoryError`, and small immutable historical/revision/tombstone views. Keep internal `MemoryRecord`, raw indexes, delta masks and hash helpers private where no valid consumer exists.
- Single/expected-revision publication: creation derives stable ID from mutation ID; valid repeated mutation is idempotent, conflicting mutation fails; a revision cannot replace its immutable semantic body. Terminal tombstone is idempotent at original coordinates and excludes ordinary updates; historical identity/retained body facts remain queryable under upper authorization.
- Grouped publication operations take or return *Memory-specific typed batches*, not Insomnia/External Source structs. Preserve source cohort revision-1 admission, compact shared bodies, retry/no-op exactness, global reservation binding and all-or-nothing completion semantics as enforced by the owner coordinator and existing carrier.
- Current and historical reads support exact local/global cuts, body availability/prune reason, source/provenance snapshots, routing metadata and side-effect-free candidate hydration. A read **never** implies Freshness accepted-use reinforcement or source authority.
- Preserve current-format compact delta field ordering and explicit optional-clear semantics. Pure delta helpers already compiled in Container may be reused; the Memory crate remains responsible for semantic field masks and validation.

**No raw Container file handle, arbitrary mutable internal maps, implicit import write bypass, generic service locator or model/provider call** escapes this owner.

## 6. Staged hard cut: bounded work, no compatibility scaffolding

1. **Confirm recovery and acceptance baseline:** check HEAD/worktree; inspect specific old Memory owner corrections and current actual consumer import paths with `rg`; measure which existing test files are genuinely Memory-owner-only. No long-running Arcana graph-generation requirement. The accepted DAG and Cargo map are the authority.
2. **Extract semantic identity, native current codecs and replay:** move the canonical Memory value/codec/compact storage code directly to `reliquary-memory-core`. Resolve the foreign Archive/Chronos/Perception/lifecycle *value shape* crossings above; do not copy their services into the lower crate. Add only `reliquary-container` as an internal dependency. Compile and run Memory codec/replay tests independently.
3. **Extract actual Memory owner storage:** move `MemoryStore` and Memory-local publish, expected-revision checks, body/prune facts, routing metadata and idempotent tombstones. Correct grouped batch methods so they operate on Memory-owned facts. Move source provenance verification and multi-owner maintenance/processing coordination to root consumers until later owner cuts. It is acceptable for root to be red during this bounded migration.
4. **Wire real consumers:** `Cva`, `Phylactery`, Insomnia completion, External Source grouped publications, Import/reconciliation, History, Perception, vectors, retrieval, Freshness, Dream and Gateway consume the compiled Memory owner (directly or through upper owner composition when they need authorization). Preserve per-frame replay order, prefix/cut validation, exact error translation and PHY/REL privacy.
5. **Delete displaced implementations:** remove `src/memory` store/codec/model/rebuild modules and their root `#[path]` registration; keep higher-owned processing-epoch, source-time, and integration code in explicit appropriate current root modules until their future extraction. Keep any supported product type exports but no parallel Memory store or temporary dependency inversion.

Do **not** extract Insomnia, External Source, Owner Lifecycle, Maintenance, Chronos, Freshness or Retrieval as independent crates in this step; just move the minimal caller code necessary for the owner cut. A red local transition is preferable to dual readers/writers.

## 7. Existing tests and measurable completion gates

**Move owner-local tests** only where they truly require Memory Core and Container: `memory_codec_tests.rs`, `memory_compact_{apply,codec,contract,writer,retry}_tests.rs` (split writer/retry tests if they drive `Cva`), `memory_publication_codec_tests.rs`, `memory_rebuild_overflow_tests.rs`, `memory_tests.rs` (split Archive-dependent fixtures), `memory_routing_tests.rs`, `memory_temporal_inference_tests.rs` (codec-only cases), and owner-local mutation/revision/version/terminal tests. Do not manufacture an entire parallel test suite.

**Retain cross-owner tests at root:** `memory_compact_maintenance_tests.rs`, `memory_lifecycle_tests.rs`, `memory_publication_{copy,prune,replay,writer}_tests.rs` where they require REL/PHY, Archive/Insomnia, history/rewrite or Maintenance; `processing_epoch_*_tests.rs` until its true owner extraction; `memory_source_time_tests.rs`, `cva_reconcile_memory_tests.rs`, historical Memory fingerprint, native import, External Source birth-prefix, Freshness accepted-use, Dream and runtime consumers.

**Behavior to verify independently (existing tests first):**
- `CVAMC002`/`CVAMD002`/`CVAMT002` creation/delta/terminal literal bytes and the **20-field** delta contract including unchanged vs. explicit optional clear; malformed and out-of-order complete records reject without changing the accepted prefix.
- Deterministic Memory/body identity, revision conflict, mutation-id retry/conflict, shared body hash, dense Memory-local/version reserve ordering and terminal idempotency across reopen and historical cuts.
- Insomnia and External Source grouped writes: precise source cohort, full body/routing/provenance, global/local clock ranges, crash-prefix/idempotent retry; root preserves error precedence after rewiring.
- REL and PHY open/create/reopen; no implicit repair, exact source-provenance/Archive Episode and node checks, no cross-owner authority leaks, pruning preserves read-only structural history and rejects unapproved marker evidence.
- Chronos body/source-time binding, Perception mention-byte spans and accepted routing metadata; passive reads do not update Freshness or vector records.
- `MemoryId` and `EpisodeId` foreign-reference conversions are checked with literal roundtrip bytes and current public facade signatures. No silently altered persisted JSON/byte representations.

**Local checks:** `cargo check -p reliquary-memory-core --locked --offline`; `cargo test -p reliquary-memory-core --locked --offline`; focused root Memory/Insomnia/Import/History/REL-PHY tests. **Final extraction gate:** `cargo check --workspace --locked --offline`; `cargo fmt --all -- --check`; relevant root owner and existing CLI tests via `cargo test --manifest-path cli/Cargo.toml --locked --offline` (CLI has its separate lockfile); Cargo manifest and `rg` inspection proving only one Memory store, only Container below it, no residual `src/memory` semantic implementations. Report any skipped expensive historical measurements. Full product suite and recovered `CVAFRS07` corpus acceptance are later whole-program integration gates, not false Memory-local passes.

Update [Architecture](architecture.md), [API](api.md), [Maintainer map](maintainer-map.md), [Storage format](storage-format.md), [Scope ownership](subsystems/scope-ownership.md), [Behavioral contracts](behavioral-contracts.md), [crate layout](crate-first-layout-spec-2026-10-25.md) and test/code maps **after code has moved**. No extra proof ledger, new ADR or wrapper framework for an already accepted DAG.

## 8. Exclusions and acceptance boundary

No new Memory feature, semantics changes, source authority redesign, Freshness event/score implementation, episodic topic detector, Insomnia model/queue migration, cross-owner lifecycle rewrite, historical Import settlement redesign, semantic temporal inference implementation, schema upgrade, old-format runtime fallback, runtime gateway refactor, or new test-count/proof apparatus.

**Complete** means the real Memory owner and its actual consumers compile and behave through one compiled library. Empty placeholder compilation, stale Arcana evidence and a final list of unexecuted tests are **not** completion evidence.

## Notes

Local extraction verification passed: locked/offline workspace check; 35 Memory
Core tests; 778 focused product tests (2 fixture-dependent tests ignored); 32 CLI
tests; workspace/CLI formatting; documentation policy and Archivist index/link
checks. The final focused run excluded four byte-prefix/scaling stress cases
that had passed in the preceding integration run. Tests use short repository-local
temporary paths to avoid Windows path limits.

The source audit confirms one MemoryStore/MemoryOpenState implementation and
Container as Memory Core's only direct Reliquary dependency. Processing epochs,
source-time resolution, external receipts and cross-owner maintenance stay above
Memory. Current publication bytes and the 20-field delta contract are preserved.

The installed Pitlord lacks its `docs` command. Architecture guard self-tests
passed, but fresh Arcana evidence/its dependent enforcement was not generated.
Full-product and promoted CVAFRS07 corpus acceptance remain later integration
gates; neither is implied by these Memory extraction results.
