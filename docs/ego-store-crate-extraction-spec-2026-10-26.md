# Ego Store crate extraction — implementation specification

**Status:** Proposed; no Ego Store extraction implemented.
**Repository:** `C:\!bin\workspace\Reliquary-crate-first`
**Branch:** `refactor/crate-first-restart` · pre-spec HEAD `7b75ffbc`
**Target:** `crates/reliquary-ego-store/` — **seventh compiled owner extraction**, following Perception Store.
**Mode:** Correction-only staged hard cut, source/format preserving, no new feature work.

## 1. Problem statement and outcome

`src/ego/**` still contains the real Ego record state machine and compact storage code within the root `reliquary-memory` package. `EgoStore` directly references root `owner_lifecycle` errors and lifecycle values; the root also owns its codecs, model types, REL/PHY entry points, and historical replay. That prevents Owner Lifecycle and History from depending on a rank-2 durable Ego owner without importing the higher-level Ego synthesis service.

The accepted architecture requires **one independently compiled `reliquary-ego-store`**, consuming only `reliquary-container` among Reliquary crates. It must own semantic interpretation of existing persistent Ego frames, document revision history, terminal facts, identity activation, validated writes, and passive replay. **Container still exclusively owns all physical persistence**: framing, file I/O, physical cursor, append and physical integrity. Ego Store neither creates a separate file/database nor invents a parallel persistence abstraction.

Observable developer outcome: REL and PHY use the same compiled Ego state implementation; no duplicate root codecs/stores remain; History can inspect Ego sequences without importing the high-level Ego synthesis service; current public behavior, file bytes, read borrowing and error semantics remain intact.

## 2. Governing decisions and existing behavior

| Authority | Binding consequence |
| --- | --- |
| [Crate-first layout](crate-first-layout-spec-2026-10-25.md), [logical DAG](architecture/component-dag.json), [crate map](architecture/crate-map.json) | `ego-store` is a rank-2 data owner. `reliquary-ego-store` has one permitted internal Cargo dependency, **`reliquary-container`**. A later `reliquary-ego` service depends on Ego Store, not vice versa. |
| [ADR 0001](decisions/0001-purpose-built-database-ownership.md), [scope ownership](subsystems/scope-ownership.md), [Ego state](subsystems/ego.md) | Ego is owner-local; PHY identity/personality must never appear in REL. Anchors and the existing text synthesis snapshot have their currently supported REL/PHY behavior. A REL cannot obtain PHY-private state by holding an Ego record ID. |
| [ADR 0040](decisions/0040-terminal-tombstones-and-import-epochs.md), [ADR 0041](decisions/0041-semantic-import-boundaries-owner-pruning-history-rewrite.md), [ADR 0042](decisions/0042-minimal-lossless-current-history.md) | Preserve dense Ego-local version sequence, document revisions, active/terminal identities and anchors, exact historical reads and read-only reopen; no extra global version ticket, terminal journal, or repair write. |
| [Storage format](storage-format.md), [behavioral contracts](behavioral-contracts.md) | Preserve the literal compact carriers `CVAECR01`, `CVAEDL01`, `CVAETM01` and activation `CVAEIDA1`, field ordering and validation; no new runtime reader or format migration. |
| [Ego status inventory](ego-status-decision-inventory-2026-10-14.md), [Memory-Web design](ego-web-synthesis-plan.md), [Cross-chat plan](ego-cross-chat-context-plan.md), [Cross-REL plan](ego-cross-rel-summary-inclusion-plan.md) | Existing singleton `EgoWebSynthesis {revision, source_memory_version, text}` is **not** the future PHY-owned, per-source-REL itemized Memory-Web summary. Do not synthesize, schedule, redesign, or persist that future feature here. |
| [Perception Store results](perception-store-crate-extraction-evidence-2026-10-26.md), [Graph results](graph-crate-extraction-evidence-2026-10-26.md) | Reuse the established compiled-owner / root-admission pattern without adding a contracts bucket or generic owner manager. |

**Recovery policy:** Review selected relevant old refactor diffs if useful for a concrete Ego correctness fix. Do not cherry-pick unrelated Phase 7 architecture ratchets, compiler evidence, proof ledgers, or dormant compatibility readers. Promoted `CVAFRS07` corpus acceptance remains separate Freshness/Import work and is not an Ego Store acceptance substitute.

## 3. Canonical source crosswalk

The reserved `crates/reliquary-ego-store/Cargo.toml` and `src/lib.rs` currently contain **no production implementation**. Move the **existing** code, not a re-export facade over `src/ego/**`.

| Source from `src/ego/` | Destination responsibility |
| --- | --- |
| `ego_model.rs`, `ego_error.rs` (after lifetime/error split) | Canonical `EgoIdentityId`, `EgoIdentity`, `EgoAnchorId`, `EgoAnchorPriority`, `EgoAnchor`, `EgoPersonality`, `EgoWebSynthesis`, bounds and store-local error shape. |
| `ego_codec.rs`, `ego_codec_scalar.rs` | Strict Ego value/activation decoding, type tags, text/length limits and canonical byte encoding. |
| `ego_compact_{codec,delta_codec,contract,apply}.rs` | Current creation/delta/terminal value codecs and materialization; `FieldChange` from Container; exact predecessor/field-mask rules. |
| `ego_store.rs` and `ego_store_{anchors,compact,documents,identities,read,validation}.rs` | **One** owner state machine: append, ingest, indexes, document versions, Identity activation, tombstones, historical views, REL/PHY kind checks and source watermark validation over caller-provided primitive Memory version. |
| `ego_{compact_contract,compact_creation_codec,compact_delta_codec,compact_replay}_tests.rs` | Move or adapt tests that exercise pure store/codec/replay without product facades. Keep one copy of each contract test. |

**Keep at root / higher owner:**

- `src/facade/cva_ego.rs`, `phylactery_ego.rs`, `cva_lifecycle.rs`, `phylactery_lifecycle.rs`, `cva_error.rs`, `phylactery_error.rs`: actual REL/PHY construction, held Container authorization, source Memory watermarks, product API, error mapping.
- `src/facade/historical_ego_fingerprint.rs`, `historical_whole_owner_fingerprint.rs`, `historical_whole_phy_fingerprint.rs`, `historical_ego_fingerprint_tests.rs`: multi-owner/history projection and fingerprints. They **consume** Ego Store's immutable decoded record view, not a second Ego state machine.
- `src/facade/cva_reconcile_ego_compact_tests.rs`, `ego_compact_maintenance_tests.rs`, `historical_ego_compact_tests.rs`, `src/imports/reconciliation_ego_compact_tests.rs`: multi-owner/source reconciliation, historical reconstruction, repair/rewrite authorization.
- `src/ego/ego_{identity,anchor,persona,lifecycle,compact_writer}_tests.rs`, `ego_tests.rs` where tests create `Cva` or `Phylactery`: retain as root integration tests even if their paths are regrouped. The test-only `ego_current_fixture.rs` remains with higher integration/fixtures unless a genuinely owner-local test requires it.
- Model inference, identity/personality synthesis, Memory-Web candidate selection, cross-REL summary composition, context budgeting, host authorization: all remain with their existing or future *higher-level* owners.

### Suggested crate tree

```text
crates/reliquary-ego-store/
  Cargo.toml                     # Reliquary dependency: reliquary-container only
  src/
    lib.rs                       # explicit narrow EgoStore + value exports
    ego_model.rs
    ego_store_error.rs           # typed local error; product mapping above
    ego_codec.rs
    ego_codec_scalar.rs
    ego_compact_{codec,delta_codec,contract,apply}.rs
    ego_store.rs
    ego_store_{identities,anchors,documents,read,validation,compact}.rs
    *_tests.rs                   # existing owner-local tests, where appropriate
```

Do not introduce `EgoManager`, a duplicate `Cva`, an independent file handle, all-domain `Lifecycle` trait, or a low-level import of `reliquary-memory`.

## 4. Required upward-dependency cuts

| Current coupling | Approved target |
| --- | --- |
| `EgoError::Lifecycle(crate::owner_lifecycle::LifecycleError)` and `impl From<LifecycleError>` in root `ego_error.rs` | Ego Store defines **store-local** terminal/not-active facts with concrete Ego Identity/Anchor target IDs. Root maps those facts to the existing externally visible `EgoError::Lifecycle(LifecycleError)` / product `CvaError` and `PhylacteryError` variants; do not make lower crate depend on root lifecycle. Preserve public error shape/matching as far as the existing API requires. |
| `EgoStore::require_live_identity`, `apply_identity`, Anchor update/delete call `LifecycleState::require_active` directly | Use exact store-local deleted/live/terminal checks, including active Identity exclusion and deletion coordinates. Product translates errors at the boundary; **do not** relax rejection or change the order of revision/terminal checks. |
| `EgoStore::identity_lifecycle` / `anchor_lifecycle` return root `LifecycleState` | Return immutable store-local lifecycle facts (`active`, `deleted`, exact Ego-domain version coordinate; global coordinate remains absent). Root maps to supported `LifecycleState::Active/Tombstoned` and correct `TombstoneCoordinates`. Avoid a permanent parallel mutable lifecycle registry. |
| `src/facade/cva_ego.rs`, `phylactery_ego.rs` validate `source_memory_version` against `self.memory_version()` | Keep live owner admission in root, using actual current Memory Core version. Store may validate supplied primitive watermark and reopen against the caller's primitive current Memory version, **never** import Memory Core or derive source authority from its own document text. |
| `EgoStore::ingest_publication` is used by `historical_ego_fingerprint.rs` and root reconciliation | Expose one narrow immutable record/replay interface (e.g. decoded `EgoRecord` plus `ego_version`) needed by actual History callers. Preserve deterministic all-prefix replay; do **not** place History fingerprinting or owner reopen coordination inside the store. |
| Store currently holds and returns root `EgoIdentity`, `EgoAnchor`, `EgoPersonality`, `EgoWebSynthesis` values | Move their **single** canonical definitions into the crate and re-export at root for supported public API. Borrowed singleton reads `Option<&EgoPersonality>` and `Option<&EgoWebSynthesis>` remain possible without duplicate caches or owned-return migration. |
| Physical replay encounters `CVAETM02` Entity merge terminal and Ego `CVAETM01` terminal family in the same stream | Preserve the existing **exact cross-owner dispatch guard**: Entity `CVAETM02` is **not** Ego evidence. An unsupported Ego-recognized version does not become silently accepted; recognized malformed Ego records still fail. Cover this collision explicitly in tests before moving the decoder. |

Use a single compiled store and explicitly mapped product error boundary. A product-facing `EgoError` adaptation is legitimate only for supported external error compatibility—it must not become an internal adapter layer or duplicate storage state. Preserve exact error precedence and `Result`/borrow signatures where feasible.

## 5. Immutable semantic and format contracts

- **Owner kinds:** `EgoStore::reliquary()` and `::phylactery()` remain distinct admission configurations, not different persistence systems. REL rejects Identity, activation, and Personality records. PHY supports those documents. Anchors and the current singleton synthesis continue per current owner behavior.
- **Identity:** stable 16-byte `EgoIdentityId`; reserved all-zero `Default` ID remains supported where current. Multiple PHY Identities retain activation/selection. Cannot delete an active Identity while another live Identity exists; terminal deletion cannot be resurrected. Random fresh IDs are generated only through the already authorized creation path.
- **Anchor:** stable 16-byte `EgoAnchorId`; priority `High/Normal/Low`, bounded text, revision and terminal history. Identical authorized update is a no-op; invalid tombstone retry/expected revision retains current failure behavior.
- **Personality and Synthesis:** single text, revision and exact source Memory-version watermark. Same supplied text/watermark remains no-op, changed values advance Ego sequence, stale invalid source cut fails before physical append.
- **Version and bytes:** Ego-local `ego_version` is strictly dense and begins at 1; it is **not** a semantic global ticket, not `CVAPUB01`, and not derived from Memory or Graph clocks. `CVAECR01` creation, `CVAEDL01` delta, `CVAETM01` Identity/Anchor terminal, and `CVAEIDA1` activation preserve literal kind, revision, mask, priority, scalar and UTF-8 layouts. Terminal record retains exact priority. Current writer must not revive legacy full-postimage formats.
- **Delta semantics:** document identity/kind may not change; predecessor revision increments exactly one; creation revision one is implicit. Zero field-mask admitted historical postimages remain replayable under current rules; ordinary setter no-ops allocate nothing. Activation flag is independent publication metadata, not inherited through a delta.
- **Open/historical:** accepted records replay in physical order; malformed recognized records reject, unrelated owner families are ignored, no silent normalization, no implicit repair or writes on open. Historical reads retain all earlier nondeleted Identity/Anchor revision bodies and terminal Ego version, with no fabricated global coordinate.
- **Physical append atomicity:** validate before append; on failed append leave in-memory versions/identities/anchors/current singleton state unchanged. No clone of all previously accumulated Ego history per accepted frame.
- **Data ownership:** all physical bytes through `reliquary-container`; all semantic Identity/Anchor/Personality/Synthesis decisions to publish remain with their existing owner call sites. No persistent PHY-owned per-source-REL itemized summary is created by this extraction.

## 6. Bounded staged hard cut

1. **Freeze and inventory.** Confirm HEAD, explicit dirty `docs/graph-crate-extraction-evidence-2026-10-26.md`, current Ego source/module/test map and locked workspace manifests. Review selected old Ego correctness diffs, no wholesale merge or generated Arcana evidence. Preserve the dirty evidence file exactly.
2. **Move the canonical values and codecs once.** Replace the placeholder crate with `ego_model`, native codec/scalar/delta and compact contracts. Update internal module imports; only Container and genuinely needed external libraries (`uuid` for existing Ego IDs) are permitted. Independently compile literal encoding/decoding tests. Do not add a global ticket.
3. **Move the real state machine.** Relocate `EgoStore` and six store submodules, cutting `owner_lifecycle` references in favor of store-local facts/errors. Preserve REL/PHY owner-kind admission and sequence checks, partial replay failures, terminality and identity activation.
4. **Rewire concrete consumers.** Root `Cva` and `Phylactery` hold the compiled crate's `EgoStore`; owner open/reopen feeds the one store in existing frame order; owner admission translates lifecycle/errors and validates source Memory watermarks. Historical fingerprint/reconcile consumers use the same crate record/codec interface without another state cache. Keep borrowed `ego_personality()`/`ego_web_synthesis()` where no contract requires changing.
5. **Remove displaced implementations.** Delete old `src/ego` production store/codec/model modules and their `src/lib.rs` path declarations; keep only genuinely root-owned integration tests/fixtures, moving paths if appropriate. Update root manifest/lockfile, public re-exports, [architecture](architecture.md), [Rust API](api.md), [maintainer map](maintainer-map.md), [storage format](storage-format.md), [Ego subsystem](subsystems/ego.md), and index after code moves. Commit extraction once scoped gates pass; do **not** stage the unrelated Graph evidence modification.

A bounded red intermediate compile interval is acceptable. Do not leave dual live Ego implementations, a crate importing root, or test-only production wrappers to bridge the move.

## 7. Existing tests and measurable acceptance

**Owner-local tests to move/split:** `ego_compact_contract_tests`, `ego_compact_creation_codec_tests`, `ego_compact_delta_codec_tests`, `ego_compact_replay_tests`, and any newly necessary precise error-mapping or append-atomicity case. Move existing tests rather than copying them, and add only tests for a genuinely new boundary.

**Keep higher integration tests:** `ego_identity_tests`, `ego_anchor_tests`, `ego_persona_tests`, `ego_lifecycle_tests`, `ego_compact_writer_tests`, `ego_tests`, `cva_reconcile_ego_compact_tests`, `ego_compact_maintenance_tests`, `historical_ego_compact_tests`, `historical_ego_fingerprint_tests`, `reconciliation_ego_compact_tests` and REL/PHY opening/owner-scope tests where they exercise real product ownership.

**Behavioral tests required:**

- Exact `CVAECR01`, `CVAEDL01`, `CVAETM01`, `CVAEIDA1` binary cases, field masks including zero, trailing data/unknown kind rejection, and `CVAETM02` Entity collision exclusion.
- REL rejects PHY Identity/Personality, PHY active Identity management, multiple Identity selection, required active Identity invariant and deterministic reserved `Default` behavior.
- Single-version dense Ego replay; Identity/Anchor revision conflicts, same-input no-ops, exact deletion coordinate and terminal-resurrection rejection across reopen; old retained historical revisions queryable.
- Source Memory watermark validation and REL/PHY privacy; Ego history does not allocate any global/Memory/Graph version.
- Failed physical write does not change Ego in-memory state; read-only reopening is byte-neutral and has no implicit historical repair.
- Reconciliation/import/historical fingerprint remains semantically identical in old vs newly compiled code, with no extra second scan or duplicate Ego owner.

**Commands:** `cargo check -p reliquary-ego-store --locked --offline`, `cargo test -p reliquary-ego-store --locked --offline`; focused root Ego/History/Reconciliation/REL/PHY tests; `cargo check --workspace --locked --offline`; `cargo fmt --all -- --check`; and `cargo test --manifest-path cli/Cargo.toml --locked --offline` with the independent CLI lockfile. Inspect Cargo and `rg` paths for one owner implementation and no forbidden upward dependency. Run the full root suite as a separate product integration gate; distinguish known existing external-source failures and expensive historical/measurement tests rather than reporting them green.

**Completion gate:** the new crate holds all **real** Ego durable semantics and codecs, root callers are rewired to it, no obsolete root Ego implementation or Cargo back-edge remains, exact bytes/error/terminal behavior pass focused tests, and the implementation is committed. A green placeholder or only a moved model module is **not** completion. Promoted `CVAFRS07` corpus and unimplemented Memory-Web Summary design remain separate.

## 8. Out of scope

No per-REL itemized PHY Memory-Web summary schema, Ego audit/selection/reconciliation model inference, Personality generation, cross-chat or cross-REL context selection, Ego budgeting/injection, Freshness policy, Owner Lifecycle extraction, History/Maintenance redesign, physical Container changes, global clock changes, new feature work, new source-backed inference calls, runtime/host wiring, or supplemental architecture proof machinery.
