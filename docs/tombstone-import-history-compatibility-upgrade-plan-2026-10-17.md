# Tombstoning, import epochs, historical pruning, and compatibility-profile upgrade — implementation plan

Parent: [Documentation index](INDEX.md).

**Date:** 2026-10-17  
**Status:** active execution; convergence steps 1–12 are complete on `main`. The authoritative continuation is the [remaining execution plan](tombstone-import-history-remaining-execution-plan-2026-10-17.md).
**Scope:** generalized REL/PHY tombstoning, Archive conversation deletion, import epoch markers, repeated-import Historical Freshness reconstruction, explicit destructive historical pruning/repack, compatibility-policy v3/profile retirement, and retained-REL normalization.  
**Migration policy:** staged hard cut internally, with compatibility isolated only at persisted REL/PHY format boundaries that must continue to open.

## Purpose

Implement the lifecycle, repeated-import history and compatibility upgrades defined below.

## Overview

The original 50-step decomposition below remains the architectural work breakdown, but its execution order has been superseded by the convergence sequence actually being used. Steps 1–26 of the convergence/integration sequence are accepted on the isolated integration branch; **step 27 is next**. Use the [remaining execution plan](tombstone-import-history-remaining-execution-plan-2026-10-17.md) for authoritative step numbering and the [execution contract](tombstone-import-history-execution-contract.md) for frozen semantics and verified progress.

## Related docs

- [ADR 0040](decisions/0040-terminal-tombstones-and-import-epochs.md)
- [Architecture](architecture.md)
- [Storage format](storage-format.md)

## Notes

No step is complete without implementation evidence and its acceptance gates. Convergence steps 1–12 are recorded as complete in the [domain convergence inventory](domain-tombstone-convergence-split-2026-10-17.md). Authoritative fixtures remain untouched before final promotion.

## 1. Problem statement

Reliquary currently has several related lifecycle/history gaps that now intersect:

1. Tombstoning exists in isolated subsystems such as Entity/Ego machinery, but there is no common REL/PHY deletion contract and Archive has no native conversation deletion.
2. Native imports are already repeatable, but Historical Freshness reconstruction is still a one-shot whole-REL operation owned by a singleton Freshness `ImportCheckpoint`.
3. Imported construction history is physically durable but indistinguishable from ordinary operational mutation history. An import can create hundreds or thousands of Archive, Memory, vector, Dream, Perception and Ego mutations that should be interpretable as one import-mediated state transition.
4. `reclaim_storage()` safely removes only proven physical garbage. It intentionally cannot destroy still-addressable tombstoned historical payload because Reliquary has no explicit destructive-history authorization.
5. Compatibility profiles are append-only and cannot be retired. Failed or superseded embedding attempts can leave redundant profile/vector state. Current compatibility policy v2 (`0.9998`) is stricter than retrieval-quality evidence justifies.
6. The retained RELs need one canonical active complete embedding space each without re-embedding or rewriting their historical profile identities.

These are one architectural family: **owner-local lifecycle, import-history boundaries, and controlled destruction of payload without destruction of history**.

## 2. Frozen architectural decisions

These decisions are final for this workstream.

### 2.1 Tombstones are terminal

A tombstoned historical object is never “untombstoned.” If equivalent content becomes live again, Reliquary creates or admits a new live revision/object according to that domain's ordinary identity rules. The old tombstone remains part of history.

### 2.2 Tombstoning is direct-only by default

Tombstoning an object affects that object's **direct ownership closure** only.

It does **not** automatically tombstone independently identified downstream/derived state merely because that state has provenance pointing at the target.

Examples:

- tombstoning a conversation retires that conversation's Archive nodes, branch heads/revisions, conversation metadata and conversation-owned attachment/link exposure;
- Memories derived from that conversation survive;
- Entities, Relationships, graph state, Ego summaries/anchors and other independently identified derived state survive;
- their provenance may continue to point to the tombstoned historical source.

Downstream cascade is optional and explicit. It must use ordinary tombstones against the selected downstream targets; it is never implicit delete propagation.

### 2.3 Current projection and historical availability are separate concepts

Lifecycle:

```text
ACTIVE
  -> TOMBSTONED
```

Payload availability for historical reads:

```text
AVAILABLE
  -> PRUNED
```

`PRUNED` is not a resurrection/deletion lifecycle state. It means a tombstoned historical object's recoverable payload was explicitly destroyed while the minimum reconstructive history was retained.

### 2.4 Normal operation requires no import markers

A REL that has existed only through normal Reliquary operation may contain no import markers at all.

There is no `InitialBaseline`.

Every actual import, including an import into an otherwise empty/new REL, uses the same pair:

```text
ImportStart(import_id)
[import construction and derived processing]
ImportComplete(import_id)
```

Both markers may be absent when no import has occurred.

### 2.5 Every import is an epoch

The mutation work caused by one import is operationally interpreted as one state transition from the state captured by `ImportStart` to the settled state captured by `ImportComplete`, regardless of how many raw mutations were needed internally.

Raw storage/history still retains the individual mutations.

An unmatched `ImportStart` means an incomplete/interrupted import.

The first implementation uses an owner mutation-admission fence for an open import epoch so unrelated live writes cannot be accidentally quarantined inside the epoch. Reads may remain available. General interleaving of live writes and an open import epoch is out of scope for this version.

### 2.6 Import identity belongs to the owner/import subsystem, not Freshness

`ImportStart` / `ImportComplete` are owner-level import metadata.

Freshness consumes an import epoch and publishes its import-specific settlement before `ImportComplete`; Freshness does not own import identity or import lifecycle.

The current singleton `CVAFRS03 ImportCheckpoint` is therefore not the target architecture.

### 2.7 Historical Freshness reconstruction applies to every import

The Historical Freshness reconstruction machinery must work for:

- an initial import into an otherwise empty REL;
- one later conversation import;
- a large later archive import;
- repeated imports from the same or different source namespaces;
- imports into a long-lived REL with pre-existing live Freshness history.

A later import starts from the canonical Freshness state at `ImportStart`; it does not replace or reinterpret the entire pre-existing REL history.

### 2.8 Historical pruning never renumbers semantic history

Destructive historical pruning must preserve:

- exact global version numbers;
- exact domain-local versions;
- transaction timestamps;
- Archive activity identity/order;
- event/tombstone/import ordering;
- stable logical IDs;
- ancestry/topology required to reconstruct history;
- surviving provenance references.

It may destroy recoverable content/payload bytes for explicitly tombstoned history.

### 2.9 `reclaim_storage()` stays conservative

`reclaim_storage()` must not infer that tombstoned historical payload is garbage.

The separation is:

```text
tombstone()
    -> changes current semantic projection
    -> history remains readable

prune_tombstoned_history()
    -> explicit destructive authorization
    -> tombstoned payload becomes historically unavailable
    -> minimum reconstructive record remains

reclaim_storage()
    -> conservative physical garbage collection
    -> may now collect payload made unreachable by the prune transform
```

### 2.10 Compatibility policy v3 uses 0.99

The calibrated current-policy threshold becomes:

```text
COMPATIBILITY_POLICY_VERSION = 3
COMPATIBILITY_MIN_COSINE = 0.99
```

Evidence: `benchmarks/compatibility-threshold-calibration-2026-10-17.json`.

The lowest tested defensible threshold is 0.99:

- 0.99 synthetic degradation remained above the 95% retrieval floor;
- 0.985 fell below it;
- real old-to-normalized retained spaces measured around 0.9963–0.9964 and retained strong retrieval parity.

### 2.11 Stored profile identity is immutable

Existing retained v2 `CompatibilityProfileId` values must not be rewritten merely because runtime policy advances to v3.

New profile artifacts use policy v3. Supported retained v2 profiles remain valid historical artifacts and can be reused when a v3 endpoint verifies compatible against them under the current verification policy.

## 3. Target ownership and module boundaries

### 3.1 Owner lifecycle contract

Add a small shared lifecycle domain module, tentatively `owner_lifecycle`, that defines common semantics only:

- typed lifecycle target identity;
- terminal tombstone state;
- direct-only vs explicit-cascade request policy;
- tombstone idempotency/error rules;
- payload-availability result for historical reads;
- common lifecycle inspection types.

It must **not** become a second semantic database or allocate another universal local version clock.

Each semantic owner remains responsible for persisting its tombstone on its existing clock:

- Archive tombstones advance Archive + global versions;
- Memory tombstones advance Memory + global versions;
- Entity/Relationship tombstones remain on their existing semantic clocks;
- Compatibility-profile retirement uses the vector/profile owner rules defined below;
- PHY domains use their corresponding existing clocks.

The common module is a semantic contract and dispatch seam; the target store remains the mutation authority.

### 3.2 Direct ownership closure

Every tombstone-capable domain must define its direct closure explicitly.

The deletion test is:

> If an object is required only to expose/operate the target itself, it belongs to direct closure. If it has its own stable semantic identity and can remain meaningful independently, it is downstream/derived and does not automatically tombstone.

No generic provenance crawler decides direct closure.

### 3.3 Archive conversation tombstone

Archive gains one aggregate `ConversationTombstone` semantic mutation.

A conversation tombstone retires the conversation's current projection in one logical operation. The tombstone contains or deterministically identifies the Archive-owned closure necessary for validation/reopen/history.

The closure includes, at minimum:

- conversation ID;
- all Archive node identities belonging to the conversation;
- branch IDs/revisions owned by the conversation;
- conversation metadata revisions;
- conversation-owned file/attachment/link exposure;
- the exact Archive/global version at which the tombstone becomes authoritative.

It does not tombstone independently identified downstream state.

A conversation tombstone is preferred over emitting one user-visible deletion event per historical node.

### 3.4 Import epoch authority

Add an owner-local `ImportEpochStore` under the import/CVA ownership boundary.

It owns:

```text
ImportStart
ImportComplete
ImportEpochStatus
```

Markers are durable metadata and do not need to consume a new semantic global-version ticket merely to describe a boundary. They record exact semantic watermarks around the interval.

This permits:

- normal RELs with no markers;
- new imports to bracket their work without altering domain semantics merely by opening the epoch;
- legacy one-shot historical-import evidence to be migrated without renumbering old global versions.

Tentative durable shapes:

```text
ImportStart {
    owner_uuid,
    import_id,
    input/import_key digest,
    source_namespace,
    source_format,
    started_at_ns,
    pre_global_version,
    pre_archive_version,
    pre_memory_version,
    pre_entity_version,
    pre_relationship_version,
    pre_graph_version,
    pre_memory_graph_version,
    pre_vector_version,
}

ImportComplete {
    owner_uuid,
    import_id,
    start_digest/reference,
    receipt_digest,
    completed_at_ns,
    post_global_version,
    post_archive_version,
    post_memory_version,
    post_entity_version,
    post_relationship_version,
    post_graph_version,
    post_memory_graph_version,
    post_vector_version,
    historical_freshness_settlement_digest,
}
```

Fields that do not exist for a particular owner kind are omitted rather than synthesized.

The exact binary codec is an implementation detail, but the semantic information above is required.

### 3.5 Import epoch sequencing

`Cva::import_native` becomes epoch-aware:

1. normalize/validate input;
2. derive deterministic `import_id` from owner + existing canonical import key/digest;
3. exact retry of a completed import returns the existing receipt and does not create a new epoch;
4. exact retry of an open matching import resumes the same epoch;
5. conflicting input for an open `import_id` fails;
6. acquire owner mutation-admission fence;
7. append/sync `ImportStart` before the first import mutation;
8. perform the existing Archive/import receipt/episode/queue work;
9. drive or hand off import-derived processing under that same epoch identity;
10. run import-scoped Historical Freshness settlement;
11. verify import-specific background work settled;
12. append/sync `ImportComplete`;
13. release mutation-admission fence.

The runtime may expose an explicit `finalize_import_epoch(import_id)` coordinator rather than holding an in-process lock across provider/model work. The durable open epoch is the authority across restart.

### 3.6 Import-completion readiness

`ImportComplete` may be written only after the import-specific work is settled.

The current `RuntimeBackgroundStatus::quiescent()` predicate is a useful base but is not sufficient by itself because it is owner-global and does not prove import attribution.

The completion coordinator must prove, for the epoch:

- all import-origin Insomnia work is complete;
- required Memory publication is durable;
- required vector work is durable or has an explicit supported unavailable disposition;
- historical semantic evidence is settled;
- import-related Dream work is complete;
- import-related Perception work is complete;
- import-related Ego work required by current product invariants is complete;
- Historical Freshness settlement for this `import_id` is accepted;
- no failed/terminal import-specific work remains unresolved.

Do not require unrelated future/background work that was not caused by the import.

### 3.7 Historical Freshness settlement

Replace the singleton whole-owner checkpoint model with an import-scoped settlement.

Tentative internal shape:

```text
HistoricalImportFreshnessSettlement {
    owner_uuid,
    import_id,
    import_start_digest/reference,
    source_manifest_digest,
    input_digest,
    start_cut,
    completion_cut,
    start_graph_version,
    completion_graph_version,
    start_memory_graph_version,
    completion_memory_graph_version,
    affected_records,
    superseded_import_event_ids,
    policy_digests,
}
```

Key rules:

- base projection is the canonical Freshness state at `ImportStart`;
- accepted-use/live events before `ImportStart` remain authoritative and unchanged;
- only provisional/synthetic scoring attributable to this import may be superseded;
- the import cohort is derived from this epoch, not `self.memory_ids()`;
- import-derived historical events may affect pre-existing Memories when ordinary policy/provenance permits; those affected records are included in the settlement;
- unaffected pre-existing records are not rewritten;
- later live events after `ImportComplete` apply normally;
- retries by the same `import_id` are idempotent;
- repeated completed imports form an ordered sequence of settlements.

Prefer a sparse affected-record settlement over storing a complete copy of every Memory Freshness record for every import.

### 3.8 Legacy `CVAFRS03` compatibility boundary

New code must stop treating `FreshnessStore.import_checkpoint: Option<_>` as the internal architecture.

Persisted current-format RELs containing `CVAFRS03` are a real on-disk compatibility boundary and must continue to open.

Isolate compatibility in decode/migration only:

- decode the legacy one-shot checkpoint into one internal completed historical-import settlement;
- because the old operation only admitted the initial one-shot import case, synthesize its pre-import boundary as the owner's empty/pre-import state;
- preserve its exact stored IDs/digests/cuts;
- never emit new `CVAFRS03` records;
- a repack/migration may materialize the new `ImportStart` / `ImportComplete` records without renumbering existing semantic versions.

No caller outside the codec/migration boundary should branch on “legacy checkpoint vs new epoch.”

### 3.9 Historical pruning transform

Add an explicit maintenance operation, tentatively:

```text
prune_tombstoned_history(output_path, request)
```

It is a transformed-copy operation, never in-place mutation.

The operation:

1. syncs and validates the source owner;
2. requires no open import epoch and no in-flight semantic mutation;
3. enumerates selected tombstones;
4. asks each participating domain for its minimum reconstructive retention plan;
5. proves no selected payload is required by a surviving active/current object;
6. writes a transformed output preserving semantic/version order;
7. records non-semantic prune metadata sufficient to distinguish intentional payload absence from corruption;
8. omits or replaces only explicitly authorized tombstoned payload;
9. reopens the output;
10. proves current semantic projection, version clocks, activity history, tombstone history and import-epoch history are unchanged;
11. proves selected historical payload now reports `PayloadUnavailable::Pruned`;
12. optionally runs ordinary reclamation afterward to collect newly unreachable chunks.

### 3.10 Minimum reconstructive retention contract

Every prunable domain must explicitly state the fields required to prove historical existence, ordering and topology.

Initial contracts:

#### Archive conversation/node history

Retain as required:

- conversation ID;
- node ID;
- parent node ID;
- role, because Archive activity eligibility depends on role;
- principal ID when present;
- timestamp;
- content ID/digest proving a body existed;
- branch ID, leaf identity and canonical flag;
- metadata revision identity/version;
- Archive/global version linkage;
- attachment/file IDs and content digests needed to reconstruct ownership/reference topology.

Destroyable after tombstone/prune:

- conversation text/content bytes;
- attachment/file content bytes when no surviving unpruned reference requires them;
- other large recoverable payload not needed for topology/identity.

#### Memory history

Retain:

- Memory ID;
- revision/version coordinates;
- immutable MemoryBodyId;
- lifecycle/provenance IDs needed for historical identity;
- graph/provenance references that surviving state may still target.

Destroyable:

- Memory body text bytes once the Memory itself is tombstoned and no surviving unpruned object requires the body.

#### Entity/Relationship history

Retain stable IDs, revisions/version coordinates, replacement/endpoint/topology identity and any minimal type discriminator required to replay historical structure.

Descriptive semantic payload may be removed only when the target itself is tombstoned and the domain's historical validator no longer requires it.

#### Compatibility/vector history

For a tombstoned compatibility profile retain:

- exact `CompatibilityProfileId`;
- dimensions/normalization/probe-suite/policy version;
- a digest proving the original probe artifact;
- vector-generation IDs, profile references, source Archive versions and vector-version/global-version coordinates;
- Memory/Archive vector binding identities needed to prove that bindings existed.

Destroyable:

- probe reference vector payload when a pruned-profile representation is implemented;
- packed vector matrix bytes for retired/pruned profile generations;
- other numeric embedding payload no longer required by an active profile.

The row/binding identity may remain even when numeric vectors are unavailable.

A domain cannot participate in destructive pruning until this retention contract is encoded and tested.

### 3.11 Pruned historical reads

Current-state APIs continue to exclude tombstoned targets.

Historical APIs distinguish:

```text
Available(payload)
Tombstoned { historical_payload_still_available }
Pruned { structural_history, PayloadUnavailable::Pruned }
```

Surviving provenance references to a pruned source are valid if the retained structural record proves that source identity existed.

Missing payload without matching prune evidence remains corruption, not “pruned.”

### 3.12 Compatibility-profile retirement

Add terminal retirement/tombstone state for compatibility profiles.

Operational behavior:

- tombstoned profiles are excluded from current profile selection and compatible-profile reuse for normal retrieval;
- current vector generations under a tombstoned profile are not current operational generations;
- historical profile/generation/vector identity remains addressable until explicitly pruned;
- tombstoning one profile directly retires that profile's operational generation/binding closure;
- it does not tombstone unrelated Memories/Archive fragments that were embedded under it.

### 3.13 Compatibility policy v3

Split profile validation into two responsibilities:

1. **stored artifact validation** — validates that the stored profile is internally valid under a supported historical format/policy version;
2. **current compatibility verification** — compares endpoint candidate behavior with a stored profile using current policy v3.

Required behavior:

- stored policy v2 and v3 profiles are supported;
- new profiles must be written as v3;
- stored profile IDs continue to hash their original stored policy version/reference floats exactly;
- v3 endpoint verification requires matching dimensions, normalization and probe-suite version;
- stored policy-version equality is not itself required for endpoint-space compatibility;
- current v3 threshold 0.99 is used for the endpoint comparison;
- a compatible retained v2 profile is reused by ID instead of being rewritten as a v3 clone;
- v1/development-only discarded shapes remain unsupported unless an actual retained persisted file requires them.

### 3.14 Profile publication and activation

Profile artifact persistence and operational activation must be distinct.

A crash or failed vector build may leave inert immutable evidence, but it must not create a new operationally active profile.

For REL retrieval, a profile is operationally active only when a validated non-tombstoned current generation exists with the required current coverage.

For PHY/Memory-only use, the corresponding owner-specific complete binding criterion is used.

Do not create a second mutable profile identity merely to represent activation.

Add explicit queries rather than overloading the current store:

```text
profile(id)                    // historical artifact, tombstoned allowed
profiles_all()                 // all stored artifacts
profiles_active()              // operational active only
compatible_evidence_with(...)  // may inspect non-tombstoned stored evidence
active_compatible_with(...)    // current usable profile only
current_generation(...)        // operational; tombstoned profile excluded
historical_generation(...)     // historical lookup
```

Exact names may differ; the separation is mandatory.

## 4. Reconciliation and transformation rules

### 4.1 Tombstone reconciliation

A tombstone replays as an ordinary terminal semantic mutation on the target domain's clock.

Rules:

- identical tombstones dedupe;
- live history with a later descendant tombstone replays the tombstone;
- an independent post-divergence mutation of the same target versus a tombstone is a structured semantic conflict unless the domain already has an explicit replacement rule;
- reconciliation must never silently resurrect a tombstoned target;
- reconciliation must never infer downstream cascade.

### 4.2 Import epoch reconciliation

Reconciliation requires no unresolved/open import epoch in either source.

Completed epochs are preserved/rebased with their import identities and their source-operation closure.

Rules:

- exact same `import_id` + exact same input/receipt evidence dedupes;
- same `import_id` with different input/receipt evidence is a conflict;
- an import present on only one side replays as one complete epoch around that import's replayed semantic work;
- destination epoch watermarks are recomputed from destination replay positions rather than copying source global-version integers blindly;
- Freshness settlement is reconciled as part of that epoch and retains the same `import_id`.

### 4.3 Physical transforms

`reclaim_storage()`, migration/repack and destructive prune must preserve import markers and tombstones.

Ordinary reclamation remains semantics-preserving and must continue to prove:

- owner identity;
- semantic version heads;
- Archive activity history;
- Historical Freshness/import-epoch equivalence.

Destructive pruning adds the stronger “payload intentionally unavailable” proof but otherwise preserves the same semantic/version invariants.

## 5. Migration strategy

Use a **staged hard cut** internally.

### 5.1 Canonical target

The only final internal architecture is:

```text
domain-owned terminal tombstones
+ owner import epoch store
+ import-scoped Historical Freshness settlements
+ explicit destructive prune transform
+ conservative reclaim_storage
+ compatibility policy v3 with profile retirement
```

Do not retain parallel singleton-checkpoint and epoch implementations.

### 5.2 Real compatibility boundaries

Compatibility is justified only for persisted data:

- current REL/PHY files containing legacy `CVAFRS03`;
- retained compatibility-profile policy v2 artifacts;
- existing Entity tombstones and other durable current-format domain history.

Compatibility is isolated in decoders/rebuild/migration. New writes use the new canonical forms only.

### 5.3 Red migration windows

During the internal migration it is acceptable for whole-repository compilation/tests to be temporarily red while:

- the Freshness store changes from singleton checkpoint to settlements;
- import callers move to epoch authority;
- current/historical read paths gain tombstone/prune semantics;
- reconciliation/repack consumers are migrated.

Do not create forwarding wrappers, dual write paths, permanent aliases or fallback branches solely to keep intermediate states green.

Each phase must still run the coherent targeted tests for the completed subgraph.

## 6. Original architectural work breakdown

> **Execution status:** this 50-step table is no longer the authoritative execution order. It is retained as the original architectural decomposition. Lifecycle convergence is committed through step 12; isolated integration has accepted steps 13–26 and continues at **step 27** in the [remaining execution plan](tombstone-import-history-remaining-execution-plan-2026-10-17.md).

### Model assignment

- **Sol** — architecture/ownership changes, durable formats, version semantics, reconciliation/recovery, destructive-history safety, compatibility-policy changes, retained-fixture promotion.
- **Luna** — bounded implementation under frozen contracts, codecs, mechanical caller migration, fixture construction, targeted tests and verification.
- A Luna step that discovers a new architectural decision stops and escalates that decision before continuing.

| # | Model | Work | Done when |
| ---: | :---: | --- | --- |
| **1** | **Sol** | Freeze source/test baseline and write ADR decisions for terminal direct-only tombstones, import epochs, and the prune/reclaim separation. Do not reopen already settled product decisions. | Baseline lists exact owners/consumers/tests and ADRs record only durable architectural decisions. |
| **2** | **Luna** | Add focused characterization tests for existing Entity tombstones, Archive version/activity preservation, repeated native imports, legacy `CVAFRS03`, reclamation and profile/generation reopen behavior. | Tests describe current behavior without changing it. |
| **3** | **Sol** | Define shared lifecycle domain types and error/idempotency semantics. | No storage ownership moves into the common lifecycle module. |
| **4** | **Luna** | Implement shared lifecycle types/helpers and wire them without changing domain persistence yet. | Shared contract compiles and has pure unit tests. |
| **5** | **Sol** | Define per-domain direct-closure rules for Archive Conversation, Memory, Entity, Relationship, CompatibilityProfile and existing Ego lifecycle state. | Closure rules explicitly separate direct ownership from downstream provenance. |
| **6** | **Sol** | Add/normalize domain tombstone persistence interfaces on existing clocks; adapt existing Entity/Ego tombstones to the common semantics without inventing a second ledger. | Existing and new tombstones expose one lifecycle model. |
| **7** | **Luna** | Add current-vs-historical lifecycle query helpers and migrate callers that currently assume every indexed object is live. | Current projection excludes tombstoned targets; historical lookup remains available. |
| **8** | **Sol** | Specify and implement Archive `ConversationTombstone` record/version format and rebuild projection. | One terminal Archive mutation hides a complete conversation-owned closure. |
| **9** | **Luna** | Implement conversation closure discovery/validation over nodes, branches, metadata and attachment/link exposure. | Closure is deterministic, bounded and reopen-stable. |
| **10** | **Luna** | Add conversation deletion API/CLI-management seam with direct-only default and exact retry behavior. | Re-delete is idempotent; no downstream Memory/Entity/etc. tombstone occurs. |
| **11** | **Sol** | Add reconciliation rules for Archive/domain tombstones and explicit update-vs-delete conflict handling. | Reconcile never resurrects and never auto-cascades. |
| **12** | **Luna** | Add tombstone crash-prefix/reopen/reclaim/reconcile tests including conversation ancestry/activity preservation. | Exact current projection and historical topology survive every cut. |
| **13** | **Sol** | Define `ImportEpochStore`, `ImportStart`, `ImportComplete`, deterministic `import_id`, and open/completed state machine. | Import identity no longer belongs to Freshness. |
| **14** | **Luna** | Implement import marker codecs/rebuild/store and exact retry/conflict logic. | No-import REL has zero markers; open and completed epochs round-trip. |
| **15** | **Sol** | Hard-cut `Cva::import_native` to open/resume an epoch before first mutation and retain current receipt idempotency. | Every real import has one durable start; exact completed reimport produces no new epoch. |
| **16** | **Sol** | Add owner mutation-admission fencing for an open import epoch, reusing mounted Gateway/instance lifecycle ownership. | Ordinary writes cannot interleave inside the epoch; reads remain allowed where already safe. |
| **17** | **Luna** | Attribute existing Import-origin Episode/Insomnia work to the epoch and add epoch-specific status queries. | Completion logic can distinguish import work from unrelated owner work. |
| **18** | **Sol** | Define the import completion coordinator and exact readiness predicate across Insomnia, vectors, semantic work, Dream, Perception, Ego and Historical Freshness. | Completion checks import-caused work, not arbitrary future owner work. |
| **19** | **Luna** | Implement open-epoch recovery/resume after crash at each native-import phase. | Same input resumes; conflicting input fails; no duplicate receipt/episode/marker. |
| **20** | **Sol** | Replace Freshness singleton `ImportCheckpoint` model with ordered import-scoped settlement ownership. | `FreshnessStore` no longer has one authoritative `Option<ImportCheckpoint>` as its internal model. |
| **21** | **Sol** | Define incremental Historical Freshness algorithm from `ImportStart` base projection, import cohort and import-attributable events. | Pre-import live history is immutable; affected pre-existing Memories are supported. |
| **22** | **Luna** | Generalize historical manifest/input building from “all Memories/whole REL” to one epoch's source/cohort/evidence. | Repeated imports produce disjoint/overlapping affected sets correctly. |
| **23** | **Luna** | Generalize historical event/provenance construction to preserve pre-start accepted-use/live events and select only import-attributable provisional events for supersession. | Accepted-use before start survives byte-for-byte/replay-equivalent. |
| **24** | **Sol** | Implement sparse `HistoricalImportFreshnessSettlement` durability and application at epoch completion. | Only affected records replace their import-result projection; unrelated records are untouched. |
| **25** | **Luna** | Move no-fill and bounded embedding-fill Historical Freshness paths to the epoch contract. | Initial and later imports use the same operation. |
| **26** | **Sol** | Isolate legacy `CVAFRS03` decoder/migration into one synthetic completed initial epoch; stop new writes of `CVAFRS03`. | Existing retained files open; internal callers see only epoch/settlement model. |
| **27** | **Sol** | Add reconciliation/repack/reclamation handling for completed import epochs and reject reconciliation with unresolved open epochs. | Epoch identity survives destination replay/rebase correctly. |
| **28** | **Luna** | Add repeated-import Freshness matrix tests: empty REL, long-lived REL, one-conversation later import, multiple sequential imports, crash/retry and post-import live accepted use. | Same policy works for every import position. |
| **29** | **Sol** | Define `HistoryPruneManifest` and per-domain retention descriptor interface. | Missing payload can be distinguished from corruption without changing semantic version clocks. |
| **30** | **Sol** | Implement Archive prune planning and payload-availability semantics for tombstoned conversations. | Node/branch/activity/version topology is unchanged while selected bodies become unreadable. |
| **31** | **Luna** | Implement Memory/Entity/Relationship retention descriptors for tombstoned targets. | Each domain explicitly proves its minimum reconstructive record. |
| **32** | **Sol** | Implement vector/profile retention descriptor: keep IDs/bindings/generation/version topology, authorize numeric payload destruction only for retired/pruned profiles. | Historical existence remains provable without vector matrices. |
| **33** | **Sol** | Implement transformed-copy `prune_tombstoned_history` and reopen verifier. | Source is untouched; output differs only in authorized historical payload availability. |
| **34** | **Luna** | Extend reclamation scanning/open validation so prune-authorized missing payload is valid, while ordinary missing payload is corruption. Do not make normal reclamation infer tombstone garbage. | `reclaim_storage()` remains conservative on unpruned sources. |
| **35** | **Luna** | Add prune/reclaim property matrix: exact global/domain versions, transaction times, Archive activity identity, tombstone/epoch sequence, surviving provenance, body-unavailable results and physical shrink. | Destructive output passes all invariants and actually releases payload bytes. |
| **36** | **Sol** | Move compatibility policy to v3/0.99 and split stored-profile validation from current endpoint compatibility verification. | v2 stored IDs remain valid; new profiles are v3; current verification uses 0.99. |
| **37** | **Luna** | Add policy-boundary tests using calibration cases around real ~0.996 drift, synthetic 0.99 pass and 0.985 fail. | Threshold behavior is deterministic and evidence-backed. |
| **38** | **Sol** | Implement CompatibilityProfile tombstone/retirement and active-vs-historical profile/generation queries. | Retired profile is operationally excluded without changing its ID/history. |
| **39** | **Luna** | Migrate vector/profile consumers so current retrieval uses active complete non-tombstoned profiles while historical inspection can still resolve retired evidence. | No current search silently selects a retired profile. |
| **40** | **Sol** | Harden profile/vector publication so a failed/crashed build cannot make an incomplete profile operationally active. | Orphan evidence may exist inertly; activation requires complete validated current coverage. |
| **41** | **Luna** | Add profile retirement/prune/reclaim tests for Memory vectors, Archive vectors, generations and packed matrices. | Numeric payload can be destroyed only after retirement + explicit prune. |
| **42** | **Sol** | Build retained-REL migration/repair command against disposable copies only. | Tool can migrate legacy checkpoint, add epoch evidence, retire selected profiles and verify before promotion. |
| **43** | **Luna** | Repair first14d scratch copy: keep profile `38b2c3d1…`, retire redundant `69aa8cff…`, preserve generation/bindings/Freshness/import history, no re-embedding. | Scratch copy passes reopen/retrieval/history verification. |
| **44** | **Luna** | Repair first28d scratch copy: keep `e012fa51…`, retire redundant `69aa8cff…` and empty junk `82418298…`, `f8a5ba9f…`, no re-embedding. | Scratch copy passes verification and has one operational active profile. |
| **45** | **Luna** | Repair Ellis scratch copy: keep `3aad70f4…`, retire `2f404c70…`, no re-embedding. | Scratch copy passes verification and retrieval benchmark. |
| **46** | **Sol** | Run optional destructive prune + ordinary reclaim on repaired scratch fixtures and decide promoted artifact based on invariant/retrieval/size evidence. | No source authoritative fixture was mutated during experimentation. |
| **47** | **Sol** | Promote verified retained artifacts and update fixture hashes/provenance only after exact comparison gates pass. | Authoritative fixture repo records new hashes and migration provenance. |
| **48** | **Luna** | Update architecture, storage format, invariants, native import, Freshness docs, current limitations, manual migration docs and ADR index. | Documentation describes only the new canonical architecture. |
| **49** | **Sol** | Final structural/ownership audit with refreshed Lexicon/Arcana plus direct source search. Remove obsolete singleton-checkpoint paths, duplicate lifecycle semantics and migration-only scaffolding. | One canonical implementation remains. |
| **50** | **Luna** | Run final root/CLI/integration/docs/retrieval/retained-fixture gates and write verification record. | Whole repository returns to intended green baseline and all acceptance evidence is recorded. |

## 7. Testing decisions

### 7.1 Completed-architecture test seams

Prefer existing high-level seams:

- `Cva` / `Phylactery` current and historical queries;
- native import entry point and mounted runtime management path;
- Archive conversation/history queries;
- Freshness records-at-cut/event-cut APIs;
- `Cva::reconcile`;
- `reclaim_storage()`;
- profile/vector generation retrieval;
- retained corpus tests.

Add dedicated seams only for:

- lifecycle/tombstone state inspection;
- import epoch status;
- explicit destructive prune maintenance.

Do not expose internal codecs merely for testability.

### 7.2 Tombstone test matrix

At minimum:

- active -> tombstoned current projection;
- exact re-delete idempotency;
- terminal history after reopen;
- native continuation/re-admission creates new state rather than untombstone;
- direct-only conversation delete leaves downstream Memory/Entity/Relationship/Ego objects present;
- optional explicit downstream cascade tombstones only selected closure;
- update-vs-delete reconciliation conflict;
- delete on one branch with no competing update replays normally;
- reclamation before prune preserves historical body;
- prune after tombstone makes body unavailable;
- surviving provenance resolves structural historical identity after prune.

### 7.3 Import epoch test matrix

At minimum:

- no-import REL contains no markers and behaves normally;
- first import into empty REL;
- later import into active REL;
- sequential imports from multiple sources;
- exact completed reimport;
- crash after start, Archive ingestion, branch publication, queueing, derived completion, Freshness settlement and before completion marker;
- open-epoch conflicting input rejection;
- completed epoch survives reopen/reclaim/reconciliation;
- no unrelated live mutation enters an open epoch;
- raw mutation history remains present;
- operational epoch interpretation collapses the import construction interval conceptually to one transition.

### 7.4 Historical Freshness repeated-import matrix

Prove:

- pre-import Freshness records at `ImportStart` are unchanged;
- accepted-use events before start remain present;
- imported new Memories receive reconstructed Freshness;
- import historical semantic/Dream events may affect eligible pre-existing Memories;
- unrelated pre-existing Memories are not rewritten;
- post-completion live use/Dream/semantic events apply normally;
- second import starts from the first import's completed canonical state;
- retry/reopen produces exactly one settlement per import ID;
- bounded embedding-fill works for later imports;
- no-fill disposition remains supported;
- import source chronology does not require rewriting pre-existing REL operational chronology.

### 7.5 Prune invariants

Before and after destructive prune, compare:

- owner UUID;
- latest global version;
- every participating domain-local version;
- transaction-time map;
- Archive activity changes/positions;
- conversation/node/branch topology;
- tombstone identities;
- import epoch identities/watermarks;
- Freshness settlement identities;
- surviving current projection;
- surviving provenance target IDs.

Only payload-availability results and physical storage size may intentionally differ.

### 7.6 Compatibility/profile gates

Prove:

- v2 profile artifact round-trip remains exact;
- v3 candidate can reuse compatible v2 ID;
- v3 new artifact uses v3 ID/policy;
- 0.99 passes calibrated floor cases;
- 0.985 fails calibrated floor case;
- tombstoned profile is excluded operationally;
- historical generation still resolves before prune;
- packed numeric payload becomes unavailable only after explicit prune;
- failed/crashed new profile/vector build cannot become active.

## 8. Retained fixture acceptance

Authoritative fixtures must not be modified in place during development.

Work from copies of:

- `chatgpt-first14d`
- `chatgpt-first28d`
- `rhelm-david-r-ellis-pre-relationship`

Canonical active profiles after repair:

| Fixture | Keep operational | Retire |
| --- | --- | --- |
| first14d | `38b2c3d1ded65fdbc2a76086f1d2875c03eb4470afa63de8bbd0689cbf51c266` | `69aa8cff…` |
| first28d | `e012fa5173c94058e83749208bd3250628a4d533d01d96fe711c86a813c14258` | `69aa8cff…`, `8241829850543532329fda348b2365c84b80f401af5ed4a2ab571a3c234d05d8`, `f8a5ba9f1c470340b67d9cf7c1cf28d8e23500901eef397ac375c3c217531338` |
| Ellis | `3aad70f4c0083a74be98cc41b51515559c7ed26669597641e562299424ddcd82` | `2f404c70725ad1162653bf2762c0f5590d0c3f7a1b83c4b9bf87e39091138fa5` |

No re-embedding is expected.

Acceptance includes:

- exact reopen;
- one operational active complete profile;
- all canonical existing profile IDs preserved;
- current Memory vector coverage preserved;
- current Archive generation valid;
- Historical Freshness equivalent before/after migration except where the new epoch representation intentionally replaces legacy framing;
- retrieval benchmark remains within recorded baseline/parity;
- legacy import is represented as a completed epoch;
- optional prune/reclaim does not alter current retrieval results;
- authoritative fixture hashes change only at the final promoted step and are recorded.

## 9. Documentation and ADR impact

Create/supersede durable decisions for:

1. terminal direct-only tombstones and explicit destructive historical pruning;
2. owner-level import epochs and repeated-import Historical Freshness settlement;
3. compatibility policy v3/profile retirement if the threshold/retained-v2 compatibility would otherwise be surprising to future maintainers.

Update:

- `docs/architecture.md`
- `docs/storage-format.md`
- `docs/invariants.md`
- `docs/native-import.md`
- `docs/current-limitations.md`
- `docs/manual/migration-and-reconciliation.md`
- Freshness historical reconstruction docs
- ADR 0007 references/supersession metadata as appropriate
- ADR 0039 references where Historical Freshness import ownership changes
- `docs/decisions/INDEX.md`
- `docs/INDEX.md`

Do not rewrite historical verification documents as if they had always described the new system. Mark them superseded where necessary and preserve them as evidence of prior implementation state.

## 10. Out of scope

This work does not include:

- automatic provenance/downstream cascade deletion;
- a general public “purge history” product workflow;
- untombstone/resurrection semantics;
- renumbering or compacting away semantic version slots;
- rewriting source chronology to pretend imported events occurred as live REL mutations in the past;
- re-embedding the retained fixtures solely because policy changes to v3;
- redesigning vector storage representation;
- making normal `reclaim_storage()` destructive;
- generic concurrent live-write interleaving inside an open import epoch;
- inventing PHY conversation imports where no product import path exists.

## 11. Completion gates

The workstream is complete only when all of the following are true:

1. One common lifecycle contract exists and semantic stores retain their proper ownership/version clocks.
2. Archive supports terminal aggregate conversation tombstones.
3. Direct-only is the default; downstream cascade never occurs implicitly.
4. Current-state APIs exclude tombstoned targets and historical APIs distinguish available vs pruned payload.
5. Normal no-import RELs require no import records.
6. Every native import, initial or later, has one durable `ImportStart` and one durable `ImportComplete` when successful.
7. Open import epochs recover deterministically and cannot silently admit unrelated writes.
8. Historical Freshness reconstruction is import-scoped and works on repeated later imports.
9. The singleton whole-owner `ImportCheckpoint` is gone from the canonical internal architecture.
10. Legacy `CVAFRS03` remains loadable only through the isolated persisted-format compatibility boundary.
11. Explicit destructive pruning preserves every semantic/version/history invariant while making selected tombstoned payload unrecoverable.
12. Ordinary reclamation remains conservative.
13. Compatibility policy v3/0.99 is active for new verification.
14. Retained v2 profile IDs remain exact and reusable when compatible.
15. Compatibility profiles can be retired without rewriting their identity/history.
16. Failed/incomplete vector publication cannot make an incomplete profile operationally active.
17. The three retained RELs each have one canonical operational profile with no re-embedding.
18. Reconciliation, migration, reclaim and prune preserve tombstone/import epoch semantics.
19. Migration-only wrappers/aliases/dual paths are removed.
20. Whole-repository build/tests return to the intended final baseline.
21. Retained-corpus/retrieval evidence is recorded.
22. Refreshed structural analysis shows no surviving old singleton-checkpoint owner path or unexpected reverse dependency.

## 12. Final target summary

The intended end state is:

```text
Normal Reliquary operation
    -> ordinary append-only semantic history
    -> no import markers required

Import
    -> ImportStart
    -> bounded/exclusive import construction epoch
    -> import-scoped derived processing
    -> import-scoped Historical Freshness settlement
    -> ImportComplete
    -> normal operation resumes

Deletion
    -> terminal domain tombstone
    -> direct ownership closure disappears from current projection
    -> downstream derived state survives unless explicitly selected

Historical destruction
    -> explicit prune_tombstoned_history
    -> exact history/version topology retained
    -> selected tombstoned payload becomes unavailable
    -> ordinary reclaim_storage may collect newly unreachable bytes

Embedding compatibility
    -> policy v3 / cosine 0.99
    -> retained v2 profile IDs remain valid evidence
    -> one active complete profile per repaired retained REL
    -> retired profile/vector payload removable only through explicit prune
```

This is the canonical target. Implementation should move directly toward it rather than preserve the current one-shot Freshness checkpoint or append-only-active-profile assumptions behind compatibility scaffolding.

## Settlement integration checkpoint (2026-10-17)

Settlement commit `10a2aaf` was cherry-picked onto `959e98c` as `44f52b0` in `integrate/import-freshness-settlements-20261017`. Main remains at `959e98c`; promotion waits for the completed import architecture at step 36.

The integration audit repaired same-activity-cut live accepted-use replay, replaced the invented-cohort legacy fixture with real accepted Memory/birth evidence and passive Cva reopen coverage, corrected the CVAFRS04 policy-corruption offset, and added persisted multi-settlement replay coverage. The 35-test focused settlement/legacy/live-use/transform/lifecycle matrix passes; formatting and `cargo check --tests --locked` pass. CLI formatting/checks and all 22 CLI tests passed. The architecture refresh passed both source guards (92 multiplexing fixtures, 166 classified APIs and 11 external-ownership fixtures), then timed out after ten minutes in Lexicon Rust analysis; Arcana/Pitlord did not run. The scoped settlement repairs and eight current-behavior docs are committed as `957bf45` after `44f52b0`. Steps 13–17 have passed their focused acceptance on the lifecycle-converged isolated branch; final structural acceptance remains open. Both documentation-policy checks report five existing findings outside the changed canonical docs (one broken Ego-plan link and four missing sections in the Ego decision inventory).

Steps 24–25 must explicitly handle zero-result and zero-activity imports, preserve the canonical Freshness base at ImportStart, and distinguish same-cut pre-start evidence from later live receipts by accepted identity/order rather than activity cut alone. The current codec/store requires nonempty replacements/cohort and an advancing cut; no completed repeated-import claim is made.

Luna prepared isolated epoch codec/storage/reopen tests in `7e2f337` on `7706ed0`; they are not integrated or Cargo-verified yet. The next integration batch remains step 18 after settlement verification and commit.


## Epoch admission integration checkpoint (2026-10-17)

Durable epoch admission was integrated on top of the accepted settlement branch as `ef86f04`, with marker/storage coverage in `9480f5f`, write-admission hardening in `5e97169`, and phase-cut crash/retry coverage in `9973ff7`.

Final hardening moved immutable source-revision conflict validation ahead of `ImportStart`, added the missing `WriteAdmissionClosed` display path, and added a regression proving invalid immutable source revisions do not open or poison an epoch and do not change bytes, clocks or epoch status. This preserves the rule that rejected admission is physically and semantically inert.

Final acceptance passed:

- multiplexing coverage guard: 92 fixtures and 166 classified public APIs;
- external-source ownership guard: 11 fixtures plus production-boundary check;
- `cargo fmt --check` and CLI formatting;
- `cargo check --tests --locked`;
- 5 epoch codec/store/identity tests;
- native-only no-marker reopen;
- exact retry with no marker/semantic-clock change;
- completed-epoch storage/rebuild/idempotent retry;
- durable start + unrelated-write fencing;
- open-epoch conflict/policy rejection;
- invalid source revision admission neutrality;
- phase-cut interrupted-import recovery without duplicate work;
- 7 import settlement storage/legacy/replay tests;
- 18 lifecycle regression tests;
- CLI check plus 22/22 CLI tests.

Steps 18–23 are accepted on the isolated integration branch. Production completion remains intentionally unavailable: no normal caller owns `append_complete()` and no derived-work/Freshness readiness coordinator exists yet. That is later step 28–31 work, not part of this checkpoint.
