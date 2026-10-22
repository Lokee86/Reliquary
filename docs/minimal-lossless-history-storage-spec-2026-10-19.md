# Minimal lossless history storage — refactor specification

Status: Implemented and accepted: all 377 steps in the [linear plan](minimal-lossless-history-storage-linear-plan-2026-10-19.md) are complete. Persisted-format policy follows the [development format hard-cut specification](development-format-hard-cut-interruption-spec-2026-10-19.md): current V2.0 owners only; legacy runtime readers and temporary converters are retired. Historical input descriptions below preserve design evidence. Measurement-gated deferrals remain explicit in the final retirement report.
Date: 2026-10-19  
Scope: REL/PHY durable history representation  
Related: [ADR 0003](decisions/0003-layered-version-clocks-and-local-ancestry.md), [ADR 0004](decisions/0004-bounded-ancestry-aware-packs.md), [ADR 0005](decisions/0005-cva-composition-and-packed-vector-objects.md), [ADR 0007](decisions/0007-compatibility-profiles-and-vector-generations.md), [ADR 0041](decisions/0041-semantic-import-boundaries-owner-pruning-history-rewrite.md), [Versioning, Historical Cuts, and Rollback](version-history-plan.md)  
Execution plan: [Minimal Lossless History Storage — Linear Implementation Plan](minimal-lossless-history-storage-linear-plan-2026-10-19.md)

## Purpose

Define the lossless compact-history target and its acceptance requirements while preserving existing owner-local semantics and persisted V1.0 compatibility.

## Overview

Creation records, owner-specific deltas and smaller proof encodings reduce repeated durable representation. A historical oracle and transformed-copy upgrader establish equivalence before compact writers are accepted.

## Problem Statement

Reliquary deliberately retains append-only owner history so old state can be inspected, reconciled, validated, and eventually reconstructed at historical cuts. That behavior is valuable and must remain.

The current physical and domain encodings are not uniformly minimal, however. Several owners persist full postimages or large repeated evidence when the prior accepted state plus a much smaller delta would reconstruct the same result. Other record families repeatedly carry long cryptographic identities or framing metadata. Some durable runtime/derived stores append replacement snapshots even though their complete revision history is not part of semantic historical state.

Fresh-history rewrite is not the solution to this problem. ADR 0041 correctly defines rewrite as intentional construction of a new history that may omit old history. This work instead changes how an ordinary REL/PHY accumulates history so that the retained history itself is compact.

The governing requirement is:

> For every retained historical semantic cut, the compact representation must reconstruct the same authoritative owner state and validation evidence as the current representation.

Storage size must fall by eliminating repeated representation, not by silently weakening retained history.

### Baseline evidence

The 2026-10-19 audit inspected every durable record family in current source and measured the three authoritative REL/PHY fixture pairs:

- ChatGPT first 14 days
- ChatGPT first 28 days
- Ellis pre-relationship fixture

Important findings:

- Archive content and standalone Memory bodies are already content-addressed/deduplicated.
- Grouped Insomnia completions are canonical publication records for their Memory bodies/records; they are not duplicate copies of separately stored Memory publications.
- Memory-vector bindings are unique per compatibility profile and Memory body.
- The large vector footprint in retained fixtures is mostly legitimate parallel compatibility-profile state, not old duplicate generations.
- Graph and Freshness are already fundamentally mutation/effect journals; their largest opportunities are encoding/framing efficiency and removal of redundant proof postimages, not replacement with a different state model.
- Memory, Entity, Relationship and Ego revision families retain more postimage data than is required to reconstruct later revisions.
- Dream first-pass history stores complete pre-existing Memory ID sets even though that set is historical Memory state.
- Dream pair history uses two full 32-byte Memory IDs for every monotone pair-membership fact.
- interaction-stream checkpoints persist cumulative text prefixes.
- the external-source index persists full derived SourceState replacements and explicitly treats damage as discardable without invalidating canonical REL semantics.

The same audit confirmed that pack/container compression is highly valuable, including on packed vectors, but compression is already owned by ADR 0004 and is not the logical-history redesign specified here.

## Terminology

**Authoritative semantic history**  
The minimum durable owner records and proofs required to reproduce and validate the accepted semantic state of REL/PHY owners at their historical cuts.

**Historical semantic cut**  
An existing owner-local cut or a CVA-global ordering point resolved according to ADR 0003 and version-history-plan.md. Global version remains ordering only; it does not become ancestry or a universal semantic owner.

**Creation record**  
The first accepted state of a durable semantic identity.

**Delta record**  
A later accepted mutation containing only information that differs from the immediately preceding accepted state for that identity or owner projection.

**Proof record**  
Evidence required to validate that an accepted state transition was authorized or correctly derived. Proof records may reference historical owner cuts where that cross-owner reference is already semantically meaningful, but must not turn another owner's local watermark into general ancestry.

**Immutable payload**  
Large bytes whose semantic identity is stable and which can be content-addressed or otherwise referenced without repeating the bytes in every revision.

**Derived/recovery state**  
State used for indexing, acceleration, scheduling, retries, crash recovery or runtime convenience that is not itself part of the semantic historical view once its authoritative effect has settled.

**Checkpoint**  
A disposable reconstruction accelerator derived from authoritative history. A checkpoint is never the sole authority for a historical fact.

## Governing Invariants

1. **Exact semantic reconstruction.** For every accepted historical cut supported by the source format, the compact format reconstructs the same authoritative state.

2. **Owner-local clocks remain authoritative.** Archive, Memory, Entity, Relationship, Graph, Vector Generation and other existing owner-local clocks retain their current semantics. Global version remains ordering only.

3. **No semantic history is removed by this refactor.** Deleting history remains the job of explicit pruning or fresh-history rewrite.

4. **Immutable semantic identity does not change.** Memory IDs/body IDs, Archive content IDs, Entity/Relationship IDs, vector/profile identities, owner UUIDs and provenance identities remain stable unless an existing migration contract already rebases them.

5. **Exact bytes remain available where bytes are semantic.** Source content, Memory bodies, files, accepted vector values and other immutable payloads required to reproduce historical behavior cannot be replaced by approximate recomputation.

6. **Derived state is not promoted into authority.** Checkpoints, caches, indexes and deterministic projections remain disposable unless an accepted proof explicitly depends on their identity.

7. **Proof minimization is allowed only when validation is equivalent.** A repeated postimage/set may be replaced by a cut, digest or delta only if reopen/replay can validate the same invariant from retained authoritative history.

8. **Crash boundaries do not weaken.** A compact publication must remain at least as atomic and fail-closed as the record family it replaces.

9. **Reconciliation remains semantic.** Compact local encodings are decoded to canonical domain state/evidence before reconciliation. Reconciliation must not depend on local compact ordinals or physical packing identities.

10. **Rewrite semantics remain unchanged.** Fresh-history rewrite continues to intentionally discard/re-express history according to its policy. It is not redefined as compaction.

## Target Architecture

### 1. Preserve the existing owner model

This refactor does not create a generalized event-sourced super-owner.

Each semantic owner retains:

- its existing identity model;
- its existing local version/watermark when one exists;
- its existing validation rules;
- its existing current-state API;
- its existing relationship to CVA-global ordering.

The compact representation is an implementation detail behind those owner interfaces.

### 2. Creation + delta for postimage-oriented owners

Owners that currently encode most or all of a revised object must move to:

    Creation(identity, full initial state)
    Delta(identity, revision/domain version, changed fields only)
    Delta(...)
    Tombstone(...)

Replay materializes the same postimage currently stored directly.

A delta must be canonical:

- fields appear in a fixed order;
- a field mask or typed operation list identifies changed fields;
- unchanged fields are omitted;
- setting an optional field to None is distinct from leaving it unchanged;
- collections use explicit add/remove/replace operations chosen per domain;
- invalid deltas fail before mutating the in-memory projection;
- retry/mutation identity remains exact.

The completed store interface continues to expose domain objects, not delta mechanics.

### 3. Keep immutable payloads separate from revision metadata

Where the current design already separates immutable payload from mutable metadata, retain that split.

Examples already aligned with the target:

- Archive CVACONT1 content;
- standalone Memory bodies;
- file/content identities;
- compatibility profiles;
- packed vector payload objects where exact bytes are legitimately unique.

New delta formats reference immutable payload identity rather than embedding payload bytes again.

Do not create a generic blob abstraction merely for this migration. Existing domain ownership remains preferable unless two or more domains genuinely require the same storage seam.

### 4. Minimal proof instead of repeated reconstructible postimages

When a durable record currently stores both:

- the causal/effect delta; and
- a complete state/set that can be exactly reconstructed from an authenticated historical owner cut,

the target representation retains the causal/effect data and replaces the repeated postimage with the smallest evidence needed to validate reconstruction.

This is only permitted when the referenced historical cut has exact, stable semantics under existing owner rules.

### 5. Derived/recovery stores use bounded durable state

A durable store that is explicitly derived or exists only for crash/retry/runtime recovery must not accumulate an unlimited replacement history unless that history is independently part of the semantic contract.

The target is one of:

- current replaceable derived state;
- append-only unresolved-operation state followed by terminal compaction;
- deterministic rebuild from authoritative owners;
- a bounded receipt history where retry identity requires it.

Derived/recovery state must remain separate from semantic historical cuts.

### 6. Physical packing/compression remains a separate layer

ADR 0004 continues to own bounded immutable packs and pack-level compression.

Logical compact records defined here must be packable/compressible but must not depend on a particular:

- compression codec;
- pack size;
- physical locality strategy;
- content dictionary;
- compressed object address.

Per-pack dictionaries or compact physical references may later reduce repeated 32-byte identities, but such references are physical codec details and never replace semantic IDs in reconciliation, APIs or proofs.

## Domain Decisions

### Archive source/history

**Keep the current content-addressed source-content model.**

Source turns, files, branches, Episodes, import receipts and provenance remain historical source authority.

Do not replace source content with summaries or reconstructed text.

Archive revision framing may be made physically smaller, but Archive version semantics and exact historical cuts remain unchanged.

CVAAREC1-style record/version indirection may be co-framed in a future compact codec only if:

- the same Archive/local and global versions are preserved exactly;
- crash-inert unpublished payload behavior remains equivalent;
- ObjectRef relocation/reclamation remains valid;
- no generalized transaction owner is introduced.

This spec does not require a new Archive semantic model.

### Memories

Memory is a primary delta target.

Current invariant: Memory body identity is immutable across revisions. Therefore later revisions should not restate unchanged body identity and complete metadata postimages.

Target:

- first publication retains complete canonical Memory metadata plus body reference;
- later revisions encode only fields changed from the previous revision;
- tombstone remains terminal and explicit;
- mutation ID/retry semantics remain exact;
- memory_revision(id, revision) reconstructs exactly the same Memory as today;
- provenance validation continues to run against reconstructed records.

The migration oracle must compare every revision of every fixture Memory, not only current state.

#### Memory field inventory (Step 42)

This table classifies the actual V1 `MemoryRecord` and hydrated `Memory` fields. “Mutable” means a later accepted V1 record may differ; it does not bypass existing draft, provenance or replay validation. In particular `created_at_ns` is not currently enforced as immutable, so migration must retain its per-revision value.

| Field | Classification | Compact responsibility |
| --- | --- | --- |
| `id` | Immutable semantic identity | Creation identity; delta target |
| `revision` | Required revision coordinate | Exact successor and predecessor binding |
| `body_id` | Immutable payload identity across revisions | Creation only; later body replacement rejects |
| `title` | Immutable body payload, hydrated | Retain exact content-addressed bytes separately |
| `content` | Immutable body payload, hydrated | Retain exact content-addressed bytes separately |
| `category` | Mutable scalar | Changed field only |
| `memory_type` | Mutable scalar | Changed field only |
| `authority_kind` | Mutable scalar | Changed field only |
| `temporal_status` | Mutable scalar | Changed field only |
| `scope` | Mutable scalar | Changed field only |
| `lifecycle_state` | Mutable scalar | Changed field only; terminal tombstone stays separate |
| `archived` | Mutable scalar | Changed field only |
| `superseded_by` | Mutable optional identity | Explicit set/clear or unchanged |
| `parent_id` | Mutable optional identity | Explicit set/clear or unchanged |
| `source_node_id` | Mutable optional provenance | Explicit set/clear or unchanged |
| `content_source_conversation_id` | Mutable optional provenance | Explicit set/clear or unchanged |
| `content_source_node_id` | Mutable optional provenance | Explicit set/clear or unchanged |
| `grounding_source_conversation_id` | Mutable optional provenance | Explicit set/clear or unchanged |
| `grounding_source_node_id` | Mutable optional provenance | Explicit set/clear or unchanged |
| `source_episode_id` | Mutable optional provenance | Explicit set/clear or unchanged |
| `source_time_ns` | Mutable optional temporal value | Explicit set/clear or unchanged |
| `source_ref` | Mutable optional structured provenance | Whole-value set/clear or unchanged; retain owner/principal/Episode/Node/content/grounding identities |
| `temporal_inference` | Mutable optional structured proof | Whole-value set/clear or unchanged; retain body/time binding, model, contract and ordered resolutions |
| `mutation_id` | Required per-publication retry identity | Retain exact value on every creation/delta |
| `created_at_ns` | Mutable V1 scalar | Changed field only; no new immutability restriction |
| `updated_at_ns` | Mutable scalar | Changed field only |
| `global_version` | Required global publication coordinate | Preserve exact accepted coordinate, including grouped carriers |
| `memory_version` | Required owner-local publication coordinate | Preserve exact accepted coordinate |
| `routing_metadata` | Hydrated independent immutable owner record | Not a revision delta field; retain separate body-bound routing publication |

`MemoryDraft` has no additional durable revision fields: its title/content map to the immutable body, its mutation and times map to the fields above, and optional source/inference values are supplied through existing owner seams. Public `MemoryRef`, `MemoryRevisionId`, retention descriptors and statistics are references/projections of these records, not additional mutable revision authority. Separate terminal tombstones, body-prune proof, routing metadata and external/grouped publication evidence retain their own record contracts.

The V1 publication API treats absent supplied `source_ref` as inherit and absent supplied inference as inherit only while its body/time binding remains valid. Delta construction must compare the **resolved accepted records**, not reinterpret absent caller arguments as clear operations. The replay codec must still represent explicit `None` postimages found in retained legal records.

#### Memory creation/delta contract (Steps 43-47)

The private owner contract lives in `src/memory/memory_compact_contract.rs`. `MemoryFieldChange<T>::Unchanged` inherits the prior accepted value; `Set(value)` replaces it. For optional fields, `Set(None)` explicitly clears and `Set(Some(value))` explicitly sets. Absence of a caller-supplied source reference or inference retains the existing API meaning; diff construction runs after those API defaults have resolved to an accepted record.

A creation retains the complete first `MemoryRecord`, including its immutable ID/body reference and exact mutation, revision, owner-local/global coordinates and times. Revision must be one, publication coordinates nonzero and mutation identity present. Body bytes remain in their separate content-addressed carrier; creation does not redefine ID derivation, routing, provenance validation or grouped atomicity. The creation envelope guard is not a substitute for full codec/body/provenance validation.

A delta retains target ID, explicit previous revision, successor revision, global coordinate, Memory coordinate and exact new mutation ID. Target identity and immutable body cannot change. Revision must be the exact nonzero successor; owner-local coordinates must advance relative to that Memory's prior record (other Memory publications may occupy the intervening versions). Global coordinates are nonzero retained publication identities; accepted V1 late-ticket bindings can put a later revision at an earlier global coordinate. Migration and replay preserve those coordinates without imposing new global ordering. A repeated/empty mutation ID is not a new delta; existing owner retry/conflict handling remains responsible for accepted retries. A new publication with identical metadata remains representable with an empty change mask and a distinct required envelope.

The mask is an unsigned 64-bit value. Bits 0-19 identify fields in this fixed order:

| Bit | Changed field |
| ---: | --- |
| 0 | `category` |
| 1 | `memory_type` |
| 2 | `authority_kind` |
| 3 | `temporal_status` |
| 4 | `scope` |
| 5 | `lifecycle_state` |
| 6 | `archived` |
| 7 | `superseded_by` |
| 8 | `parent_id` |
| 9 | `source_node_id` |
| 10 | `content_source_conversation_id` |
| 11 | `content_source_node_id` |
| 12 | `grounding_source_conversation_id` |
| 13 | `grounding_source_node_id` |
| 14 | `source_episode_id` |
| 15 | `source_time_ns` |
| 16 | `source_ref` |
| 17 | `temporal_inference` |
| 18 | `created_at_ns` |
| 19 | `updated_at_ns` |

Changed values follow ascending bit order; unchanged/equal values are omitted. Bits 20-63 are reserved and reject. Mutation/version/revision fields belong to the mandatory envelope, not this mask. An exhaustive `MemoryRecord` pattern makes added record fields a compile-time contract decision.

There are no independently mutable collection fields in the Memory revision record. Structured `source_ref` and `temporal_inference` use whole-value replacement/clear. The inference resolution vector retains exact accepted element order and duplicates; it is not silently sorted or converted into a set. Routing mention collections belong to the independent body-bound immutable routing record and are not delta operations. The codec/replay phase supplies pure application, structural decoder validation, redundant-operation rejection and exact wire bytes; current implementation acceptance is tracked in the linear plan. This contract does not itself enable compact writers.

### Entities

Entity creation retains full initial semantic state.

Later Entity revisions use typed deltas for mutable fields and evidence collections. Merge/retirement and direct terminal tombstone semantics remain explicit terminal operations rather than generic field changes.

Entity historical lookup must reconstruct every prior accepted revision exactly.

#### Entity field contract — Step 92

This contract is based on `entity_model.rs`, R1/R2 decoding and `EntityStore::insert_rebuilt`, with actual V1 REL/PHY characterization and pure contract tests. No Entity writer or persisted record changes are enabled by this table.

| Field | Retained-history rule | Compact responsibility |
| --- | --- | --- |
| `id` | Stable per Entity revision chain; retain exact 32 bytes | Creation identity; delta binds the same ID |
| `global_id` | Optional per revision. R1 has `None`; replay does not enforce continuity across R2 revisions | Mutable typed optional field; preserve set/change/clear without inventing UUIDs |
| `revision` | Exact per-Entity successor; terminals advance existing revision semantics | Explicit envelope and predecessor revision |
| `canonical_name` | Mutable exact accepted string | Changed value only |
| `aliases` | Mutable sorted unique vector under existing validation; exact order is semantic | Whole ordered replacement; do not renormalize migration input |
| `kind` | Mutable subject to per-record principal validation | Changed value only; existing principal rules remain owner-local |
| `summary` | Mutable; authored empty and pruned unavailable are distinct | Changed value only plus explicit separate pruning evidence |
| `mutation_id` | Per-publication identity, with existing owner retry/conflict rules | Explicit envelope; a new mutation with unchanged metadata remains legal |
| `created_at_ns` | Existing replay enforces stable creation time | Creation value inherited by deltas |
| `updated_at_ns` | Mutable; existing record validation requires it to be at least creation time, not at least the previous update | Changed value only; no new historical monotonicity restriction |
| `global_version` | Retained publication ticket, nonzero and admitted against the observed container frontier | Exact envelope; do not add ordering requirements beyond actual owner admission |
| `entity_version` | Dense owner-local publication sequence, interleaved across Entity IDs | Exact envelope; per-ID successors need not be adjacent owner-local tickets |

Creation must represent legacy optional global identity without forcing current writer defaults. Delta derivation is over resolved accepted records, not draft arguments. An empty changed-field mask is legal for a distinct accepted mutation. Existing name/kind/summary/alias limits, timestamp checks and principal deterministic identity validation remain authoritative. Migration preserves accepted spelling/whitespace and values rather than rerunning draft normalization.

Merge retirement (`CVAENTT1`) retains replacement identity; direct tombstone (`CVAENTT2`) has no replacement. Neither is a metadata-field delta. Both keep exact revision and publication coordinates and terminal/no-resurrection behavior. An absent legacy Entity format marker with no Entity publications retains the existing empty-owner case. Replay currently also admits versioned Entity records without that marker; conversion must not discard them merely because the marker is absent. This is temporary input characterization, not final legacy-runtime support.

Pruned descriptions retain structural revision fields and explicit evidence separate from authored text. `EntityDescriptionProof.original_carrier_digest` is SHA-256 of the original unpruned record payload, excluding its publication marker; `manifest_digest` is SHA-256 of the complete canonical encoded `CVAHPR01` manifest, including its framing and checksum. File-level validation must check that manifest's descriptor authorizes the exact Entity revision. Canonical redaction is name `[pruned]`, empty aliases and empty summary; principal descriptions cannot be pruned. Placeholder text alone cannot prove pruning. Pure construction and wire decoding do not authenticate this evidence: shared file-level proof admission must bind the descriptor, original carrier and source history before making unavailable payload claims.

Existing `CVAHPR01.source_history_digest` depends on original publication carriers. The temporary converter explicitly rejects these inputs until an owner-specific preservation or authenticated-equivalence implementation is accepted; copying proofs across rewritten carriers is forbidden. Shared proof admission (Step 101) and both REL/PHY pruning adapters precede writer/migration cutover. Old `CVAEPRN1` remains temporary conversion input. Reconciliation, backing liveness, retained-description structure and raw-carrier-dependent history digests have explicit separate execution steps. Step 92 freezes representation and authority boundaries, not completion of those adapters.

### Relationships / Observations

The current transitional Relationship owner and its future Observation-converged form must not force this refactor to preserve a soon-obsolete schema.

For whichever representation is canonical when this work executes:

- creation is full;
- later revisions are typed deltas;
- participant/evidence collections use canonical add/remove/change operations rather than complete repeated arrays when possible;
- terminal lifecycle remains explicit;
- historical reconstruction is tested at every owner-local revision/version.

If Relationship-to-Observation convergence lands first, implement compact history only on the canonical Observation representation and retain old Relationship codecs as migration input.

Execution Step 108 selects Relationship: no separately persisted Observation owner has landed. The Relationship contract preserves exact validated values, including whitespace that ordinary draft publication normalizes. Creation binds all record fields with implicit revision 1; delta binds stable ID, exact predecessor/successor revision, global/local tickets and fresh mutation ID. Five mutable fields are kind, participant set, evidence set, summary and update timestamp. ID and creation time are immutable. Nonzero global tickets may arrive earlier than a previous revision's ticket; owner-local publications remain dense and interleaved across IDs. Update time may decrease relative to its predecessor while remaining at least creation time. A fresh mutation with no changed fields is legal.

Participants are canonical sorted sets of complete `(EntityRef, optional role)` values; changing a role removes the old tuple and adds the new one. Evidence is a sorted set of `(owner ID, Memory ID)` values. Collection deltas carry sorted, unique, disjoint removal/addition lists; application must reject absent removals, existing additions and redundant encodings before validating the full candidate. Existing participant/evidence and text bounds apply. Local-reference admission stays at the existing writer/rebuild boundary; codec construction does not invent reference authority.

Relationship description pruning clears only summary. Explicit proof carries SHA-256 of the original unpruned record payload and the complete canonical CVAHPR01 manifest; it grants no authorization alone. Shared file-level validation still binds exact manifest descriptor, ID/revision, original carrier and source history. Authored empty summary remains distinct from unavailable summary. Terminal deletion remains the existing explicit revision-plus-one operation; no resurrection or replacement target is introduced.

### Ego

Identity, Anchor, Personality and synthesis revisions should stop repeating unchanged text/metadata.

Target:

- immutable/repeated textual payloads receive stable payload identity within Ego storage;
- revision records reference text payload and encode only changed metadata/activation/deletion state;
- active-identity transitions remain explicit;
- terminal Identity/Anchor deletion semantics remain unchanged;
- REL/PHY ownership restrictions remain unchanged.

Do not add a shared text-blob subsystem unless measurements show existing content storage cannot serve the need cleanly.

### Semantic Graph

Graph is already delta-oriented and remains so.

Do not replace Graph mutation history with snapshots.

Work here is restricted to encoding/framing:

- preserve mutation meaning exactly;
- preserve full-Graph and Memory-Graph local versions;
- preserve node admission order and active relation state;
- reduce repeated framing/identity overhead only behind Graph's codec/store interface;
- any compact reference table must be physical/local and must decode to canonical semantic node identities before reconciliation.

Graph rewrite baselines remain a fresh-history rewrite mechanism, not ordinary-history compaction.

### Freshness

Freshness is already an effect journal and remains an effect journal.

The canonical historical information is:

- owner policy;
- enrollment/admission;
- accepted event/use identity;
- source/activity cut;
- effect deltas;
- provenance/proof evidence;
- accepted import settlements where applicable.

Derived Freshness postimages must not be persisted merely for replay convenience when they can be recomputed and validated from the canonical effect delta. Existing current-format compatibility may still read and validate legacy postimages.

Any removal of current records postimages from CVAFRS02 requires a proof-equivalence test showing that malformed/forged effects which are rejected today remain rejected by the new codec/replay path.

Historical record_at / records_at / exact event-cut behavior must remain identical.

### Dream first-pass history

InitialDreamPass.preexisting_ids is a confirmed repeated historical set and is a primary normalization target.

The target Begin record retains:

- pass identity;
- source Memory identity and required first-publication proof;
- accepted REL activity cut;
- graph cut;
- a compact Memory-history cut identifying the exact pre-existing population;
- a canonical digest of that population when required to preserve existing collision/forgery detection.

The full preexisting_ids set is reconstructed from Memory history at the pinned cut when validating roots or retained completed passes.

This is an explicit, semantically meaningful cross-owner cut, not generalized ancestry. It may be introduced only after the Memory owner has an exact internal historical projection for the required cut.

Migration from CVDREAM1 materializes the legacy set, derives the equivalent Memory cut, proves equality, and emits the compact representation. If an old set cannot be represented by one exact Memory cut, migration must retain an explicit set exception rather than silently change meaning.

### Dream pair history

Dream pair membership is already a monotone delta set; do not change its semantics.

CVADRP01 is physically expensive because every entry stores two complete Memory IDs.

The logical target remains a set of canonical (MemoryId, MemoryId) pairs. Physical compact references may be used only through a local validated dictionary/table that decodes back to those exact IDs.

Do not use another owner's local version number as Dream pair semantic identity.

### Entity Resolution

Entity-resolution history must be divided into:

- accepted semantic decision/proof that later Entity state depends on;
- retry/attempt state needed for deterministic recovery;
- candidate/postimage material that is reconstructible or no longer needed after terminal settlement.

The compact format must retain enough to reproduce the accepted resolution decision and validate its source mention/candidate evidence. It should not retain complete repeated candidate snapshots across attempts when an initial candidate set plus attempt deltas is equivalent.

Before changing this codec, tests must identify which current EntityResolution fields are authoritative evidence versus recovery state. This classification is a required implementation gate, not an invitation to delete fields speculatively.

### Communities and semantic names

Community membership is derived from Graph state, but Community identity is referenced by accepted Freshness/Dream proofs and semantic names may contain non-deterministic inference output.

Therefore this refactor must not blanket-delete Community history.

Target:

- retain any non-recomputable semantic name/history required by accepted behavior;
- retain or reconstruct the exact Community proof identity referenced by accepted Freshness/Dream evidence;
- avoid repeated complete membership snapshots only after a deterministic graph-cut + algorithm-version reconstruction test proves equivalence;
- otherwise leave Community persistence unchanged in the first migration.

Community compaction is a later domain phase, not a prerequisite for the core delta format.

### Vectors

Do not treat additional active compatibility profiles as historical waste.

Current fixture evidence shows most vector bulk comes from legitimate separate profiles.

Memory vectors already bind one vector location per (CompatibilityProfileId, MemoryBodyId) and do not need a history redesign for this project.

Archive vector generations have a future repeated-generation risk because rebuilding a profile can publish another complete fragment matrix. The lossless target, when measurements show repeated same-profile generations matter, is:

- exact immutable vector bytes stored in reusable row/block objects;
- generation state references exact row/block identities;
- changed/new rows add new immutable objects;
- historical generations remain able to recover the exact vectors they used;
- numerically close but byte-different compatible embeddings are distinct immutable values, not silently deduplicated.

Do not key vector bytes only by (profile, fragment) because compatibility profiles permit small numerical drift.

This vector-generation redesign is separable from the core history-delta migration and should not block it.

### Insomnia grouped completion / external Memory publication

Grouped Insomnia completion and external Memory publication are canonical atomic publication carriers. They must not be split into a duplicate Memory publication plus a receipt.

Their compact target is an atomic batch whose Memory creations use the new canonical creation encoding while retaining:

- batch identity;
- attempt/timing/model evidence required by the Insomnia contract;
- exact embedded contiguous global/Memory version range;
- routing metadata;
- Freshness birth evidence;
- external-owner references where applicable.

Fields proven to be retry-only and no longer needed after terminal settlement may move to bounded operational state, but the atomic publication itself remains authoritative.

### Interaction stream checkpoints

Current interaction-stream records repeat the entire cumulative content prefix at each checkpoint.

Target:

    StreamBegin(message/session/parent/role/timestamp)
    StreamAppend(message_id, exact UTF-8 suffix)
    StreamInterrupted(message_id)

or an equivalent append-delta representation.

Requirements:

- each durable checkpoint reconstructs exactly the cumulative text visible after that checkpoint;
- the invariant that displayed streamed text is already durable remains unchanged;
- metadata is immutable after Begin;
- interruption remains terminal;
- reconciliation produces the same latest stream state;
- once a stream is converted into authoritative Archive state, retention of pre-terminal stream recovery deltas is governed by operational-retention policy, not semantic historical-cut policy.

This change must not make incomplete-stream crash recovery weaker.

### External source index

CVAXIDX1 is explicitly derived and may be discarded on damage without invalidating canonical REL semantics. It must not accumulate as permanent semantic history.

Target:

- remove external index revisions from authoritative historical-state guarantees;
- persist only the current recoverable index state or a compact incremental index journal sufficient to rebuild that current state;
- terminally superseded index revisions become reclaimable without semantic prune markers;
- external Memory publications (CVAXMEM1) remain authoritative and are not affected by this rule.

The exact mutable/replaceable derived-storage mechanism is an implementation choice, but it must remain isolated behind ExternalIndexStore.

### Import epochs and Historical Freshness settlements

Do not weaken the semantic-evidence model established by ADR 0041/CVAFRS06.

These records already exist to authenticate historical import attribution. Optimize their encoding only after all current import/reconciliation work is integrated and verified.

The compact-history migration must preserve:

- exact ImportStart/ImportComplete identities and cuts;
- accepted event evidence;
- predecessor settlement linkage;
- replacement cohorts/records;
- semantic digests;
- legacy normalization behavior.

No physical-prefix authority may be reintroduced.

### Echo and project-history correlation

Echo is source/history evidence, not disposable cache. Project revision correlations are durable cross-system historical references.

Retain semantic content and ordering exactly. Physical packing/compression can reduce their cost; no logical delta redesign is required unless future measurement identifies repeated postimages.

## Version and Publication Framing

The audit found material overhead in global tickets plus domain-local publication/link records. Reducing that overhead is allowed, but the compact format must preserve ADR 0003.

A compact publication may physically co-frame:

- global version;
- transaction time;
- owner/domain tag;
- owner-local version;
- mutation payload or backing-object reference;

provided that this is only a physical publication envelope.

It must **not** create:

- a CVA semantic transaction aggregate;
- a global parent chain;
- a requirement that unrelated owners publish together;
- a replacement for owner-local dense clocks.

Crash semantics must continue to distinguish inert/unpublished backing data from accepted semantic publication.

This framing change should be implemented only after one representative owner (Memory) proves the pattern.

## Historical Reconstruction Test Seam

Add one internal/test-only canonical historical projection seam rather than exposing delta internals through public APIs.

The seam must be able to produce deterministic fingerprints for:

- Archive state at every Archive version;
- Memory state/revision history at every Memory version;
- Entity state at every Entity version;
- Relationship/Observation state at every local version;
- full Graph and Memory Graph state at every relevant Graph version;
- Vector Generation activation at every vector version;
- Ego state at every Ego version;
- Freshness at every accepted REL activity/event cut;
- other authoritative owner cuts added by this refactor.

For a CVA-global version G, the whole-owner oracle resolves each semantic owner according to the existing version-history rule: latest visible owner state at or before G. The oracle records per-owner cuts. It does not treat G as ancestry.

The oracle is a verification tool, not a new persisted manifest.

For clock-neutral records, the test seam fixes an accepted source snapshot and uses the existing replay scanner's post-observation global high-water mark: frames are visible through the last frame whose observed high-water is at most G. This boundary is a mechanical replay prefix, never a new ticket assigned to Ego, Dream, processing or Freshness. Later clock-neutral appends at the same global head change that head's snapshot; pre-ticket recovery writes for a subsequent publication can also extend the earlier prefix. The source must therefore be fixed before recording historical fingerprints; G alone does not freeze an evolving file. Within a grouped publication whose embedded version range crosses G, individually clocked records are selected by their own retained global/local coordinates, while the clock-neutral prefix stops before the grouped frame. Prefix byte offsets and disposable copies are excluded from semantic fingerprints. Validated rewrite-generation, genesis and Graph baseline identities are included as semantic proof state.

## Migration Strategy

### Persisted-data compatibility boundary

Existing REL/PHY files are a real external compatibility obligation.

Use a **staged hard cut with an on-disk compatibility boundary**:

1. Existing V1.0 decoders remain available only for opening/validating and lossless format migration.
2. Define one canonical compact writer format.
3. New compact files use a new container format version so older binaries reject them rather than silently skipping unknown semantic history.
4. Do not dual-write legacy and compact representations.
5. Do not add forwarding/shadow stores whose only purpose is to keep both internal shapes live.
6. Lossless format migration creates a separate destination file, preserves durable owner identity, stable semantic identities, global versions, owner-local versions, transaction times and all retained historical cuts, then reopens and verifies it before replacement.
7. Fresh-history rewrite remains a separate destructive-history operation and must not be used to upgrade format.
8. Conservative reclaim_storage remains semantically preserving but is not silently redefined as the format upgrader.

### V1.0 mutation policy after compact format lands

Do not append compact records to a V1.0 header.

The preferred final state is:

- V1.0 may be opened read-only and migrated;
- mutation returns a typed StorageFormatUpgradeRequired;
- new owners are created in the compact format;
- the explicit lossless upgrader is the only path from V1.0 history to compact history.

If product requirements later demand transparent upgrade, implement it as an orchestrated transformed-copy replacement using the same upgrader; do not introduce mixed-format ordinary writes.

### Migration window

This is an internal storage refactor with a real persisted-data boundary. During implementation:

- old readers stay isolated in codec/rebuild compatibility modules;
- new writers move directly to compact formats;
- individual stores may temporarily have red whole-repo integration while their new codec/rebuild tests are green;
- no temporary dual-writer compatibility layer is permitted;
- integration gates restore full current test baseline.

## Implementation Order Constraints

This specification intentionally does not assign final numbered execution steps; a separate linear plan should do that. It does impose dependency order:

1. Historical reconstruction oracle and fixture fingerprints must exist before changing semantic codecs.
2. Container format-version rejection/upgrade boundary must exist before first compact writer can ship.
3. Memory creation/delta format should be the first postimage owner migrated and becomes the pattern for Entity/Ego/Relationship.
4. Graph and Freshness should be validated as already-delta owners before any encoding-only optimization.
5. Dream preexisting_ids normalization depends on an exact Memory historical-cut projection.
6. Interaction stream and ExternalIndex can proceed largely independently because they are runtime/derived stores.
7. Vector-generation row/block redesign is independent and measurement-gated.
8. Community compaction is proof-dependent and should follow Graph/Freshness historical equivalence work.

## Testing Decisions

### Differential historical oracle

For every authoritative fixture, compare source V1.0 and compact-migrated destination exhaustively where tractable:

- every Archive version;
- every Memory version and every Memory revision;
- every Entity/Relationship version;
- every Graph version represented by source history;
- every vector version;
- every Ego version;
- every accepted Freshness turn/event cut;
- current state.

The comparison must use canonical semantic values, not encoded bytes.

### Whole-CVA global cuts

For every global version in fixture history, or a complete deterministic sample if the largest fixture makes exhaustive comparison unreasonably slow:

- resolve the latest visible state of each semantic owner at/before the global ordering point;
- compare source and compact destination;
- compare selected owner-local cut coordinates;
- compare transaction time availability/values.

Any difference is a migration failure.

### Proof/retry behavior

For compacted proof-bearing domains:

- exact retry remains idempotent;
- same identity with different payload remains a conflict;
- malformed/truncated/forged deltas fail before state mutation;
- missing referenced immutable payload fails closed;
- replay after crash produces either the prior accepted state or the fully accepted new state.

### Reconciliation

Run identical/divergent reconciliation fixtures between:

- V1.0 source and V1.0 source;
- compact source and compact source;
- V1.0 decoded canonical history and compact canonical history where the public reconciliation boundary supports it.

Local compact encoding must not affect semantic conflict decisions.

### Size evidence

Maintain a benchmark artifact that reports per record family:

- source record count/bytes;
- compact record count/bytes;
- immutable payload bytes;
- semantic-history bytes;
- derived/recovery bytes;
- total uncompressed file bytes.

Compression results are reported separately under ADR 0004 so logical-history gains are not conflated with physical compression gains.

### Performance

At each integration gate measure:

- cold reopen;
- warm reopen;
- historical lookup at recent/middle/old cuts;
- migration throughput and peak memory;
- current retrieval parity where vector/Community state is involved.

No historical-size optimization is accepted if it causes unbounded replay/open behavior. If replay becomes material, add derived checkpoints under version-history-plan.md; do not make checkpoints authoritative.

## Out of Scope

- Changing fresh-history rewrite selection semantics.
- Changing pruning eligibility or retention policy.
- Choosing a compression codec or final pack size; ADR 0004 owns that work.
- Introducing whole-CVA semantic ancestry.
- Restore-and-continue/timeline branching.
- User-facing historical-view API design beyond the internal verification seam required here.
- ANN/vector-search algorithm changes.
- Removing legitimate compatibility profiles merely to save space.
- Re-running inference to approximate old semantic state.
- Making Communities authoritative when they are currently derived.
- Replacing source chat/history with Memory summaries.

## Completion Gates

The refactor is complete only when:

1. The compact format is the only writer for new/upgraded REL/PHY files.
2. V1.0 compatibility exists only at the persisted-data read/migration boundary.
3. Older binaries reject the new container version instead of partially opening it.
4. Lossless format migration preserves owner UUID and all stable semantic identities.
5. Global versions and transaction times are preserved exactly.
6. Every owner-local semantic version/cut is preserved exactly or has a formally equivalent mapped representation.
7. Authoritative fixtures pass differential historical-state comparison.
8. Freshness historical cuts and Dream proof validation remain exact.
9. Source Archive turns/content remain byte-identical unless an existing content codec migration explicitly preserves identical decoded bytes/content identity.
10. Vector generations that survive migration recover exact vector bytes and activation state.
11. No derived/recovery store is accidentally included in the semantic historical guarantee without a documented reason.
12. No temporary dual writer, alias, fallback or shadow history implementation remains.
13. Whole repository build/tests return to the accepted baseline at the final integration gate.
14. Arcana is refreshed and used as supporting evidence that old writer paths have no surviving callers; direct search/source verification confirms deletion.
15. Updated storage-format/architecture/invariant documentation names the new canonical record families and compatibility boundary.
16. Size evidence separates logical-history savings from ADR 0004 compression savings.

## Architectural Decision Record

This work is sufficiently durable and format-defining to warrant a new ADR when implementation is accepted.

The ADR should record only the hard-to-reverse decisions:

- authoritative owner history is creation + deltas where state revisions permit it;
- historical reconstruction equivalence is the governing invariant;
- derived/recovery state is not semantic history merely because it is durable;
- V1.0 remains an on-disk compatibility/migration boundary;
- new compact history uses one canonical writer and a container version that older readers reject;
- global ordering/local-clock semantics from ADR 0003 remain unchanged.

Do not put implementation step numbering or codec field layouts in the ADR.

## Related docs

- [Linear execution plan](minimal-lossless-history-storage-linear-plan-2026-10-19.md)
- [Architecture](architecture.md)
- [Storage format](storage-format.md)
- [Versioning and historical cuts](version-history-plan.md)

## Notes

### Why this is not rewrite

Fresh-history rewrite deliberately creates a new history from selected state and may omit old revisions, transactions, source material or derived state according to policy.

Minimal lossless history storage instead changes the representation of ordinary retained history. Its success condition is that historical semantic state remains reconstructible.

### Why this is not compression

Compression can reduce the byte cost of repeated representation without understanding it. This refactor removes repeated representation before compression.

The two compose:

    minimal authoritative logical history
        -> bounded physical packs
        -> optional pack compression

### Why vectors are not the first target

The authoritative fixtures contain multiple legitimate compatibility profiles. Their vector bytes are state, not necessarily garbage. The first structural targets are therefore postimage revision families and redundant historical/proof sets, while repeated same-profile vector generations remain a measured follow-up.

### Current structural evidence

The 2026-10-19 Lexicon/Arcana snapshot is current enough to show the storage work crosses src/container, src/archive, src/memory, src/perception, src/semantic_graph, src/freshness, src/retrieval, src/ego, src/runtime_inference, src/imports, src/facade and their integration tests. The final migration plan must therefore use domain-local implementation gates and a final whole-repository integration gate rather than attempting one monolithic codec replacement.
