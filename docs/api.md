# Rust API Reference

Parent index: [Documentation index](INDEX.md)

## External Index Store crate boundary

`reliquary-external-index-store` owns the current `CVAXIDX2` codec and
disposable source-index state; root reexports the canonical persisted
`ExternalMessageRef`, `ExternalChunkRef`,
`ExternalMessageDescriptor` and `ExternalSourceError` types.
Root-facing `ExternalIndexStatus` retains its public representation.
Public source-provider, gateway authorization, query and hydration
contracts do not change. `SourceState` remains borrowed by internal
index consumers through the same concrete type; no mirror cache is used.
See [implementation evidence](external-index-store-crate-extraction-evidence-2026-10-27.md).

## Memory Core crate boundary

The root product reexports canonical Memory values from `reliquary-memory-core`.
The crate's `MemoryStore` and `MemoryOpenState` expose Memory admission and replay;
Archive provenance checks, REL/PHY capabilities and grouped carrier receipts are
composed by the product. Memory source Episode references carry the exact stable
32-byte foreign identity. Chronos results become Memory-owned temporal snapshots
before publication. `MemorySourceRef` retains foreign owner lineage; a PHY does
not resolve that reference against a different REL Archive during open. Product
lifecycle APIs map owner terminal coordinates into
`LifecycleState`, `TombstoneOutcome` and historical payload availability.


## Perception Store boundary

The root reexports the canonical Entity and Relationship identities from
`reliquary-perception-store`. Durable Entity, mention-resolution and existing
Relationship records are independent owner stores backed by Container. Product
facades retain Memory evidence validation, owner authority and Graph coordination.

`Cva::entity_resolution(key)` and `Phylactery::entity_resolution(key)` return
`Option<MemoryEntityResolution>` by value. The mention-key parameter and the
record's key remain Memory Core values; root converts the store's exact foreign
wire coordinates without keeping another persistent implementation or cache.
Callers no longer need `.cloned()` and compare against `Some(record)` rather
than `Some(&record)`. This extraction preserves storage bytes and does not add
Observation synthesis.

## Freshness Store boundary

`reliquary-freshness-store` is the compiled owner of Freshness policy and score
values, durable records, accepted event and use receipts, semantic/import
settlements, rewrite baselines, and replay. It depends on Container for physical
history. Root facades retain the public `MemoryId` API and explicitly convert
Memory, Fragment, and history-generation bytes at product boundaries. Candidate
selection, graph propagation, semantic planning, historical event construction,
and owner authorization remain in higher-level services.

The source ownership and root integration are complete. This does not establish
the separate promoted CVAFRS07 recovery gate: recovery and verification against
the authoritative six-file current-format corpus remain unverified. See the
[extraction specification](freshness-store-crate-extraction-spec-2026-10-26.md)
and [implementation evidence](freshness-store-crate-extraction-evidence-2026-10-26.md).

## Insomnia work read

`Cva::insomnia_work(episode_id)` returns an owned `Option<InsomniaWork>`. A
returned work value remains valid while the Cva continues processing and
mutating that episode. This is an intentional API signature change from the
previous borrowed result.

## Dream Store boundary

`reliquary-dream-store` now contains the `CVDREAM2` Begin codec, canonical
population digest and `CVDREAM1` journal projection. It uses fixed 32-byte
Memory identities and an owned graph-cut proof value. The root adapter
`src/freshness_dream_journal.rs` converts between these values and product
identities/Freshness proof values. The crate currently declares no direct
Reliquary crate dependency; its implemented work uses external `sha2` (and an
optional test-oracle `serde_json` feature).

The caller remains responsible for authenticating the compact Begin
population against the physical prefix and for authorizing graph/community
proofs. Replay retains the physical `before_frame` order and prefix-before-
Begin rule. Dream inference, pair ranking, settlement decisions, Graph edits,
Community derivation, Memory population selection and Freshness policy remain
higher-level responsibilities. The compiled lower implementation and root
adapter are integrated. Lower, workspace, formatting, CLI and full library
checks pass. See the [Dream Store specification](dream-store-crate-extraction-spec-2026-10-27.md)
and [implementation evidence](dream-store-crate-extraction-evidence-2026-10-27.md).

## Archive source owner

The product `Archive` API is backed by the independently compiled
`reliquary-archive` crate. That crate owns Archive source records, codecs,
ordered replay and read APIs, and directly depends only on
`reliquary-container`. The root `Cva` facade composes Archive with Memory,
Import and Maintenance. Archive publication bytes and the current storage
format are preserved by this boundary. Runtime session/stream behavior, Echo
reasoning evidence and retrieval policy remain in `src/runtime_inference/`,
`crates/reliquary-echo/` and `src/retrieval/`, respectively.

## Echo source owner

The root product's public Echo event types are re-exports from
`reliquary-echo`. That crate owns the `CVAECHO1` codec, replay, source-keyed
ordering and append behavior, and depends only on `reliquary-container` among
Reliquary crates. The product retains REL open/write admission and maps
`EchoError` into `CvaError`; Runtime retains access grants and source selection,
and Context Engine retains rendering policy. `ToolContextAccepted` persists as
private receiver acknowledgment and is excluded from rendered model evidence.

## Physical Container access

The existing product exports for `Container`, `ContainerIdentity`, `FileKind`,
`FormatVersion`, `ObjectRef` and `ContainerError` refer to the independently
compiled `reliquary-container` implementation. Root stores consume the crate
directly. Ordinary opens preserve bytes and report incomplete final physical
frames with `ContainerError::RepairRequired`; complete corrupt records reject.
Concurrent cooperating writers fail with `WriterConflict`, and read-only
sessions cannot append or repair. Higher owners retain all semantic replay,
authorization and explicit recovery decisions.

Low-level consumers use `physical_cursor()` or `physical_cursor_through(end)`
to pull opaque frames with physical positions and the observed global frontier.
Owner opening uses `open_for_replay` and accepts the complete cursor state only
after ordered semantic ingestion succeeds. Until then, physical writes are
blocked; an earlier owner error retains precedence over later frame corruption.
`read_source()` retains the held file lease for subsequent bounded reads; an
active cursor fences local mutation. Import closes general physical admission
and retains a session-bound `WriteAdmission`; its owned `WriteScope` guard
restores the closed state on normal return or unwind. The token grants no
semantic permission by itself.

For an explicitly approved recovery, `open_for_repair` acquires writer exclusion
but blocks ordinary append/allocation. `plan_tail_repair` validates the physical
prefix and returns opaque identity/length-bound evidence. The higher owner
validates that prefix and authorizes its loss boundary before calling
`repair_incomplete_tail`. Repair rechecks the plan and rescans physical clocks;
the caller then drops the repair session and reopens the semantic owner for
verification. A complete corrupt record cannot be repaired as an incomplete
tail.

Ordinary `Cva::open` and `Phylactery::open` validate semantic state but do not
append Entity-resolution repairs or reclaim stale compaction chunks. REL
compaction supersession is projected in memory without rewriting the source.
When a caller explicitly approves canonicalizing retired Entity references,
`Cva::repair_retired_entity_references()` or
`Phylactery::repair_retired_entity_references()` returns the number of durable
revision repairs and synchronizes on change. Storage reclamation remains an
explicit owner maintenance operation. `ObjectRef` keeps its private fields;
read-only physical offset and serialized-byte accessors are retained for
persisted owner codecs and historical queries, but do not grant I/O authority.

## Ego terminal lifecycle

REL and PHY expose `ego_anchor_lifecycle(id)` and `historical_ego_anchor(id, revision)`. PHY additionally exposes `ego_identity_lifecycle(id)` and `historical_ego_identity(id, revision)`. Lifecycle inspection returns the shared `LifecycleState`; historical getters return an optional retained nondeleted revision, including after deletion. A deletion revision itself has no document payload. Existing `delete_ego_identity`/`delete_ego_anchor` return `true` on publication and `false` on an exact retry using the original expected revision; mismatched revisions reject.

## Native historical import API

`Cva::prune_tombstoned_history(output, request)` and `Phylactery::prune_tombstoned_history(output, request)` create separately validated copies under shared owner-maintenance evidence. Both support terminal Memory, Entity, Relationship and retired CompatibilityProfile payload when the concrete domain retention proof succeeds; REL additionally supports Archive conversation payload. Shared/active or unknown references reject. A second prune pass over an already pruned owner is currently refused. Ordinary reclamation is a separate conservative operation.

Legacy normalization and migration APIs have been removed after canonical promotion. `rel repair` and `phy repair` require current semantic settlements, retain the requested active profiles, optionally prune eligible retired vectors and reclaim storage, and verify a separate output without re-embedding.

## Fresh-history rewrite API

`rewrite::{RewriteRequest, RewritePlan, RewriteReport}` provides the policy, read-only resolved plan and execution report. `plan_rewrite_file(source, request)` dispatches by REL/PHY file kind; `rewrite_file(source, output, request)` plans and executes; `rewrite_from_plan(source, output, plan)` executes a frozen plan after checking the source snapshot. The policy supports new-owner or preserve-owner identity, Memory filters and revision policy, zero or more compatibility profiles, Freshness mode, REL Archive selection/provenance policy and optional REL synthesized Ego state. Existing outputs and source drift reject before output creation. PreserveOwner advances history generation and records predecessor lineage; reconciliation across generations fails with typed `HistoryGenerationConflict`. See [CLI](cli.md), [operator workflow](manual/migration-and-reconciliation.md#rewrite-a-fresh-history), and [behavioral contracts](behavioral-contracts.md#fresh-history-rewrite-contracts).

Historical reconstruction refuses to reuse an earlier import's settlement as a retry of a different open epoch. After an accepted scoped settlement, `Cva::complete_native_import(import_id, now_ns)` is the public exclusive completion API; it derives attributed readiness and returns exact typed blockers. Production marker append is restricted to the coordinator by an opaque authority; direct store append exists only in test builds. Routed PHY references remain unsupported. Divergent reconciliation preserves the left prefix and replays completed right-tail imports through concrete owners, including grouped Memory publication and accepted Freshness/Dream/Perception evidence. It regenerates destination epoch evidence. Open epochs, external PHY publication and unsupported owner payloads remain refused. Mounted completion remains unavailable; see [current limitations](current-limitations.md).

The public `reliquary_memory::imports` module exports schema-1 DTOs, epoch status/completion types, `ImportReadinessOwner`, `ImportReadinessStatus`, `ImportReadinessBlocker`, `ImportReadinessReport`, `ImportVectorReadiness`, and the four byte-only normalizers. `Cva::import_native(&NativeImport, now_ns)` and `Cva::complete_native_import(import_id, now_ns)` are exclusive/offline operations. `Cva::native_import_readiness(import_id)` is a read-only report for the current open epoch: it returns the import ID, an optional typed owner blocker, and vector status; unknown, completed and noncurrent IDs reject. Readiness derives exact receipt, accepted scoped-settlement, imported Episode, Insomnia-output, local Memory, initial Dream and active, non-superseded Dream-processed output Memory mention-key Perception evidence; callers cannot provide proof booleans or digests. Every nonzero local output ID must have its exact retained same-epoch InsomniaCompletion record, a revision-1 Memory record attributed to that same Episode, and a matching retained first mutation. An output ID without that evidence is `Memory/ConflictingEvidence`; owner-supplied IDs or caller-provided Memory provenance alone do not suffice. A pending readiness outcome identifies one owner, exact identity and `Missing`, `Pending`, `Failed`, `ConflictingEvidence` or `Unsupported` status. PHY external Memory refs return `Memory/Unsupported` because this REL-only API cannot authenticate a second owner. Vector status is `NotRequired` only when there are no source Nodes and no Memory outputs; otherwise it is `SupportedUnavailable` and does not certify complete vector coverage. For an empty receipt (`receipt.conversations.is_empty()`), the authenticated scoped settlement must also be a true no-op: no source Nodes, births, cohort, replacement records or superseded IDs, and `base_cut == settlement_cut`. Dormant/unresolved mention keys for an active, non-superseded Dream-processed output Memory remain blockers; only inactive, archived or superseded Memories have no mention obligation. A Dream pass already pending for an import-owned birth remains a blocker if its Memory becomes archived or terminal; an archived birth without pending work does not start a new pass. The report does not wait for unrelated work or unscheduled Ego work. Completed marker layout is unchanged; completion still syncs its marker before reopening writes. There is no mounted completion operation. [Native historical import](native-import.md) owns the detailed readiness boundary.

## Agentic Memory retrieval

`AgenticMemoryController::new()` creates empty host-owned, in-memory state for one requesting-agent turn. `current_query()`, `searched_ids()`, and `selected_ids()` expose read-only views over owner-qualified `MemoryRef` values; there is no gateway registry or durable controller state. The REL-only gateway workflow exposes exactly three model operations: query, refinement, and one-hop graph traversal. Query unions the full lexical and exact-semantic candidate pools before Freshness ordering and the top-ten cut. Refinement considers previously declined candidates in deterministic batches of at most ten without rerunning ordinary retrieval. Traversal searches each anchor in both directions, concurrently, and applies a separate top-ten new-candidate cap per anchor; duplicate neighbors return once while retaining all applicable anchor reports.

`PreparedAgenticMemoryResult` is ephemeral and does not itself prove handoff. Only the host receiver's confirmed exact-content transfer may complete a manifest, update turn-wide `SelectedIDs`, and submit the existing accepted-use receipt. The receiver acknowledgment and exact-manifest restart recovery are evidenced in Warlock integration candidate `cf41c66` (eight focused host Memory tests pass); this is not a deployed release. The previous CVAFRS03 reader mismatch is removed by the consolidated Freshness implementation; frozen-corpus fidelity must be rerun on consolidated `main` before release. See the [retrieval design](agentic-memory-retrieval-design-2026-10-15.md), [host seam](agentic-memory-host-handoff-seam-2026-10-16.md), and [verification record](agentic-memory-retrieval-verification-2026-10-16.md).

## Freshness retrieval boundary

Memory fetch/search, graph/provenance inspection and `ego_memory_freshness` are non-reinforcing reads. Only a trusted host receiver's confirmation of exact content handoff authorizes completion and `record_accepted_memory_use`; API owner/turn checks cannot prove consumption. Warlock candidate `cf41c66` implements receiver acknowledgment and exact-manifest replay, but is not a deployed release; the previous CVAFRS03 format blocker is removed, but consolidated frozen-corpus acceptance must be rerun before release. See the [host contract](freshness-external-consumption-contract.md) for receipt ownership, access control and durable retry semantics.

## C8 historical embedding-only operation

After offline native import and complete Insomnia/first-cycle Dream generation, use ConfiguredRuntime::reconstruct_historical_freshness(&mut Cva, options, limits, &AtomicBool). It selects the configured embedding route only when fill_missing_embeddings=true and no checkpoint is accepted. Cva::reconstruct_historical_freshness_with_endpoint exposes the same exclusive owner operation for an explicit bounded EmbeddingEndpoint. The ordinary endpoint-free operation still returns EmbeddingFillUnavailable for a new fill request; accepted retries resolve first. All defaults keep fill disabled.

HistoricalEmbeddingLimits can only lower the frozen C7 ceilings. HistoricalReconstructionReport.embedding holds optional transient coverage/cost; HistoricalReconstructionError::EmbeddingFill retains a structured HistoricalEmbeddingFailure, including durable generation progress and whether physical staging writes were attempted. No-fill and accepted retries return no fill report. Cancellation is cooperative at provider/write/freeze/acceptance boundaries; an already-started native checkpoint acceptance completes atomically.

Only missing rows of existing genuine Archive fragments are requested, including mixed-turn fragments. Ordinary Archive rendering preserves exact role/content text with separators under a total byte bound. Existing rows retain identical bytes; invalid retained rows remain explicit coverage. Fill creates no Fragment records. No target-Memory embedding, Insomnia/Dream rerun or synthesized text occurs. Requests run serially through EmbeddingEndpoint::embed_bounded; implementations default to unsupported, while the simulator and OpenAI-ready endpoint enforce bounded output. Remote bounded requests do not follow redirects, retry or fan out and keep the existing 120-second timeout. Provider work stays outside runtime locks. This is an offline/exclusive post-generation surface; Gateway mounted historical management is unavailable. See the [C7 contract and C8 verification](subsystems/freshness.md#historical-reconstruction).

Coverage identifies each `fragment_id`, its canonical `source_keys`, availability `position`, reason and row counts. Availability is the latest accepted Archive activity represented by the fragment; every represented Node remains provenance evidence. `unavailable_fragments` reports absent source evidence, and `new_fragments` is always zero. The operation has no exact-turn embedding or fragment-creation lane.

Historical fill preserves partial rows and generation history without activating live semantic retrieval. Live publication requires exact current `FragmentId` coverage; equal row counts with different identities do not qualify. A later complete fill can activate the profile. Retiring a compatibility profile excludes its generation from current selection but leaves packed bytes and Memory/Archive vector mappings readable. Numeric packed data becomes unavailable only after explicit manifest-authorized transformed-copy pruning; reopen retains the profile, binding, and generation topology and returns `PayloadUnavailable::Pruned` for numeric reads.

Historical Dream authority resolves accepted duplicate components together with supersession chains, including an archived alias whose representative was later superseded. Each supersession pointer must agree with the unique accepted Supersedes edge and a proved owner-local publication; cycles or conflicting pointer/edge evidence reject. Resolution reads existing authority and does not revise Memory or Graph history. Ordinary imports retain genuine first-cycle proof. Missing proof is unsupported; the one-time retained-fixture legacy approximation is not an automatic import fallback.

## Purpose
This document owns the current public Rust library surface exposed by `reliquary_memory`.
## P5-C exact admission and transcript continuation

The isolated P5-C integration adds `conversation_submission_status` (ViewRel) and `process_conversation_submission_if_next` (SubmitConversation). Matching occurs inside the existing coordinator's ordered reservation; a mismatch returns no receipt and leaves queued work untouched. This is not exact-ID scheduling around the queue.

Gateway transcript reads join native prefixes under the existing owner → coordinator → journal observation order. Native stream records are excluded from the legacy session projection; only the exact requested leaf's journal-acknowledged continuation is reconstructed through the existing byte-length/hash-validated checkpoint reader. Historical/sibling leaves do not receive that continuation, completed generations remain Archive turns, and ViewRel/mount fencing still applies. Legacy non-native streams retain their existing projection. No stored format or second mutable authority changes.


## Overview
Graph and Community state are implemented by `reliquary-graph`, whose only
Reliquary dependency is Container. Supported `Cva`/`Phylactery` Graph entry
points retain typed product identities and cross-owner endpoint admission;
the storage crate uses opaque foreign identity bytes. Persisted Graph and
Community formats, relation semantics and naming precedence are unchanged.

Vector storage implementation is partially extracted to `reliquary-vector-store`; the public REL/PHY facade remains at the root boundary, and root focused vector/retrieval and detached CLI verification are pending. See the [extraction evidence](vector-store-crate-extraction-evidence-2026-10-26.md). The public API exposes `ReliquaryConfig` and model-switchboard types for machine-local runtime routing; `InteractionRuntime` for transport-neutral live session coordination; `Reliquary` for the homogeneous non-user scope storage/runtime model; and the separate `Phylactery` type for user-global state. Every newly created REL/PHY has a durable owner UUID. Current REL owner IDs use `rel-<uuid>` and PHY owner IDs use `phy-<uuid>`. Reliquary includes Archive history, Episodes, Memories, Graph, derived community snapshots, Insomnia state, embedded files/source attachments, vector backing/publication, compatibility profiles, reconciliation, hybrid retrieval, durable REL metadata (`type_label` plus directed REL dependencies), REL Anchors, and one cached owner-local Ego web synthesis. Phylactery exposes mutable principal profile metadata (`display_name`), Memories with optional owner-qualified external source references, Graph, derived community snapshots, Packed Vectors, Memory Vectors, Compatibility Profiles, Identity, Personality, Anchors, and one cached owner-local Ego web synthesis. `reliquary-ego-store` is the compiled durable-state implementation; REL/PHY methods preserve the existing product API while translating store errors at the facade. `Cva` remains exported as the Reliquary compatibility/internal type name. Ego inference/context assembly is not yet part of this API; this surface currently establishes only the durable owner-state substrate. The API remains development-stage.
## Exact contract
### Exact-owner background operations

`GatewayRuntime::set_routes(routes)` replaces the trusted machine-local endpoint bundle and wakes all mounted owners. It rejects gateway shutdown and clears each REL worker dispatch cooldown from the previous provider. Persisted Episode retry deadlines remain unchanged; no instance selection or durable semantic state changes. Endpoints preserve their existing finite fallback semantics.

`background_status_for(instance, rel_id)` requires exact ViewRel and returns the existing `RuntimeBackgroundStatus` with REL counters only. `phy_background_status_for(instance, phy_id)` requires exact attached UseOwnPhy; its REL fields are zero and its private counters never enter REL feeds. `wake_rel_background_for` requires ManageRel; `wake_phy_background_for` requires UseOwnPhy. Detached, revoked and stale owner generations fail through canonical admission.

`set_rel_background_enabled_for(instance, rel_id, enabled)` controls new Dream/Perception/vector dispatch for that REL; `set_rel_processing_enabled_for` remains the separate Insomnia switch. `set_phy_background_enabled_for(instance, phy_id, enabled)` controls all three private workers. Switches are ephemeral and may not undo an already admitted commit. `background_worker_state_for(instance, owner_id)` returns `(live_worker_count, background_enabled)` under the applicable ViewRel/UseOwnPhy boundary; quiesced owners require reopen before instance operations. Read counters and dispatch switches do not acknowledge semantic writes or publish private data.

Only ConversationExecution is eligible for Host authority. Insomnia/Dream/Embedding/ProjectEnvironment lack executable host result paths and retain their existing Native implementation, regardless of enum declarations or advertised queue guarantees. Native work with no configured endpoint remains pending. Existing durable host status/cancel/reconciliation remains authoritative for unresolved operations; the [B capability ledger](multiplexing-implementation-plan.md) supplies the C/D handoff.

### Local configuration
`ReliquaryConfig::new(path)` creates an in-memory default config, `ReliquaryConfig::open(path)` loads `reliquary.cfg`, and `save()` validates and atomically replaces the current file. Public fields are `fragments: FragmentConfig`, `retrieval: RetrievalConfig`, `models: ModelSwitchboardConfig`, and `credentials: CredentialsConfig`; `path()` reports the configured path. `validate_runtime()` verifies model-route and credential compatibility without executing a provider request. `load_or_create_master_key()` generates or reloads a 256-bit local master key from temporary sibling `reliquary.master-key.json`. Credential objects are encrypted/decrypted automatically on save/open. Unknown framed objects are preserved across saves, except the retired `models.chronos` object, which is rejected. Configuration is separate from `.rel` and has no semantic history.

`ConfiguredRuntime::open(config_path)` is the high-level configured execution seam for operator/runtime adapters. It owns `ModelSwitchboard` construction and configured provider endpoint composition. `runtime_routes()` returns a `ReliquaryRuntimeRoutes` bundle containing the exact configured General/Insomnia/Insomnia-metadata/decision/Dream/Embedding endpoints; the bundle owns effective fallback semantics for Gateway-owned REL/PHY workers. `run_insomnia_files` owns the finite REL/optional-PHY Insomnia workflow, including ownership-route fallback, queue finalization, publication, Memory-vector completion, sync, and reporting. `probe_embedding` and `build_archive_vectors` own configured live embedding execution. Adapters should pass operator options or exact provider adapters into these library surfaces rather than reconstructing semantic workflow routing.

`ReliquaryRuntimeRoutes::new(...)` accepts exact host-provided endpoints. Effective main Insomnia is `insomnia ?? general`; `with_temporal_decision(...)` supplies the temporal choice endpoint; configured routing selects General Decision, then General, or leaves ambiguity unresolved; effective Dream is `dream ?? general`; fixed-group metadata classification is optional; ownership classification is `insomnia_metadata ?? effective_insomnia`; and post-wording **Entity-mention enrichment** uses `insomnia_metadata ?? effective_insomnia`. Lexical Memory routing is deterministic and uses no model route. Embedding has no fallback. `ModelCapability::route_key()` exposes the canonical external keys for hosts with their own generic configuration stores. There is no separately persisted ownership or enrichment route yet.
### REL activity reads

`Cva::rel_turn_count() -> u64` returns the REL-local count of first accepted exact `user`/`assistant` identities. `Cva::activity_position_for_turn(conversation_id: &str, node_id: &str) -> Option<u64>` returns an eligible turn's original one-based position, including on replay, or `None` for missing/noneligible Nodes. Archive exposes the same reads through `Cva::archive()`. Both turn acceptance paths share the Archive projection; existing `append_node*` and `ingest_turn` return types remain unchanged. Reads do not write or sync, and the low-level mutation durability contract is unchanged. Existing version-linked history reconstructs the projection without a mutating migration. `Cva::activity_cut_at_archive_version(version: u64) -> Result<u64, ArchiveError>` returns the preceding eligible activity count at a validated Archive boundary using binary search over sparse transitions. Version zero returns zero; versions above Archive head return `InvalidArchiveRecordVersion`. Archive exposes the same query. Live-runtime receipts expose the canonical activity outcome after sync; see [REL activity clock](subsystems/rel-activity-clock.md).

Principal backfill, project-backed attachment repack, storage reclamation and copy-only reconciliation preserve activity positions and every historical cut exactly, validated after reopening. Fresh migration preserves the eligible identity set, count and acceptance order; divergent reconciliation accepts the left history followed by novel right identities, counting compatible duplicates once. Replayed histories may rebase Archive versions, so numerical source cuts require explicit mapping before reuse.

### Owner-local Memory Freshness (development-stage)

`Cva::freshness_record(id)` exposes the current owner-local signed Freshness record; `freshness_record_at(id, turn)` and `freshness_record_at_cut(id, cut)` provide turn-only or event/graph-qualified historical projections. Reads do not allocate Memory revisions or new global semantic versions. Normal first-accepted Memories initialize at +100 with an authenticated publication birth; ordinary extracted Memories do not start decay until accepted active first Dream settlement, while first publications explicitly already in `knowledge` or `canonical` may admit at birth.

`Cva::enroll_legacy_freshness_baseline(id)` is the **explicit** one-time migration surface for an existing nonarchived active Memory with no first-publication Freshness proof. It anchors +100 at the *current accepted owner REL turn*, never backdates or fabricates Dream first-cycle birth authority, and is idempotent after reopen. Proven new Memories must use their accepted publication intent instead. Low-level initial Dream settlement and admission are internal owner adapters, not public arbitrary event publication APIs.

`Cva::record_accepted_memory_use(AcceptedMemoryUseReceipt, workers)` requires a caller-attested real consumed-context/use identity, matching owner UUID, already accepted REL cut and explicitly named Memory sources. It deduplicates per actual delivery before recomputing topology, publishes one atomic accepted-use batch, and rejects incompatible reuse of a receipt identity. Internal search/hydration is **not** consumption and emits no receipt. The library does **not** currently provide a universal live delivered-context producer. The source contract, pre-append validation and explicit legacy enrollment were implemented under [R2](freshness-r2-verification-2026-10-03.md); R2 and R3 focused runtime gates passed; broader core evidence is recorded separately in the [core verification report](freshness-core-release-verification-2026-10-03.md). Historical reads reject future REL turns, unavailable future Graph versions and blank event-ID bounds.

### Context Engine

Reliquary owns deterministic context-compaction policy through the exported Context Engine types and functions. `ContextView`, `ContextTurn`, `ContextRole`, and `ContextMessage` are provider-neutral context inputs. `request_context(view, checkpoint)` applies a valid branch-local `CompactionCheckpoint`, emits the compacted-history continuation instructions when the checkpoint belongs to the current branch, and returns the provider-neutral `ContextRequest`; stale checkpoints are ignored. `request_messages` remains the lower-level message-only projection.

`compaction_budget`, `compaction_needed`, `budget_satisfied`, `plan_compaction`, and `plan_session_compaction` own trigger/target hysteresis, raw-tail preservation, bounded compaction-input selection, continuation-map instructions, checkpoint advancement, and summary-token limits. Provider adapters supply a `ContextTokenCounter` because exact token accounting remains model/provider specific. `normalize_summary` enforces the deterministic summary-token bound.

`render_assistant_content_from_echo` maps durable Reliquary Echo events into deterministic prior-assistant evidence context, including reasoning/commentary/tool evidence while excluding activity-only events. The generic `render_assistant_content` variant accepts already-normalized `ContextEvidence`.

Host applications may map `ContextRequest` and `ContextMessage` into provider wire DTOs and execute the summarization request, but they do not own checkpoint applicability, compaction selection, raw-tail policy, evidence rendering, summary instructions, or normalization.

### Project Environment host
`ProjectRepositoryMode` selects `ManagedLore`, `ManualLore`, or `Git` bootstrap policy. `GatewayRuntime::create_project_environment_rel` and `adopt_project_environment_rel` own repository discovery/bootstrap, durable repository identity, initial revision correlation, and REL creation/adoption. `inspect_project_adoption_source` validates an uncorrelated durable-owner REL before adoption.

For mounted Project RELs, `validate_project_environment_for` verifies the supplied local project directory against the durable repository identity and refreshes only a moved repository-relative `project_path`; `validate_project_environment_cva` provides the same check before mount. `checkpoint_manual_project_for` owns explicit externally-managed Lore checkpoints. `project_repository_root_for` resolves the repository root after identity validation.

`ingest_project_attachment_for(instance, owner_id, ...)` owns Lore/Git attachment ingestion, exact snapshot/checkpoint revision creation, correlation, and `ProjectFileRef` registration. `resolved_file_bytes_for` resolves repository-backed files from their exact historical revision and falls back to embedded REL bytes for ordinary files. Existing `warlock.repositoryId`, `.warlock/`, and `warlock/uploads` names are retained only as repository-format compatibility identifiers.

### Managed conversation selection

`GatewayRuntime` owns mounted execution and durable conversation coordination; each attached `InstanceRuntime` owns its REL, conversation, branch and historical-leaf selection. `admit_conversation_writer(instance, owner_id, conversation_id, branch_id)` binds the shared owner continuation to an existing Archive branch. `select_managed_conversation_leaf` changes only the requesting instance's historical cursor; `prepare_conversation_branch_at_leaf` requires SubmitConversation and creates an explicit durable continuation. `clear_managed_conversation_selection` clears only that instance's selection.

`LiveSession` provides authorized durable generation, checkpoint, interruption, completion and stream access. Accepted work stays in the REL journal across viewer detach, selection changes and restart. The old host-wide active REL/PHY/session selector and managed-session facade have been removed. Lower-level `InteractionRuntime` and persisted Archive/non-native checkpoint readers remain available.

### Cross-owner semantic access

Gateway ambient reads take the initiating `InstanceRuntime`; explicit reads take both that instance and the exact owner ID. Visible RELs are the selected REL's granted dependency closure. Private retrieval additionally requires the instance's exact bound PHY and UseOwnPhy grant. Merely mounting an owner grants no visibility.

`search_visible_memories`, `search_visible_archive`, owner-qualified Knowledge mutation and provenance resolution, and exact-path Echo expansion use the canonical Gateway authorization and delivery fences. Query embedding and filesystem/provider work run outside owner locks; original authorization is revalidated before publication or delivery. No instance-free legacy selector or query fallback remains.

### Sequential finalization (P3.4)

Fresh submissions must name the selected Archive branch head as their expected parent. A branch head remains valid when another branch has extended its prefix. Existing accepted submission IDs are immutable request retries, so matching retries can return their durable status after the branch advances or the payload compacts; changed parent/content/scope still fails journal validation. Sequential processing appends at the current head only when the submitted parent belongs to that branch ancestry. Unrelated ancestry leaves the entry Pending with an error. Captured finalization and restart refuse to move an independently advanced branch backward. Pending work contributes no Archive turn, Activity tick or Episode.

### Branch continuation processing and durable scheduling (P3.2–P3.3)

`admit_conversation_writer` binds an existing Archive branch through the one owner runtime. `process_next_conversation_submission(instance, owner_id, conversation_id)` returns `Result<Option<GatewaySubmissionReceipt>, ReliquaryRuntimeHostError>` and selects the oldest eligible journal entry across branches; `None` means idle or every candidate is blocked. Selection and branch reservation are atomic. Held host continuations are skipped so unrelated branches can progress. `resume_conversation_submission(instance, owner_id, conversation_id, submission_id)` reconciles an already dispatched host continuation, Finalizing capture, unfinished post-commit work or terminal receipt; it rejects fresh Pending work. Both APIs require current SubmitConversation and retain delivery revalidation. Exact-entry fresh processing is test-only. Detach does not release accepted owner work; restart restores journal order and branch reservations. Native assistant generation/checkpoint control is implemented in [P3.6](multiplexing-implementation-plan.md). See [P3.3 acceptance](multiplexing-implementation-plan.md).

### Durable conversation policy (P3.1)

`ConversationPolicy` exposes SequentialChronological and VisibleAutoBranch. `ConversationMetadata` contains `policy` and `policy_revision` alongside ID/title/active. `Cva::conversation_policy(conversation_id)` and Archive's corresponding read return `(ConversationPolicy, u64)`; absent legacy metadata means sequential at revision 1. `Cva::set_conversation_policy(conversation_id, policy)` publishes only a changed value for an existing conversation and returns a change flag; direct callers synchronize before acknowledging durability. Title and active changes preserve policy state.

`GatewayRuntime::conversation_policy(instance, owner_id, conversation_id)` requires current exact-owner ViewRel. `set_conversation_policy(instance, owner_id, conversation_id, policy)` requires current exact-owner ManageRel and synchronizes before returning. Pending submissions capture the current policy revision at durable FinalizeWon; already Finalizing/Committed ancestry and revision survive subsequent changes and restart. Default sequential finalization validates selected-branch ancestry and captures its current safe head. VisibleAutoBranch preserves an accepted stale expected parent on a deterministic non-canonical branch; FinalizeWon freezes that branch ID, parent and display label before Archive publication, and only the original submitter's current attachment may move its instance-local selection after durable commit. See [P3.5 acceptance](multiplexing-implementation-plan.md), [P3.4 acceptance](multiplexing-implementation-plan.md), and [P3.1 acceptance](multiplexing-implementation-plan.md).

### Gateway and instance runtime (Phase 1 verified)

The legacy host constructors and ambient selection state are removed. Gateway retains process-local exclusive owner mount claims; no legacy deployment or selector fallback is exposed.

`GatewayRuntime::search_visible_memories(instance, query, limit, depth)` embeds the query once outside owner locks, then searches the instance-selected REL's independently ViewRel-granted dependency closure and its exact bound PHY only when UseOwnPhy is granted. Lanes use established owner-local compatibility profiles and report retrieval failures independently; the query does not establish profiles or copy private data into REL storage. Original attachment, grant and mount authority is checked again before delivery. A bound PHY without private permission is omitted from both this search and `visible_semantic_owners(instance)`.

Owner-qualified semantic reads require ViewRel for REL or UseOwnPhy for the exact bound PHY; guessed foreign PHY IDs fail before consulting mount topology. `phylactery_profile(instance)` and `set_phylactery_profile(instance, display_name)` access only that PHY. Profile mutation wakes only its personal execution. `refresh_principal_projection_for(instance, rel_id)` explicitly updates an existing principal Entity keyed by the verified bound PHY ID in that SubmitConversation-authorized REL; it never creates a principal or follows selected REL. Both captured source and destination capabilities, and the private source mount, remain valid through commit. Projection wakes only the destination REL. Existing REL-only search/read APIs retain their scope.

`GatewayRuntime::new(state_dir, routes, config, episode_policy)` creates the trusted application gateway. `mount_rel_path(path)` and `mount_phy_path(path)` inspect the existing typed header and physical file identity before mutable open. Same-file aliases return the existing owner ID; conflicting UUID copies, inconsistent kind/identity and a second live gateway or legacy runtime-host execution fail closed. `owner_mount_is_path_scoped(owner_id)` reports the weaker canonical-path-only fallback when OS file identity is unavailable. These trusted mount/maintenance APIs are not untrusted request endpoints. The same restriction applies to raw mounted_rel_ids/mounted_phy_ids, owner_admission, owner_mount_is_path_scoped, instance-free quiesce_rel/reopen_rel, reconnect_host, unmount and shutdown. Client owner listing uses viewable_rel_owners(instance); client maintenance uses the ManageRel-authorized instance-taking methods. Ambient operations resolve only the initiating instance selection; explicit-owner operations authorize that exact owner. The [F3 inventory](multiplexing-implementation-plan.md) enumerates the Phase 1 public surface; Phase 2 adds the owner-qualified private and PHY lifecycle methods documented here.

Trusted service controls `quiesce_phy(owner_id)` and `reopen_phy(owner_id)` fence/drain that exact PHY, join its personal workers, sync and reconcile durable state, then restart one worker set with a new mount epoch. `quiesce_phy_for(instance, owner_id)` and `reopen_phy_for(instance, owner_id)` additionally require current UseOwnPhy authority for the instance's exact bound PHY. Original client tokens become stale. Frozen owner-admitted plans remain Held while that exact destination is fenced or offline and can resume at the same durable PHY after reopen/remount; detach retains accepted owner work. No other PHY is substituted, and unrelated owner workers continue.

Install `GatewayAdmissionAdapter` using `with_admission_adapter(adapter)` before exposing request handlers. `attach_authenticated_instance(credential)` invokes that configured verifier, which returns `VerifiedInstanceAdmission { principal_id, phy_id, grants }`; the public attachment API never accepts that result directly or lets a request choose its verifier. With no configured verifier, authentication fails closed. `refresh_authenticated_instance(instance, credential)` replaces verified grants only when the credential resolves the existing immutable principal/PHY. REL and PHY grants must name mounted canonical owners of the exact kind, and a bound PHY requires `UseOwnPhy`. Direct value-based attachment/regrant and preopened gateway mounts are internal/test-only. `select_rel` and managed-conversation APIs keep selection in instance state; `detach_instance`, `unmount_rel`, and `shutdown` release lifetimes.

Public receipt aliases include `GatewaySubmissionState`, `GatewaySubmissionStatus`, `GatewayHostOperation`, `GatewayHostOperationPhase`, and the gateway conversation/submission ID and Archive/postcommit receipt types. These expose response interpretation without exposing a raw journal writer.

After durable enqueue, accepted processing belongs to the REL coordinator. The public processing method requires a currently attached SubmitConversation caller, who may differ from the admitted principal; Archive publication retains the admitted principal. Receipt delivery revalidates that caller after processing, so detach/revocation can deny delivery while accepted owner work remains durable. An internal owner coordinator can finish retained work without attachments. Detach and grant revocation deny future client operations while retaining accepted work. Undispatched pending cancellation requires the current caller's CancelOwnPendingSubmission grant and matching durable principal. Dispatched generation/host operation control separately requires CancelSharedGeneration and a stable operation ID. Processing still checks owner mount/fence and durable admission/finalization identity. Restart keeps Pending/missing-publication Finalizing entries queued for explicit processing.

The current conversation surface includes `admit_conversation_writer`, `attach_conversation_viewer` / `detach_conversation_viewer`, `submit_conversation`, `cancel_conversation_submission`, `process_next_conversation_submission`, and `resume_conversation_submission`. `GatewayConversationSubmission` carries the composite conversation/submission IDs, expected conversation revision and parent, branch, content, attachment bytes, and optional client timestamp. A durable admission response follows `RelJournal` enqueue; finalization records the captured parent/principal/time and reconciles a stable Archive node before reporting committed status. With a configured conforming ConversationExecution host, processing first journals a stable stage identity and payload digest, then calls the host outside owner/journal locks. Receipts expose per-capability host operation phases and sticky cancellation state. Unknown status keeps the native entry pending without blind replay; definite host cancellation permits native cancellation, and Completed permits stable Archive finalization of the original user turn. Mount restores unresolved journal operations before workers start. `reconnect_host` cannot clear locally unresolved operation IDs on an empty-queue report, even after capability configuration removal. A withdrawn contract fences handoff retries, including operation updates racing reconciliation. Durable Completed/Cancelled proof is consumed without requiring a reconnected adapter; completion that won after cancellation intent remains terminal. This does not synthesize a host assistant result. Viewer attachment is a read/view registration; it does not itself acquire the writer continuation. This is the verified Phase 1 API foundation. The [F1–F6 record](multiplexing-implementation-plan.md) documents final authorization/lifecycle and integration verification; full consumer migration, privacy, branch policy and feed/subscription contracts remain later phases.

### Container
Public types include `Container`, `ContainerError`, `ChunkRef { offset, len }`, `FormatVersion`, `ContainerIdentity`, `FileKind`, `RelMetadata`, and the legacy compatibility enum `ReliquaryScopeKind`. `FileKind` is `Reliquary` or `Phylactery`. Current files persist a UUID in the container header. `owner_id()` derives `rel-<uuid>` for a current homogeneous REL and `phy-<uuid>` for a PHY; `owner_uuid()` exposes the raw 16 bytes. Legacy typed REL headers retain their historical `proj-`, `org-`, or `con-` owner prefix so existing identities remain stable while opened/reconciled. `ReliquaryScopeKind` and `legacy_scope_kind()` exist only to inspect those old typed headers and must not drive current behavior. Container still exposes opaque append/read/chunk enumeration, sync, path/format inspection, and `latest_version()` for the existing file-global semantic clock. Current semantic version allocation persists owner-local wall-clock transaction time. `transaction_time_ns(version)` returns that timestamp when the version has one. `version_at_or_before(transaction_time_ns)` returns the highest representable global-version prefix cut whose every included version has a known transaction timestamp no later than the supplied wall-clock time. It never skips an unknown or later-timestamped earlier version merely because a higher global version has an older timestamp after divergent reconciliation. Legacy `CVAVERS1` tickets and older embedded Insomnia version ranges return no transaction timestamp; the API never fabricates historical times.

`Reliquary` and `Phylactery` also expose `latest_global_version()`, `transaction_time_ns(version)`, and `version_at_or_before(transaction_time_ns)` as owner-level projections of the Container transaction-time index.

`Reliquary::create(path)` creates an unlabeled homogeneous REL. `create_project`, `create_organization`, and `create_connection` are compatibility/convenience constructors that create the same generic REL identity and set only the optional metadata type label. Likewise `open_project`, `open_organization`, and `open_connection` are compatibility aliases for `open`; they do not enforce a class. `rel_metadata()` returns `RelMetadata { type_label, dependencies }`. `set_rel_metadata(type_label, dependencies)` durably replaces that metadata after trimming, sorting, and deduplicating dependencies and rejecting self-dependency. Type labels are organizational metadata only and may not select storage/retrieval behavior. Dependencies are durable owner-ID edges for host/Ego context composition.

Repository-associated RELs expose the repository-neutral project-history correlation seam. `ProjectRepositoryRef { kind, repository_id, project_path }` identifies the Lore/Git repository and selected Warlock project subtree; `ProjectRevisionRef` adds one opaque repository revision; and `ProjectFileRef` adds a revision-relative path plus optional 32-byte content hash. `ProjectRepositoryManagement` records whether ordinary repository revisions are `WarlockManaged` or `External`. `current_rel_semantic_cut()` reports the current `RelSemanticCut { global_version, archive_version, memory_version }`; `correlate_project_revision(revision)` captures that cut using the compatibility default (`WarlockManaged` for Lore, `External` for Git), while `correlate_project_revision_with_management(revision, management)` persists an explicit policy in the same `ProjectRevisionCorrelation`. `project_revision_correlations()` and `latest_project_revision_correlation()` read that durable history after reopen; the runtime host exposes the same correlation without reopening the REL. Existing `PRJCOR01` records remain readable and infer the compatibility default; new writes use the management-aware correlation format. Correlation is valid for any REL with repository association, does not itself advance the semantic global clock, and requires repository kind plus durable `repository_id` to remain stable across the REL's correlation history; `project_path` and repository-management policy may evolve within that same repository.

`Cva::register_project_file(...)` registers a transcript/provenance attachment backed by an exact `ProjectFileRef`. REL stores attachment metadata plus the durable FileId-to-`ProjectFileRef` binding, but stores no duplicate payload body for that project-backed file. The resulting `FileId` incorporates repository kind, durable repository ID, selected project subtree, repository revision, repository-relative file path, content hash, byte length, filename, and MIME metadata. Identical bytes referenced at different repository revisions therefore keep the same content hash while receiving distinct historical `FileId` identities. `project_file_ref(file_id)` resolves the durable repository reference used by transcript and provenance views.

### Conservative storage reclamation

`Reliquary::reclaim_storage(output)` and `Phylactery::reclaim_storage(output)` write a separate verified copy and return `StorageReclamationReport`. The pass preserves the source owner UUID and semantic watermarks while reclaiming only never-published/staged backing objects, orphan Archive content and standalone Memory bodies, unactivated Archive-vector backing, packed matrices without surviving bindings, explicit free conversation-compaction chunks, and superseded current derived external-index snapshots. `StorageReclamationReport.superseded_external_index_chunks` counts the latter; they require no semantic prune marker. Published semantic revision/generation history is not retention-pruned. Version records containing physical `ObjectRef`s are relocated during the copy; the completed output is reopened through the normal owner lifecycle before success is returned. The source file is never modified or replaced.

### Explicit migration

The temporary `migrate_file` API and migration result types are retired. Normal open requires current V2.0 files with durable identity. The canonical corpus was converted and promoted before this deletion; frozen evidence remains in the [migration results](canonical-history-migration-results-2026-10-21.md).

### CVA comparison and reconciliation

Public reconciliation types are `CvaComparison`, `CvaRelation`, `CvaReconcileResult`, `CvaReconcileConflict`, and `CvaReconcileError`. `Cva::compare(left, right)` opens and validates two Reliquaries, requires matching durable `owner_id()` values, compares their physical append-only chunk histories, and reports `Identical`, `LeftExtendsRight`, `RightExtendsLeft`, or `Diverged` plus common/total chunk counts. Files without current durable identity reject before reconciliation.

`Cva::reconcile(left, right, output)` writes a new output REL. Identical/strict-extension cases copy the complete valid side, including its Project revision-correlation history. True divergence is handled as a fresh semantic repack that preserves the original owner UUID, replays the complete left semantic history plus the right divergent tail through normal owner APIs, and therefore allocates fresh valid global/Archive/Memory/Graph version numbers rather than retaining either branch's physical/version layout. Replay preserves known transaction timestamps from the source semantic mutations and preserves legacy unknown transaction time as unknown. Replay covers source Nodes, attachment-bearing ingested turns, standalone Files, Branch revisions, Conversation Metadata revisions, immutable Episodes, immutable Fragments, Memory revisions, Entity revisions, typed Graph relationship transactions, durable Insomnia completion receipts, file-to-Memory links, interaction-stream checkpoints, and compatibility profiles. Entity revisions are replayed before Graph transactions that reference them. Existing Project correlation cuts are not replayed into that renumbered semantic timeline. Repository-correlation histories must instead have prefix-compatible `(ProjectRevisionRef, ProjectRepositoryManagement)` sequences; branch-local `RelSemanticCut` values are intentionally ignored for this compatibility check because those clocks are being replaced. When the merged canonical state changes, reconciliation appends one fresh correlation at the final merged REL cut using the latest compatible project revision and management policy. A right-side repository-correlation extension counts as a canonical change even if that side contributes no Archive/Memory mutation. Truly divergent repository revision sequences fail closed rather than silently choosing one project-history association.

Memory replay preserves stable IDs and mutation IDs and uses ordinary expected-revision rules, so identical mutations deduplicate while incompatible concurrent revisions surface as Memory conflicts. Entity revisions replay after Memories and before typed Graph transactions, so every replayed Graph endpoint already exists. Single-edge and atomic `GraphRelationChange` batches are decoded from their source histories and republished through the Graph owner so destination global/Graph versions and dense topology mappings are allocated afresh. For each oriented `(source, target, kind)` relationship, left/right divergent active-state sequences are compared by semantic prefix: an already-present prefix is deduplicated, a stale candidate contributes nothing, and only an unabsorbed right suffix is replayed. A non-prefix history fails closed as `CvaReconcileConflict::GraphRelation`; with the current binary toggle model, valid histories from one shared base are naturally prefix-comparable. Grouped Insomnia completions are decomposed into their embedded Memory revisions, those revisions are replayed normally, and a completion-only receipt is re-emitted against the merged Memory IDs. Existing Fragment ranges are revalidated and replayed, so the disposable lexical in

[TRUNCATED at 50000 chars]

## Configured decision routes

`ModelSwitchboardConfig::general_decision: Option<DecisionModelEndpoint>` configures a shared decision service. `ModelSwitchboard::general_decision()` and `general_decision_auth()` expose only that configured field and its resolved API-key auth; they do not perform inheritance. `ConfiguredRuntime::runtime_routes()` supplies each consumer's effective `Arc<dyn DecisionEndpoint>` using specialized → shared → General precedence. Absence of all three retains the existing unavailable-route policy.

`with_entity_resolution_decision(...)` and `with_memory_retrieval_decision(...)` still accept exact host-injected endpoints. The reusable General adapter and canonical selector are private implementation details. General choice answers never include manufactured confidence; Entity's independent generative resolver and its route requirement are preserved. Retrieval's top-level versioned approval and query-rewrite contracts remain unchanged. Runtime errors never select another decision tier.

## Related docs

- [Local configuration](configuration.md)
- [Behavioral contracts](behavioral-contracts.md)
- [Maintainer map](maintainer-map.md)

## Notes

Decision inheritance changes configured endpoint composition only; semantic persistence and owner lifecycle are unchanged.

Import epoch values exposed through `imports` are the canonical `reliquary-import-store` values. The Cva epoch inspection methods continue to return owned values; completion remains an exclusive Cva coordinator operation.
