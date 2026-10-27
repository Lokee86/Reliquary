# Import Store crate extraction — implementation specification

**Status:** Implemented; verification and known gaps are recorded in [extraction evidence](import-store-crate-extraction-evidence-2026-10-27.md) (2026-10-27).
**Branch:** `refactor/crate-first-restart`. **Reference HEAD at specification:** `8d474279`.
**Target:** `crates/reliquary-import-store/`.
**Prerequisite:** Integrate and commit the in-progress Dream Store work before beginning this implementation. Do not mix its uncommitted changes with this extraction.
**Scope:** Existing import-epoch durable state only; correction-only hard cut. No native importer, reconciliation, historical settlement, readiness policy or format redesign.

## Goal and ownership

Import Store owns the **durable ImportStart/ImportComplete epoch records**, canonical current-format codecs, deterministic import ID, open/completed epoch projection, duplicate/conflict checks, replay and owner validation. The `reliquary-import` semantic coordinator retains provider parsing, source normalization, Archive receipt validation, import attribution, readiness, completion authorization, Freshness settlement and cross-owner reconciliation.

The lower crate's **only direct Reliquary dependency is `reliquary-container`**, as prescribed by `docs/architecture/crate-map.json`. It may use `serde`, `serde_json` and `sha2` to preserve the existing encoding and canonical digest. No root, Archive, Insomnia, Dream, Freshness, History, Maintenance, Runtime or Import service dependency.

## Source crosswalk

**Move lower:**
- `src/imports/epoch_store.rs`: epoch maps, open status, strict Start/Complete transitions, owner validation, canonical import identity, authenticated append and passive ingestion.
- `src/imports/epoch_codec.rs`: `CVAIEPS1` Start and `CVAIEPC1` Complete framing, fixed length, JSON payload and strict retired `CVAILEG*` rejection.
- `ImportWatermarks`, `ImportStart`, `ImportComplete`, `ImportEpochStatus` durable value definitions currently in `src/imports/contract.rs`; preserve public root representations through explicit re-exports where type-compatible or owned conversions.
- `src/imports/epoch_store_tests.rs` and owner-local epoch codec tests (migrate, do not duplicate).

**Remain higher:** `src/imports/{completion,commit,prepare,readiness,epoch_scope,reconciliation,reconciliation_replay,providers,chatgpt,claude,hermes}.rs`, and the Cva/native import/historical facades. Import readiness, source receipt digest *authority*, Dream/Freshness settlement, retry policy and exclusive completion coordinator must not move into storage.

## Mandatory boundary corrections

1. **Completion authority token:** Current `ImportEpochStore::append_complete_coordinated` takes `super::completion::CompletionAuthority`, deliberately constructible only in higher Import coordination. **Do not move this token, make it publicly constructible, or let the lower store authorize completion.** The higher completion coordinator must validate readiness/receipt/settlement and exclusively own the actual Container append. Lower store exposes an exact `validate_complete` plus `apply_accepted_complete`/codec surface, or a narrow authenticated append operation used only after higher authorization; choose the simplest contract that preserves no-append-on-failure and crash atomicity. Do not open a public unguarded lower completion writer that callers can mistake for authorization.
2. **Receipt validation:** `epoch_store.rs::validate_receipts` currently calls higher `prepare::digest(ImportReceipt)`. Higher caller computes the canonical receipt digest from authoritative Archive receipts, then passes matching immutable `(import_key, digest)` evidence to lower validation; store checks exact equality against completed markers. Never import Archive or reimplement a second receipt canonicalizer.
3. **Error ownership:** `ImportError` mixes Container, JSON, Archive and Insomnia errors. Move only store-local epoch/format/Container/JSON errors below and map them at the root Import boundary; preserve observable variants and message precedence. No lower `crate::ImportError` or root import.
4. **Durable IDs and public borrowing:** `ImportStart` and `ImportComplete` are owned, serializable values. Inventory `open()`, `status()`, `statuses()` and public API call sites before migration. Re-export the **same canonical value type** if it retains the public API without violating dependencies; otherwise convert at root using **owned** returns for internal projections, preserving public signatures where possible. Never invent a duplicate persistent epoch cache or unsafe reference conversion. Any required public signature break must be separately reviewed before code.
5. **Epoch writer semantics:** `append_start` validates deterministic import ID, canonical key, owner UUID, no overlap and no existing start before Container append. `append_complete_inner` checks previously accepted identical completion as idempotent, rejects conflicting completion, enforces open matching start and digest/watermark invariants, then appends before changing projection. Preserve exact ordering, error precedence and synchronization at higher caller. Do not turn read-only owner open or historical inspection into a writer.
6. **No semantic settlement in the store:** `historical_freshness_settlement_digest` is persisted, but its actual source/acceptance belongs above. Treat it as an exact committed value after higher validation; never infer a valid settlement from its presence alone.

**Implementation freeze gate:** Determine whether `ImportStart`, `ImportComplete` or `ImportEpochStatus` are externally exported and whether existing consumers borrow them; document concrete call sites and chosen type ownership *before* relocating the structs. This avoids repeating the Insomnia borrowed-ID blocker.

## Invariants

- Single Container physical store, one accepted writer; ImportStart and ImportComplete are current-format evidence, not a generic transaction log.
- Byte-for-byte `CVAIEPS1`/`CVAIEPC1` compatibility including existing serde JSON field order, length prefix and digest identity; never add `CVAILEG*` compatibility readers.
- One open epoch at most, owner-bound, deterministic import ID, canonical key, no overlapping starts, exactly matching Complete and immutable completed watermarks; idempotent exact completion and fail-closed conflicts.
- The import receipt, readiness, Freshness historical settlement and authority provenance are verified **before** the completion marker is written.
- Replay/History reconstruct exactly the same open/completed states without writing; malformed/truncated, foreign-owner, missing-start, mismatched digest and unsupported records fail closed.
- Preserve all existing root/Cva, CLI, and historical import behavior and tests; no separate data plane, no new reverse dependency, no additional persistent cache.

## Implementation checkpoints

1. **Baseline and caller inventory.** Confirm Dream Store work is complete and committed; inspect Git status. Preserve unrelated `docs/graph-crate-extraction-evidence-2026-10-26.md`. Enumerate callers and recorded real append/replay order using `rg`. Freeze golden Start/Complete frames and existing epoch/readiness test cases.
2. **Lower values and current codecs.** Move canonical epoch values and exact ID/codec logic, run lower golden wire tests. Make the public root type boundary deliberate and verify borrowed/owned call-site feasibility.
3. **Lower projection and authority seam.** Move state maps, ingestion, validation and owner checks. Separate higher receipt digest authority and completion token. Preserve the coordinator's sole authorization and Container append-before-projection rule. Unit-test duplicate/overlap/mismatch.
4. **Root product cutover.** Move live Cva/REL owner open, import readiness, completion, reconciliation, historical replay, retention and CLI to the **single lower store**. Remove old root store/codec definitions; do not retain forwarding caches or duplicate writers.
5. **Verify and commit.** Update Cargo/lock (including detached CLI lock), docs (`architecture.md`, `api.md`, `maintainer-map.md`, `native-import.md`, `storage-format.md`), record exact test evidence at `docs/import-store-crate-extraction-evidence-2026-10-27.md`, and commit only scoped work. Avoid custom architecture ratchets or full-tree inference scans.

## Acceptance matrix

**Lower crate:** `cargo check -p reliquary-import-store --locked --offline`; `cargo test -p reliquary-import-store --locked --offline`. Check manifest and imports: Container only internally.

**Exact codec/state:** Golden bytes and hash/import-ID equivalence; Start/Complete roundtrip, invalid prefix/length/trailing bytes/JSON, retired `CVAILEG`, invalid owner/key, duplicate/conflicting starts or completions, orphan Complete, incomplete settlement digests, watermarks and replay of current accepted prefixes.

**Authority/product:** Exercise Import completion readiness rejection with **zero appended Complete frames**; successful exclusive completed epoch with canonical Archive receipt and historical Freshness settlement; exact retry/idempotence; state after reopen; import isolation and second import after completion. Keep existing scope/attribution tests in higher service.

**Integration:** `cargo check --workspace --locked --offline`, `cargo fmt --all -- --check`, focused root `import`, `epoch`, `reconciliation`, `historical`, `freshness` tests, and `cargo test --manifest-path cli/Cargo.toml --locked --offline`. Run one real native import through completion/reopen. Record separately any broader root regression suite.

**Complete only when:** Real production root uses lower epoch store, no duplicate authoritative state or reverse Cargo dependency, current wire and historical replay are preserved, exact negative completion and CLI/root coverage pass, evidence is documented and implementation committed. The separate `CVAFRS07` six-file corpus recovery and any still-pending Dream Store acceptance remain distinct gates.

## Exclusions

No Import service extraction, new import providers, semantic readiness changes, relaxed completion admission, automatic import settlement, Dream/Freshness redesign, new format, legacy reader, multi-epoch concurrency, additional owner lifecycle framework or general-purpose receipt database.
