# Retrieval service crate extraction — implementation specification

**Status:** Specified, not implemented (2026-10-28).
**Repository / branch:** `Reliquary-crate-first`, `refactor/crate-first-restart`.
**Baseline:** last committed product `a2313af7`. Vectors service implementation is independently active and **uncommitted** in this working tree. Its verified source cut and consumer rewiring are the intended lower service input, **not** a license to include any Vectors edits in this specification commit.
**Target:** `crates/reliquary-retrieval/` (currently a placeholder with no production code).
**Objective:** A behavior-preserving hard cut of deterministic Retrieval policy and derived indexes out of product Root. No new retrieval product, search algorithm, inference call, storage file, corpus migration or permission mechanism.

## Purpose and problem

Today Retrieval's lexical indexing, token scoring, hybrid fusion/diversification, vector scoring, Community routing, Graph traversal, complete agentic candidate union and public result models reside across `src/retrieval/`, `src/runtime_inference/` and the root `Cva`/`Phylactery` package. Numerous engine functions are `impl Cva` or take a concrete `GraphOwner`/`Container` or root-owned error. Consequently changes to ranking or routing depend on owner I/O, transaction APIs, provider changes, and runtime call sites, violating the intended small-blast-radius architecture.

Extract the deterministic search engine into `reliquary-retrieval`, with **caller-supplied authorized facts** and canonical lower-crate types. The engine may own disposable indexes and calculation results; it must **never** own Memory/Archive content, Graph relationships, vector generation authority, user/REL grants, accepted-use receipts, agent state lifetime, or model transport. Existing product entrypoints keep their signatures and delegate to the service.

**Developer outcome:** alter a lexical scorer, hybrid fusion or Memory routing/traversal strategy inside one crate, run its focused tests, and integrate without edits to Container, Archive, Memory Core, Graph, Vector Store, Vectors, Freshness, or unrelated services.

## Baseline and authoritative rules

- [Crate-first architecture](crate-first-architecture.md), [layout](crate-first-layout-spec-2026-10-25.md), [crate map](architecture/crate-map.json): compiled dependency direction is down. Retrieval may consume **only actual needed** approved lower crates: Archive, Echo, Freshness, Graph, Memory Core, Perception Store, Switchboard, Vector Store and Vectors. Prefer Archive + Graph + Memory Core + Vector Store + Vectors, and add others only for concrete source contracts. Do not add a Container edge (not permitted), the product root, Gateway, Dream, Ego, Insomnia or higher coordinator. No lower crate imports Retrieval.
- [ADR 0026](decisions/0026-community-routed-memory-retrieval.md) and [Retrieval manual](manual/retrieval.md): Memory/PHY retrieval is owner-local; the cached index is derived; only **Memory-graph** mutations invalidate Community routing. Default 4 subcentroids, top 4 Communities, 4 seeds, 32 Graph budget, depth 3; global fallback and unclustered residual lane; depth always before boundary crossings. No semantic Community summaries or persisted centroids.
- [Agentic Retrieval](agentic-memory-retrieval-design-2026-10-15.md) and [implementation](subsystems/agentic-memory-retrieval.md): full lexical + exact-semantic union before any top-10, owner-qualified Freshness order, excluded already selected results, per-anchor one-hop max 10, deterministic overlap, previous-content-only `already_provided` with **zero new delivery**, no reinforcement on passive reads. The turn/controller/delivery owner remains Runtime/Gateway; see [handoff](agentic-memory-host-handoff-seam-2026-10-16.md).
- Existing hybrid defaults `DEFAULT_LEXICAL_WEIGHT=0.45`, `DEFAULT_SEMANTIC_WEIGHT=0.55`, candidate limit `30`, result limit `10`; single-lane scores remain unscaled, lexical coverage/density calculation unchanged; stable tie ordering, duplicate-source ranges and overlap diversification unchanged.
- [Vectors extraction](vectors-service-crate-extraction-spec-2026-10-28.md): `reliquary-vectors` now supplies embedding contracts and cosine/profile verification; Retrieval is a consumer only. Vector Store owns actual matrices/generations, not Retrieval. The Vectors extraction must be **committed and integrated** before running this migration; refresh the baseline then.
- Current semantic-corpus limitation: the separate `CVAFRS07` six-file admission is not verified; the agentic answer/evidence comparison previously measured **1/128 (0.78%)** recall. Neither is solved or claimed by changing crate ownership.

## Canonical type and responsibility boundaries

| Owner | Must remain authoritative |
| --- | --- |
| `reliquary-retrieval` | Lexical normalization/scoring, disposable posting indexes, pure F32 row scoring, hybrid weighting/dedupe/diversification over supplied overlap data, complete Memory candidate union, deterministic Community centroids/routing, exact Memory seeds, bounded Graph traversal, pure agentic Freshness ordering/candidate slicing. |
| `reliquary-archive` | Canonical `Fragment`/`FragmentId`, source text, branch/source versions, file descriptors, tombstones and Archive error/reads; never copied into an independent Retrieval source of truth. |
| `reliquary-memory-core` | Canonical `MemoryId`, `MemoryBodyId`, current/archived/revision truth and Memory text/provenance. |
| `reliquary-graph` | `CommunityId`, Memory relation and Community snapshot/algorithm authority; active relation/owner eligibility is validated **before** passing a projection to Retrieval. |
| `reliquary-vector-store` / `reliquary-vectors` | Canonical profile/generation/packed-vector types; vector/policy computation. Retrieval can read immutable supplied vector rows, not authorize or mutate bindings. |
| `reliquary-freshness` / Store | Freshness scores and accepted-use event authority. Retrieval can order supplied score snapshots; no Freshness notification or reinforcement event from a query. |
| Product Root `Cva` / `Phylactery` | REL/PHY owner snapshots, validated source versions, source text hydration, active profile/generation checks, Container access, vector row decoding from the authorized owner, durable source provenance, error mapping, query result hydration and public facade methods. |
| Runtime/Gateway | Per-instance grants, REL dependency closure, caller-selected profile and embedding route, model decision and refinement, turn-scoped `CurrentQuery/SearchedIDs/SelectedIDs` controller, concurrent anchor jobs, handoff confirmation and exact accepted-use receipt publication. No Gateway or model API in the Retrieval crate. |
| Echo/Perception Store/Ego | Existing independent evidence visibility, Entity resolution, and Ego selection/audit behavior. Retrieval does not take their persistent owner state or start processing them. |

**Rust ID decision:** Use the exact lower-owner `FragmentId`, `MemoryId`, `CommunityId`, `CompatibilityProfileId`, `VectorGenerationId` and `GraphRelation` where available. No invented parallel ID families, generated reflection layer or projection cache. If root has a type alias with a different public path, make a **direct re-export** from Retrieval/Root; compile-time conversions are localized at existing store boundary and cannot change identity bytes.

**Error boundary:** Root `SearchError`, `SemanticSearchError` and `MemoryRetrievalError` currently include Archive, Memory, Vector and other foreign-owner errors. Preserve their existing public variants and precedence in Root, and give the service a minimal **pure** `RetrievalPolicyError` for bad query/config/index/corrupt *supplied* math. Root maps those through explicit `From` or narrow match arms. Do not import Container into the new crate to move a public error or build a generic translation framework.

**Derived data:** `MemoryRetrievalIndex`, `LexicalIndex` and routing centroids are disposable, caller-owned lifetime, not serialized. To avoid proliferating copies, the **one** authoritative derived-index representation moves to Retrieval and its product API re-exports the same type. Because old fields are partly `pub(crate)`, provide the few necessary constructors/accessors for owner-supplied snapshots; do **not** make all index internals public, mirror the index in Root, or change the six existing public watermark/profile/stat fields.

## Source crosswalk — move policies, not owners

| Current source | Required final action |
| --- | --- |
| `src/retrieval/lexical_search.rs`, `lexical_index.rs`, `lexical_index/{memory,raw}.rs` | Move term extraction, scoring, posting/index management, deterministic tie policies and disposable index into Retrieval. Split existing functions that read `Archive`, `MemoryStore` or `Container`: **Root enumerates/dereferences source records and supplies immutable ordered additions/deactivations**, Retrieval updates its single index; no closure that lets the service fetch arbitrary source content. Preserve incremental update and rebuild-after-reopen semantics for fragment/raw/file/Memory indexes. |
| `src/retrieval/search_model.rs`, `search_rank.rs` | Move `RetrievalConfig`, `SearchCandidate`, weighting, dedupe and pure selection. `Cva::rank_search_candidates` and `fragment_overlap` are currently owner-aware: Root supplies verified overlap counts/membership of known fragment node spans and authorized candidates, then Retrieval performs the **same** iterative diversification in the same order. Avoid calculating all pair overlap values eagerly if it materially changes work/allocations; a narrowly scoped supplied overlap table can be built for the candidate set. No callback trait/dependency on `Cva`. |
| `src/retrieval/memory_retrieval_index.rs`, `memory_retrieval_model.rs`, `memory_retrieval_traversal.rs`, `memory_retrieval.rs` | Move deterministic centroid/routing, GlobalExact seeds, Graph scheduling, config/index model, stale-index **watermark comparison** and output statistics. Root supplies an owned/borrowed current-version snapshot and active Memory-projection edges. Retain exact index invalidation and no-route fallback. `GraphOwner` is **never** passed downward. |
| `src/retrieval/memory_retrieval_vectors.rs` | Split pure `validate_query`, exact scoring and f32 decoding from owner-specific `load_memory_vectors`/`load_searchable_memory_vectors`. Root owns profile checks, Memory body/version/archived eligibility, Store row loading and error mapping. The service receives *current, authorized* ID/vector maps, not container handles. |
| `src/retrieval/memory_candidate_pool.rs` | Move `MemoryCandidate{Evidence,Pool}` and **pure union** of complete lexical and exact-semantic score lanes (stable by Memory ID). Root `Cva::collect_memory_candidate_pool` retains current-profile/active-Memory population checks, then invokes the service. No top-10 truncation before the agentic Freshness lane. |
| `src/retrieval/semantic_search.rs`, `semantic_search_model.rs` | Move the pure packed-row cosine scoring, validation, ordinal tie order, source-filter application and result-model types; **Root** retains endpoint/profile verification and Query embedding, current-generation binding/source authority, packed rows/fragment lookup and error translation. Do not copy the Vectors cosine algorithm and do not make Retrieval start model I/O. |
| `src/retrieval/search.rs`, `archive_search.rs`, `conversation_search.rs`, `memory_search.rs`, `lexical_search.rs` (their `impl Cva/Phylactery` sections) | **Keep Root entrypoints** with public signatures, REL/PHY query-scoping, source/hydration and tombstone checks. Move only current local policy/calculations; remove displaced copies after rewire. Retain Archive-only and branch/path-specific behavior. |
| `src/runtime_inference/agentic_memory_retrieval.rs`, `runtime_host_gateway_agentic_memory_{query,refinement,graph,completion}.rs` | Keep controller lifecycle/staging, owner-qualified `MemoryRef`, token grants, provider/decision calls, concurrency, completion and durable handoff proof at Runtime. Move only their **pure candidate score ordering/limit/filter and graph-neighbor presentation** into Retrieval with explicit owner-qualified score and relation inputs. `SelectedIDs` still changes only after host-success. Never move handoff receipts or model selection into the library. |
| `src/retrieval/{compatibility_profile_*,archive_vector_*,memory_vector_*,packed_vector_*,vector_generation_*}.rs` | **Do not indiscriminately migrate**: these filenames currently contain Root-side owner-store validation, foreign error conversion, version/rebuild coordination, and tests. Persisted types and codecs already live in Vector Store; compatible embedding checks belong Vectors. Retain Root adapters needed for cross-owner admission; delete obsolete duplicate forwarding/proxies only after wiring. |
| `src/retrieval/search_error.rs`, `semantic_search_error.rs`, `memory_retrieval_error.rs` | Root owns product-composed errors; service owns only pure errors. Preserve externally exposed types and meanings. |
| `src/retrieval/*_tests.rs`, `src/runtime_inference/*retrieval*_tests.rs` | Move engine-only unit tests with their policy; retain Root integration, ownership, CLI/Gateway and producer-consumer tests at original seams; no copy of entire suite into both places. |

### Special correctness boundaries

1. **No invisible retrieval limits:** Collect complete lexical/semantic Memory evidence and perform Freshness ranking before the agentic ten-candidate cut. A raw previously selected match may cause `already_provided` only when the operation delivers no new Memory; do not automatically call it relevant.
2. **Passive means passive:** Search, low-level Memory hydration, Graph exploration, agent decision review, previous-results reporting and failed handoff must create zero accepted-use events. Ownership validation precedes any returned content.
3. **Cached index:** Rebuild is caller-owned; compare Memory version, Memory-Graph version, Community generation, selected-profile binding count and subcentroid configuration; an Entity-only Graph mutation is not stale. Index model and its projection data remain one authoritative in-memory implementation.
4. **Semantic score semantics:** Query profile compatibility, positive-score handling, invalid/zero/non-finite row handling, exact F32 little-endian interpretation and stable ordinal/ID ties must match existing results. An empty/invalid matrix never becomes a neutral success.
5. **Graph scheduling:** Active, same-owner, searchable endpoints only. Nonvectorized nonarchived Memories can be reached from vector seeds; Graph-isolated vectorized Memories are not lost through Community routing. Traversal ordering remains depth, then boundary crossings, then stable discovery.
6. **Hybrid/source search:** Preserve original 0.45/0.55 rank blend, single-lane scores, max result limits, duplicate fragment range collapse, conversation overlap attenuation and exact current Archive tombstones/branch snapshots. Root performs final Archive text hydration.
7. **No provider work under an owner lock:** Retrieval never embeds or decides. Runtime prepares owner snapshots, releases locks for endpoint/decision I/O, revalidates owner cuts before selection/delivery.

## Migration — one staged hard cut, deliberately sliced

Do not start until **Vectors is committed** and the root workspace is green; then record the updated commit and a clean `reliquary-vectors` import API. Treat each stage as one small, reviewable move. A bounded red migration window is allowed, but no second Retrieval engine, forwarding module or large temporary cross-crate adapter may survive the cut.

1. **R1 — Inventory and freeze executable behavior.** Inspect the physical import/use map (ordinary `rg` and Cargo metadata, not an architecture analyzer), the `src/lib.rs` public exports and each REL/PHY/Gateway caller. Capture fixed lexical/hybrid scores, source-range tie order, Community route/GlobalExact fallback and `MemoryRetrievalIndex` watermark behavior, complete Memory candidate union, archived-Memory exclusions, F32 exact scores, owner/PHY isolation, agentic Freshness ordering and consumed-handoff neutrality. Capture known pre-existing limitations without changing fixture data.
2. **R2 — Compiled engine models and lexical policy.** Add only needed lower Cargo dependencies. Move the lexical tokenizer, scoring/postings and derived-index engine with existing tests. Replace Archive/Memory/Container calls with narrow ordered source additions, archived/deactivated flags and exact owner snapshot versions prepared by Root. Make public and private index APIs just sufficient for existing consumers; compile Retrieval focused tests and preserve the full-scan/index parity fixtures.
3. **R3 — Semantic and hybrid engine.** Move pure row decode/cosine/ordinal scoring, default configs, rank fusion, dedupe/diversification and related result models. Reuse Vectors' cosine for comparable float slices; preserve the existing byte-row implementation if necessary to avoid a full packed-matrix copy. Keep source access/overlap proof, embedding calls, current-generation checks and composed errors at Root. Pin numeric and golden tie outputs before deleting old code.
4. **R4 — Memory routing and indexed search.** Move `MemoryRetrievalIndex`, centroid generation, global/routed candidate choice, exact fine scoring, traversal and stats. Root builds an authorized immutable Memory+Graph snapshot and supplies version/watermark proof; service never imports `GraphOwner`. Include REL/PHY reusable-index tests, stale profile/bindings/Memory-Graph changes, Entity-only no-stale case, residual unclustered lane and depth-over-boundary regression.
5. **R5 — Agentic candidate evaluation.** Move complete lexical+semantic union and pure Freshness ordering/limits/presentation into Retrieval. Gateway continues to stage turns, route decisions, hydrate content, detect authorization drift, manage concurrent anchors and confirm handoff. Include no-top-10-before-ranking, more-than-ten-neighbor traversal, overlapping anchors, `already_provided` fallback, selected exclusion, and no reinforcement before successful host completion. Move no model inference or context-budgets.
6. **R6 — Integrate owners and delete old algorithms.** Replace Root engine calls in Archive/Memory/PHY/semantic/hybrid branches, file search, conversation search, agentic Gateway, Ego/Perception/Insomnia consumers. Preserve `Cva` and `Phylactery` public APIs, Root error variants, owner watermarks and provider validation order. Delete algorithm copies from Root and leave only concrete owner adapters and product error projection. Demonstrate one index and one scorer per policy; no API alias maze.
7. **R7 — Verification, docs, evidence and commit.** Run final gates; update `docs/manual/retrieval.md`, subsystem/agentic code maps, architecture/maintainer/API references, and doc ledger. Capture actual test results and any restrictions. Stage **only** Retrieval changes; preserve the pre-existing Graph evidence work and other unrelated edits. Commit after integration, not after compiling the placeholder.

**Work slicing rule:** R2–R6 are ordered *work packages*, not giant all-in-one implementation tasks. Split each into independently reviewable source moves (e.g. lexical token/scoring; index; pure hybrid; semantic scorer; centroid; traversal) and run the corresponding current test seam after each. No new analyzer/ratchet, speculative trait family, multi-file algorithm copy or mass generated test suite.

## Verification and acceptance

### Focused service tests
- `cargo test -p reliquary-retrieval --locked --offline -j 1` after implementation. Must exercise public/integration seams as well as relocated private tests. A placeholder test pass is not acceptance.
- Lexical: Unicode token normalization, duplicate-term suppression, coverage/frequency caps, raw/source/file/Memory posting parity, incremental source additions/deactivations, reopen/rescan identity, deterministic Archive version and Memory-ID ties.
- Hybrid: 0.45/0.55 defaults, 30/10 defaults, single-lane behavior, source duplicate range collapse, stable fragment IDs, overlap diversification under ordered owner-supplied evidence, no accidental early result truncation.
- Semantic/Vector: exact packed-row byte scoring and ordinal sort, no full matrix clone, malformed/NaN/zero/wrong-width rejection, selected-profile dimension validity, generation/source filtering and positive-score logic.
- Community: deterministic four-per-community centroid, selected four Communities, equal-score routing identity and residual lane, exact/global fallback, stale index on Memory/Memory-Graph/Community/bindings change but not Entity-only semantic change, Graph depth before Community boundaries, no archived source leakage.
- Agentic passive: complete raw candidate union, Freshness score with missing=neutral per current contract, owner-qualified stable ties, top ten only after full ranking, query-state and traversal exclusions, per-anchor ten independent, overlap reporting, no reinforcement until confirmed delivery; existing controller tests stay in Runtime.

### Product integration checks
- Existing `search_policy`, `lexical_index`, `search_archive`, `search_conversation`, `semantic_search`, `search`, `memory_retrieval`, `memory_candidate_pool`, `memory_lexical` and `community_routing` test groups.
- REL and PHY search/retrieval both compile and behave identically; error variants and current-public Rust entrypoints are source-compatible. Archived/tombstoned Memory/source records do not appear unexpectedly.
- Runtime/Gateway agentic `memory_query`, `refine_query`, `traverse_graph`, per-instance authorization, accepted-use handoff, cancellation/retry, exact-once receipt and no cross-owner leaking, with selected/declined controller state unchanged by failed work.
- Ego and Perception retrieval consumers, historical replay/rewrite and source provenance expansion retain their existing query/read behavior; no unintended dependency on a Retrieval-internal type.
- Compare frozen 14-day, 28-day and Ellis fixture eligibility/rank/evidence identities where those fixtures are available; do not alter them. Treat prior 1/128 agentic evidence recall as a separately **known product-quality limitation**, not a crate-extraction regression to conceal.

### Final executable gates

```text
cargo test -p reliquary-retrieval --locked --offline -j 1
cargo test -p reliquary-vectors --locked --offline -j 1
cargo test -p reliquary-vector-store --locked --offline -j 1
cargo check --workspace --all-targets --locked --offline -j 1
cargo test --lib --locked --offline -j 1
cargo test --manifest-path cli/Cargo.toml --locked --offline -j 1
cargo fmt --all -- --check
ddocs check --indexes --links
git diff --check
```

Run relevant tests with owner fixtures and allowed local loopback as necessary, record exclusions (notably slow measurement cases), and distinguish independently verified evidence from historical reports. Ordinary `rg` and Cargo metadata suffice to prove the old search engines are absent and dependencies follow the crate map. If Arcana is current, use it as **supplemental** structural evidence; a stale Arcana snapshot must not block the Rust/Cargo-enforced boundary or justify building another architecture analyzer. Existing unrelated documentation-policy and `CVAFRS07` acceptance limits remain separately reportable.

**Completion requires all of:** one compiled retrieval engine and derived-index implementation; root uses it rather than a copied algorithm; no product/Container/Gateway/Cva import in `reliquary-retrieval`; no reverse lower-crate dependency; no new durable state, inference calls or reinforcement; existing public REL/PHY and agentic behavior preserved; relevant test and doc gates pass; docs reflect source; commit includes exactly the integrated extraction.

## Non-goals

No new retrieval heuristic or evaluation-model prompt, no reweighting based on Freshness, no semantic-only third search path, no alternate ranking schema, no new Graph/Community projection, no optimizer or ANN backend, no persistent retrieval index, no proactive Ego changes, no new multi-owner query authority, no corpus migration/compatibility reader, no additional model route, no change to host acknowledgment semantics, and no general architectural tooling.

## Related documents

- [Crate-first architecture](crate-first-architecture.md) and [crate map](architecture/crate-map.json)
- [Vectors extraction spec](vectors-service-crate-extraction-spec-2026-10-28.md)
- [Retrieval operator manual](manual/retrieval.md)
- [ADR 0026](decisions/0026-community-routed-memory-retrieval.md)
- [Agentic retrieval design](agentic-memory-retrieval-design-2026-10-15.md) and [implementation](subsystems/agentic-memory-retrieval.md)
- [Agentic host handoff seam](agentic-memory-host-handoff-seam-2026-10-16.md)
- [Freshness retrieval boundary](freshness-external-consumption-contract.md)

## Notes

This is an **implementation specification**, not a declaration that the Retrieval crate exists beyond its placeholder or that the Vectors source cut is committed. Avoid trading implementation completeness for a clean module list; the migrated engine must be the one called by real Root and Gateway paths.
