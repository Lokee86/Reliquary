# Archive crate extraction — implementation specification

**Status:** Target specification only; extraction not started.  
**Branch:** `refactor/crate-first-restart`, current HEAD `f5ae61d1` (Container extracted).  
**Target:** `crates/reliquary-archive/`. **Migration:** staged hard cut; correction-only, no new product features.

## 1. Problem and outcome

Archive persists conversations, Nodes, Branches, content, deterministic Episodes, fragments and source history. Its current implementation is scattered across `src/archive/`, root `src/lib.rs` and `src/facade/`. Archive's lower-rank code imports higher-level Import, Memory, Maintenance, Owner Lifecycle and product Runtime values; blindly moving those files would reproduce the monolithic dependency knot as Cargo cycles.

Deliver one independently compiled Archive **source-truth owner**. After extraction, the product's actual `Cva` uses `reliquary-archive`, Archive depends directly only on `reliquary-container`, and there is only one codec, store and replay implementation. No internal compatibility proxies.

## 2. Governing docs and precedence

| Document | Binding constraint |
| --- | --- |
| [Crate layout](crate-first-layout-spec-2026-10-25.md), [logical DAG](architecture/component-dag.json), [crate map](architecture/crate-map.json) | `archive-store` is rank 2; physical target `reliquary-archive` has only one permitted Reliquary dependency, `reliquary-container`. |
| [Container extraction](container-crate-extraction-spec-2026-10-25.md), [API reference](api.md), [storage format](storage-format.md) | Container owns V2 framing, clocks, physical cursor, writer/read leases, and explicit repair. Archive owns semantic record decoding and local versions. |
| [ADR 0001](decisions/0001-purpose-built-database-ownership.md), [architecture](architecture.md), [scope ownership](subsystems/scope-ownership.md) | Archive is a semantic source owner inside a REL, not a second physical database or coordinator. |
| [ADR 0012](decisions/0012-deterministic-episodes-and-insomnia-memory-authority.md) | Conversations remain open-ended. Episodes are immutable ancestry ranges built from indivisible response cycles, default 32 KiB budget, default 15-minute inactivity. Insomnia exclusively owns Memory extraction. |
| [ADR 0014](decisions/0014-echo-historical-reasoning-traces.md) | Echo belongs to its own future crate. Archive is visible transcript/source history, not reasoning/tool-trace evidence. |
| [ADR 0038](decisions/0038-gateway-instance-runtime-multiplexing.md), [multiplexing](subsystems/runtime-multiplexing.md), [external sources](subsystems/external-sources.md) | Runtime owns sessions, stream dispatch, host transcript authority and access grants; Archive owns persisted source facts. |
| [ADR 0041](decisions/0041-semantic-import-boundaries-owner-pruning-history-rewrite.md), [ADR 0042](decisions/0042-minimal-lossless-current-history.md), [native import](native-import.md), [semantic maintenance](subsystems/semantic-maintenance.md), [behavioral contracts](behavioral-contracts.md) | Import coordinates epochs; History composes immutable multi-owner cuts; Maintenance authorizes reclaim/rewrite. Archive exposes read-only history and marker evidence and preserves source/provenance semantics. |

Original convergence execution is **not** the migration plan. For salvage consult the Step 1 recovery inventory and the original repository/bundle at `C:\!bin\backups\Reliquary-crate-restart-2026-10-25\`; inspect concrete fixes in `1380d0b4`, `408d7751`, `957733a4`, `e2d1045b` (Archive read/error ordering), `50101068` and relevant `e5d62b7f`/`7c74b511` consumer changes. Do not cherry-pick bulky verification machinery or Phase 7 source mapping.

**Current-format rule:** Preserve existing V2/`CVAPUB01` Archive body bytes, local/global version ordering, source identities, errors and history. The authoritative corpus requires separately recovered `CVAFRS07` settlement changes; Archive's extraction does not prove that corpus's end-to-end admission. No legacy runtime readers, format conversion or schema redesign.

## 3. Destination and source crosswalk

Use the existing Cargo package, and only add actual dependencies. Logical module groups (not mandatory one-file modules):

```text
crates/reliquary-archive/
  Cargo.toml
  src/
    lib.rs               # intentional public owner API / typed values
    archive.rs           # conversation source owner
    model.rs             # Node, Branch, content/file and error values
    codec.rs             # exact Archive publication, metadata and content bodies
    replay.rs            # ArchiveOpenState and ordered local-version verification
    index.rs             # content/Node/Branch/source indexes
    episode.rs           # deterministic response-cycle Episode builder/store
    fragment.rs          # source fragment generation/lookup
    file.rs              # source files, attachments and project binding facts
    import_receipt.rs    # durable imported-source receipt DTO and validation
    compaction.rs        # durable compaction store and logical projection
    stream_store.rs      # durable interaction checkpoint records only
    project_history.rs   # source project revision correlations
```

Keep existing fine-grained implementation files where they remain coherent: `archive_{store,rebuild,codec,publication_codec,activity,conversation*,history*,model,error,record_index,object_index,lookup}.rs`, `conversation_metadata_*`, `episode_{builder,codec,import,index,model,policy,store}.rs`, `fragmenter.rs`, `fragment_{model,store}.rs`, `file_{index,model,store}.rs`, `source_attachment_index.rs`, `file_memory_link_{codec,index,model,store}.rs`, `project_file_binding_{codec,store}.rs`, `archive_import.rs`, `turn_ingest_{codec,model,store}.rs`, `project_history_{codec,model,store}.rs`, `conversation_compaction_{allocator,codec,model,store}.rs`, and `interaction_stream_{codec,record,store}.rs`. This is a classification of the current neighborhood, not a requirement to move each file unchanged.

The current source neighborhood contains **86 Rust files** (61 implementation and 25 named test modules). Treat source location as evidence, not ownership.

**Do not move into Archive:**
- `echo_{codec,model,store}.rs`, `echo_tests.rs`: later independent `reliquary-echo` extraction.
- `interaction_stream.rs`, `interaction_stream_persistence.rs`: implement `InteractionRuntime` and belong with Runtime/Gateway session orchestration.
- `archive_search.rs`: implements high-level `Cva::search_archive`, query policy and `SearchError`, therefore Retrieval/product consumer, not Archive.
- Multi-owner retention descriptor/manifest validation and physical maintenance operations currently mixed into `archive_history_prune.rs`: retain Archive-local marker/immutable-source facts in Archive, move higher-level proof/authorization upward.
- Any high-level facade transaction that invokes Insomnia, Memory, Import, History, Runtime, Project Environment or Maintenance.

## 4. Existing upward dependencies to cut

| Current concrete import/owner mix | Canonical target and cut |
| --- | --- |
| `Archive.import_receipts`, `archive_import.rs`, `archive_rebuild.rs` and `archive_codec.rs` import `crate::imports::ImportReceipt` | Persisted `ImportReceipt`, `ImportedConversation`, `ImportedMessage`, `ImportedArtifact`, `ImportedBranch` from `src/imports/contract.rs` are **Archive source DTOs**. Move their canonical definitions and serde shape into Archive unchanged. `NativeImport`, epoch start/completion, settlement/readiness and Import orchestration remain in Import. Keep any actually supported `imports::ImportReceipt` product path as a public re-export only; no duplicate type or codec. |
| `file_memory_link_store.rs::validate_file_memory_targets(&Archive, &MemoryStore)` | Archive stores/validates `FileMemoryLink` source IDs and the file endpoint. Owner Lifecycle/root validates historical Memory target existence after replaying both owners. Archive must not import Memory. |
| `archive_store.rs::validate_references`, `file_store.rs::validate_files`, `turn_ingest_store.rs::ingest_turn` depend on the root `ProjectFileStore` | Project-file binding records are Archive source facts. Validate locally over Archive-owned binding/index state or accepted immutable binding evidence. Higher Project Environment/Gateway performs actual filesystem/Git/Lore authority and scope checks. Preserve exact file IDs, hashes, attachment de-duplication and import artifact binding. |
| `archive_history_prune.rs` imports `OwnerMaintenanceStore`, `HistoryRetentionDescriptor`, `RetentionTarget`, `PrunedPayload` | Keep `CVAPRND1` marker codec, source content/closure facts and local validation in Archive. Maintenance consumes these facts to build/verify descriptors, validate manifests/accepted cuts and perform explicit mutation. No Archive→Maintenance dependency, no ordinary-open reclamation. |
| `archive_conversation_tombstone.rs` imports `owner_lifecycle` state/errors | Archive owns the persisted conversation tombstone, closure and local idempotent publication. Generic lifecycle-state translation, cross-owner disposition and API errors remain above Archive. |
| `interaction_stream_store.rs` and `project_history_store.rs` reference product `CvaError` | Use Archive-local typed persistence errors; map to existing product error semantics at the caller. Keep Runtime's `InteractionRuntime` logic above Archive. |
| `archive_search.rs` calls root `Cva` retrieval candidate/policy code | Leave `Cva::search_archive` as a high-level product/Retrieval implementation. Archive supplies narrow source fragment/content facts; it never owns inference or retrieval policy. |
| Project revision/file correlation values are re-exported from root | Canonical value definitions already live in `project_history_model.rs`; Archive owns source revision/correlation facts. Project Environment consumes these values and retains authorization and host repo actions. |
| Echo shares the folder | Exclude from Archive entirely; preserve its distinct persisted owner and `CVAECHO1` behavior for the future Echo crate. |

**Hard-cut decision:** remove actual reversed imports, not mask them with a generic `ArchiveAdapter`, trait-object registry, product-root handle or circular facade. If a high-level coordinator still lives in root until its later cut, it may call the new Archive crate directly; it cannot provide an alternate Archive store.

## 5. Public API and behavior

- **Replay:** `ArchiveOpenState` constructs an empty projection, ingests Container-supplied `(ObjectRef, &[u8], latest_global)` frames, checks Archive publication/record version monotonicity, content hashes, repeat links, source identities and codec errors, and finishes without mutation. Root/Owner Lifecycle pulls ordered frames from Container and routes the same physical stream to other owners. Container does not decode Archive.
- **Publication:** narrow Archive owner operations for content, conversation/Node/Branch/metadata/tombstone, ingested turn, immutable Episode/fragment/file/attachment, import-source receipt, durable stream checkpoint, compaction and source project correlation. The caller holds validated physical writer/admission rights; Archive cannot bypass Import or Maintenance authorization.
- **Read:** current and historical ancestry, branch turns, content and fragments, source file/attachment evidence, Episode coverage/open tail, record versions, source receipts, source compaction projection and immutable project correlations. Tombstoned history remains accessible only through historically authorized paths.
- **Validation:** preserve Archive-owned `ArchiveError` and exact wrong-parent, missing-content, duplicate/conflicting source revision, invalid global/local version, incomplete-tail and unsupported record failure behavior. Cross-owner Memory target checks happen in composition, not Archive.
- **Episode semantics:** complete response cycles remain indivisible, branch ancestry forms the source of truth, finite imported tails are finalizable without closing the conversation. No model call or Insomnia scheduling inside Archive.
- **Compaction:** normal `Cva::open` already uses `finish_without_reclaim`; retain a byte-neutral visible projection and leave actual free/coalesce to explicitly authorized higher maintenance.
- **Stream:** persist begin/append/interrupted checkpoint records and accepted-prefix state only. Runtime owns message sessions, when to persist/notify/resume, and host transcript authority. Echo owns reasoning/tool activity traces.
- **Public visibility:** no arbitrary writable store fields or raw file access. Export only concrete operations and value/typed-error types required by current consumers, not wholesale `pub` for every former `pub(crate)` helper.

## 6. Move-once implementation sequence

A bounded red migration window is acceptable, with local coherent compilation checkpoints. The old main repository is not modified.

1. Confirm current branch, source/test and consumer inventory. Inspect the specific post-baseline changes for useful Archive fixes. Use direct source searches; Arcana only if current/useful. Do not generate architecture proof datasets.
2. Move core Archive models, codecs, indexes, publications, local versioned replay and `ArchiveOpenState` into the existing crate. Move persisted source `ImportReceipt` DTO definitions *once* out of `imports/contract.rs`; use actual `reliquary-container` dependency. Independently compile.
3. Move Archive-owned Episode, fragment, file/attachment, source binding, import-receipt, source project correlation, compaction and stream record implementations. Lift cross-owner Memory target validation, retention/manifest policy, project authorization and Runtime calls above Archive. Do not pre-extract another full crate.
4. Migrate root `Cva` create/open/replay, import, history, retrieval, project and interaction consumers **directly** to the actual crate. Maintain original physical replay order and owner-error precedence; keep existing product exports only where truly public.
5. Delete displaced `src/archive` owner implementation paths and root `#[path]` declarations. Keep Echo and higher-level runtime/search source in their rightful pending owners. Move existing Archive-local tests to Archive; keep mixed-owner tests at the root seam. Update documented file locations and behavior. Finish as one reviewable implementation commit, not a series of migration-only wrappers.

## 7. Existing test seams and acceptance

**Crate-local candidates:** `archive_publication_{core,writer,replay,codec}_tests.rs`, `archive_activity_{tests,preservation_tests}.rs`, `episode_tests.rs`, `fragment_tests.rs`, `file_tests.rs`, `file_memory_link_tests.rs`, `turn_ingest_tests.rs`, `conversation_compaction_tests.rs`, `interaction_stream_compact_tests.rs`, `project_history_tests.rs`, `archive_tests.rs`. Move tests with their owners when no dependency on `Cva`/Import/Memory/Runtime remains. Do not add a dev dependency on root to keep tests under the Archive package.

**Integration behavior to preserve:**
- REL create/reopen and bounded historical replay yield identical source facts and version clocks; open preserves bytes and earlier semantic failure ordering.
- Node/branch chronology and tombstone closures, current-vs-historical views, exact Episode ancestry ranges, live/finite import finalization.
- Source import receipt retry idempotence, source revision conflict rejection, attachments and provenance closure. Import epochs and completion are still coordinated above Archive.
- File-to-Memory links validated only after both owner replays; project file binding hash/ID remains correct and independent of Git/Lore host authority.
- Explicit prune/reclaim and valid historical cuts operate under Maintenance authorization; stale compaction chunks remain physically intact on ordinary open.
- Partial/interrupted runtime checkpoint bytes and continuation reopen match prior behavior without duplicating host canonical transcript or Echo.
- Existing product `Archive`/model API exports and `Cva::search_archive`/CLI behavior still work against one Archive owner.

**Actual gate:** `cargo check -p reliquary-archive --locked --offline`; focused `cargo test -p reliquary-archive --locked --offline`; relevant root source/Import/History/runtime tests; `cargo check --workspace --locked --offline`; `cargo fmt --all -- --check`; and independently locked CLI check (`cargo check --manifest-path cli/Cargo.toml --locked --offline`). Confirm Cargo imports with `rg`: `reliquary-archive` has **only** `reliquary-container` as an internal dependency, root points downward, old source definitions are deleted, no Echo/Import/Memory/Maintenance/Runtime reverse edges and no dual implementation survives.

Authoritative six-file data must never be mutated in place. Post-baseline `CVAFRS07` semantic corpus admission remains separately blocked until recovery; identify any skipped expensive root tests explicitly rather than claiming a full-suite pass. Full product integration gate follows later owner recovery.

**Completion = real Archive ownership and working consumers**, not empty-crate compilation or synthetic graph receipts.

## 8. Out of scope

No Echo extraction, Memory or Insomnia changes beyond needed caller wiring, fresh-history schema migration, inferred semantic import settlement, new retrieval ranking, provider/host integrations, Project Environment feature work, rewrite redesign, global transcript search, extra crate extraction, custom compiler graph engine or roadmap features.
