# Semantic import boundaries, owner-neutral pruning, and history rewrite — implementation plan

Parent: [Documentation index](INDEX.md).

Date: 2026-10-18  
Status: **Complete.** Runtime implementation and acceptance are integrated across semantic import normalization, shared owner maintenance, PHY pruning, retained-profile repair, rewrite policy/planning/emission and CLI. Step 65 closes the final integration/compatibility gate; see [execution evidence](semantic-import-pruning-history-rewrite-evidence-2026-10-18.md).  
Baseline: main at 288bbdb318bd20428a8d42ec4460c7ec89cad4cd.  
Decision: [ADR 0041](decisions/0041-semantic-import-boundaries-owner-pruning-history-rewrite.md).  
Migration policy: staged hard cut internally; compatibility only for persisted REL/PHY formats that must continue to open.

## Purpose

Define the dependency order and acceptance gates for semantic import settlements, shared owner maintenance and explicit fresh-history rewrite.

## Overview

The work proceeds through the phases below; implementation presence is not itself a passed acceptance gate. Completed gates and observed limitations are recorded in the [execution evidence](semantic-import-pruning-history-rewrite-evidence-2026-10-18.md). Current runtime behavior remains documented by the architecture and storage references.

## Problem Statement

Three related gaps now need to be fixed together.

First, import epochs were intended to distinguish imported history from ordinary operation. The implementation correctly needed physical ordering while building a settlement so that same-cut events could be classified deterministically, but it promoted that temporary ordering mechanism into permanent physical authority. CVAFRS05 persists start/freeze prefixes plus an immutable-prefix digest, legacy framing authenticates a complete file prefix, and pruning/reclaim therefore refuse to alter otherwise disposable payload merely because it lies before an authenticated prefix. This makes imported packed vectors and other payload effectively immortal and becomes worse as additional imports advance the protected prefix.

Second, destructive pruning is asymmetric. REL has explicit retention proofs and prune markers for selected payload. PHY has Memory, Entity, Relationship, CompatibilityProfile, PackedVector and MemoryVector ownership plus reclaim support, but no owner-level maintenance evidence that can authorize intentional payload destruction. The existing HistoryPruneManifest is conceptually owner-neutral but is currently stored and validated through Archive.

Third, Reliquary lacks an intentional fresh-history rebuild operation. Migration preserves history and format semantics; pruning preserves history while dropping selected payload; reclaim removes only unreachable physical garbage. None of these can say: select the useful current Memories and semantic state, select the embedding profiles worth carrying forward, construct a fresh REL/PHY, and intentionally leave years of low-value revisions, duplicates, superseded Memories, retired vectors and obsolete derived records behind.

The concrete costs are:

- imported payload can become unprunable for reasons unrelated to semantic correctness;
- PHY cannot perform the same validated destructive maintenance as REL;
- retained fixtures cannot be compacted into clean development corpora without preserving irrelevant history;
- long-lived production owners have no explicit history-reset path and can grow indefinitely even after old history has lost practical value;
- current REL repair is unnecessarily single-profile even though the storage model permits multiple active compatible profiles.

## Target Architecture

### 1. Semantic import settlement

ImportStart and ImportComplete remain the canonical epoch boundaries.

Physical offsets are allowed only as transient construction inputs. During settlement construction, the coordinator may scan the finite physical interval between the persisted ImportStart position and the freeze point to decide which same-cut events were accepted inside the epoch. Once accepted, the settlement persists semantic event evidence and no longer requires those offsets or original surrounding bytes.

Add one canonical epoch settlement format, provisionally CVAFRS06, with an ImportSemanticScope containing:

- owner UUID;
- import ID;
- ImportStart digest;
- born Memory IDs;
- affected recipient Memory IDs;
- exact source conversation/node identities;
- base activity cut;
- settlement/freeze activity cut;
- exact accepted event evidence selected for the epoch;
- exact superseded/provisional event identities;
- optional predecessor legacy-settlement digest when normalizing CVAFRS03/04/05.

Each accepted event evidence item contains at minimum:

- stable event ID;
- digest of the canonical accepted event payload/effects/provenance;
- accepted activity turn;
- affected recipient IDs.

The settlement retains the existing sparse resulting Freshness records and policy/input/manifest evidence that remains semantically meaningful. It does not retain start_prefix, freeze_prefix, input_bytes as authority, or an immutable physical-prefix digest.

Reopen validates the settlement by resolving the named event identities from accepted Freshness history and checking their canonical digests and semantic cuts. Physical location is irrelevant.

### 2. Persisted-format compatibility

New writers emit only the canonical semantic settlement.

CVAFRS03, CVAFRS04, CVAFRS05 and CVAILEG1 remain readers at the persisted-format boundary. No owner/runtime code outside the compatibility decoder/normalizer branches on those formats.

A legacy normalization operation performs a one-time proof:

1. validate the old physical settlement using its existing rules;
2. derive exact semantic accepted-event evidence using the current physical interval;
3. materialize CVAFRS06 that names the legacy settlement digest as predecessor;
4. reopen and prove equivalent Freshness/import state;
5. thereafter treat CVAFRS06 as canonical authority.

Old bytes may remain as historical compatibility records, but they no longer pin unrelated payload once superseded by an accepted semantic settlement.

Stop emitting CVAILEG1. Existing frames remain readable only for normalization of old files.

### 3. Shared owner maintenance and pruning

Move prune-manifest ingestion/validation from Archive ownership to a shared owner-maintenance module.

Target modules:

- owner_maintenance::manifest — HistoryPruneManifest, HistoryRetentionDescriptor, RetentionTarget, PrunedPayload;
- owner_maintenance::store — owner-level CVAHPR01 ingestion, lookup and validation;
- domain retention providers remain in Memory, Entity, Relationship, PackedVector/Profile and Archive owners.

Both Cva and Phylactery contain one OwnerMaintenanceStore reconstructed during open.

Archive no longer owns the manifest store. It consumes shared maintenance evidence for ArchiveContent markers.

Pruning remains transformed-copy only.

REL supports:

- Archive conversation content;
- terminal Memory bodies where independently removable;
- terminal Entity/Relationship descriptions where supported;
- retired CompatibilityProfile packed-vector payload;
- later supported payloads only after an explicit owner retention contract exists.

PHY supports:

- terminal Memory bodies;
- terminal Entity/Relationship descriptions;
- retired CompatibilityProfile packed-vector payload;
- no Archive-specific targets.

Reclaim remains conservative and never infers that retirement/tombstoning alone authorizes destruction.

### 4. Multiple retained profiles in repair

Generalize retained-owner repair from one canonical profile to an explicit retained profile set.

The CLI flag remains named --keep-profile but becomes repeatable. One use preserves current syntax. Multiple uses preserve multiple profiles.

Repair rules:

- at least one retained profile for repair mode unless an explicit no-profile mode is later added;
- every retained profile must exist and not be retired;
- by default each retained profile must have complete current coverage for the owner data it is expected to serve;
- all unselected profiles are retired;
- --prune-retired authorizes eligible retired packed-vector payload;
- --reclaim performs ordinary post-prune compaction;
- source is never modified in place;
- output is reopened and active-profile set must equal the requested retained set.

Implement the same behavior for rel repair and phy repair.

### 5. History Rewrite

Add a separate deep module, rewrite, for intentional fresh-history construction.

Public conceptual interface:

- plan_rewrite(source, RewriteRequest) -> RewritePlan
- rewrite_file(source, output, RewriteRequest) -> RewriteReport

The request is file-kind agnostic where semantics are shared and contains REL-only options only when the source is REL.

Rewrite never calls migration as its semantic policy. It may share extracted owner snapshot/emission helpers where migration and rewrite truly need the same domain primitive.

The planner is read-only. It resolves all filters and referential closure before output creation and returns exact retained/dropped counts, closure expansions, selected profiles, profile coverage, expected unsupported conditions and a policy digest.

Execution consumes an accepted plan against the exact source head/digest it was planned from. Source drift rejects before output publication.

### 6. Memory-centered rewrite policy

Memory selection is a primary input.

RewriteRequest contains MemorySelection with:

- base population: all current Memories or active current Memories;
- explicit include IDs;
- explicit exclude IDs, which win over include;
- include/exclude archived Memories;
- include/exclude superseded Memories;
- include/exclude terminal/tombstoned Memories where their retained representation is requested;
- drop-known-duplicates option using only canonical persisted duplicate/supersession evidence; rewrite does not perform new inference;
- optional minimum effective Freshness;
- optional created/updated/source-time lower bounds where canonical timestamps exist;
- revision policy: current-only, all retained revisions, or retained revisions since a global/version boundary.

The plan reports why every Memory is retained or omitted.

Current-only rewrite republishes a retained Memory as the initial revision in the new history while preserving stable semantic Memory identity where the destination identity policy allows it. Historical revision numbers are not required to survive a fresh-history rewrite.

### 7. Embedding profile and vector selection

RewriteRequest accepts zero or more CompatibilityProfileIds.

The caller may retain any number of profiles. There is no one-profile assumption.

For each selected profile:

- copy the immutable profile definition;
- select only Memory-vector rows whose MemoryBodyIds survive;
- for REL, select only Archive-vector rows whose FragmentIds survive;
- build new compact PackedVector objects from those selected rows instead of copying dead rows wholesale;
- publish only the minimal destination MemoryVector/ArchiveVector mappings and current VectorGeneration baseline needed by the rewritten state;
- preserve retired/active status unless the request explicitly changes activation;
- reject incomplete requested coverage by default; an explicit allow-incomplete policy may retain an inactive partial profile.

If no profiles are selected, the destination contains no copied compatibility/vector state and may be embedded later through ordinary production APIs.

Rewrite never invokes an embedding provider unless a separate explicit re-embed mode is added in future work.

### 8. Archive and provenance selection

REL Archive selection is independent from Memory selection.

ArchiveSelection supports at minimum:

- all retained source history;
- current/recent source history from a semantic/time boundary;
- explicit conversation IDs;
- none except required provenance closure.

Default provenance policy is PreserveClosure:

- if a retained Memory requires a source Node/Episode/Fragment for valid durable provenance, the planner expands Archive selection to the minimal required closure;
- closure expansion is reported before execution;
- no dangling source reference is emitted.

An explicit StripProvenance policy may rewrite selected Memories without source provenance only where Memory invariants allow it. This is a deliberate lossy rewrite and must be reported. Reject is also available for callers that prefer failure to automatic expansion.

### 9. Entity, Relationship/Observation, Graph and Ego state

Selections must preserve referential validity.

The default for semantic objects is current referenced closure plus explicitly selected current state:

- retain Entities required by retained Relationships/Observations and selected Graph relations;
- retain Memory evidence required by selected Relationship/Observation state unless policy explicitly drops that evidence;
- retain current owner-valid Graph relations whose endpoints survive;
- do not replay obsolete graph transactions merely to reproduce the current graph;
- rebuild Communities and lexical/search indexes from retained current state rather than copying historical derived generations.

PHY authoritative Ego Identity/Profile/Personality state is user-owned and retained by default. Derived Ego synthesis caches, Community generations, duplicate indexes and similar acceleration/derived state are rebuilt or omitted by default.

REL Ego synthesized state is selectable separately from user-owned/core semantic data.

### 10. Freshness rewrite modes

RewriteRequest defines one FreshnessRewriteMode:

- PreserveEffectiveCurrent — default; initialize each retained Memory from its effective current Freshness state at the frozen source cut;
- Reset — initialize retained Memories as new under the current Freshness policy;
- RecomputeRetainedHistory — replay only the history selected into the destination and compute a new result.

PreserveEffectiveCurrent requires a new explicit Freshness baseline/genesis representation. It must not manufacture fake historical access/Dream events.

The baseline binds owner, destination history generation, Memory ID, effective score/state, admission/last-activity information required by current policy, source cut and rewrite-policy digest.

### 11. Rewrite lineage and owner identity

IdentityPolicy has two modes.

NewOwner:

- create a new UUID;
- retain source lineage only as rewrite provenance;
- intended for development fixtures, experiments and intentional forks.

PreserveOwner:

- preserve owner UUID;
- create a new HistoryGenerationId;
- persist RewriteGenesis as the first maintenance/lineage record in the new history;
- record predecessor owner UUID, predecessor generation, frozen source head/digest, rewrite policy digest and creation time;
- reconciliation requires both owner UUID and history generation to match;
- different generations fail closed with HistoryGenerationConflict rather than being treated as ordinary divergent replicas.

Existing owners without a generation record are generation zero.

Mount/registry identity may remain keyed by owner UUID, so two generations of the same owner cannot be mounted concurrently through a single-owner registry. That is acceptable and prevents accidental simultaneous operation.

### 12. Rewrite safety and UX

Rewrite is never in place.

The planner exposes a dry-run report with:

- retained/dropped Memories and revision counts;
- selected/dropped Archive conversations/nodes/fragments for REL;
- retained Entities/Relationships/Graph edges;
- selected profile IDs and row counts;
- closure expansions;
- Freshness mode;
- source identity/generation/head;
- expected destination identity/generation;
- estimated payload bytes where measurable;
- explicit lossy decisions.

Execution rejects if output exists.

After writing, reopen and validate the destination, produce RewriteReport, then leave source unchanged. Promotion/replacement of a production file is a separate caller/operator action.

## User / Developer Outcomes

- Imported vectors are prunable when their semantic owner evidence permits it.
- Import history remains distinguishable without pinning arbitrary file bytes forever.
- REL and PHY have symmetric destructive owner maintenance.
- Repair can retain any requested set of embedding profiles.
- Developers can produce clean fresh-history fixtures from existing REL/PHY files without rerunning upstream inference/import pipelines.
- Users can compact long-lived stores by selecting useful Memories and semantic state rather than retaining every historical revision forever.
- Rewrite produces an inspectable plan before any destructive/lossy destination is created.
- Migration, prune, reclaim and rewrite have non-overlapping meanings and test surfaces.

## Implementation Decisions

1. CVAFRS06 is the only new import-settlement writer.
2. CVAFRS03/04/05/CVAILEG1 compatibility remains read/normalize-only.
3. Physical offsets are transient construction mechanics, not durable semantic authority.
4. Existing import source/cohort/event attribution rules remain; only the enduring proof representation changes.
5. HistoryPruneManifest remains CVAHPR01 unless implementation evidence requires a format revision. Its current owner_uuid/source_history_digest/descriptors model is already owner-neutral.
6. Archive stops owning prune manifests.
7. Cva and Phylactery each reconstruct OwnerMaintenanceStore directly from container records.
8. Domain retention proof remains domain-owned; the maintenance store never decides semantic eligibility by itself.
9. Repair accepts a retained set of profiles, not one special canonical profile.
10. Rewrite is a new operation and does not weaken migration/reconciliation semantics.
11. Rewrite planning is pure/read-only and deterministic for a frozen source.
12. Rewrite execution consumes a plan/policy digest and rejects source drift.
13. Memory selection is explicit and first-class.
14. Any number of embedding profiles may be selected.
15. Selected vector profiles are row-compacted around retained Memory bodies/Archive fragments.
16. Rewrite does not call model/embedding providers.
17. Referential closure is explicit and reported.
18. PreserveEffectiveCurrent Freshness uses a baseline record, not fabricated history.
19. Preserve-owner rewrite creates a new history generation and reconciliation fails across generations.
20. New-owner rewrite remains the default development mode.

## Migration Strategy

This is a staged hard cut with one persisted-format compatibility exception.

Internal target:

- one semantic epoch settlement path;
- one shared owner-maintenance prune store;
- one rewrite planner/executor;
- no permanent physical-prefix authority.

During the import-settlement migration window, focused import/Freshness tests are the gate. Do not add wrappers that preserve the old prefix-authority API internally.

Persisted files are the external compatibility boundary. Old settlement readers stay isolated in compatibility code until supported persisted files have a semantic normalization path.

Once all internal producers/consumers use CVAFRS06, delete prefix-based authority from ordinary prune/reclaim/reconciliation logic. Do not retain dual-authority fallback.

Owner-maintenance extraction is also a move-once hard cut: move manifest storage/validation to its final shared module, migrate REL and PHY open paths, then delete Archive-owned manifest state.

Rewrite is additive and does not require an intermediate compatibility layer.

## Testing Decisions

Primary completed-architecture seams:

- import_native / historical reconstruction / complete_native_import for import epochs;
- Cva::prune_tombstoned_history and Phylactery::prune_tombstoned_history for history-preserving destruction;
- rel/phy repair for profile retirement + optional pruning/reclaim;
- plan_rewrite and rewrite_file for fresh-history construction;
- Cva::open / Phylactery::open for all persisted validation.

Important existing tests to retain/adapt:

- historical_import_attribution_tests;
- historical_import_accepted_use_tests;
- historical_import_transform_tests;
- imports epoch/readiness/completion/sequential/reconciliation tests;
- history_prune_tests;
- storage_reclamation_tests;
- migration_tests;
- memory_lifecycle_tests;
- retained fixture repair/retrieval parity evidence.

Required new fixture classes:

- imported packed vector that is retired and successfully pruned after semantic-settlement normalization;
- same-cut pre-start and in-import events with identical activity turn but distinct accepted identities;
- CVAFRS05 source normalized to CVAFRS06 then physically repacked with changed offsets;
- PHY retired profile vector prune;
- PHY terminal Memory/Entity/Relationship prune;
- multi-profile REL/PHY repair;
- rewrite all-current/new-owner parity;
- rewrite preserve-owner/new-generation conflict against predecessor;
- Memory filtering for superseded/archived/tombstoned/Freshness/time/explicit IDs;
- profile row compaction for zero/one/multiple profiles;
- provenance closure expansion and explicit strip/reject modes;
- PreserveEffectiveCurrent Freshness baseline;
- source-drift rejection between plan and execution.

Whole-repository gates remain required at the final integration points.

Arcana is supporting structural evidence only. The planning snapshot at specification time was sha256:8dc23c5cddc11225aefb635ccbf28fc3c2a41815fca06a7a1c7dd8b56cf54d8f with 85,185 nodes and 217,775 relationships. Refresh before final structural acceptance.

## Out of Scope

- automatic age-based background rewrite/rollover scheduling;
- SaaS/cloud retention policy;
- implicit destructive rewrite without explicit user/operator request;
- new embedding inference or re-embedding during rewrite;
- automatic semantic deduplication beyond already persisted canonical duplicate/supersession evidence;
- cross-generation reconciliation;
- in-place rewrite;
- PHY Archive semantics;
- deletion of user-owned source files outside the destination REL/PHY;
- changing Freshness scoring policy itself;
- changing Memory identity rules outside explicit rewrite publication semantics.

## Linear Execution Plan

Assignments: Sol owns schema/ownership changes, hard-cut migrations and integration. Luna is appropriate for bounded source audits, fixture construction, focused regression additions and documentation checks where the task has an exact interface and acceptance gate.

The plan now has 65 steps: retained-fixture retrieval exposed Graph admission/order preservation as an independent failure domain. The steps below are cut at natural implementation and verification boundaries. Step count is not a target; if implementation reveals a genuinely independent failure domain, split the affected step rather than forcing work into the current numbering.

### Phase A — Semantic import settlement hard cut

Status: runtime V6 publication/replay/reopen, explicit legacy normalization and reconciliation wiring are present. Final root tests and the supported old-format matrix remain to be confirmed.

| Step | Owner | Work | Gate |
| ---: | --- | --- | --- |
| 1 | Sol | Freeze baseline 288bbdb: persisted-format inventory, focused import/prune/reclaim commands, current Arcana snapshot and authoritative fixture hashes. | Baseline record exists; no source change. |
| 2 | Luna | Audit every producer/consumer of ImportEventScope, epoch_prefix_digest, start_prefix, freeze_prefix, source_prefix_len, CVAFRS03/04/05 and CVAILEG1; classify construction-only use versus enduring authority. | Exact symbol/file inventory with compatibility boundary identified. |
| 3 | Sol | Define the canonical CVAFRS06 settlement model, ImportSemanticScope and AcceptedImportEventEvidence, including canonical event digest rules, ordering, bounds and codec validation. | Round-trip, corruption, truncation, duplicate and noncanonical-order tests pass. |
| 4 | Sol | Change historical reconstruction to derive exact semantic event evidence from the transient ImportStart-to-freeze physical interval before settlement commit. | Same-cut pre-start versus in-import attribution passes without persisting physical authority in V6. |
| 5 | Sol | Make FreshnessStore ingest, replay and reopen-validate CVAFRS06 by semantic event identity/digest rather than physical position. | Relocated-copy reopen reproduces sparse records and superseded identities. |
| 6 | Sol | Migrate import readiness/completion to treat CVAFRS06 as the semantic work receipt. | Readiness/completion focused suites pass, including empty/zero-activity imports. |
| 7 | Sol | Migrate import reconciliation/replay to regenerate destination CVAFRS06 evidence from replayed semantic events. | Sequential and divergent import reconciliation tests pass. |
| 8 | Sol | Implement CVAFRS05 -> CVAFRS06 normalization using legacy prefix authority exactly once, including predecessor digest binding. | Valid V5 normalizes; damaged prefix/evidence rejects; normalized copy no longer needs V5 physical authority. |
| 9 | Sol | Implement CVAFRS03/04/CVAILEG1 normalization for initial legacy imports and stop all new CVAILEG1 emission. | Legacy fixtures normalize without score change; source search finds no production legacy-frame writer. |
| 10 | Luna | Add adversarial semantic-settlement tests: missing/changed event, conflicting digest/effects, wrong source/cohort, duplicated identity and same-cut ordering. | All invalid cases fail closed at the V6 validation boundary. |
| 11 | Sol | Hard-cut current internal producers/consumers to CVAFRS06 and delete obsolete immutable-prefix authority helpers from ordinary runtime paths. | Source/Arcana search finds old prefix authority only inside persisted-format compatibility/normalization code. |
| 12 | Sol | Integration gate A: format/check plus focused Historical Freshness, import epoch, readiness/completion and reconciliation suites. | No task-related failure; supported old persisted formats still open/normalize. |

### Phase B — Remove physical-prefix maintenance restrictions

Status: pruning/reclaim now consume semantic maintenance evidence after explicit working-copy normalization; transformed-copy acceptance and authoritative fixture measurements remain open.

| Step | Owner | Work | Gate |
| ---: | --- | --- | --- |
| 13 | Sol | Remove import freeze/source-prefix offset prohibitions from Cva destructive prune; require legacy files to normalize semantic authority before destructive maintenance. | Eligible imported payload is no longer rejected solely because of physical offset. |
| 14 | Sol | Remove protected-prefix suppression from reclaim_container so reclamation depends on live references and maintenance evidence only. | Reclaim no longer pins arbitrary pre-import bytes. |
| 15 | Sol | Change validate_historical_import_copy and related transform/repack validation to compare semantic import authority rather than physical prefix digests. | Physically relocated V6 copies validate. |
| 16 | Luna | Add a retained-profile fixture whose packed vectors lie inside a former authenticated import prefix; normalize, prune and reclaim it. | Output shrinks and semantic/Freshness state survives reopen. |
| 17 | Sol | Run copy-only semantic-normalization/prune experiments against authoritative retained REL fixtures and record reclaimable bytes/blockers. | Original fixtures remain unchanged and results are recorded. |
| 18 | Sol | Integration gate B: retained REL reopen and retrieval parity before versus normalized/repacked copies. | Current retrieval results are identical when semantic state is equivalent. |

### Phase C — Owner-neutral maintenance substrate

Status: shared owner-maintenance store is wired into REL and PHY. Full existing pruning/reopen tests remain part of the pending integration gate.

| Step | Owner | Work | Gate |
| ---: | --- | --- | --- |
| 19 | Sol | Move HistoryPruneManifest/retention types and CVAHPR01 codec to owner_maintenance without changing persisted bytes, and add OwnerMaintenanceStore ingestion/lookup/validation. | Existing manifest fixture bytes remain identical; shared store round-trips. |
| 20 | Sol | Remove generic manifest storage from Archive and wire OwnerMaintenanceStore into Cva open/rebuild. | Existing pruned RELs reopen unchanged; Archive retains only Archive-specific retention logic. |
| 21 | Sol | Wire OwnerMaintenanceStore into Phylactery open/rebuild, rejecting malformed or unauthorized maintenance evidence. | Ordinary PHYs still reopen; malformed shared evidence fails closed. |
| 22 | Sol | Move Memory, Entity and Relationship prune-marker authorization/validation to shared maintenance evidence. | Existing REL domain-prune tests pass without Archive manifest lookup. |
| 23 | Sol | Move CompatibilityProfile/PackedVector retention descriptors and prune-marker validation behind owner-neutral helpers. | Existing REL retired-vector pruning remains valid. |
| 24 | Luna | Audit CVAHPR01 ownership/consumers with source search and Arcana; identify any remaining generic maintenance responsibility under Archive. | No surviving Archive-owned generic manifest path. |
| 25 | Sol | Integration gate C: full existing REL prune/reclaim suite on the shared owner-maintenance substrate. | Persisted compatibility and REL behavior remain unchanged. |

### Phase D — PHY destructive pruning and retained-profile repair

Status: PHY prune and REL/PHY repeated-profile repair implementations are present. CLI/library verification and disposable PHY fixture measurements remain open.

| Step | Owner | Work | Gate |
| ---: | --- | --- | --- |
| 26 | Sol | Implement Phylactery transformed-copy prune coordinator and update Phylactery::open to accept prune markers only when exact owner-maintenance evidence validates them. | Empty/live/unauthorized targets reject; authenticated markers reopen. |
| 27 | Sol | Add terminal PHY Memory-body pruning using existing Memory retention descriptors and markers. | Selected tombstoned bodies reopen as intentionally pruned while current state remains valid. |
| 28 | Sol | Add terminal PHY Entity and Relationship description pruning. | Current and retained-history validation pass after transformed-copy reopen. |
| 29 | Sol | Add retired PHY CompatibilityProfile packed-vector pruning with active/shared/unknown-reference protection. | Eligible payload shrinks; unsafe/shared payload rejects. |
| 30 | Sol | Generalize rel repair from one --keep-profile to a repeated retained-profile set with exact active-set verification. | One or many requested profiles work; unselected profiles retire. |
| 31 | Sol | Add phy repair with repeated --keep-profile plus --prune-retired and --reclaim, matching REL copy-only safety semantics. | Source is unchanged; reopened output matches requested active profile set. |
| 32 | Luna | Add REL/PHY repair fixtures for one/multiple retained profiles, shared matrices, unsafe active references and prune/reclaim behavior. | Valid selections succeed; unsafe destructive cases reject. |
| 33 | Sol | Run repair/prune/reclaim against disposable authoritative PHY copies and record active profiles, bytes removed and legitimate blockers. | No authoritative fixture is mutated. |
| 34 | Sol | Integration gate D: lifecycle, shared-prune, reclaim and repair CLI focused suites for both file kinds. | REL/PHY maintenance symmetry is established without Archive semantics leaking into PHY. |

### Phase E — Rewrite planner

Status: public policy/plan surfaces and selection planning are present; final adversarial test results and unsupported-policy matrix remain to be confirmed.

| Step | Owner | Work | Gate |
| ---: | --- | --- | --- |
| 35 | Sol | Add rewrite module with RewriteRequest, RewritePlan, RewriteReport, IdentityPolicy and deterministic frozen-source snapshot identity. | Types compile; source drift can be detected before output creation. |
| 36 | Sol | Implement MemorySelection and revision-policy planning, including explicit include/exclude precedence, lifecycle/supersession/known-duplicate/Freshness/time filters and retention reasons. | Table-driven planner tests produce exact deterministic Memory/revision sets. |
| 37 | Sol | Implement zero/one/many CompatibilityProfile selection and coverage/row analysis for the planned Memory/Archive population. | Plan reports exact selected profiles, rows, coverage gaps and active/retired state. |
| 38 | Sol | Implement REL ArchiveSelection plus provenance closure/strip/reject policy. | PreserveClosure expands only required source state and reports every expansion. |
| 39 | Sol | Implement Entity/Relationship/Observation/Graph referential-closure planning. | No planned semantic object has an unresolved required local reference. |
| 40 | Sol | Classify derived-state and Ego transfer policy: authoritative PHY user-owned state, selectable REL/Ego state, and rebuild/omit treatment for indexes, Communities, caches and processing receipts. | Every current store/domain has an explicit rewrite disposition. |
| 41 | Sol | Implement canonical RewritePlan encoding/digest, dry-run statistics, lossy-decision reporting and source-head binding. | Same frozen source/request yields byte-identical plan digest. |
| 42 | Luna | Add planner adversarial tests for conflicting explicit selection, closure expansion, missing coverage, unsupported provenance stripping and source drift. | Planner fails before output creation for invalid requests. |
| 43 | Sol | Integration gate E: rewrite planning is read-only/deterministic for representative REL and PHY fixtures. | Fixture hashes remain unchanged; plans reproduce exactly. |

### Phase F — Rewrite lineage and Freshness baseline

Status: generation, rewrite-genesis, typed reconciliation conflict and Freshness baseline support are present in source; lineage/baseline acceptance is pending final tests.

| Step | Owner | Work | Gate |
| ---: | --- | --- | --- |
| 44 | Sol | Add HistoryGenerationId, generation-zero interpretation for existing files, and persisted RewriteGenesis lineage/provenance record. | Existing files open as generation zero; new record round-trips/corruption-checks. |
| 45 | Sol | Expose history generation/lineage through REL/PHY open state and make reconciliation require matching owner UUID plus generation. | Cross-generation reconciliation fails with a typed conflict. |
| 46 | Sol | Implement NewOwner and PreserveOwner destination identity rules, including generation advancement and predecessor evidence for PreserveOwner. | NewOwner gets a new UUID; PreserveOwner retains UUID and advances generation. |
| 47 | Sol | Add explicit Freshness rewrite-baseline representation for PreserveEffectiveCurrent and rebuild it during open without synthetic historical events. | Retained Memories reopen with the source effective Freshness state at the frozen cut. |
| 48 | Luna | Add lineage, destination-identity, Freshness-baseline and cross-generation reconciliation adversarial tests. | Invalid lineage/baseline fails closed; same-generation behavior remains valid. |
| 49 | Sol | Integration gate F: REL/PHY generation and Freshness-baseline suites. | Existing generation-zero behavior is preserved and rewritten generations validate. |

### Phase G — Rewrite emission

Status: REL/PHY rewrite emitters and row-selection paths are integrated and still under semantic-coverage review. Do not treat the phase gate as passed until the REL/PHY acceptance matrix completes.

| Step | Owner | Work | Gate |
| ---: | --- | --- | --- |
| 50 | Sol | Implement fresh destination publication for selected Memories/revisions according to the RewritePlan. | Memory identity/content/selected metadata match the plan and destination clocks begin cleanly. |
| 51 | Sol | Implement REL Archive/source emission for the selected source/provenance closure. | Destination reopens with no dangling Memory provenance. |
| 52 | Sol | Implement Entity/Relationship/Observation state emission and fresh Graph baseline construction from retained current semantic relations. | Referential and graph validation pass without replaying obsolete transaction history. |
| 53 | Sol | Persist a lineage-bound fresh Graph admission baseline, preserving admitted Memory node order and isolated nodes independently of obsolete relation transactions. | All-current Community routing and retrieval remain identical; forged baseline owner/generation/policy rejects; same-generation reconciliation preserves the baseline. |
| 54 | Sol | Implement PHY authoritative Ego-state transfer plus selectable REL/derived Ego handling. | User-owned PHY state is retained by default; derived state follows plan. |
| 55 | Sol | Implement selected-profile copy with row-compacted Memory PackedVectors for REL and PHY. | Destination matrices contain only rows for retained Memory bodies. |
| 56 | Sol | Implement REL Archive-vector row compaction and minimal destination VectorGeneration publication for selected profiles. | Only retained Fragment rows/profile generations exist; unselected profile state is absent. |
| 57 | Sol | Implement Freshness rewrite modes PreserveEffectiveCurrent, Reset and RecomputeRetainedHistory. | Deterministic fixtures prove each mode independently. |
| 58 | Sol | Integration gate G: all-current fresh-history rewrite for REL and PHY reopens and matches the planned semantic projection. | No omitted/unplanned state appears in the destination. |

### Phase H — CLI and final acceptance

Status: CLI repair/rewrite surfaces, strict policy-file parsing and dry-run dispatch are integrated. CLI compile/tests, six disposable authoritative-fixture rewrites, hash invariance and full integration remain pending.

| Step | Owner | Work | Gate |
| ---: | --- | --- | --- |
| 59 | Sol | Add rel rewrite and phy rewrite dry-run/execute CLI surfaces with policy-file support and essential repeated profile/Memory-selection overrides. | Dry-run writes nothing; execute refuses existing output; PHY rejects REL-only options. |
| 60 | Luna | Add development-clone and selective-compaction acceptance fixtures covering superseded/archived/known-duplicate/low-Freshness Memories and zero/one/multiple selected profiles. | Destination contains exactly the planned Memory/profile/vector population. |
| 61 | Sol | **Complete.** All-current retrieval parity for equivalent selected profiles, including the Ellis gold benchmark, plus PreserveOwner rollover verification. | Fresh current-tree rewrite produced exact 27-case source/destination parity for hybrid and Community-routed retrieval; PreserveOwner lineage/generation contract passed and predecessor remained unchanged. |
| 62 | Sol | **Complete.** Ran disposable authoritative REL/PHY rewrite experiments and recorded source/destination size, state counts, vector rows and retrieval deltas. | All six authoritative source hashes remained byte-identical; six zero-profile outputs reopened, three REL retained-profile outputs reopened, and Ellis retained exact 27-case retrieval parity. |
| 63 | Luna | **Complete.** Updated architecture, invariants, storage format, API, CLI, operator manual, current limitations, ADR 0040 cross-reference and behavioral-contract mapping. | Both required documentation-policy checks pass; stale authority/status language is removed from current-reference docs and every critical semantic-maintenance/rewrite invariant maps to focused tests or retained benchmark evidence. |
| 64 | Sol | **Complete.** Hard-cut obsolete prefix-authority/Archive-manifest aliases and the dead CVAILEG1 writer, refreshed Lexicon/Arcana, and ran Pitlord/architecture enforcement. | Destructive maintenance has no prefix guard; legacy physical evidence remains isolated to persisted-format validation/normalization, OwnerMaintenanceStore is the direct shared owner, and fresh snapshot `8b081bb6…` passes all 23 Pitlord rules. |
| 65 | Sol | **Complete.** Final integration gate: root + CLI format/check/tests, retained reopen/repair/prune/rewrite matrix, V3/V4/V5/legacy/V6 compatibility matrix, final evidence record and cleanup of migration-only artifacts. | Root fmt/check pass; full root test reaches 1,497 pass / 1 classified Windows Lore file-lock flake / 5 ignored and the failed case passes in isolation; CLI 32/32, rewrite contracts 16/16, explicit compatibility/prune matrix all pass; obsolete migration wrapper removed; final Pitlord enforcement passes. |

## Completion Gates

The work is complete only when all of the following are true:

1. New imports write semantic CVAFRS06 settlements and no new internal writer emits CVAFRS03/04/05/CVAILEG1.
2. No ordinary runtime/prune/reclaim path uses import physical offsets as enduring semantic authority.
3. Existing supported physical-prefix settlements can be validated and normalized to semantic evidence.
4. Imported retired vectors can be pruned when domain retention rules permit.
5. HistoryPruneManifest ownership is shared owner maintenance, not Archive.
6. PHY validates authenticated prune markers and supports Memory/Entity/Relationship/retired-vector destructive pruning.
7. REL and PHY repair accept an explicit retained set of profiles.
8. Rewrite planning is deterministic, read-only and reports every lossy/closure decision before execution.
9. Rewrite can select exact Memories and deterministic Memory filters.
10. Rewrite can select zero, one or many embedding profiles and compacts vector rows to retained data.
11. REL rewrite handles Archive/provenance closure without dangling references.
12. PHY rewrite preserves authoritative user-owned Ego state by default.
13. PreserveEffectiveCurrent Freshness starts from an explicit baseline, not fabricated events.
14. New-owner rewrite produces an independent owner.
15. Preserve-owner rewrite produces a new history generation with predecessor evidence.
16. Reconciliation fails closed across history generations.
17. All-current rewrite has current-state and retrieval parity under equivalent selected profiles.
18. Authoritative source fixtures remain unchanged during verification.
19. Whole-repository and CLI integration gates meet the accepted baseline.
20. Arcana/Pitlord structural checks pass and no old internal prefix-authority consumer remains.

## Related docs

- [Architecture](architecture.md)
- [Storage format](storage-format.md)
- [Behavioral contracts](behavioral-contracts.md)
- [ADR 0041](decisions/0041-semantic-import-boundaries-owner-pruning-history-rewrite.md)
- [Execution evidence](semantic-import-pruning-history-rewrite-evidence-2026-10-18.md)

## Notes

The execution steps are intentionally cut by dependency and verification boundaries rather than a target count. Split a step when implementation exposes a genuinely independent failure domain; do not split or merge work merely to preserve the current numbering. The three architectural changes should not be re-expanded into separate incompatible implementations:

- semantic import settlement is one hard-cut migration;
- owner maintenance is one shared pruning substrate;
- rewrite is one planner/executor used by both file kinds.

Do not turn Rewrite into a general mutable-object framework. Its job is deterministic selection, closure, fresh-history publication and verification.

Do not make long-term retention automatic in this work. Once rewrite exists and production experience provides real data, policy presets or scheduled retention can be designed separately.
