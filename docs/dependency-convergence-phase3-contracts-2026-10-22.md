# Dependency Convergence — Phase 3 interface contracts

Date: 2026-10-22
Status: **P3.1 interface design reviewed locally; implementation gates not passed**
Parent: [Phase 3 specification](dependency-convergence-phase3-history-maintenance-import-spec-2026-10-22.md)
Predecessor: [Phase 3 owner-contract handoff](dependency-convergence-phase3-handoff-2026-10-22.md), [Phase 2 owner contracts](dependency-convergence-phase2-contracts-2026-10-22.md)
Authority: [Accepted component DAG](dependency-convergence-target-dag-2026-10-21.md) and [machine-readable allowlist](../tools/pitlord/convergence-component-dag.json)

## Purpose

Freeze the correction-only Phase 3 interfaces and hard-cut obligations over the accepted dependency DAG.

## Overview

This document freezes the intended contracts for P3.1.01–P3.1.08 only. It does not move code, select a settlement representation, authorize a format migration, or claim any implementation gate passed. The Phase 3 before-image and receipts are recorded in the [baseline](dependency-convergence-phase3-baseline-2026-10-22.md); this contract does not independently rerun them. The six-source settlement field/alternative audit belongs to the later S3 gate after I3, so it is deliberately not a B1 prerequisite.

The contract names existing symbols where source inspection confirms them and freezes operation-specific borrowed-input values for service extraction. These are not generic frameworks. Where a lower owner-lifecycle helper must be introduced, its contract is described at that exact boundary and tied to an existing field access or validation obligation.

## P3.1.01 — History: read-only cuts and proof views

History has no write-capable Container handle and no Freshness publication capability. Its first concrete seam is the source-history digest, currently implemented by `owner_history_digest(&mut Container, watermark, &GraphStore)`. The target removes the broad mutable Container argument while retaining the already accepted local graph proof API:

```rust
// owner-lifecycle: narrow source access. Fields remain private.
pub(crate) struct HistorySource<'a> {
    container: &'a Container,
    owner_uuid: Option<[u8; 16]>,
    source_head: u64,
    prefix_end: u64,
}

// History composes accepted evidence without receiving a Container.
pub(crate) fn owner_history_digest(
    source: &HistorySource<'_>,
    watermark: u64,
    graph: &GraphStore,
) -> Result<[u8; 32], String>;
```

`HistorySource` is private owner-lifecycle state, not a value imported into History with direct Container access. Owner-lifecycle holds `&Container`, captures owner UUID/head/prefix end and supplies bounded cursors yielding `HistoryFrame { payload, end, latest_global }`. Its frame decoder interprets `CVAPUB01` and clock data; History receives the bounded DTO and does not depend directly on Container publication-envelope modules. History checks referenced frame membership against the complete accepted `ObjectRef` set produced by a first bounded cursor scan, including references to a later physical frame; preserving that check matters because the old digest rejected a VectorGeneration reference unless it named an actual frame object, not merely an in-bounds byte range. No payload collection is retained: the capability scans bounded frames and retains only object membership evidence.

History's `owner_history_digest(&HistorySource, watermark, &GraphStore)` consumes the DTO stream, the global watermark, `GraphStore::compact_carrier_digest`, and owner-local normalized marker/evidence functions. Archive, Memory, Entity, Relationship, Graph and vector owners provide their comparison marker/value bytes with relocation-sensitive references normalized inside the owner codec (including zero-reference marker forms where already used); History does not import Container's publication decoder or root `ObjectRef` encoding. SHA-256 is a crypto primitive, not a dependency on the `pure-codec` DAG component; canonical marker bytes and per-owner semantic validation remain owner responsibilities. The digest behavior remains semantic publications through the frozen global watermark with normalized relocatable references; physical offsets do not enter. `String` preserves the current helper's error surface until each owner error mapping is pinned.

History's corresponding bounded input is a borrowed `HistorySource` plus an explicit watermark and `GraphStore`. The source capability and watermark do not carry an entire Cva/Phylactery or Import policy. Owner cut composition consumes existing local owner records/coordinate getters, not mutable Cva fields. The currently test-only `GlobalOwnerCuts` helpers are evidence of cut behavior, not production interface symbols. Cut selection must reject a global cut above source head and derive local clocks independently; no global ancestry is inferred.

The History read path also validates a `HistoryPruneManifest` using the immutable `OwnerMaintenanceStore::get`/`manifests` evidence admitted by the accepted `history → maintenance-store` edge, together with owner-local retention proof APIs. Decoded manifest evidence is never authorization by itself. Read-only replay comparison of Archive and Memory retention proofs belongs to History (`archive_retention.rs` and `memory_retention.rs`); these checks remain available to lower lifecycle reconstruction and do not call Maintenance. Maintenance separately authorizes a requested mutation. This read is nonmutating and cannot ask Maintenance to authorize it. History may expose historical Freshness preparation/inspection over the explicit cut, but cannot publish events, reinforce use, append settlement, repair, prune, rewrite, or reconcile.

Historical import base is a separate Freshness read operation over a concrete cut and `MemoryStore`, not an Import API:

```rust
pub(crate) struct HistoricalFreshnessBaseCut {
    pub owner_uuid: [u8; 16],
    pub prefix_end: u64,
    pub activity_cut: u64,
    pub global_cut: u64,
}

pub(crate) fn historical_freshness_base(
    source: &HistorySource<'_>,
    memories: &MemoryStore,
    cut: HistoricalFreshnessBaseCut,
    external_birth_facts: BTreeMap<u64, Result<ExternalBirthFacts, MemoryError>>,
) -> Result<BTreeMap<MemoryId, FreshnessRecord>, HistoricalFreshnessBaseError>;
```

This is read-only Freshness reconstruction: A bounded cursor is supplied by owner-lifecycle with frames limited through `prefix_end`; Freshness consumes them into an ephemeral, owner-bound replay store and uses Memory's `first_mutation_id`/`first_global_version` to validate pre-cut births before returning records at `activity_cut`, bounded by `global_cut`. It receives no Container or Import types and has no settlement writer authority on this call. Current code is `Cva::historical_import_base(&ImportEpochScope)` in `facade/historical_import_base.rs`; its behavior is the source to decompose. The exact cursor item type is the `HistoryFrame` DTO above. The outward Cva adapter preserves typed source and Memory errors and maps replay failures to its current Freshness error. The upper external-source adapter decodes immutable birth facts; History verifies their exact grouped-frame coordinates and consumes them in physical replay order.

## P3.1.02 — Maintenance: explicit mutation operations

Maintenance owns service-level policy for explicit prune, reclaim, repair, rewrite, and generic divergent-history reconciliation. No generic `MaintenancePlan<T>`, `OwnerSnapshot`, `AuthorizedOwner`, or catch-all operation struct is introduced. Each operation keeps its existing concrete request, plan, source owner and result types while its implementation moves behind the Maintenance responsibility. Domain owners remain the authority for tombstones, retention eligibility, payload identity and owner-local publication validation. Container remains the physical frame, file identity, lease, scan, and explicit repair authority.

The existing outward entrypoints remain `Cva/Phylactery::prune_tombstoned_history(HistoryPruneRequest) -> HistoryPruneReport`, `Cva/Phylactery::reclaim_storage(&Path) -> StorageReclamationReport`, explicit `repair_incomplete_tail` and `repair_retired_entity_references`, `RewriteRequest`/`RewritePlan`/`RewriteReport` in `rewrite/model.rs`, and the `Cva::compare`/`Cva::reconcile` source/output flow in `facade/cva_reconcile.rs`. They stay compatibility adapters. Operation ownership and exact internal argument lists are frozen below.

Lifecycle open/reopen is lower owner-lifecycle responsibility. Move the existing concrete REL/PHY aggregate construction currently represented by `Cva::open_from_container` and `Phylactery::open_from_container` into that component with the same constituent fields and replay order. Maintenance receives borrowed constituent stores or operation-specific borrows from that lifecycle result; it never accepts `&Cva`/`&Phylactery`. This rehomes existing lifecycle composition; it does not add an all-purpose transaction manager.

The destructive operation signatures are operation-specific:

```rust
pub(crate) struct ReliquaryPruneParts<'a> {
    pub container: &'a mut Container,
    pub archive: &'a mut Archive,
    pub memories: &'a mut MemoryStore,
    pub graph: &'a GraphStore,
    pub entities: &'a EntityStore,
    pub entity_resolutions: &'a EntityResolutionStore,
    pub relationships: &'a RelationshipStore,
    pub packed_vectors: &'a PackedVectorStore,
    pub memory_vectors: &'a MemoryVectorStore,
    pub compatibility_profiles: &'a CompatibilityProfileStore,
    pub vector_generations: &'a VectorGenerationStore,
    pub freshness: &'a FreshnessStore,
    pub import_epochs: &'a ImportEpochStore,
    pub owner_maintenance: &'a OwnerMaintenanceStore,
}
pub(crate) fn prune_reliquary(
    parts: ReliquaryPruneParts<'_>, output: &Path, request: HistoryPruneRequest,
) -> Result<HistoryPruneReport, OwnerError>;

pub(crate) struct PhylacteryPruneParts<'a> {
    pub container: &'a mut Container,
    pub memories: &'a mut MemoryStore,
    pub graph: &'a GraphStore,
    pub entities: &'a EntityStore,
    pub relationships: &'a RelationshipStore,
    pub packed_vectors: &'a PackedVectorStore,
    pub memory_vectors: &'a MemoryVectorStore,
    pub compatibility_profiles: &'a CompatibilityProfileStore,
    pub profile: &'a PhylacteryProfileStore,
    pub owner_maintenance: &'a OwnerMaintenanceStore,
}
pub(crate) fn prune_phylactery(
    parts: PhylacteryPruneParts<'_>, output: &Path, request: HistoryPruneRequest,
) -> Result<HistoryPruneReport, OwnerError>;
```

`ReliquaryPruneParts` and `PhylacteryPruneParts` are justified by the concrete fields read directly or by the retention helpers in `cva_history_prune.rs` and `phylactery_history_prune.rs`; they are not interchangeable generic owner state. Their field lists track those implementations exactly, and no field may be added for convenience. The `PhylacteryProfileStore` field is the existing `phylactery_profile_store::PhylacteryProfileStore` (not a `PhylacteryProfileStore` invented in a new owner). Each operation validates the destination through owner-lifecycle using the corresponding existing REL/PHY open-from-container replay construction; it does not call a facade open method.

Reclamation separates physical liveness from semantic proof. The owner-lifecycle/Container operation owns the physical `Container` scan, identity, lease, candidate emission and reopen; Maintenance owns the policy decision. REL reclamation consumes an explicit History-derived `ReliquaryReclamationFacts` value containing the settlement sequence, epoch statuses, accepted policy, activity/global/archive/graph/memory-graph cuts, `records_at` result and historical evidence digest, plus the existing Archive and retention inputs required by the scan. PHY consumes the corresponding owner UUID and current clock tuple with its retention inputs. These are operation-specific immutable facts, not an ambient owner snapshot and not a substitute for reopening the emitted candidate. The entrypoints are `reclaim_reliquary(source: &mut ReliquaryReclamationSource, facts: &ReliquaryReclamationFacts, output: &Path) -> Result<StorageReclamationReport, OwnerError>` and `reclaim_phylactery(source: &mut PhylacteryReclamationSource, facts: &PhylacteryReclamationFacts, output: &Path) -> Result<StorageReclamationReport, OwnerError>`. The source handles are private owner-lifecycle capabilities over the existing physical owner and do not expose `Container` to Maintenance. Exact leaf fields of the retention inputs remain those of the existing scan; no broader service/session abstraction is added. The existing `ReclamationScan` remains a disposable in-memory liveness plan, never persisted authority.

Explicit path repair remains a Container/owner-lifecycle operation with existing outward signature `repair_incomplete_tail(path) -> TailRepairOutcome`; public adapters map its Container and owner replay errors to CvaError/PhylacteryError. The Container opens exclusive repair authority, stages `copy_accepted_prefix`, owner-lifecycle opens and validates the staged REL/PHY state, and Container truncates only after that validation. Mounted `repair_retired_entity_references(&mut self) -> usize` becomes two narrow direct-reference operations: `repair_reliquary_retired_entity_references(container: &mut Container, entities: &EntityStore, resolutions: &mut EntityResolutionStore) -> Result<usize, OwnerError>` and the corresponding PHY form also returning `OwnerError`. No automatic repair is introduced.

Rewrite already has file-oriented concrete APIs: `rewrite::plan::plan_rewrite_file(source, &RewriteRequest) -> Result<RewritePlan, RewriteError>`, `rewrite::file::rewrite_from_plan(source, output, &RewritePlan) -> Result<RewriteReport, RewriteError>`, and `rewrite_file(source, output, &RewriteRequest)`. These are the Maintenance operation boundary; the methods `Cva::plan_rewrite`/`Phylactery::plan_rewrite` become internal owner-lifecycle planning over direct owner reads and retain existing `RewritePlan` fields. The plan binds owner UUID, generation, semantic head, source file identity, request and closure decisions. No generic plan wrapper is added.

Generic reconciliation signatures are `maintenance::compare_reliquaries(left: &Path, right: &Path) -> Result<CvaComparison, OwnerError>` and `maintenance::reconcile_diverged_reliquaries(left: &Path, right: &Path, output: &Path, comparison: CvaComparison) -> Result<(CvaReconcileResult, owner_lifecycle::ReliquaryOwner), OwnerError>`. `CvaComparison` and `CvaReconcileResult` are existing result DTOs. `owner_lifecycle::ReliquaryOwner` is a proposed private owner-lifecycle aggregate name for the validated REL state returned by the existing crate-private held-output path; it is not an existing source symbol and must contain owner components, not a facade `Cva`. Import-specific replay remains a separate Import path. The public adapter, at the public-consumers rank, performs the existing Import-first epoch dispatch and falls back to Maintenance generic reconciliation when Import reports no epoch-specific case; neither service imports the other. Comparison remains read-only and output collision/source lease checks remain mandatory.

Maintenance manifest evidence remains the existing `OwnerMaintenanceStore`, `HistoryPruneManifest`, `HistoryRetentionDescriptor`, `RetentionTarget`, `PrunedPayload`, and owner-produced retention proofs. The manifest store is immutable to History readers; only the owning maintenance mutation path may append manifest evidence. A manifest is evidence, never authorization by itself. Before deleting payload, Maintenance matches each selected target and payload to the owner’s accepted terminal/tombstone proof, retained structure, surviving references and exact source history digest.

Generic reconciliation owns common-prefix comparison, source-head consistency, destination copy/validation/publication and generic semantic conflict reporting. It does not infer an Import epoch policy. It may not call Import. Epoch-specific continuation remains exclusively with Import; public-consumer dispatch is described with the operation signatures above.

## P3.1.03 — Import: epoch and scoped settlement coordinator

Import owns epoch start identity, open-epoch admission fencing, import readiness, historical Freshness settlement admission, completion binding/synchronization/recovery, and Import-specific divergent continuation. `ImportEpochStore` remains durable owner-local evidence: `ImportStart`, `ImportComplete`, `ImportWatermarks`, `ImportEpochStatus` and `ImportEpochStore::{statuses, append_start, append_complete_coordinated, validate_owner}`. The store does not import History, Maintenance, Runtime or facade policy.

The coordinator has only its accepted direct DAG dependencies: `import-store`, `archive-store`, `insomnia`, `dream`, `perception`, `memory-store`, `freshness`, `history` and `owner-lifecycle`. It has no direct Container, GraphStore, EntityStore, FreshnessStore, DreamStore or Maintenance edge. Physical write admission is represented by the narrow owner-lifecycle `ImportSession`, not by exposing constituent raw stores. `ImportSession` is an exclusive offline REL import capability (the supported import flow is not a mounted-owner operation). It owns the exclusive writer admission fence and accepted epoch/start/receipt facts needed by Import; it exposes bounded owner facts through existing owner operations, and its only physical publication operation accepts the opaque completion value described below. It does not expose `Container`, return raw stores, accept callbacks, or act as a general transaction/session manager. PHY import support is not added.

```rust
pub(crate) fn import_native(
    session: &mut ImportSession<'_>, input: &NativeImport, now_ns: i64,
) -> Result<ImportReceipt, ImportError>;

pub(crate) fn native_import_readiness(
    session: &ImportSession<'_>, import_id: &str,
    insomnia: &Insomnia, dream: &Dream, perception: &Perception,
    freshness: &Freshness, memories: &MemoryStore,
) -> Result<ImportReadinessReport, OwnerError>;

pub(crate) fn prepare_historical_freshness_settlement(
    session: &ImportSession<'_>, history: &History, freshness: &Freshness,
    options: HistoricalReconstructionOptions,
) -> Result<PreparedHistoricalImport, OwnerError>;

pub(crate) fn commit_historical_freshness_settlement(
    session: &mut ImportSession<'_>, freshness: &Freshness,
    prepared: &PreparedHistoricalImport,
) -> Result<bool, OwnerError>;

pub(crate) fn complete_native_import(
    session: &mut ImportSession<'_>, import_id: &str, now_ns: i64,
    insomnia: &Insomnia, dream: &Dream, perception: &Perception,
    freshness: &Freshness, memories: &MemoryStore,
) -> Result<ImportCompletionOutcome, OwnerError>;

pub(crate) fn reconcile_import_epochs(
    left: &Path, right: &Path, output: &Path, comparison: CvaComparison,
) -> Result<Option<(CvaReconcileResult, owner_lifecycle::ReliquaryOwner)>, OwnerError>;
```

The signatures are target module-operation notation. `History`, `Freshness`, `Dream`, `Perception` and `Insomnia` above denote allowed owner component operations, not newly invented empty service wrapper structs; their inputs and outputs are the existing DTOs and the narrowly specified cut/fact values. The notation does not assert compiler-verified symbols or completed extraction. `ImportSession` is the sole owner-lifecycle capability and is limited to the offline REL operation: it reads accepted owner/epoch scope, admits the one exclusive import writer, and accepts only a coordinator-created completion publication for physical append/sync. It does not provide arbitrary read/write access or callback execution.

`OwnerError` is the owner-lifecycle public alias of the existing `CvaError` during this convergence; this is error ownership/re-export only, not a new error taxonomy. The public facade adapters retain their current error mapping. `ImportError` remains the native import validation error, and the historical reconstruction public adapter retains `HistoricalReconstructionError`.

The narrow completion handoff is an opaque private `ImportCompletionPublication`, constructible only inside the nested Import coordinator after it has re-read and validated durable start, receipt, readiness and settlement state. `ImportSession::append_completion(publication)` is the owner-lifecycle operation that takes the scoped physical writer authority, appends and syncs the publication, then acknowledges the staged ImportEpochStore state and reopens the fence only after accepted durability is known. The value carries the exact completion marker and bound post-watermarks; callers cannot construct it, and the Import store cannot depend on owner-lifecycle. Uncertain append/sync leaves the fence closed; retries rebuild from durable bytes and are idempotent. This preserves the existing private completion authority without restoring a facade completion token.

Historical settlement preparation consumes the existing `History` read result, the Freshness owner service, and `ImportSession` accepted epoch scope; it does not reach through Cva fields or pass raw store references. Commit is a distinct Import-authorized call through the existing Freshness settlement operation, with `ImportSession` maintaining the admission fence. The historical manifest/event proof continues to request facts from allowed owners through their services and History; no broad constituent-parts bundle is frozen. The bounded replay function is Import-specific: owner-lifecycle opens each side, History supplies historical cuts/proofs, and Import applies epoch continuation policy without calling Maintenance. Reconciliation callers keep their current path/comparison semantics.

Readiness must preserve the present exact receipt identity (`Archive::import_receipts`), source namespace/format, ImportStart/Complete digest binding, scoped work discovery, and pending outcomes. It cannot assert readiness from caller-provided booleans or digests. Settlement preparation reads accepted historical evidence without reinforcement; the explicit commit is a separately authorized Import operation using the narrow Freshness settlement write contract. Completion rereads durable markers on every attempt, retains a closed write fence through uncertain append/sync outcomes, validates owner/start/receipt/settlement/post-watermark binding, syncs before reopening admission, and returns accepted idempotent completion on retry.

The P3.1.03 table in the spec has a lower-level phrase that could be read to allow Import→Maintenance. That phrase is superseded by the same spec’s overarching invariant and accepted DAG: **there is no Import→Maintenance edge**. Import-specific divergence uses History plus owner-lifecycle and lower owner operations. Generic reconciliation code is not shared via a new coordinator; genuinely owner-local copy primitives may be shared only if they already belong below both services and the DAG explicitly permits the edge.

## P3.1.04 — Operation shapes, authority and error boundaries

All three services use the same bounded operation progression without introducing a common transaction manager:

1. **Inspect:** use the existing operation-specific source/read contract and resolve the requested owner and cut. Error distinctions must preserve the current public refusal behavior and underlying Container/owner error evidence.
2. **Plan:** for rewrite/reconcile/prune, retain the existing concrete `RewritePlan`, reconciliation state and `HistoryPruneRequest`/proof values. Other operations do not gain a fabricated shared plan type. Planning has no persistent side effects.
3. **Authorize:** validate the explicit request, supported domain, owner scope, source identity, destination collision and owner proof immediately before destructive publication.
4. **Emit:** use existing owner publishers and physical Container writer APIs. Never overwrite a pre-existing destination. A failed operation leaves source bytes untouched; cleanup of a new candidate is allowed only if its identity still proves it is the candidate created by this attempt.
5. **Verify:** reopen the emitted candidate under the existing read-source contract and compare operation-specific owner, generation/lineage, semantic state, historical proof and receipt evidence. A missing or corrupt required proof fails closed.
6. **Return:** validate source and destination identities, complete required sync/publication sequencing, then return the existing report/result. Exact internal error types remain operation-specific and must be frozen from the current source, not replaced by one generic Maintenance or Import error.

Error mapping remains at existing public Cva/Phylactery/API/CLI adapters. Internal owner operations use `owner_lifecycle::OwnerError`, the public alias of existing `CvaError` for this phase; this does not claim new variants or semantics. History source digest retains its current `String` helper surface; rewrite retains `RewriteError`; native import validation retains `ImportError`; public historical reconstruction retains `HistoricalReconstructionError`. Public adapters preserve their existing specific error mapping. I/O, stale identity, collision, malformed evidence, unsupported retention domain, owner/generation mismatch and readiness pending retain current distinctions. No shared error taxonomy or conversion of integrity failure to “not found” is introduced.

## P3.1.05 — File, lease, snapshot, collision and recovery contract

The source identity and exclusion authority remain Phase 1 `Container` mechanisms, including `Container::validate_path_identity`, `Container::read_source`, `PhysicalReadSource::validate_path_identity`, writer lease exclusion, scan limits, `Container::sync`, and explicit tail repair. No service duplicates OS file identity logic or weakens the held-source protocol.

- A read/proof operation holds an immutable accepted owner borrow and its Container lease, or the existing `PhysicalReadSource`, for the full scan and proof-use interval. A writer lease held by a mounted mutable owner may coexist only as allowed by current Container rules; an incompatible reopen/write is refused. Snapshot evidence is immutable for that held source.
- A transformed-copy operation first syncs the source as currently required, holds/validates the read source, and captures the source file identity and accepted owner/generation/head proof. Before emit, before success, and before any cleanup it revalidates source identity. Replacement, truncation, unexpected append/head movement, or stale plan fails closed.
- Destination publication uses the existing atomic no-overwrite hard-link contract in the destination directory. Existing paths are collisions and are never truncated, replaced, or deleted; unsupported filesystems fail closed. The candidate is verified from a separate read snapshot before success. On failed verification, cleanup is allowed only for the exact newly created candidate whose identity is still held and validated; if identity cannot be proven, leave it untouched and report the failure. Unix directory synchronization follows successful publication.
- A plan is invalid after source identity, owner UUID, generation, semantic head, relevant watermarks, or exact normalized proof changes. Replanning is explicit; no automatic stale-plan rebasing.
- In-place physical tail repair remains an explicitly requested Container repair under exclusive writer authority. It is not an ordinary open/recovery side effect. Corrupt complete history and malformed retained metadata remain errors; only the existing incomplete-tail rule may be repaired.
- A partial append or failed sync cannot reopen Import write admission. Retry reconstructs accepted epoch/settlement/completion state from bytes and is idempotent. An uncertain append is never duplicated merely because the earlier caller did not receive acknowledgement.

These clauses preserve observed behavior in `rewrite/file.rs`, `cva_reconcile.rs`, `storage_reclamation.rs`, `imports/completion.rs`, `imports/epoch_store.rs`, and Container lease/repair code. No cross-volume publication guarantee or unimplemented failure-injection point is implied.

## P3.1.06 — Public adapter freeze

Existing public entrypoints remain source-compatible through Phase 6, including Cva/Phylactery history prune, repair, rewrite, reclaim, reconciliation, historical reconstruction and explicit import completion methods. Each wrapper validates only outward argument syntax that is already part of its contract, delegates once to the canonical operation owner, and maps the owner error into the existing public error type. It holds no second policy implementation and does not call between high-level services.

`Cva` and `Phylactery` remain public facades and owner-lifecycle/mount inputs; they are not passed as ambient callbacks to History/Maintenance/Import. REL and PHY scope remains explicit. A PHY operation cannot traverse Archive, Import, or REL content merely because a unified facade type exposes those fields. CLI behavior follows the same public adapter and reports the same refusal/result semantics.

## P3.1.07 — Settlement design gate prerequisites; no selection

This contract selects **no** replacement for `CVAFRS06`. Current settlement structures and bytes remain the sole development representation until a separately approved P3.8/S3. The P3.1 design freeze defines what evidence must precede selection:

1. Freeze the exact six canonical source files, hashes, original V2 clock/proof oracle, current source checkpoint, and per-file/current `CVAFRS06` bytes.
2. Produce a field-by-field byte and authority table for every accepted event, recipient/born/cohort set, source/node identity, superseded ID, digest, complete `FreshnessRecord` postimage, and other persisted field. Classify each as irreducible authority, compact commitment, or deterministically reconstructible duplication, with exact authoritative owner and cut.
3. Compare at least two materially different candidate proof shapes. For each, record authenticity/domain binding, exact ImportStart/Complete binding, corruption and duplicate rejection, superseded-event and same-turn attribution cases, historical reconstruction, read locality and bounded memory/time, pruning/relocation/repack survival, bytes per fixture, whole-file effect, and failure modes.
4. Demonstrate every omitted field is recoverable from accepted retained owner facts through prune/reclaim/rewrite/repack. If any member is no longer available, a digest-only candidate fails. Do not restore physical-prefix/offset authority.
5. Approve one minimum correct shape, canonical field ordering, domain-separated commitment, limits, codec/version cut, temporary offline converter, six-file promotion procedure, old-path deletion point and semantic-oracle receipts before codec or corpus edits.

The existing plan reports 7,415,741 bytes versus 395,533 bytes across the six fixtures (18.75×, +7,020,208 bytes) as prior measured evidence. Those aggregate numbers do not satisfy the field inventory, alternatives comparison, or reconstruction proof. No target percentage is imposed, and ADR-0004 compression remains separate. No P3.1 gate may claim this design work is complete without the per-field and alternatives artifacts.

## P3.1.08 — Hard cut and exact move/delete checkpoints

The phase uses bounded red windows only while a single cohesive set of callers is migrated. At each checkpoint, there is one production owner; no indefinitely retained internal forwarding layer, parallel policy path, duplicate history engine, or dual settlement codec remains.

| Checkpoint | Move into canonical responsibility | Delete once replacement proof is accepted |
| --- | --- | --- |
| **P3.2.01–.08 / H3** | Read-only cut enumeration/frontier, historical reconstruction/proof composition, normalized source digest, historical Freshness preparation/inspection → History. | Production duplication in `facade/historical_global_cuts.rs`, `historical_global_frontier.rs`, `historical_cut_enumeration.rs`, `historical_reconstruction.rs`, `historical_*_fingerprint.rs`, `historical_whole_*_fingerprint.rs`, and `owner_history_proof.rs` only after each caller is migrated. Test-only fingerprint/oracle modules stay test-only and are not promoted to History. Split publication methods from `cva_historical_freshness.rs` / `cva_historical_import.rs`; no History writes. |
| **P3.3–.4 / M3 and P3** | Explicit prune, reclaim, semantic/physical repair and retention proof composition → Maintenance; owner-local retention descriptors and `OwnerMaintenanceStore` remain owner evidence. | Duplicated production policy in `facade/cva_history_prune.rs`, `phylactery_history_prune.rs`, `storage_reclamation.rs`, `semantic_maintenance.rs`, `archive_prune_admission.rs`, `memory_prune_admission.rs`, and `owner_history_proof.rs` as the relevant operation becomes canonical. Keep public outward wrapper files only as thin compatibility adapters. Preserve Container repair implementation. |
| **P3.5 / W3** | Existing `rewrite/{model,plan,semantic_state,lineage,file}.rs` coordination → Maintenance responsibility; preserve existing request/policy/identity semantics. | Any facade-side duplicate planner/emitter and internal bridge after parity; retain public rewrite callers as outward delegates. No new product feature or second VCS. |
| **P3.6 / R3** | Common-prefix comparison, generic conflict classification, copy/verify/promotion → Maintenance; epoch continuation decisions → Import. | Generic duplicated policy in `facade/cva_reconcile*.rs` and generic portions of `imports/reconciliation*.rs` only after exact source-only replay/refusal tests pass. Keep Import-specific epoch replay in Import. Do not create Maintenance→Import or Import→Maintenance. |
| **P3.7 / I3** | ImportStart/Complete/readiness/settlement/completion/fence/recovery and epoch-specific divergence → Import coordinator over existing Import store and lower evidence. | Cva-local import policy in `facade/import_epoch_admission.rs`, `historical_import_base.rs`, `historical_import_input.rs`, `historical_import_transform.rs`, `cva_historical_import.rs`, and `imports/{readiness,completion,reconciliation}.rs` split by ownership; delete duplicate Cva decision logic only after exact completion/readiness/retry behavior passes. |
| **P3.8–.9 / S3** | Selected compact semantic proof remains under Freshness settlement ownership; Import binds and commits the accepted epoch proof. | Only after six-file parity and approved promotion: delete superseded settlement decoder/codec and temporary offline converter immediately. Do not retain runtime compatibility reader or old-format authoring path. |

The source lists above are routing targets, not assertions that every named file contains production code; B0 must classify production versus `cfg(test)` and identify exact symbols/callers. Never delete a path merely because it appears in this table. Tests move with their behavior to the canonical owner boundary, or remain explicit test-only fixture/oracle files.

## Exact DAG slice and forbidden edges

Consumer-to-dependency direction follows the accepted 34-component DAG. These rows are the complete direct dependencies for the three new/target services; transitive reachability grants no additional direct permission.

| Component | Rank | Exact permitted direct dependencies | Consequence for this contract |
| --- | ---: | --- | --- |
| `history` | 4 | `owner-lifecycle`, `archive-store`, `memory-store`, `graph-store`, `perception-store`, `ego-store`, `freshness-store`, `vector-store`, `insomnia-store`, `dream-store`, `import-store`, `external-index-store`, `maintenance-store` | Read-only composition can inspect those owner facts; no `maintenance`, `import`, facade, Runtime, or Container direct edge is listed. Physical reads are mediated through accepted owner-lifecycle/source handles. |
| `maintenance` | 9 | `history`, `owner-lifecycle`, `container`, `archive-store`, `memory-store`, `graph-store`, `perception-store`, `ego-store`, `freshness-store`, `vector-store`, `insomnia-store`, `dream-store`, `import-store`, `external-index-store`, `maintenance-store` | Explicit mutation/rewrite/reconcile may use History and listed owners. No `import` service dependency. |
| `import` | 10 | `import-store`, `archive-store`, `insomnia`, `dream`, `perception`, `memory-store`, `freshness`, `history`, `owner-lifecycle` | Epoch policy may use History and listed owner services/evidence. **No `maintenance` edge.** |

`maintenance-store` is the low-level evidence owner, not the `maintenance` service. Therefore History’s DAG edge to `maintenance-store` is read-only access to manifest evidence; it does not authorize History→Maintenance service calls or mutation. `import-store` means Import epoch persistence, not Import coordinator policy. Import may use lower owner-lifecycle physical primitives; it does not invoke generic Maintenance reconciliation. The phrase in spec P3.1.03 about “lower ... Maintenance primitives” is treated as a typo and does not override this DAG or the spec’s explicit no-Import→Maintenance prohibition.

All three services may be called by `runtime-gateway` and `public-consumers` as allowed by those consumer rows. No service imports Cva/public facade, CLI, Runtime, host adapters, or a callback supplied by an ancestor. No owner store imports these services. The DAG remains closed and default-deny; any desired edge requires a separately reviewed DAG and machine allowlist change before source depends on it.

## Gate ledger

- **B0:** source/corpus before-image and executable receipts are recorded in the [Phase 3 baseline](dependency-convergence-phase3-baseline-2026-10-22.md). That record reports the accepted Phase 2 source receipt and exact six-file hashes; it does not claim new Rust-suite or exhaustive-oracle execution in this contract turn.
- **B1:** interface contract is frozen here for review. The History source capability and explicit cut, operation-specific Maintenance and Import argument borrows, error mapping, public adapter routes, file semantics, exact DAG, and hard-cut checkpoints are specified. Root review accepts B1 as an interface design checkpoint against the exact DAG. This does not certify implementation or authorize adding unlisted edges.
- **S3:** settlement field measurements, two candidate shapes, proof comparison, and design selection are later S3 work after I3. They are expressly not performed or selected in B1, avoiding the P3.7→P3.8 dependency cycle.
- **H3/M3/P3/W3/R3/I3/S3:** future implementation gates; no pass is implied.

If implementation encounters evidence that an existing symbol or invariant differs from this contract, stop that operation and amend its concrete signature through review before source migration. Do not silently invent an adapter, widen the DAG, weaken evidence, or mark an implementation gate passed on documentation alone.

## Related docs

- [Phase 3 specification](dependency-convergence-phase3-history-maintenance-import-spec-2026-10-22.md)
- [Local execution](dependency-convergence-phase3-execution-2026-10-22.md)
- [Frozen baseline](dependency-convergence-phase3-baseline-2026-10-22.md)

## Notes

This interface design does not certify implementation gates or select the settlement format. Later measured design, source and corpus acceptance remain mandatory.


## Read-checkpoint refinement during M3 implementation

Review of retained-vector and scoped-corpus errors showed that eagerly constructing a fallible `HistorySource` before a logical preflight changes refusal precedence, and can introduce physical I/O to an empty proof. The concrete `owner_lifecycle::HistoryReadAccess` therefore holds only an immutable owner borrow and is constructed without I/O. It exposes bounded `snapshot()` capture and owner-local immutable record reads, with no Container accessor, writer, callback, or authorizing boolean. History captures `HistorySource` at the original physical-read checkpoint, after the applicable owner checks; a captured snapshot retains the existing identity and extent validation. This refines source acquisition rather than widening the component DAG. The accepted `HistorySource` remains the bounded scan capability. Root review accepts this concrete refinement against the unchanged DAG; error-order and empty-proof regressions remain required evidence.


## Ordered REL reconstruction refinement

The existing REL open sequence interleaves owner-local finishing with Archive, Memory and Graph History proofs, then derived Freshness due-index reconstruction, vector finishing, and final settlement/completion proofs. A monolithic lower lifecycle function cannot invoke History under the accepted DAG. The concrete `ReliquaryReplay` therefore keeps its physical Container private and supplies operation-specific finishing stages plus immutable read access. History composes those stages and its own proofs in the existing order; no stage invokes History or an ancestor service. The resulting concrete `ReliquaryOwner` and derived due index are returned to the public facade or an allowed Maintenance/Import consumer. This is the existing REL reconstruction sequence with explicit lower stages, not a generic transaction or owner framework.

The due index in `freshness_due.rs` is exclusively a rebuildable owner-local projection over Freshness records, policy and the boundary index already classified as Freshness-store. It performs no publication, notifications, gateway work or filesystem access. Classifying this exact module as Freshness-store permits its fallible rebuild at the original reconstruction checkpoint; notification delivery remains above the store. Deferring the rebuild until after other owner checks is rejected because it changes error order. Root review accepts these concrete ownership refinements against the unchanged DAG; central compilation, refusal-order review and reopen/maintenance tests remain required before acceptance.
