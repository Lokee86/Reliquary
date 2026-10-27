# Vector Store crate extraction — implementation specification

**Status:** Specified; not implemented.
**Repository:** `C:\!bin\workspace\Reliquary-crate-first`
**Branch:** `refactor/crate-first-restart`
**Baseline:** committed Freshness Store extraction `310b6664` (confirm HEAD and worktree before implementation).
**Target:** `crates/reliquary-vector-store/`
**Scope:** correction-only, one compiled owner-store hard cut. No vector policy, search, embedding, durable-format, or roadmap changes.

## 1. Purpose and result

Extract the real durable Vector Store from the root product into a compiled lower-level crate. It owns accepted packed-vector objects, Archive/Memory vector row bindings, compatibility-profile records and retirement, vector-generation records, codecs, replay projections and owner-local lookups. Container continues to own the only physical persistence system. The higher Vectors service selects models, embeds, evaluates compatibility, validates foreign Archive and Memory authority, and coordinates publication. Retrieval consumes authorized read results but never becomes a second vector writer.

**Mandatory direct Reliquary dependency:** only `reliquary-container`. External dependencies are permitted only when actually required by the moved implementation (for example `lodestone_packed`, `sha2`). Do not introduce edges to Archive, Memory Core, Retrieval, Vectors, Owner Lifecycle, Runtime, History, Import or the root package. See `docs/architecture/crate-map.json` and `docs/crate-first-layout-spec-2026-10-25.md`.

The existing `crates/reliquary-vector-store/` is a placeholder, not a completed extraction.

## 2. Source ownership — concrete inventory

Migrate and consolidate the existing owner-local modules in `src/retrieval/`:

- `packed_vector_{model,codec,error,store,rebuild,tests}.rs`: native packed payload objects, hash/identity, retrieval, accepted prune/retention state and replay.
- `archive_vector_{model,codec,error,store,rebuild,tests}.rs`: durable Archive-vector row mapping, object identity and projections.
- `memory_vector_{model,codec,error,store,rebuild,tests}.rs`: immutable Memory-body vector mapping, profile binding uniqueness and replay.
- `compatibility_profile_{model,codec,error,store,rebuild,tests}.rs`: persisted profile identity, validation, retirement and projections. Split `compatibility_profile_probe.rs`: canonical persisted descriptor/identity validation needed for acceptance stays lower; provider/model probing and higher compatibility decisions stay above.
- `vector_generation_{model,codec,error,store,rebuild,tests}.rs`: durable profile-to-generation identity, per-owner version sequence, source version constraints and replay. Split `vector_generation_validation.rs`: deterministic persisted identity and invariant checks lower; authority to publish a source Archive version higher.
- Owner-local storage portion of `compatibility_vector.rs` only if it is an actual persisted codec/value consumer; do not move embedding/comparison policy merely because the file lives beside these stores.

Inspect root exports, owner-open scanning/rebuild, rewrite/prune, facade publication/read, historical inspection, semantic retrieval and detached CLI call sites before editing. Move the implementation, not broad wrappers; remove displaced root stores and duplicate codec/model definitions after the hard cut. Keep public product signatures stable.

**Leave higher:** `archive_search`, `memory_retrieval*`, `search*`, `lexical_*`, `semantic_search*`, `conversation_search*`, embedding generation, model/provider calls, candidate ranking, source selection and authorization. Retain cross-owner orchestration under existing root/Vectors services until their own extraction.

## 3. Known upward couplings: mandatory pre-move corrections

The source inventory confirms these specific problematic paths:

| Current coupling | Required correction |
| --- | --- |
| `archive_vector_store.rs::put` imports root `Archive` and uses Archive/Fragment eligibility to validate mappings | Move Archive existence, accepted source version and fragment provenance checks to the higher caller before append. Lower store accepts a finite, authenticated mapping fact containing the exact IDs, ordered row list and accepted source version; still validates shape, hash, row count, duplicate/collision and immutable record consistency. |
| `memory_vector_store.rs::put` imports root `MemoryStore` and checks Memory-body binding eligibility | Caller checks immutable body existence, revision/binding ownership, profile eligibility and immutable row order. Lower store keeps exact per-profile/body uniqueness, identity, schema/row counts, retry/collision rules. Never give it a MemoryStore or a callback into product state. |
| `archive_vector_store.rs` / `memory_vector_store.rs` reach sibling stores and upper values through `crate::*` | Relocate one canonical lower collection of packed, profile, Archive mapping, Memory mapping and generation stores. Concrete internal cross-store reads are allowed inside this owner crate; do not create a second separate mapping database. |
| `packed_vector_store.rs` and mapping stores import `crate::owner_lifecycle::PayloadUnavailable::Pruned` | Define a narrow store-owned payload-unavailable/pruned error value, map it back to the existing public error at the root boundary. Prune authorization and source-cut checks remain in Maintenance/product. |
| `vector_generation_store.rs::publish` references root generation/profile/Archive IDs and allocates Container global versions | Canonicalize persisted IDs and exact generation codec in lower store, preserving byte layout and monotone vector-version sequence. Higher caller authorizes the source Archive version and requested profile; Container remains the sole allocator for global versions. |
| Store model/codec modules reference root foreign types | Use direct fixed-width wire identities or narrowly store-owned opaque IDs preserving exact original bytes. Convert at root boundaries. Avoid an extra generic ID crate, duplicated large model graphs, or Cargo references to producing owners. |
| Compatibility probe and validation mix wire rules with provider/model policy | Keep only persisted descriptor identity, format checks and accepted retirement semantics in store. Route actual compatibility choice, probe/external provider logic and deployment eligibility to higher Vectors/service callers. |

**No validation gap:** Authorization must occur before the first append. The lower store must independently reject malformed records, mismatched identity/hash, duplicates with conflicting payload, unsupported versions, illegal sequence and corrupted reads. Caller authentication is not a substitute for intrinsic journal validation. Preserve exact prior error precedence and no-append behavior; locate and record any ambiguous checks before cutting code.

## 4. Contract invariants

1. **Single physical owner:** all bytes/objects/versions reside in Container; no independent vector files, append log, index backend or daemon. Owner open/replay performs no implicit write, repair, embedding or provider access.
2. **Existing wire format only:** preserve every current packed, Archive-vector, Memory-vector, profile and generation family byte-for-byte, including content-addressed hashes, schema/scalar type, row order, checksums and version ordering. Reject retired formats; no fallback compatibility readers or on-the-fly migration.
3. **Immutable binding identity:** Archive Fragment and Memory Body foreign IDs must round-trip exactly; row-to-ID correspondence must be stable across reopen and rewrite. Identical retries are no-ops; conflicting identities fail closed.
4. **Profile/generation consistency:** persisted profile digests, retirement, vector-generation ID, global/version clocks and current generation selection preserve existing semantics. Lower replay never chooses a new embedding model or profile.
5. **Prune/rewrite:** explicit higher Maintenance authority, exact pruned/unavailable error, no resurrection of payload or missing mapping, replay preserves valid remaining identities. No eager second vector copy.
6. **Memory/performance:** preserve lazy payload loading, current projections/lookup complexity and bounded rebuild behavior; do not repeatedly clone the full vector corpus. Retrieval does not reinforce Freshness by reading vectors.
7. **Public API:** preserve root/CLI identifiers, accepted search behavior, error mapping and read/write compatibility. Higher search uses exactly the same packed rows/profile bindings and ordering as before.

`CVAFRS07` and the authoritative six-file current-format corpus remain a separate cross-owner product acceptance gate; extracting Vector Store does **not** certify them.

## 5. Execution plan — bounded hard cut

1. **Freeze and map:** inspect branch, commit, and status. The unrelated edit to `docs/graph-crate-extraction-evidence-2026-10-26.md` must not be modified, staged or committed. Enumerate actual root references with `rg`, root exports and Cargo; record the five store families and test locations. Do not run Arcana/Lexicon rebuild merely to count call sites.
2. **Define minimum lower identity/error contracts:** isolate exact persisted ID types, codec values, packed schema and unavailable/pruned error. Record source bytes and golden IDs; test byte round trips before moving stores. Keep existing public root-facing values via explicit conversion/re-export where legal.
3. **Move packed and profile stores:** compile Container-backed packed storage/replay and profile validation/retirement first. Cut `owner_lifecycle` references and provider probe dependencies. Run owner-local codec/reopen tests.
4. **Move mappings:** separate external Archive/Memory validation from lower accepted mapping insertions before moving `archive_vector_store` and `memory_vector_store`. Preserve exact row order, immutability and idempotency, with focused negative tests for bad proofs and duplicate bodies.
5. **Move generations/replay:** relocate generation codec/store/projection and root owner scanner integration. Keep Container allocation and high-level source authorization explicit; verify sequence/reopen/incremental replay and write-free inspection.
6. **Rewire root callers:** migrate facade writers and reads, owner lifecycle, rewrite/reconciliation, historical reads, semantic retrieval and CLI to the single lower crate. Remove old production store definitions, displaced modules and temporary forwarding. No dual read/write path.
7. **Integrate, verify and commit:** update root and crate Cargo manifests/locks, API/architecture/subsystem/maintainer documentation, then run the focused acceptance matrix below. Document results in a separate `docs/vector-store-crate-extraction-evidence-2026-10-26.md`. Commit the completed hard cut without staging unrelated Graph evidence.

Intermediate red builds are permitted only within the declared migration; do not claim completion on lower-crate green while root/CLI consumers remain broken. Report first compiler error and outstanding exact caller conversions during the cut.

## 6. Acceptance matrix

**Crate boundary:** `cargo check -p reliquary-vector-store --locked --offline`; `cargo test -p reliquary-vector-store --locked --offline`. Verify only Container appears in its direct internal Cargo dependencies, no source imports upward, no duplicate root vector journal/projection or score/embedding pipeline inside store.

**Storage/wire:** golden byte and ID equivalence across packed/profile/Archive mapping/Memory mapping/generation formats; full and prefix replay; profile retirement; current-generation selection; collision, corruption, unsupported version, truncation, duplicate body binding and exact retry; payload prune/unavailable and source-cut behavior.

**Higher product:** focused root tests named `packed_vector`, `archive_vector`, `memory_vector`, `compatibility_profile`, `vector_generation`, `semantic_search` and `memory_retrieval`; also owner open, historical/rewrite/prune/reconcile and relevant source-provenance tests. Preserve expected public API compile and end-to-end search results. Prefer targeted filters to redundant expensive full builds.

**Integration:** `cargo check --workspace --locked --offline`; `cargo fmt --all -- --check`; `cargo test --manifest-path cli/Cargo.toml --locked --offline`. Perform one appropriate scoped integration/reopen test against a real owner. Distinguish recorded implementation results from independently reproduced results.

**Completion:** actual production owner in compiled lower crate; root consuming it; all five durable families and exact replay present; no reverse Cargo edge; no duplicated owner state; production-facing tests, CLI and workspace compilation pass; commit and evidence recorded. `CVAFRS07` remains explicitly unverified unless independently executed.

## 7. Exclusions

Do not implement `reliquary-vectors` (higher embedding/generation service), Retrieval, Memory/Archive refactors, History, Owner Lifecycle, an upgraded index, new vector format, a profile redesign, query ranking changes, model provider changes, current-format corpus recovery, or broad architecture verification machinery in this extraction.
