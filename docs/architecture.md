# Architecture

Parent index: [Documentation index](INDEX.md)

## External Index Store crate

`reliquary-external-index-store` owns disposable `CVAXIDX2` snapshot/discard
encoding, structurally validated external index rows, per-source CAS state,
and Container-backed append/sync installation. Container is its only internal
Reliquary crate dependency. Root `src/external_source/` retains provider
authorization, feed/rebuild preparation, content hydration and search; root
lifecycle validates packed-vector/profile backing and supplies invalid
source namespaces to the lower store. Extracting this index changes no
external transcript authority, stored bytes or source API.
See [the extraction evidence](external-index-store-crate-extraction-evidence-2026-10-27.md).

## External Index Store crate

`reliquary-external-index-store` owns disposable `CVAXIDX2` snapshot/discard
encoding, structurally validated external index rows, per-source CAS state,
and Container-backed append/sync installation. Container is its only internal
Reliquary crate dependency. Root `src/external_source/` retains provider
authorization, feed/rebuild preparation, content hydration and search; root
lifecycle validates packed-vector/profile backing and supplies invalid
source namespaces to the lower store. Extracting this index changes no
external transcript authority, stored bytes or source API.
See [the extraction evidence](external-index-store-crate-extraction-evidence-2026-10-27.md).

## External Index Store crate

`reliquary-external-index-store` owns disposable `CVAXIDX2` snapshot/discard
encoding, structurally validated external index rows, per-source CAS state,
and Container-backed append/sync installation. Container is its only internal
Reliquary crate dependency. Root `src/external_source/` retains provider
authorization, feed/rebuild preparation, content hydration and search; root
lifecycle validates packed-vector/profile backing and supplies invalid
source namespaces to the lower store. Extracting this index changes no
external transcript authority, stored bytes or source API.
See [the extraction evidence](external-index-store-crate-extraction-evidence-2026-10-27.md).

## Physical Container crate

`reliquary-container` is the independently compiled owner of REL/PHY headers,
opaque object framing, physical publication clocks, OS file leases, bounded
read sources/cursors, pure typed review clocks and explicit incomplete-tail repair. Its implementation and physical tests live under `crates/reliquary-container`; the root product depends downward on it.
Semantic decoding and replay remain with each semantic owner, including
`reliquary-archive`; product lifecycle modules route frames and compose
cross-owner validation and mutation admission. In particular,
Container recognizes only the validated grouped physical clock prefix of an
Insomnia or external publication, not its semantic completion evidence.

Ordinary open validates without truncating an incomplete final frame. A held
writer session excludes another cooperating writer; active read cursors fence
local mutation, while held read sources retain their file/lease lifetime. A bounded
cursor yields one frame at a time so the consuming owner controls replay and error ordering.
Repair is a separate operation bound to validated prefix, file identity and
observed length, and requires higher owner approval. OS locks coordinate
cooperating access; they do not protect against hostile direct file writes.

This extraction preserves current physical V2 bytes. The restart baseline's
CVAFRS06 semantic Freshness implementation remains a separate recovery
obligation: it cannot establish promoted CVAFRS07 product acceptance. See
[current limitations](current-limitations.md#crate-first-recovery-boundary).

Current runtime settlement, normalization, owner maintenance and rewrite behavior is summarized in [Semantic maintenance and history rewrite](#semantic-maintenance-and-history-rewrite). Historical framing references below describe stable comparison schemas or frozen development evidence; runtime admission is current-only.

## Archive source-truth crate

`reliquary-archive` is the independently compiled semantic source owner for REL
conversation history. Its codecs, replay, indexes and source records live in
`crates/reliquary-archive/src/`; its only direct Reliquary dependency is
`reliquary-container`. The root product depends downward on both crates and
composes Archive with the other owners. The extraction preserves current Archive
publication and `CVAPUB01` body bytes, version ordering, source identities and
error semantics. It does not change the storage format.

Archive owns visible transcript/source facts, Nodes, Branches, Episodes,
fragments, files and source receipts. Runtime stream/session coordination stays
in `src/runtime_inference/`; Echo reasoning/tool evidence belongs to the
independently compiled `reliquary-echo` crate; `Cva::search_archive` and query
policy stay in Retrieval under `src/retrieval/`. Multi-owner maintenance proof
and lifecycle composition remain in root facades.

## Echo execution-evidence crate

`reliquary-echo` is the independently compiled owner of source-keyed historical
reasoning, tool and activity evidence. Its model, `CVAECHO1` codec, replay and
ordered source reads live under `crates/reliquary-echo/src/`; its only direct
Reliquary dependency is `reliquary-container`. The root product holds and
replays the crate's `EchoStore` while opening REL files, and keeps its public
`EchoEvent`, `EchoEventKind` and `EchoError` re-exports for compatibility.
Opening and replay remain byte-neutral; Echo does not allocate semantic
versions or add a container or publication envelope.

The root facade retains REL admission and lifecycle composition, and
Maintenance retains cross-copy reconciliation. Runtime retains grants,
authenticated source selection and generation/correlation checks. Context
Engine retains evidence rendering and excludes private `ToolContextAccepted`
acknowledgments. Archive transcript truth, Memory truth and provider
continuation state remain with their existing owners. Ordinary Echo retrieval
is scoped to one selected visible source; explicit conversation enumeration is
for authorized transcript/restart materialization, and full enumeration is for
maintenance/recovery.

## Memory semantic owner crate

`reliquary-memory-core` owns Memory identity, immutable bodies, revisions,
mutation admission, dense local clocks, terminal coordinates, retained history,
routing snapshots and current-format codecs under `crates/reliquary-memory-core/src/`.
Its only direct Reliquary dependency is `reliquary-container`. REL and PHY hold
separate instances of this same compiled owner.

The root product validates Archive provenance and source authority, decodes
Insomnia and External Source carriers in physical order, and passes typed Memory
batches to the owner. External Source retains its window retry receipts and
provenance evidence. Chronos supplies validated inference converted to immutable
Memory snapshots. Root lifecycle and maintenance compose Memory-owned terminal
and prune facts with cross-owner authorization. Processing epochs and source-time
resolution remain above Memory. Opening and historical reads do not repair bytes
or reinforce Freshness. Persisted identities and publication bytes are unchanged.

## Graph and Community owner crate

`reliquary-graph` owns typed Graph topology, node ordinals, relationship
transactions, ordered replay, Memory projection and durable derived Community
snapshots, names and lineage. Its implementation lives in
`crates/reliquary-graph/src/`; its only Reliquary dependency is
`reliquary-container`. Arcana supplies runtime topology and Leiden supplies the
existing deterministic structural clustering kernel.

Graph represents foreign Memory and Entity identities as exact opaque 32-byte
values. Product lifecycle and facade code retain Memory/Entity existence,
principal-kind admission and historical rewrite authorization under the held
owner context. Graph validates intrinsic relation shape, transaction/version
ordering, ordinal bindings and physical publication. Current and historical
endpoint admission remain distinct; ordinary replay and historical reads do
not modify owner bytes.

Community membership and names share the Graph owner's Container stream.
Community currentness follows the Memory projection watermark, while names
consume no semantic or global version. Dream retains relation decisions,
refresh scheduling and inferred naming. Retrieval retains vector routing,
representative selection and traversal policy in root consumers. This cut
preserves the existing Graph/Community codecs and does not establish the
separate CVAFRS07 whole-product recovery gate.

## Perception durable owner crate

`reliquary-perception-store` owns the separate Entity, mention-resolution and
transitional Relationship stores. Their values, compact codecs, replay,
retention and indexes live under `crates/reliquary-perception-store/src/`.
Container is its only Reliquary dependency. This does not extract the Perception
decision service or introduce Observation synthesis.

Foreign Memory evidence and mention coordinates retain their exact wire values.
The product boundary converts Memory Core values and validates immutable mention
binding, current or historical existence, owner authority and principal rules.
Inference, candidate selection, merge coordination, Graph projection and
destructive maintenance authorization remain above the store. Ordinary replay
does not repair owner bytes. Entity and Relationship publication keep their
existing global and owner-local clocks; resolution metadata remains clock-neutral.

`Cva::entity_resolution` and `Phylactery::entity_resolution` return an owned
`MemoryEntityResolution` so the public record retains Memory Core's mention key
without duplicating the store's in-memory projection. Mention-key inputs and
persisted bytes are unchanged. This return-value adjustment was explicitly
approved for the extraction.

## Insomnia work owner crate (integration in progress)

`reliquary-insomnia-store` owns Episode-keyed work and attempt values,
`CVAINSF1`/`CVAINSW1`/`CVAINSA1` codecs, scheduler indexes, deterministic lease
tokens, transitions, and work reconstruction. Its only direct Reliquary crate
dependency is `reliquary-container`. The product's
`src/runtime_inference/insomnia/product_adapter.rs` converts fixed-byte IDs and
work values to public root types and supplies Archive-authenticated Episode
ordering and accepted Episode metadata. Archive and Memory validation, full
`CVAINSC7`/`CVAINSC8` completion decoding and authoring, grouped
Memory/Freshness validation, processing, and worker policy remain above the
store.

The Cva facade and owner replay now use the lower store. After Archive and
Memory reconstruction, the product makes a bounded second physical pass,
validates each full completion, and supplies its accepted projection in frame
order. This preserves the existing wire formats and queue persistence
lifecycle. `Cva::insomnia_work` returns an owned value as the
approved API adjustment. Focused product and CLI acceptance remain pending;
see the [extraction specification](insomnia-store-crate-extraction-spec-2026-10-26.md)
and [implementation evidence](insomnia-store-crate-extraction-evidence-2026-10-26.md).

## Dream Store crate (compiled integration complete)

`reliquary-dream-store` now contains the `CVDREAM2` Begin codec, canonical
population digest and deterministic `CVDREAM1` journal projection. The root
adapter `src/freshness_dream_journal.rs` consumes the lower journal and maps
fixed-byte Memory identities and the wire-exact accepted graph-cut proof to
product/Freshness values. The crate currently declares no direct Reliquary
crate dependency. Root retains physical-prefix traversal and compact
population reconstruction, including Memory-local publication evidence,
source fence, cohort/source exclusions and contemporaneous birth/intent proof.
Root also authenticates Freshness graph and Community authority.

Dream inference, pair ranking, settlement policy, Graph editing, Community
generation, Memory selection and Freshness reinforcement remain above the
store. Current bytes and strict prefix-before-Begin ordering are preserved by
the implementation. Lower, focused Dream, workspace, format, CLI and full
library checks pass; see the
[specification](dream-store-crate-extraction-spec-2026-10-27.md) and
[implementation evidence](dream-store-crate-extraction-evidence-2026-10-27.md).

## Vector Store crate extraction (in progress)

`reliquary-vector-store` contains lower-level vector wire models, codecs and
store/replay modules under `crates/reliquary-vector-store/src/`. Its direct
Reliquary dependency is Container; it also depends on the packed-matrix codec
and SHA-256 implementation used by persisted records. The root product
continues to own the public REL/PHY facade, cross-owner Archive and Memory
authorization, embedding/provider work, compatibility policy, search and
lifecycle integration. This is a partial extraction: root consumers and
production verification remain pending, so the crate is not yet the sole
production owner of vector persistence.

The extraction targets the existing packed-vector, Archive-vector,
Memory-vector, compatibility-profile and vector-generation wire families.
The crate's seven vector wire tests and workspace check have been reported as
passing. Root focused vector/retrieval tests and the detached CLI suite are
pending. `CVAFRS07` and its authoritative six-file corpus gate remain
unverified and are independent of this work. See the [extraction
specification](vector-store-crate-extraction-spec-2026-10-26.md) and
[implementation evidence](vector-store-crate-extraction-evidence-2026-10-26.md).
## Development-format lifecycle

[ADR0042](decisions/0042-minimal-lossless-current-history.md) records the owner-specific compact history architecture. The [six-source oracle](canonical-history-migration-results-2026-10-21.md) now passes every concrete owner history and global ordering point. The six canonical files are promoted; reader and upgrader deletion is implemented, with post-retirement acceptance tracked in the linear plan.

The runtime accepts one canonical V2 format. During development, translate retained data with temporary offline tooling, verify exact history and authentic proof on disposable copies, commit canonical corpus and implementation separately, then delete old readers, dispatch and migration APIs/CLI. Old normal-runtime opens must fail closed with typed errors after that transition. There is no approved V1 or CVAFRS03/04/05/CVAILEG1/2 exception. Profile compatibility remains live semantic data.

The [controlling amendment](minimal-lossless-history-storage-format-policy-amendment-2026-10-19.md) requires complete owner converters and corpus promotion before deletion. The [caller/removal map](development-format-interruption-execution-2026-10-19.md) tags temporary runtime and offline seams. The linear plan records post-retirement verification separately from conversion and promotion.

## Semantic maintenance and history rewrite

New import settlements use CVAFRS06 semantic event identity and digest evidence. Physical offsets are used transiently to select the settlement interval; they are not persisted as continuing authority. CVAFRS03/04/05 and CVAILEG1/2 readers and the temporary normalization API are deleted. Repair requires current semantic evidence before prune/reclaim.

`OwnerMaintenanceStore` is shared by REL and PHY. Domain owners still prove each target's retention eligibility. `Cva::prune_tombstoned_history` and `Phylactery::prune_tombstoned_history` create a separate validated copy; REL additionally owns Archive conversation payload. PHY supports terminal Memory, Entity and Relationship payload plus retired-profile vector payload. Reclaim remains a separate conservative transformed-copy operation.

`rel repair` and `phy repair` accept repeated `--keep-profile` values, require every selected profile to be active and complete for current owner data, retire unselected profiles, and verify the reopened copy. Optional `--prune-retired` and `--reclaim` run only against transformed copies.

The `rewrite` module exposes `RewriteRequest`, `RewritePlan`, `RewriteReport`, `plan_rewrite_file`, `rewrite_file`, and `rewrite_from_plan`. Plans bind the source head and policy digest; execution rechecks the source before creating output and refuses an existing destination. The policy supports identity/generation, Memory selection, selected profiles, Freshness mode, REL Archive selection/provenance and optional REL synthesized Ego state. PHY has no Archive selection and retains authoritative user-owned Ego state. PreserveOwner records a new history generation and predecessor lineage; reconciliation across generations reports `HistoryGenerationConflict`. The plan is the read-only review surface. Step 61 verifies exact Ellis all-current retrieval parity plus PreserveOwner rollover, Step 62 records six authoritative disposable-copy rewrites plus retained-profile vector compaction, and Step 65 closes the final compatibility/integration gate; see the [execution evidence](semantic-import-pruning-history-rewrite-evidence-2026-10-18.md).

Import/CVA derives an open epoch's attribution from its exact physical ImportStart marker and Archive receipt. Source Nodes, complete Import Episode paths and born-Memory identity remain distinct from the eligible existing recipient population. Freshness uses finite physical intervals and exact source identities to select provisional events; the Cva historical adapter reconstructs the pre-start base without later same-cut receipts. New epochs publish sparse `CVAFRS06` settlements with semantic accepted-event evidence rather than durable physical-prefix authority. Replay consumes only included receipt identities at an equal activity cut; accepted use appended later remains live. Zero-result and zero-activity epochs settle without inventing admission anchors.
Import completion publication is confined by Rust visibility and an opaque coordinator authority. Direct epoch-store completion append is available only to storage tests; production callers must pass through `Cva::complete_native_import` and its owner-derived readiness checks. New Dream passes obey the owner's archived/terminal eligibility exclusions. An already pending import-owned pass still blocks completion after archival or terminal deletion; non-archived superseded births also require their initial pass.

## Ordered Historical Freshness settlements

FreshnessStore owns ordered sparse `HistoricalImportFreshnessSettlement` records, with no singleton ImportCheckpoint. Accepted settlements use standalone `CVAFRS06` semantic evidence. Birth, Dream and accepted-use receipts remain durable. Exact settlement retries preserve later live effects, including accepted use appended at the same activity cut.

Multiple ordered settlements are supported by the codec/store foundation. Native reconstruction resolves retries by the current epoch identity; without an open epoch it retains the legacy latest-settlement retry behavior. Durable import admission and the exclusive completion coordinator are integrated. `Cva::native_import_readiness(import_id)` provides a read-only report for the current open epoch, deriving import-attributed Insomnia/Memory/Dream/Perception blockers from durable owner evidence; routed PHY references are `Memory/Unsupported`. Each nonzero local output is tied to its retained same-epoch Insomnia completion, revision-1 same-Episode Memory record and first mutation. A previously pending import-owned Dream pass remains a blocker even if its Memory is archived or terminal; otherwise archived births follow Dream's no-new-pass eligibility rule. Vector status is `NotRequired` only when no source Nodes and no Memory outputs exist, otherwise `SupportedUnavailable` without completeness certification. Import schedules no Ego work, and unrelated work is outside the query. Production completion append is restricted to the coordinator with an opaque authority. Sequential imports across live activity and reopen are covered by `imports/sequential_completion_tests.rs`. Ordered epoch reconciliation preserves the left physical prefix, replays supported accepted owner frames in physical order, and rebuilds destination settlement/completion evidence. Transform validation retains exact completed epoch markers and settlement authority.

## Domain tombstone convergence

Memory, Entity and Relationship stores own terminal direct tombstones on their existing domain/global clocks. Exact retry preserves the original coordinates; later publication under the deleted ID rejects. Current lookup, lexical candidates and graph projection exclude terminal identities without rewriting historical graph transactions or cascading to independent owners. Historical revisions and owner retention descriptors preserve payload, topology, evidence and provenance for later explicit pruning. Community projection additionally validates current graph membership, so a clock-neutral Memory deletion invalidates the old derived snapshot until refresh while retaining its historical record. Entity merge tombstones retain their replacement chain; direct Entity tombstones have no replacement.

## Ego terminal lifecycle

EgoStore remains the sole owner of Identity and Anchor lifecycle. Existing deleted Identity/Anchor records are terminal on runtime mutation and replay. Exact deletion retries with the original expected revision return `false` without appending or advancing any clock; conflicting retries reject. Current projections exclude deleted IDs, while retained nondeleted revisions remain available through explicit historical getters. Lifecycle coordinates use the existing Ego version and omit the global version because Ego records allocate no semantic global ticket.

## Native historical import ownership

The import/CVA boundary owns durable `ImportStart` / `ImportComplete` metadata in `ImportEpochStore`; Archive owns source mappings and `CVAIMPT1` receipts. `Cva::import_native` opens or resumes the deterministic owner-plus-import-key epoch before mutation and schedules Import-origin Episodes through Insomnia. An open epoch closes generic Container write admission except for its private import scope. The exclusive `Cva::complete_native_import` derives evidence from the exact receipt, accepted scoped `CVAFRS06` settlement, complete import-source Episode paths, durable Insomnia completions and local Memory outputs. Each nonzero output ID is proved by the exact retained same-epoch completion record, revision-1 same-Episode Memory record and matching first mutation; IDs without that evidence are conflicting, not publication proof. It blocks on missing/failed/conflicting import-attributed evidence, requires import-born initial Dream settlement, and requires terminal Resolved/Rejected outcomes for every mention key on active, non-superseded Dream-processed output Memory IDs. Dormant/unresolved keys block; only inactive, archived or superseded Memories have no mention obligation. For an empty receipt (`receipt.conversations.is_empty()`), the authenticated settlement must be a true no-op with empty source Nodes, births, cohort, replacements and superseded IDs, and equal base/settlement cuts. It ignores unrelated owner work. It returns `Memory/Unsupported` for any routed PHY Memory reference because this REL coordinator cannot validate the other owner's state. Import schedules no Ego mutation. Vector readiness reports `NotRequired` only with no source Nodes and no Memory outputs, otherwise `SupportedUnavailable` without vector completeness certification. The coordinator accepts no caller-supplied proof; it binds the exact receipt digest and semantic settlement payload hash, and syncs the marker before reopening writes. Reopen verifies settlement-before-completion order; uncertain retries rescan physical markers. Mounted import admission remains unsupported.

### Ordered divergent import-epoch reconciliation

Completed native import reconciliation retains the left physical prefix and replays the right divergent intervals in physical order through the Archive, grouped Insomnia/Memory, Entity, Relationship, Graph, Community, Dream, Perception and Freshness owners. Grouped completions retain complete record/body/routing/birth evidence; only destination version and activity coordinates are rebased. Scoped settlements and completion markers are rebuilt through native reconstruction/completion. Exact normalized accepted source evidence deduplicates; conflicting import identity fails closed. Open epochs, external PHY publications, unsupported owner payloads and outstanding source Insomnia work remain refused. Transform validation compares exact epoch markers/statuses and settlement authority.

## Dream population prefix boundary

REL's private Dream population coordinator combines accepted first Memory identities at the exact Memory-local head of a physical prefix with birth/cohort facts present in that prefix. Memory retains first-revision identity/global coordinates through later revisions, archival state and terminal pruning; the Dream journal supplies direct Intent/Consume/ReplayBirth facts, while Insomnia and external publication proofs retain their existing validation. The coordinator excludes the source cohort and applies the original strict first-global ordering rule. Container reports physical frames and global frontiers without interpreting these relationships or repairing later bytes.

The read-only reconstruction seam is in `facade/cva_dream_population.rs`. Current CVDREAM2 Begin replay is coordinated by `facade/cva_dream_journal_replay.rs` after accepted Memory and source metadata are available. It scans the held container in physical order, checks compact cut/digest authority before each Begin, and admits explicit exception sets literally into the existing materialized journal API. end-of-file pending-intent Freshness recovery remains separate from Begin-prefix reconstruction. Runtime Begin capture now uses that same prefix. `facade/cva_dream_begin_transport.rs` resolves source Begins before import normalization and permits destination compact encoding only after exact set equality; otherwise it emits literal exceptions. Historical completed-proof readers share ordered replay. Copy, prune and reclaim retain prefix facts; fresh-history rewrite intentionally discards old pass history under its existing generation policy. Ordinary Begins now use CVDREAM2; the temporary offline graph-envelope stage converts legacy Begins at their original slots. Further retirement and integration remain tracked in the [linear storage plan](minimal-lossless-history-storage-linear-plan-2026-10-19.md). See the [population contract](dream-population-contract-2026-10-21.md).

## Compact Dream pair history

DreamPairStore owns a per-file physical Memory-ID table and canonical semantic
pair membership. The private `runtime_inference/dream_pair_codec.rs` encodes
only mapping and pair fields; the store validates dense unique mappings,
backward references and resolved identity order before admitting membership.
Memory remains authority for historical endpoint existence. Reconciliation and
owner APIs exchange stable Memory IDs and reconstruct destination mappings;
physical references do not cross owner boundaries or allocate semantic clocks.

## Freshness retrieval and consumption

All internal Memory retrieval, including content, provenance, graph and Freshness metadata inspection, is non-reinforcing. The [Agentic Memory retrieval subsystem](subsystems/agentic-memory-retrieval.md) implements the REL-only three-operation workflow: full lexical/exact-semantic candidate union before the Freshness top-ten cut, deterministic refinement of declined candidates in bounded batches, and concurrent one-hop exploration in both directions per anchor with independent ten-candidate limits. The host owns turn-scoped `CurrentQuery`, `SearchedIDs`, and `SelectedIDs`; only confirmed content handoff qualifies for reinforcement through the existing `Cva::record_accepted_memory_use` boundary. Reliquary's controller and frozen functional scenarios are implemented; retained-corpus evidence recall remains a product/evaluation limitation, and Warlock handoff/replay remains consumer release work. See the [host handoff evidence](agentic-memory-host-handoff-seam-2026-10-16.md) and [retrieval verification](agentic-memory-retrieval-verification-2026-10-16.md).

## Freshness durable owner

`crates/reliquary-freshness-store/` is the compiled owner of canonical
score/policy values, durable records, event/use receipts, scoped semantic
settlements, rewrite baselines, and Freshness replay. Container remains the
physical persistence boundary. Root facades retain explicit Memory, Fragment,
and History-generation byte conversions and owner authorization. Higher
Freshness services retain Graph propagation, semantic candidate policy, and
historical event planning. The extraction is integrated; the separate
CVAFRS07 recovery and six-file corpus gate remain unverified. See the
[specification](freshness-store-crate-extraction-spec-2026-10-26.md) and
[implementation evidence](freshness-store-crate-extraction-evidence-2026-10-26.md).

## C8 bounded historical source vectors

The private Cva historical_embedding plan/provider adapters coordinate existing Archive fragment bindings, compatibility probes and immutable PackedVectors/ArchiveVectors/VectorGeneration records. The configured offline post-generation management operation invokes this owner path; no second store, queue, coordinator or runtime is introduced. Owner/corpus/profile/generation/prefix proofs are revalidated around serial provider calls. Provider snapshots carry no mutable owner access and no runtime mutex is acquired by this operation.

Preflight bounds sources, fragments, text, dimensions, rows, repeated component/inspection work and conservative temporary/proof buffers before provider calls or writes. It includes full union-copy amplification and a conservative retained-prefix/proof reserve, so population ceilings do not guarantee that all combinations fit. Failed/cancelled fill accepts no historical checkpoint; completed vector batches may remain and retry reuses their exact row bytes. Final H1 freeze derives requested-complete embedding coverage from retained authority and then uses the sole FreshnessStore/CVAFRS06 acceptance path. Mounted Gateway execution remains unavailable; exclusive offline ownership is required.

## Live F8 fragment ownership

Archive owns genuine retained fragments and accepted source positions. The private `cva_semantic_fragment` adapter derives each fragment's availability from its latest represented activity and pins every represented Node plus selected row/generation/profile evidence. F8 creates no exact-turn fragments. Existing Archive producers retain window/materialization authority. The live facade rediscoveries are bounded by fragment count and publish through the existing Freshness journal only; no work queue is persisted. Freshness owns owner/domain/fragment event identity, local score effects and replay. Re-embedding cannot create another award. Current-body matching excludes all represented provenance and generating Episode cohorts before candidate ranking; Graph/Community never participates in live propagation. Reconciliation maps every represented activity identity and takes its destination maximum rather than rebasing only the former last-turn anchor. Historical planning shares this fragment-evidence adapter. Source/cohort exclusion includes grounding evidence outside the generating Episode and excludes every member of that cohort.

## Compact Memory reconstruction

`crates/reliquary-container/src/codec.rs` shares only value-level mechanics: typed unchanged/set changes (including explicit optional clear), known-mask checking, and pure candidate-field application. It has no owner, storage, revision, clock or publication authority. Memory and each later owner retain field ordering, wire tags, identity rules, terminal behavior, error types and canonical-delta validation. Validate-before-apply remains a concrete owner sequence: validate the envelope, build a disposable typed candidate, validate canonical semantics, then admit/publish through that owner. A generic owner/candidate-validation framework is not introduced.

Memory owns its creation/delta codec and pure candidate application; common Memory value encoding also supplies canonical comparison evidence. Standalone publication writes compact creations/deltas; atomic Insomnia and external publications embed compact creations within their existing owning transactions. Converted grouped publications retain original clock observability in the current retained variant. Existing `MemoryOpenState` remains the sole replay owner and retains exact object/version-marker admission, deferred unversioned frames and terminal tombstones. New compact payloads do not create another store, history clock or public delta API. See the [wire and replay contract](storage-format.md#compact-memory-wire-carriers).

## Lossless format-upgrade staging

The temporary offline converter and physical transport are deleted after canonical promotion. Concrete owners retain current replay, proof, maintenance and historical-query responsibilities. Frozen conversion evidence is separate from current runtime code.

## Purpose
This document owns the current Reliquary Memory v2 implementation boundaries, state ownership, lifecycle, and code map.
## Durable conversation policy

Archive conversation metadata owns SequentialChronological/VisibleAutoBranch and its separate policy revision. The existing Cva facade delegates publication; Gateway uses exact-owner ViewRel reads and ManageRel mutation with sync-before-acknowledgement. The shared observation contract imports Archive's canonical enum.

## Archive conversation tombstones

Archive owns terminal aggregate conversation tombstones on its existing Archive and global clocks. Publication records a deterministic direct closure of conversation nodes, branches, metadata record versions, fragments, Episodes, and attached-file exposure. Current Archive queries exclude that closure after publication and reopen; explicit historical queries continue to resolve retained records and payload. All later writes using the same conversation ID fail closed, so continuation requires a new conversation identity.

The tombstone does not traverse provenance or mutate independently identified Memory, Entity, Relationship, Graph, vector, or Ego state. Divergent reconciliation replays a noncompeting tombstone, deduplicates an already terminal target, and reports a typed conversation-tombstone conflict when the other branch independently changed the same conversation. The future shared owner-lifecycle contract may normalize inspection types; it does not replace Archive persistence or clocks.

Policy publication and FinalizeWon capture share the existing owner runtime mutex. Capture synchronizes policy, then acquires the REL journal while retaining the runtime lock; no journal-held path acquires that runtime lock. Already captured finalization ancestry/revision is never rebound to current metadata during recovery. Default sequential finalization and VisibleAutoBranch stale-head conflict handling are implemented. [P3.1 evidence](multiplexing-implementation-plan.md) records policy ownership; [P3.5 evidence](multiplexing-implementation-plan.md) records visible conflict branching.

## Sequential finalization (P3.4)

Fresh admission matches the selected Archive branch head; exact accepted retries are verified by the journal against their immutable request, without reinterpreting the original parent against a newer head. Sequential dispatch permits advancement only along the admitted parent's branch ancestry and captures the current safe head in FinalizeWon. Archive publication and recovery refuse to rewind an independently advanced branch. Idempotent branch publication synchronizes before the Archive receipt; stable Node and scheduling identities preserve Activity/Episode/Insomnia effects across restart and missing markers. The focused finalization module holds no mutable authority. [P3.4 evidence](multiplexing-implementation-plan.md) records the handoff cuts and gates.

## Visible automatic branching (P3.5)

Under VisibleAutoBranch, finalization compares the admitted expected parent with the authoritative selected-branch head. A stale but related parent produces one deterministic non-canonical conflict branch whose stable ID and display label are frozen in FinalizeWon before Archive publication; an unrelated parent fails closed. Recovery reuses the frozen branch identity and never duplicates or rewinds a branch. After the Archive receipt is durable, only the original submitting instance's still-current attachment may move its instance-local selection to the new branch; detached/revoked submitters do not block accepted work and other viewers are never moved. [P3.5 evidence](multiplexing-implementation-plan.md) records the multi-writer and restart gates.

## Branch continuation ownership and scheduling (P3.2–P3.3)

The same coordinator now selects and reserves the oldest eligible journal entry under coordinator-then-journal locks. No duplicate queue is stored. Ordering uses verified receive time, persisted random tie key and admission sequence; completed ancestry is not rewritten. Held host work is skipped by fresh dispatch and resumes only through its existing durable continuation. Fresh exact-entry processing is test-only. [P3.3 evidence](multiplexing-implementation-plan.md) records the API hard cut and verification.

The existing ConversationCoordinator now owns live branch leases and reservations derived from unfinished REL journal work. Same-branch attempts serialize; independent branches may dispatch concurrently subject to capability/resource policy. Mount reconstructs branch inventory from Archive and reservations from the journal before workers start. Unknown host work and missing Archive/post-commit receipts reserve only their own branch.

Archive branch heads remain authoritative. The single owner runtime uses its existing conversation session as a temporary cursor, explicitly rebound to the branch head or captured receipt under the runtime mutex; an in-flight native message cannot be rebound. Post-commit scheduling selects the receipt's path. Coordinator-to-journal eligibility locks are released before runtime acquisition; runtime-to-journal capture remains the other allowed nested order. Provider I/O holds neither. [P3.2 evidence](multiplexing-implementation-plan.md) records scope and verification; native generation/checkpoint integration is implemented in [P3.6](multiplexing-implementation-plan.md).

## P5-B native worker and host capability boundary

The canonical gateway now owns route replacement and exact-owner background status, wake, enable and worker diagnostics through `runtime_host_gateway_background.rs`. One REL worker set remains mounted independently of instance selection; each PHY has one private set shared by same-principal attachments. The REL background switch controls Dream, Perception and vectors; Insomnia retains its existing independent switch. PHY compatibility profiles and vector batches synchronize before active-profile publication or worker wake. Worker restart checks the owner fence and does not spawn a second live set.

Only ConversationExecution has an executable host payload/receipt path. Explicit `HostExecutionMode::ExecutionOnly` negotiation reconciles to `CapabilityState::ExecutionOnlyReady` while queue authority stays Native. In this mode the application executes an already bound native generation; the router never hands pending admission to the adapter. Complete queue flags do not override the selected execution-only mode. Full pending ownership still requires the separate PendingQueue mode and all five queue guarantees. The finite capability router filters Insomnia, Dream, Embedding and ProjectEnvironment declarations to Native; their existing endpoint/project implementations retain authority. Unknown recovered debt still fences a capability until local durable terminal recording and empty reconciliation. The router binds handoff ID/digest and dispatch readiness under one short lock, then releases it before adapter I/O; the cache is validation state, not a second queue or durable journal. The [B ledger](multiplexing-implementation-plan.md) records boundaries and the deletion handoff. Legacy host deletion and Warlock migration remain separate convergence gates.

## Overview
A homogeneous Reliquary `.rel` is one physical file containing the same explicit concrete owners as the former CVA format. Normal opens require current V2.0 identity regardless of the filename suffix; preserved typed identity discriminators do not enable old-format readers:
```text
Reliquary (internal compatibility type: Cva)
├── Container
│   ├── durable typed owner UUID
│   └── global_version: u64
├── Archive
│   ├── archive_version: u64
│   ├── conversation/session-local ancestry
│   ├── source turns + native attachments + embedded files
│   ├── explicit file-to-Memory links
│   └── immutable Episodes
├── Memories
│   └── memory_version: u64 + immutable revisions
├── EntityStore
│   └── entity_version: u64 + durable canonical referent revisions
├── Graph
│   ├── graph_version: u64 + append-only relationship mutations
│   ├── persistent typed SemanticNodeRef ↔ dense NodeId catalogue
│   └── active topology/traversal via Arcana graph kernel
├── CommunityStore
│   └── owner-local derived Leiden snapshots keyed to graph_version
├── InsomniaOperational
│   └── queue / priority / leases / retries / compact completion receipt
├── EchoStore
│   └── ordered turn-attached reasoning / tool / activity evidence
├── PackedVectorStore
│   └── immutable numeric matrices
├── MemoryVectorStore
│   └── immutable (profile, MemoryBodyId) -> packed row bindings
├── ArchiveVectorStore
│   └── immutable row -> FragmentId bindings
├── CompatibilityProfileStore
│   └── immutable vector-space compatibility contracts
└── VectorGenerationStore
    └── vector_version: u64 + current generation per profile
```
`Reliquary` is the public product-facing type; `Cva` remains the compatibility/internal implementation name. It owns physical composition and the single Container handle. New REL files carry authoritative Reliquary file identity plus a durable UUID in the header and use canonical owner ID `rel-<uuid>`. Optional `RelMetadata` stores a human-facing type label and directed REL dependency IDs; the type label is organizational metadata only, while dependency topology is the explicit context-inheritance substrate. Legacy typed REL headers retain their historical owner prefixes for compatibility. Filenames may supply display names to the host but are not identity. Physical maintenance follows the same ownership boundary: `reclaim_storage` computes owner-aware reachability above Container, rewrites a separate file, relocates only physical `ObjectRef` metadata, and reopens the result before success; Container itself remains unaware of semantic reachability.

The same low-level container now also backs a distinct `Phylactery` type for user-global Identity state:

```text
Phylactery (.phy)
├── Container
│   └── global_version: u64
├── Memories
│   └── memory_version: u64 + immutable revisions
├── EntityStore
│   └── entity_version: u64 + durable canonical referent revisions
├── Graph
│   └── graph_version: u64 + semantic topology; published relations currently Memory-to-Memory
├── CommunityStore
│   └── owner-local derived Leiden snapshots keyed to graph_version
├── PackedVectorStore
│   └── immutable numeric matrices
├── MemoryVectorStore
│   └── immutable (profile, MemoryBodyId) -> packed row bindings
└── CompatibilityProfileStore
    └── immutable vector-space compatibility contracts
```

Phylactery does not own Archive/history, Episodes, Files/attachments, Insomnia state, Archive Vectors, Vector Generations, interaction-stream checkpoints, or the Archive lexical index. It does participate in the same disposable owner-local **Memory lexical index** as REL: the index is built lazily from current non-archived Memory title/content after reopen, then follows later Memory revisions incrementally by `memory_version`; it is never persisted in the PHY. Direct PHY Memory publication rejects REL-local provenance fields. A PHY Memory may nevertheless retain an identifier-only `MemorySourceRef` back to an originating REL; that reference does not copy or transfer the source records into PHY.

PHY principal identity and presentation metadata are deliberately separate. The header UUID derives the stable `phy-<uuid>` principal identity. `PhylacteryProfile` stores one optional mutable `display_name` as latest-wins owner metadata. Ego `Identity.name` remains the label of a selectable persona/identity document, not the human principal's durable name. When a REL has materialized the matching reserved principal Entity, RuntimeHost synchronizes current profile values only into that Entity's aliases/summary; canonical name and EntityId remain derived from `phy-UUID`, and principal aliases remain excluded from ordinary Entity candidate indexing.

Ego persistence is a separate owner-local layer over the same container, not another Memory category. Its semantic records and state machine are compiled in `crates/reliquary-ego-store/`, which depends on `reliquary-container` for physical persistence. Root REL/PHY facades retain product admission, current Memory watermark checks, and public error mapping; History and reconciliation consume the store's replay seam. PHY may durably store multiple stable Identity documents with exactly one active selection whenever the Identity set is non-empty, plus Personality, Anchors, and one cached Memory-Web synthesis; REL may store Anchors and one cached Memory-Web synthesis but rejects Identity/Personality records. Identity activation is independent of Identity content revision: creating the first Identity activates it, later identities remain inactive until explicitly selected, and swapping the active Identity writes only selection state. Legacy single-Identity PHY records reopen as one active `Default` Identity. The Ego store has an independent contiguous mutation version plus optimistic per-record revisions. Personality may record the PHY `memory_version` from which it was synthesized, and each cached web synthesis records its owner-local source `memory_version`; these are deterministic stale-input watermarks, not new semantic clocks or model-confidence scores. This owner-local singleton cached synthesis is the **implemented storage substrate**, not the future PHY-owned, source-REL-keyed three-size Memory-Web Summary bundle. That bundle's identity, persistence and generation behavior remain future work under [Ego Memory-Web Summary design](ego-web-synthesis-plan.md). Actual synthesis inference, cross-owner context assembly, scheduling, and prompt injection remain higher-level Ego/runtime responsibilities.

Machine-local application configuration is a separate owner:
```text
ReliquaryConfig
└── reliquary.cfg
    ├── archive.fragments
    ├── retrieval.default
    ├── models.general
    ├── models.insomnia (optional; falls back to general)
    ├── models.insomnia_metadata (optional)
    ├── models.dream (optional; falls back to general)
    ├── models.embedding
    └── credential.<id> (AES-256-GCM)
```
`reliquary.cfg` is current-state configuration only. It is not contained in a `.rel`, consumes no semantic clocks, and has no append-only/history semantics.
## P3/P4 internal observation boundary

The gateway declares a data-only `observation` contract beside its existing conversation coordinator. [The frozen contract](multiplexing-implementation-plan.md) defines REL summaries, conversation snapshots, post-durability facts and separate checkpoint-content values. P3 owns the authoritative runtime/coordinator/journal state and reusable observation, receipt and checkpoint builders. P4 consumes those source facts for mounted snapshots and distribution; subscriber and cursor state remain outside the P3 coordinator. The contract types themselves do not own mutable state.

## P4 non-conversation distribution boundary

In the converged distribution path, gateway Memory/Graph/community/metadata/project and principal-projection commits, concrete REL worker commits, and P3 conversation transition publications use the mounted sequencer across short mutation, synchronization, source capture and summary publication stages. Provider/model, private-owner, and Git/filesystem work stays outside the mounted gate. Source clocks remain authoritative; processing switches are explicitly ephemeral, and maintenance invalidates the epoch instead of implying incremental continuity. P3 supplies body-free conversation summaries at durable journal/Archive transitions; P4 owns their sequencing and recipients. See the [P4.4 record](multiplexing-implementation-plan.md), [P4.7 convergence record](multiplexing-implementation-plan.md), and [P4.8 audit record](multiplexing-implementation-plan.md). P4.7 verification and the P4.8 coverage/lock/privacy audit are complete; P4.9 local/convergence functional acceptance passed; see the [P4.9 acceptance record](multiplexing-implementation-plan.md) for repository-wide verification exceptions.

## P4 reconnect and lifecycle distribution

The internal reconnect operation reads a fresh authorized snapshot and replaces one recipient atomically under the mounted sequencer gate. Validation or reader failure preserves the prior recipient. Neither stale attachment/grant generations nor old mount cursors can reconnect; fresh authorization and subscription are required. No retained event window exists, so same-epoch cursors are hints rather than replay authority. Quiesced reopen reconciles and synchronizes source state, rotates the gateway epoch and publishes a body-free lifecycle summary before opening admission and restarting workers. See the [P4.5 record](multiplexing-implementation-plan.md).

## P4 exact conversation streams

EntityResolutionStore persists `CVAERS03` literal decisions and exact deltas for
unchanged unresolved evidence. Candidate/fingerprint/timing/count fields remain
accepted authority, inherited only from the preceding state of the same mention.
The owner validates before append and rebuilds its existing reverse/maintenance
indexes. Completed-import comparison materializes source history before hashing;
ordered destination replay re-encodes those postimages against destination state.
No semantic clock or shared delta framework is introduced. The frozen oracle retains accepted changes, including revision resets.

ExternalIndexStore persists disposable `CVAXIDX2` current snapshots and explicit
derived discard barriers. The compact binary representation removes repeated
namespace/message references and reconstructs contiguous chunk starts. Snapshot
revisions remain writer CAS state; they are not retained semantic history.
Reclamation retains the latest state per namespace and the required damage
barrier. Offline conversion replaces legacy index chains while preserving
`CVAXMEM1`/`CVAXMEM2` publication/evidence authority through the existing Memory converter. Native reclamation retains those payloads byte-for-byte. Existing authorization,
hydration, backing validation and damaged-cache rebuild behavior remain at
their concrete owners. See the [index contract](external-index-compact-contract-2026-10-21.md).

InteractionStreamStore owns materialization of REL `CVAISTR2` metadata, exact
UTF-8 suffixes and irreversible interruption markers. The physical byte position
is a content coordinate, never a semantic clock. Checkpoint validation and
reconciliation retain their existing latest-state contract; destination replay
uses the same compact writer. Archive still owns completed turns and supersedes
stream state by message ID. Offline translation has been retired after canonical promotion.

The P4 stream hub lives inside each mounted REL sequencer and shares its publication gate while keeping exact conversation/operation/generation/branch cursors and queues separate from broad REL summaries. Production joins use P3's authoritative snapshot and a validated durable full prefix before registration; delivery follows only persisted checkpoint/status facts. If Archive has advanced ahead of the journal, the prefix is bounded to the journal-authoritative UTF-8 length and checked against its SHA-256 before delivery. Count/byte overflow requires fresh prefix replacement. Authority and epoch teardown apply to both namespaces. The deterministic-provider tests remain the P4.6 mechanical evidence; production convergence is recorded in [P4.7](multiplexing-implementation-plan.md), and the coverage/lock/privacy audit is recorded in [P4.8](multiplexing-implementation-plan.md). P4.9 local/convergence functional acceptance passed with repository-wide verification exceptions recorded in [P4.9 acceptance record](multiplexing-implementation-plan.md).

## Semantic Freshness source preparation

`Cva::prepare_accepted_semantic_source` coordinates immutable accepted Archive identity/activity with retained generation evidence and reports pending versus complete (possibly empty) genuine-fragment vectors. FreshnessStore owns local semantic receipts; `cva_semantic_live` rediscovers pending work from Archive without a separate queue/ledger. F8 is intrinsic REL behavior: normal Archive generation publication drains at most 64 pending fragments, while the canonical REL vector worker prepares source fragments under owner mutation authority, embeds outside owner locks, then publishes and accepts local effects through the existing durability seam. A historical checkpoint cut (or legacy pre-release activation watermark) prevents already-accounted history from being credited again; it is not an enable/disable switch. Background status counts pending semantic fragments. Candidate search streams current compatible Memory-body rows with bounded top-64 selection, authoritative birth/admission and source exclusion. This lane has no Graph/Community propagation. See [F8 release verification](freshness-semantic-release-verification-2026-10-12.md).

## Responsibilities

### Gateway ConversationExecution recovery

The REL conversation journal owns durable handoff identity, immutable payload digest, sticky cancellation and terminal stage proof. Mount restores unresolved operations into capability debt before starting owner workers. The capability router preserves every locally unresolved ID across reconnect, contract downgrade and configuration removal; an adapter's empty-queue response cannot erase journal-owned work. Contract withdrawals fence new dispatch and recovered intent retries even if an operation changes concurrently with reconciliation. Status and cancellation remain available for previously owned work. Durable terminal proof is consumed locally after restart without requiring the adapter again; Completed permits exact-once publication of the original admitted turn, while Cancelled permits terminal native cancellation. Running and unknown outcomes remain nonterminal. This does not define host assistant-result semantics or migrate later worker capabilities.
### Durable owner identity
Every newly created REL/PHY receives a UUID in the container header before creation returns. Current RELs derive canonical owner ID `rel-<uuid>` and PHY derives `phy-<uuid>`. Copies, moves, renames, cloud-conflicted replicas, and reconciliation repacks preserve that UUID; intentionally new durable owners generate a new UUID. REL `type_label` is optional persisted organizational metadata and has no storage/retrieval authority. Directed REL dependencies are persisted separately from identity and are resolved by a multi-REL host/Ego context policy.

### Reliquary comparison and reconciliation
`Cva::compare` requires the same durable owner ID on both files and compares physical chunk streams to find their longest common prefix. `Cva::reconcile` builds on that detection seam: identical/strict-extension cases copy the complete valid side, while true divergence semantically replays the complete left history plus the right divergent tail. If the right/candidate side contributes new semantic state, the result is a fresh workspace CVA with reallocated global/Archive/Memory/Graph version numbers while preserving each replayed mutation's known transaction timestamp; legacy untimestamped mutations remain untimestamped; if every candidate record is already semantically present, the fresh probe is discarded and an exact copy of the canonical/left CVA is returned instead. This no-op path prevents repeated cloud-conflicted copies from rewriting the workspace and avoids any reconciliation-receipt metadata inside the CVA. Replay is owner-ordered: Archive source/Files/Branches/Episodes/Fragments first, Memory revisions next, then Graph transactions after their Memory endpoints exist, followed by cross-owner file-to-Memory links and durable Insomnia receipts; interaction-stream state and compatibility profiles are also preserved through their own seams. Graph single-edge and atomic batch mutations are republished through the Graph owner with fresh destination clocks and dense node mappings. For each oriented `(source, target, kind)` identity, divergent active-state histories are prefix-compared so already-absorbed/stale mutations are skipped and only a novel right suffix is replayed; non-prefix histories fail closed. Graph topology, Dream's duplicate index, and community snapshots remain derived and rebuild from merged relationship authority. Memory mutation/revision invariants and completion identity similarly decide duplicates versus genuine conflicts. Immutable Fragments are revalidated and retained, and the disposable lexical index rebuilds lazily from a changed merged state. Packed vectors, Memory/Archive vector bindings, and Vector Generations are retired only for a real changed merge; `canonical_change_required` and `vector_rebuild_required` report those outcomes separately. Semantic replay collisions are normalized into public `CvaReconcileConflict` values carrying the stable owner identity and the competing state needed by a host UI; corruption and transport failures remain ordinary errors. `Cva::reconcile_and_promote` skips replacement entirely for semantic no-ops and otherwise uses a hidden sibling candidate, pre-promotion fingerprint check, synced recovery copy, atomic path replacement, post-replacement reopen/sync, recovery on finalization failure, and cleanup. It detects ordinary canonical changes during preparation but does not introduce a general interprocess writer lock or cloud-provider discovery. Cloud providers remain responsible for storage transport and conflicted-copy preservation. See [ADR 0019](decisions/0019-cloud-backed-cva-reconciliation.md).

### Repository correlation and repository-backed files

Repository-associated RELs own the semantic side of the boundary between Reliquary history and the associated project repository. `ProjectHistoryStore` persists append-only `ProjectRevisionCorrelation` records that associate a `RelSemanticCut` with one exact Lore/Git `ProjectRevisionRef` plus the repository-management policy. The store does not own repository commits, trees, branches, working-tree mutation, or project-file ancestry; Lore/Git remains authoritative for those concerns.

The correlation sequence is REL-local bookkeeping and consumes no semantic version. Repository kind and durable repository ID are invariant across one REL correlation history. The selected project subtree and management policy may evolve within that repository. Reopen validates sequence continuity, repository-relative path normalization, and stable repository identity. Strict-copy reconciliation preserves the history; divergent semantic repacks compare repository revision/policy histories independently of obsolete branch-local REL cut numbers and emit a fresh final correlation only when the merged canonical state changes.

`ProjectFileStore` owns immutable `FileId -> ProjectFileRef` bindings for transcript/provenance attachments whose bytes are authoritative in the project repository. It validates repository-relative paths and subtree containment, rejects conflicting rebinding, and consumes no semantic clock. Archive continues to own attachment/file identity and source provenance; the Project-file binding supplies the exact historical external content location. Reliquary therefore does not duplicate repository-backed payload bytes into `CVACONT1` merely to preserve transcript provenance.

Reliquary's Project Environment owns repository discovery, Lore/Git integration policy, repository bootstrap/adoption, stable repository-identity validation, managed/manual checkpointing, Git snapshot creation, upload materialization, exact historical repository reads, repository-root resolution, and correlation of those operations with REL semantic history. A host application supplies only the machine-local project directory association and invokes the typed runtime-host operations. Legacy names such as `warlock.repositoryId`, `.warlock/`, `warlock/uploads/`, and `refs/warlock/snapshots/*` remain compatibility identifiers in existing project repositories rather than ownership markers. See [ADR 0027](decisions/0027-warlock-project-repositories-and-reliquary-storage-boundary.md), [ADR 0028](decisions/0028-project-folder-and-repository-bootstrap-contract.md), and [ADR 0037](decisions/0037-reliquary-context-memory-project-environment.md).

### Local configuration
`ReliquaryConfig` owns one purpose-built replaceable config file. Logical objects have stable keys and typed payload schemas; replacing a setting rewrites one complete current file image through a temporary-file + atomic-replace lifecycle. Unknown objects are preserved so the object vocabulary can expand. Ordinary objects are unencrypted; credential objects are authenticated encrypted payloads.
### Repo-local CLI
`cli/` is a separate, non-installed Cargo package that depends only on the public library API. It owns argument parsing, secret prompting, human-readable rendering, and explicit operator command dispatch only; it owns no CVA/config/auth/retrieval or configured-runtime semantics and can be removed or detached without changing the core package. `ConfiguredRuntime` is the library-owned seam for configured finite Insomnia/vector execution, while library constants own defaults exposed by optional CLI flags.
### Model switchboard
`ModelSwitchboardConfig` owns machine-local endpoint selection for explicit model capabilities. The current capabilities are `General`, `Insomnia`, `InsomniaMetadata`, `Dream`, and `Embedding`; the providers are `OpenAiCodex` and `OpenAiReady`. Main Insomnia and Dream may each have dedicated routes and otherwise resolve to General. Fixed-group metadata classification remains optional and uses `InsomniaMetadata` only when configured. Post-wording **Entity-mention enrichment** is required by the configured/runtime-host Insomnia workflow: it uses the metadata route when present and otherwise falls back to effective main Insomnia. Lexical Memory routing is deterministic and therefore uses no model route. When owner routing is enabled, ownership classification likewise prefers the metadata route and otherwise falls back to effective main Insomnia. Each configured route carries a stable `CredentialId`. `ModelCapability::route_key()` is the canonical external capability-key vocabulary used by host adapters.

`ReliquaryRuntimeRoutes` is the long-lived host boundary. A host may supply exact provider-adapter endpoints for General, Insomnia, Insomnia metadata, Dream, and Embedding, but Reliquary applies capability fallback and chooses which endpoint each background worker uses. `GatewayRuntime` therefore never receives or mutates a single generic inference endpoint: Insomnia uses effective main + optional metadata/ownership routes, Dream uses its effective Dream route, and vector workers use the embedding route. `ConfiguredRuntime::runtime_routes()` builds the same route bundle from `reliquary.cfg`, so the standalone configured path and embedded hosts share the same runtime composition rules.

`CredentialsConfig` owns decrypted in-memory credentials loaded from encrypted `credential.<id>` config objects. Constructing `ModelSwitchboard` validates that every selected route resolves to a credential of the provider's required auth kind. The switchboard can then produce request auth: bearer API key for `OpenAiReady`, or bearer ChatGPT access token plus optional `ChatGPT-Account-ID` for `OpenAiCodex`. `OpenAiReadyEmbeddingEndpoint` consumes the validated embedding route/auth, performs direct OpenAI-compatible HTTP embedding requests, preserves input order across concurrent batches, and normalizes output according to the declared route contract. `OpenAiReadyGeneralEndpoint` uses strict JSON-schema `response_format` for ordinary General/Insomnia OpenAI-compatible calls, while its dedicated Dream constructor forces exactly one named function/tool call whose parameters are the requested schema and accepts only that function's JSON arguments. This keeps Dream's post-model relation/direction/evidence validation strict even on providers that do not reliably honor `response_format` schema shape. Provider/model/URL/credential choices remain routing policy and do not enter Compatibility Profile identity or decide vector compatibility.
### Master key
`MasterKeyStore` owns retrieval/creation of the 256-bit key that protects credential objects. The current `JsonMasterKeyStore` is a temporary implementation that stores plaintext `reliquary.master-key.json` beside `reliquary.cfg`; production storage will move behind the same ownership boundary to the operating-system credential store. Credential ciphertext uses AES-256-GCM with fresh nonces and object-key-bound authenticated data. The master key is machine-local and is never CVA semantic state.
### Container
Container V2.0 is the sole current physical format. Other versions reject with typed `UnsupportedVersion` before owner dispatch, writable access or recovery. Current owners retain ordinary incomplete-tail recovery and Entity-resolution reference repair; no legacy inspection route remains.

REL/PHY reopen validates maintenance-history and exact description-prune evidence before durable recovery. REL compaction reclamation and Entity-resolution reference repair therefore cannot mutate a source that fails prune-proof admission. Entity compact proof metadata remains opaque until owner-level descriptor and source-history validation; an unpublished proof cannot authorize an available revision.

Container owns the fixed header, opaque length-prefixed chunks, `ChunkRef`, file I/O, sync, the single physical reopen scan, and file-global monotonic version tickets. Current headers distinguish `FileKind::Reliquary` from `FileKind::Phylactery`; both use semantic-scope discriminator `0`. Legacy typed REL discriminators remain readable compatibility data only. Organizational REL type is stored in `RelMetadata`, not file identity. Global version remains the canonical semantic mutation order and current `CVAVERS2` tickets additionally carry owner-local wall-clock transaction time. `transaction_time_ns(version)` exposes the exact persisted mapping when known; `version_at_or_before(transaction_time_ns)` resolves the highest provable contiguous global-version prefix whose included transaction timestamps are known and no later than the requested wall-clock cut. It cannot skip an unknown or future-timestamped earlier version to expose a higher version. Current `CVAVERS1` reservations remain explicitly untimestamped; `CVAVERS2` carries its exact time. Current `CVAINSC7` and retained-layout `CVAINSC8` grouped publications preserve their recorded range and optional timestamp without fabricating time for older evidence. Transaction time is knowledge/system time and does not replace source chronology or Chronos valid-time interpretation.
### Generic review evaluation

`crates/reliquary-container/src/review_clocks.rs` retains typed source/value validation and fixed/proportional source-progress math for independent summary scheduling. The per-Memory Freshness engine is separate: `crates/reliquary-freshness-store/src/score.rs` owns bounded score, admission/accounting anchors and the rebuildable `FreshnessBoundaryIndex` data structure; it can derive/drain crossed boundaries. `src/freshness_due.rs` owns the rebuildable bounded dispatcher; `src/facade/cva_freshness_due.rs` connects it to Cva, rebuilds after projection replay, refreshes only affected IDs after enrollment/admission/events, and advances at most 64 transitions on each accepted REL activity seam. Notices retain exact boundary turns; derived notifications may recur after reopen, and reversal notices coalesce to the latest state per Memory. `src/freshness/propagation.rs` evaluates event-scoped maxima over a pinned undirected adjacency: roots with the same immutable originating Community share one serial multi-source strongest-path traversal, distinct origins merge only by recipient maximum, and independent accepted-use events use the existing bounded parallel batch workers. The isolated P1 grouped-origin change passes focused regressions and exact-output matched release benchmarks on synthetic and three authenticated real graph topologies; P6 integrated local release is accepted with documented scheduling and microsecond retry tradeoffs; deployment-host acceptance remains separate. See [P6 integrated verification](freshness-performance-verification-2026-10-03.md). `crates/reliquary-freshness-store/src/storage.rs` owns the `CVAFRS02` owner-UUID-bound journal, immutable accepted effect receipts and derived score projections; `crates/reliquary-dream-store/src/freshness_journal.rs` owns `CVDREAM1` first-pass identity, frozen source turn/graph baseline/pre-existing IDs, root receipts, accepted-settlement cut and settled marker. `src/facade/cva_freshness.rs` supplies explicit enrollment, admission, historical projection and one-chunk event publication. Dream adapters record initial-settlement roots through both direct processing and the REL runtime host, recovering committed Dream-origin edges after the frozen baseline on retry; explicit accepted-use receipts are the only direct-access input. Pass/admission/event markers are separate synchronized chunks with focused crash-prefix recovery coverage. `src/facade/cva_reconcile_freshness.rs` validates provenance before destination creation and rebases accepted effects by exact Archive conversation/node identities; divergent pending Dream passes/publication intents fail closed. `crates/reliquary-freshness-store/src/freshness_storage_ordering.rs` reconstructs canonical `(accepted turn, event ID)` order in one accepted-prefix scan using bounded sorted runs and disposable two-way merges. The 128-entry/256-KiB recent receipt cache uses exact durable lookup after eviction. P4 adds a fixed 8,192-byte negative membership bitset plus a completeness flag in `crates/reliquary-freshness-store/src/freshness_storage_membership.rs`: only a proven absent owner-local use/event identity skips the durable scan. Positive or incomplete hints use the exact path. Validated receipts update the hint before cache eviction; prefix/path rebind invalidates it and reopen reconstructs it from a complete accepted-prefix scan. Both preappend and ingest projections remain unchanged. This is derived acceleration, never receipt authority; see [P4 verification](freshness-performance-p4-verification-2026-10-04.md). Runtime Dream prepares frozen settlement inputs under its mutex, evaluates propagation after releasing it, then reacquires it for owner validation and commit. No low-level Memory read/search automatically counts as access. The library exposes `record_accepted_memory_use`, the graded score/state query, and `select_ego_routine_

[TRUNCATED at 50000 chars]

## Decision endpoint inheritance

The configured-runtime composition boundary owns decision-route precedence for Entity Resolution and agentic Memory Retrieval. The private selector in `configured_runtime_decision.rs` chooses operation-specific decision configuration, shared `general_decision`, general-model configuration, or unavailable. `ConfiguredRuntime::runtime_routes()` invokes it once per consumer. Consumers receive the existing `Arc<dyn DecisionEndpoint>` and contain no inheritance ladder.

Specialized and shared routes reuse `ConfiguredDecisionEndpoint` and selected API-key authentication. The private `GeneralDecisionEndpoint` wraps the already constructed General endpoint, preserving provider authentication, reasoning and transport. Its choice contract validates exact answer IDs/types/choices and excludes probabilities; the General interface has no calibrated likelihood evidence. Retrieval supplies its own root response schema and retains owner-qualified identity validation. Selection never reacts to inference failure. Entity's existing resolver fallback is a separate operation policy, and its hosted worker still requires the separate general resolver route.

Protecting tests are the selector matrix, runtime decision integration tests, general decision contract tests, both local provider transport suites, and the Entity confidence/guard tests. No semantic state, storage owner, clock or worker lifecycle is added.

## Code Map

Crate-root Rust modules are composition/publication seams owned by the overall architecture; focused subsystem docs additionally own their internal trees.

- `src/*.rs`

## Related docs

- [Local configuration](configuration.md)
- [Behavioral contracts](behavioral-contracts.md)
- [Maintainer map](maintainer-map.md)

## Notes

Decision inheritance changes configured endpoint composition only; semantic persistence and owner lifecycle are unchanged.

The import owner persists current standalone semantic evidence. CVAILEG1/2 runtime validation and transport have been removed after source-validated corpus conversion. Container framing does not interpret import semantics.

Import epoch persistence is owned by `reliquary-import-store`, which depends only on Container among Reliquary crates. The Import coordinator retains receipt authority, readiness and Freshness settlement checks, and alone appends completion markers after those checks. The lower projection validates and replays accepted records without authorizing completion.
