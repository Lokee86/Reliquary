# External Index Store crate extraction — implementation specification

**Status:** Specified; implementation not started (2026-10-27).
**Branch:** `refactor/crate-first-restart`. **Baseline:** `d2c00dfe` (Import Store extraction).
**Target:** `crates/reliquary-external-index-store/`.
**Scope:** Correction-only extraction of the existing *disposable derived index*, not a new external-source service, provider integration, or Memory/Archive authority.

## 1. Ownership boundary

External Index Store owns the accepted `CVAXIDX2` snapshot/discard codec, validated index-state values, per-source CAS revision projection, damaged/discard state, and Container-backed append/sync installation. Its only direct internal Reliquary dependency is **`reliquary-container`**. The higher external-source service owns provider authorization, feed/manifest acquisition, embedding/profiles, canonical content hydration, lexical/semantic search and cross-owner packed-vector/profile validation. External source material **never becomes authoritative REL Memory or Archive data** by indexing it.

The target currently contains a placeholder crate. The component mapping and dependency ceiling are `docs/architecture/crate-map.json`.

## 2. Source inventory and placement

**Move lower:**
- `src/external_source/store.rs`: `ExternalIndexStore`, `SourceState`, `IndexedMessage`, `IndexedChunk`, intrinsic structural validation, canonical encode/discard, CAS installation and malformed/discard projection.
- `src/external_source/compact_index.rs`: `CVAXIDX2` exact encoding/decoding, snapshot/discard framing and SHA-256 integrity.
- Focused codec, atomic-install, CAS and recovery tests from `compact_tests.rs`, `publication_compact_tests.rs` and `tests.rs` that exercise owner-local invariants.

**Keep higher:** `src/external_source/index.rs` (feed/rebuild preparation, endpoint/profile choice and matrix production), `processing.rs`, `provenance.rs`, `publication.rs`, provider trait and runtime gateway, authenticated hydration and search. `model.rs` must be **split**, not wholesale moved: stable descriptor/ref, bound constants and applicable error values can be re-exported from the lower crate if this preserves current public types; `ExternalConversationSource`, feed changes, snapshots, hydrated messages, and provider authorization remain higher. Keep `ExternalIndexStatus` public representation stable even if conversion at root is necessary.

## 3. Mandatory interface corrections

1. **Vector/profile backing:** `ExternalIndexStore::validate_backing` currently depends directly on `reliquary_vector_store::PackedVectorStore` and root `CompatibilityProfileStore`, `ScalarType` and ID types. Move backing inspection to the higher caller, which consults the authoritative profile/vector stores and submits a bounded set of invalid source namespaces or per-source verified results. Lower applies only the resulting invalidate/discard projection. No direct dependency on Vector Store or root, no opaque callback that reaches higher product from the lower crate.
2. **Public descriptor types:** `ExternalMessageRef`, `ExternalChunkRef`, `ExternalMessageDescriptor` and `ExternalSourceError` currently sit in mixed `model.rs`. Choose **one** canonical lower-owned representation for persisted descriptor/identity structures and re-export at root when compatible. Avoid a duplicated public/private descriptor cache. Never move the provider trait, plaintext, feed DTOs or transport APIs into the storage crate.
3. **Owned/borrowed API inventory before cut:** list root uses of `&SourceState`, `&IndexedMessage`, `&ExternalIndexStatus`, `external_index.sources`, and status/lookup accessors. Prefer type-preserving re-export for persisted values; any necessary internal owned projection is acceptable, but public API signature changes require explicit approval. No unsafe reference conversions or permanent duplicate state.
4. **Sync/CAS failure behavior:** `install` presently rejects unexpected revision, serializes/validates, appends and syncs before exposing state. On append/sync error it clears derived state and sets `damaged`; preserve this exact ordering, including error class and no stale data exposure. Root controls write admission and provider authentication; lower only validates the presented state and expected CAS revision.
5. **Damaged derived data:** malformed `CVAXIDX2` or any `CVAXIDX*` family record is treated as **disposable**, clears derived sources and marks damaged, not as corrupt authoritative Memory history. Preserve current current-format rejection and recovery strategy. Snapshot revisions are writer CAS state, not full retained history.
6. **Search authorization:** never return cached source text; index retains only IDs, hashes, byte ranges, packed matrix/row pointers and sorted term hashes. Higher caller re-authorizes each hydration and validates content hash/current active descriptor before release. Do not let lower index act as a provider grant.

## 4. Preservation invariants

- Exact `CVAXIDX2` framing, kind byte, LE payload length, SHA-256 envelope digest, descriptor/chunk serialization, ordering and explicit Discard bytes; no new version or compatibility readers.
- One Container remains the only physical writer; no independent file, SQLite index, daemon or duplicate index cache.
- Source contract, bounded manifest/message/chunk and sorted term limits, cursor/base/revision/generation consistency, positive dimensions, nonoverlapping contiguous chunk ranges, inactive-message no-chunk rule and deterministic CAS.
- Durable append + sync before publishing projection; errors invalidate derived state; reopening and compaction preserve discard/rebuild semantics.
- Vector/profile integrity checked above and invalid backing causes identical source invalidation with identical damaged flag; no silent acceptance of inaccessible matrices.
- Maintain root public status/query/error types and external-provider per-call authorization semantics. No extra inference/provider call due to extraction.

## 5. Bounded implementation sequence

1. Freeze current Git state and golden snapshot/discard bytes. Preserve `docs/graph-crate-extraction-evidence-2026-10-26.md` unmodified. Enumerate root caller references and exact validation/authorization order before code moves.
2. Extract the **canonical persisted model subset** and current `CVAXIDX2` codec into the lower crate with Container and actually required `sha2`/`serde` dependencies. Verify byte and hash parity, reject truncated, over-limit and tampered records.
3. Move `ExternalIndexStore` CAS/ingest/install/invalidated projection to lower; move `validate_backing`'s cross-owner inspection to higher product, pass only verified invalidation identities. Test sync failure vs CAS failure behavior and reopening/discard.
4. Cut root product call sites to one lower crate, including lifecycle, search/readiness, publication, maintenance and historical inspection. Delete displaced root owner code; root conversion/adapters must remain narrow, not another index database.
5. Verify focused root and CLI, update Cargo and CLI lock, documentation and `docs/external-index-store-crate-extraction-evidence-2026-10-27.md`; commit only the bounded extraction after tests.

## 6. Acceptance matrix

**Lower:** `cargo check -p reliquary-external-index-store --locked --offline`; `cargo test -p reliquary-external-index-store --locked --offline`; inspect Cargo dependency direction.

**Wire/state:** frozen `CVAXIDX2` snapshot/discard golden bytes, integrity errors, conflicting revisions, CAS/no-write behavior, bounds, missing source, inactive entries, malformed hash/length/unknown type, discard and damaged recovery.

**Root:** focused `external_source`, `compact_index`, `publication` and lifecycle/reopen tests, including vector/profile removal and stale backing, incremental feed vs rebuild, authorization denial, rehydration mismatch and exact results.

**Integration:** `cargo check --workspace --locked --offline`; `cargo fmt --all -- --check`; `cargo test --manifest-path cli/Cargo.toml --locked --offline`; relevant broader root/integration tests. Record actual command results, not inferred passing status.

**Completion:** one compiled production External Index Store, single physical Container owner, no upward/internal Vector dependency, identical recoverable/disposable data semantics, root/CLI green, evidence and commit. Separate `CVAFRS07` corpus and unrelated documentation-policy gates must not be silently marked complete.

## Exclusions

No external-source integration redesign, provider adapters, embedding model selection, query-ranking redesign, canonical Archive/Memory import, extra database, new format, generic owner callback framework, or architecture ratchet machinery.
