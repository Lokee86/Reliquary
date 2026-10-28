# Vectors service crate extraction — implementation specification

**Status:** Specified, not implemented (2026-10-28).
**Repository / branch:** `Reliquary-crate-first` · `refactor/crate-first-restart`.
**Specification baseline:** `0cfba254` (Freshness service extraction specified). Freshness extraction is concurrently in progress in the working tree; **do not modify, stage, reset or merge its uncommitted work**.
**Target:** `crates/reliquary-vectors/` — currently a placeholder.
**Scope:** Behavior-preserving, compiled service hard cut after the established Vector Store owner extraction. No new embedding model, provider, numerical policy, persisted schema, search strategy, REL/PHY authority or release feature.

## Purpose

Turn the root-owned model-embedding and compatibility policy into an independent, deep Vectors service. Today `src/runtime_inference/embedding_endpoint.rs`, `src/retrieval/compatibility_{vector,profile_probe,profile_model}.rs`, duplicated embedding-matrix assembly and detached vector workers span Root, Retrieval, Archive, Memory and Switchboard. A local change to validation or probing currently risks changing many unrelated modules. The final crate must own deterministic **embedding execution contracts, probe policy, batch validation and matrix construction**, while the Vector Store remains the only durable vector owner and the product retains owner authorization/transactions.

This is **not** a second Vector Store. The previous extraction implemented `reliquary-vector-store` with Container-backed packed/profile/Archive/Memory/generation codecs, replay and store APIs. Its documented architecture and corpus qualifications remain in [the Store spec](vector-store-crate-extraction-spec-2026-10-26.md) and [evidence](vector-store-crate-extraction-evidence-2026-10-26.md).

## Overview and expected developer outcomes

The final `reliquary-vectors` crate should let a developer change compatibility evaluation, embedding response validation or pure f32 packing without modifying Container, Store, Archive, Memory Core, Retrieval or Runtime. Existing REL/PHY methods and Switchboard endpoints should use its narrow interface. Changes requiring owner-side writes remain in explicit higher coordinators.

This eliminates the root-only `EmbeddingEndpoint` trait/implementation, root compatibility probe algorithm and duplicated f32-to-packed assembly, while keeping:
- byte-for-byte existing compatibility-profile IDs, probe strings/order, versioned policy, threshold and normalized vector behavior;
- exact packed f32 row ordering, row count, dimension/scalar validation and existing error precedence at public methods;
- active profile selection/retirement and vector-generation commit under the held owner;
- Retrieval's scoring/ranking/authorization and Dream/Insomnia/F8 consumer decisions in their own domains.

### Frozen responsibility boundaries

| Concern | Final owner |
| --- | --- |
| Provider-specific HTTP/credentials, route choice, retries/limits, endpoint configuration and auth | **Switchboard**, unchanged; Vectors is a typed consumer, not a second transport. |
| `EmbeddingEndpoint` trait, deterministic simulated endpoint, Switchboard adapter implementation, Query/Document embedding invocation contract | **Vectors service**. It may import Switchboard's `EmbeddingEndpointError` and provider interface, but cannot load credentials/config itself. |
| Deterministic compatibility probe list, profile construction from observed endpoint, 0.99 cosine policy, batch/dimension/normalization validation, pure packing | **Vectors service**, using canonical Vector Store profile types. |
| `CompatibilityProfile`, `CompatibilityProfileId`, `EmbeddingMode`, `VectorNormalization`, persisted probe references and IDs | **Vector Store**, already canonical. Service imports/re-exports, never duplicates persisted representations. |
| Packed objects, Archive/Memory bindings, profiles/retirement, generations and replay | **Vector Store**; no relocation, wrapper store, additional journal or independent persistent index. |
| Archive fragment text/version/existence, Memory body text/provenance, current/active bindings, source-cut and generation admission | **Archive/Memory owners and Root Cva/Phylactery coordinators**. Vectors receives immutable supplied facts, not broad stores or a mutable Cva. |
| `Cva::build_archive_vector_generation`, `Cva/Phylactery::build_missing_memory_vectors`, `publish_vector_generation`, profile enrollment/retirement, Container append/sync | **Root owner facade**, still required for multi-owner authorization and durable write sequencing. |
| Worker locks, cancellation, source-version revalidation, background retries, status/events, configuration and synchronous/async integration | **Runtime/Gateway** and existing source coordinator. No new workers/schedulers. |
| Search candidate eligibility, hybrid rank fusion, exact scoring and top-k, external-source source-authorization/hydration, Dream/Insomnia selection, F8 reinforcement receipts | **Retrieval/External Sources/Dream/Insomnia/Freshness** respectively; Vectors supplies query/document embeddings and validation, not selection/side effects. |
| Historical import and rewrite profile selection, complete-generation barrier, admissible timeline and fixture/corpus policy | **History/Import/Maintenance/Root**, with existing Vector Store and Vectors service helpers only. |

## Governing contracts and dependencies

- [Crate-first architecture](crate-first-architecture.md), [layout](crate-first-layout-spec-2026-10-25.md), [crate DAG](architecture/crate-map.json): Vectors has at most `reliquary-archive`, `reliquary-memory-core`, `reliquary-switchboard` and `reliquary-vector-store` as direct internal dependencies. **Use only those actually required.** The minimum target is **Switchboard + Vector Store**, plus existing external `sha2` and `lodestone_packed` where exercised. No root package, Retrieval, Runtime, Dream, Freshness, History or Container dependency; no lower crate imports Vectors.
- [Vector Store extraction](vector-store-crate-extraction-spec-2026-10-26.md), [Storage format](storage-format.md#packed-vectors): store-owned immutable identities, profile/version/replay and actual packed bytes remain unchanged.
- [Semantic maintenance](subsystems/semantic-maintenance.md#compatibility-profiles-and-vectors), [API](api.md), [behavioral contracts](behavioral-contracts.md): externally observable REL/PHY API, error variants, source/visibility rules, current-generation semantics, safe retries, retirement and no-op/idempotence remain intact.
- [F8 semantic policy](freshness-live-semantic-policy-contract.md): vector-generation completion may trigger intrinsic F8 under the existing root coordinator. Vectors cannot take Freshness ownership, credit memory, or start a semantic worker.
- [Current limitations](current-limitations.md#crate-first-recovery-boundary): `CVAFRS07` six-file corpus admission is a separate incomplete product gate; no claim that extracting Vectors recovers this format.

### Critical Rust trait-coherence decision

`EmbeddingEndpoint` currently lives in `src/runtime_inference/embedding_endpoint.rs` and is implemented there for the foreign `reliquary_switchboard::OpenAiReadyEmbeddingEndpoint`. **Move the trait and that concrete implementation together into `reliquary-vectors`**, where the trait is local and implementing it for a Switchboard type is legal. Move the existing `SimulatedEmbeddingEndpoint` and its deterministic hash/normalization behavior with it. Root `src/lib.rs` re-exports those **same** types; it does not define a duplicate trait, a mirror provider, or a bridging proxy. Existing test/end-user implementations of the public trait must compile unchanged.

Switchboard's `EmbeddingEndpointError` remains canonical and is re-exported; `EmbeddingMode` and `VectorNormalization` continue to be re-exported from Vector Store. The current provider-mode and normalization conversions live alongside the new local trait implementation. Preserve `embed`, `embed_bounded`, `supports_bounded_embedding`, `dimensions`, `normalization`, response limits and error variants. Do **not** move provider network code out of Switchboard.

The root `CompatibilityProfileError` includes **Container** errors and existing Store error conversions, which this pure service must not depend on. Do not drag Container into Vectors merely to move that error enum. The service uses a narrowly named **probe/validation error** describing only endpoint failure, invalid profile/vector and size overflow, with an explicit mapping at existing Root error boundaries to the current `CompatibilityProfileError` variants. Preserve error discrimination and prior validation order; avoid a general error-wrapper framework. The pure `CompatibilityReport` and `COMPATIBILITY_*\u0060 policy constants move to Vectors; the store `CompatibilityProfile` remains untouched. (Use the original numeric versions/threshold; no recalibration.)

### Exact source ownership map

| Existing source | Migration decision |
| --- | --- |
| `src/runtime_inference/embedding_endpoint.rs` | Move the **one** trait, deterministic simulation and Switchboard adapter into `crates/reliquary-vectors/src/embedding_endpoint.rs`. Root re-exports directly. |
| `src/retrieval/compatibility_profile_probe.rs` | Move probe inputs, endpoint profile measurement, verification and cosine comparison to service; use the Store's profile hash and types; keep version/digest and comparison order. Root's `Cva/Phylactery` coordinator still chooses among admitted, unretired stored profiles. |
| `src/retrieval/compatibility_profile_model.rs` | Move `CompatibilityReport` and policy-only constants; retain only necessary root public re-exports, not a mirrored model. Stored profile/value types remain Store-owned. |
| `src/retrieval/compatibility_vector.rs` | Move vector finite/norm/cosine/batch checks into service. Retrieval and Dream import the exact math directly, without their own cosine clone. Their ranking algorithms and policies stay higher. |
| `src/retrieval/vector_generation_validation.rs` | **Split by authority, not by filename:** `validate_generation_reference` and `has_complete_current_coverage` stay Root because they inspect Archive truth and the admitted Store; `vector_generation_id` already delegates to Vector Store and must not be duplicated in service. Move only genuinely pure service-level checks if reused; do not force extraction of mixed cross-owner validation. |
| `src/facade/cva_{compatibility_profiles,vector_generations,memory_vectors}.rs`, `phylactery_{compatibility_profiles,memory_vectors}.rs` | Keep owner `impl Cva/Phylactery` and Container transactions in Root; call service probe/validate/pack helpers. Preserve all public IDs, return types, authorization and error precedence. |
| `src/runtime_inference/runtime_vector_step.rs` | Keep REL/PHY prepare and commit, mutable ownership, current Memory-body selection and retry race checks. Replace only repeated batch validation/f32 packing with the service helper, preserving buffer ownership and row correspondence. |
| `src/facade/cva_semantic_live.rs`, `src/facade/historical_embedding*.rs` | Keep F8 and historical import source proof, off-lock embedding and publication. Use service-owned vector checks/packing when semantics match; no F8 policy change or unguarded publication. |
| `src/runtime_inference/configured_runtime_vectors.rs`, `runtime_host_{rel,phy}_vectors.rs` | Keep route construction, runtime lock/lease, batching, retries, cancellation and event publication. Their configured endpoints implement the moved trait; remove no legitimate host adapter solely for style. |
| `src/retrieval/{memory_retrieval_vectors,semantic_search,memory_candidate_pool,...}.rs` | **Keep in Retrieval**: current searchable Memory population, query/candidate eligibility, exact score/ranking, source ordering and results. Import service cosine/compatibility math, not a new vector-search policy. |
| `src/runtime_inference/dream_owner_vectors.rs`, `src/external_source/index.rs` | Keep Dream source selection and external host binding authority. Reuse service numeric helpers if identical; don't make Vectors read arbitrary host transcripts or search globally. |
| `src/retrieval/{archive_vector_store,memory_vector_store,compatibility_profile_store}.rs` | Keep only actual **Root foreign-owner validation**, memory/profile admissibility and public error projections; durable state belongs Vector Store. Do not pull these Root wrappers into service or add duplicates. |
| `crates/reliquary-vector-store/**` | **Leave unchanged**; it owns the five durable vector families, profile/generation IDs and exact codecs. |

### Shared f32 batch assembly (one pure operation)

Several Root paths currently perform near-identical `Vec<Vec<f32>>` validation followed by f32 little-endian packing and `PackedVectors::from_bytes` (Archive generation, REL/PHY missing Memory, runtime commit and F8 archive batch). Provide **one** pure Vectors operation taking a validated profile or its immutable schema and the ordered rows. It checks finite values, row count, fixed dimensions, normalization, checked capacity and f32 schema; it returns exactly the existing `PackedVectors` bytes in input row order and a small service-local error. Root maps errors to its existing public `MemoryVectorError`, `VectorGenerationError` or `CvaError` and performs all authorization/append/sync.

**Preserve known differences** where a path intentionally validates different preconditions or returns before packing. Do not force a central helper if it changes error precedence, embeds under a mutex, reorders rows, or allocates an extra full-matrix copy. Keep exact stable f32 encoding and equivalent peak working-set behavior. A partial pure helper is preferable to moving a Root transaction.

## Implementation strategy — staged hard cut

An intentional, bounded red integration window is permitted between the first code move and Root consumer rewire. Make coherent slices; no temporary duplicated modules, forwarding trait, adapter proxy, parallel writers, service registry, new graph proof system or compatibility reader.

1. **V1 — Freeze and inventory.** Record committed HEAD and dirty paths (Freshness work must remain untouched), existing public `EmbeddingEndpoint` surface, `COMPATIBILITY_*` constants, representative profile IDs, probe ordering, exact f32 bytes, public errors and current REL/PHY tests. Identify all `compatibility_profile_probe`, `compatibility_vector`, `EmbeddingEndpoint` consumers by ordinary source search. Freeze a fixed simulated-seed probe and a drift threshold on both sides of 0.99; preserve the present v2-profile reopen under v3 policy.
2. **V2 — Build the independent endpoint contract.** Add only actual Cargo dependencies. Move the trait, simulated endpoint and Switchboard provider implementation together. Reuse lower Store `EmbeddingMode/VectorNormalization` and Switchboard error; validate direct provider-mode/normalization conversions and bounded response behavior. Compile lower crate and its migrated tests; Root may temporarily be red until V5. No root-dependent trait or provider shim.
3. **V3 — Move compatibility policy and mathematical validation.** Move probe inputs, profile construction/verification, pure comparison/report and cosine/finite/vector validation, with policy constant and exact-error handling. Do not relocate Store profile IDs/digests. Add focused public-interface tests for exact profile ID, provider incompatibility/threshold, dimensions/normalization, malformed output and policy version. Root's Store compatibility selection uses the service comparison as an input; it remains the authorized existing-profile selector.
4. **V4 — Consolidate pure batch packing.** Move only repeated validation/encoding that belongs to the Vectors service; use a single `PackedVectors` type from `lodestone_packed`. Preserve row-to-Fragment and row-to-MemoryBody mapping, checked byte sizes, scalar, finite values, error order and no new intermediate matrix copy. At a local gate, compare golden bytes, generated content IDs and simulated probes against the frozen baseline.
5. **V5 — Rewire real consumers; delete displaced algorithms.** Update Cva/Phylactery, configured/runtime vector workers, F8, historical embedding, Retrieval/Dream query-vector consumers and root `pub use` to import the service. Keep `validate_generation_reference` at the Root foreign-owner admission boundary. Remove the old root embedding trait implementation, probe/cosine engine, and actual copied packing logic; preserve pure local error mapping only where Root owns the error type. Prove no upward crate dependency, dual provider invocation or durable writer. Do not fold Retrieval, external host indexing or runtime scheduling into this crate.
6. **V6 — Product verification, documentation and commit.** Verify focused lower tests, related Root/CLI coverage, workspace compile/format, links and public examples. Update current code maps and store/service distinctions in owner docs. Record genuine outcomes; stage/commit only this extraction after the active Freshness work is separately integrated. Do not stage uncommitted Freshness changes or unrelated Graph evidence.

This spec is implementable independently of Freshness's unfinished service cut, but its **integration baseline must be refreshed after Freshness commits**. No implementation agent may absorb another agent's working tree as part of a Vectors commit.

## Verification and completion gates

**Service unit/contract checks**
- `cargo test -p reliquary-vectors --locked --offline -j 1` after implementation (the placeholder's current test result is not acceptance).
- Compatibility: fixed seed and exact probe strings/order/profile digest, `COMPATIBILITY_PROBE_SUITE_VERSION == 1`, `COMPATIBILITY_POLICY_VERSION == 3`, `COMPATIBILITY_MIN_COSINE == 0.99`, v2 stored profile still reopens, compatible drift and incompatible endpoint both behave as before.
- Provider trait: external consumers can implement the re-exported `EmbeddingEndpoint`; Switchboard adapter and simulated endpoint preserve query/document modes, dimensions, L2, transport errors, zero/nonfinite/invalid-size rejection, bounded response and no surprise network attempts.
- Pack/validate: F32 little-endian output, ordered rows, exact `PackedVectorId` on representative input, positive/negative/zero/un-normalized vectors according to policy, count/width/NaN/infinity/overflow errors without writes. No duplicate f32 packing in REL/PHY/F8/generation code paths when shared helper applies.

**Root integration checks**
- REL/PHY compatibility profile establishment, verification, reuse, retirement and current coverage. Preserve error variants, Store idempotence and unaffected global/Archive/Memory clocks.
- Archive generation publication: accepted Archive version, nonempty and complete coverage, current generation, proper source binding, wrong scalar/dimension/incomplete source rejection and exact vector version ordering.
- Memory vector creation: current body IDs, missing-only selection, async/profile-change retry, row alignment, no rebinding; REL and PHY share identical pure packing but retain distinct owner transactions.
- Existing Retrieval and search tests: stored/query vector validation, passive/no-Freshness reinforcement, exact scores and stable tie ordering, semantic/hybrid/agentic candidates unchanged; Dream vector loading and F8 intrinsic processing remain operational.
- Historical/offline import, rewrite/replay and external index generation consume exact existing rows and current format. Failures before an authorized commit must append no partial event; reopened/retired or incompatible profiles must not be resurrected.
- Configured runtime/CLI with Switchboard real or controlled local endpoints, configured probe and background worker lifecycle; preserve cancellation/retry/error mapping. No root helper calls provider secretly while owner locks are held.

**Final commands** (run only at coherent integration gate; report actual results, known ignored tests and any pre-existing restrictions):

```text
cargo test -p reliquary-vectors --locked --offline -j 1
cargo test -p reliquary-vector-store --locked --offline -j 1
cargo check --workspace --all-targets --locked --offline -j 1
cargo test --lib --locked --offline -j 1
cargo test --manifest-path cli/Cargo.toml --locked --offline -j 1
cargo fmt --all -- --check
ddocs check --indexes --links
```

Run the existing `compatibility_profile`, `vector_generation`, `archive_vector`, `memory_vector`, `semantic_search`, `memory_retrieval`, `dream`, `semantic`/F8 and runtime embedding test filters as appropriate. Inspect Cargo metadata and displaced paths with ordinary search; use existing Arcana evidence only when current, without refreshing an expensive stale graph just to satisfy a spec. Validate that `reliquary-vectors` has no direct dependency on Root, Retrieval, Freshness, Dream or Container.

**Not complete until:** the real trait, compatibility policy, validation and pure matrix construction are implemented in Vectors; Root/CLI consume that one implementation; root algorithm copies are deleted; Vector Store retains sole durable ownership; public APIs, semantic identities and bytes remain unchanged; final relevant checks pass and evidence/commit correspond to the exact integrated tree. The **separate six-file `CVAFRS07` product gate remains unverified** unless explicitly run and recorded independently.

## Out of scope

No new semantic/vector search algorithm, ANN index, model selection policy, embedding API shape, calibration of 0.99 cosine, shift of Retrieval ranking or Memory authority, cross-REL/PHY embeddings, fresh provider transport, model inference loops, storage/corpus migration, change to actual REL/PHY host leases, rewrite of Dream/F8, extra independent release, generic capability registry, or novel trait hierarchy.

## Related docs

- [Vector Store specification](vector-store-crate-extraction-spec-2026-10-26.md) and [evidence](vector-store-crate-extraction-evidence-2026-10-26.md)
- [Crate-first architecture](crate-first-architecture.md), [layout](crate-first-layout-spec-2026-10-25.md), [crate map](architecture/crate-map.json)
- [Semantic maintenance](subsystems/semantic-maintenance.md#compatibility-profiles-and-vectors), [Storage format](storage-format.md#packed-vectors)
- [API](api.md), [Behavioral contracts](behavioral-contracts.md)
- [F8 semantic Freshness contract](freshness-live-semantic-policy-contract.md)
- [Current limitations](current-limitations.md#crate-first-recovery-boundary)

## Notes

This is a specification, not an implementation result. The six stages describe bounded dependency order, not six simultaneous agents. A required source or policy invariant cannot be relaxed merely to simplify the extraction. Keep the public surface small and retain existing tested ownership instead of moving source files indiscriminately.
