# External Conversation Index Integration Plan

Parent index: [Documentation index](INDEX.md)

## Purpose

Define the future Reliquary integration mode for hosts that already own a canonical conversation transcript.

This is an **additional integration mode**, not a replacement for Reliquary's native Archive-backed interaction model.

Native Warlock/Reliquary operation may continue to persist source turns in the REL Archive. External-derived mode instead treats the host transcript as canonical and stores only rebuildable derived index state plus identifier-only provenance.

The first target host is Hermes. The design must remain reusable for any framework that can provide stable message identity, canonical hydration, and a durable or reconstructable change stream.

## Overview

Reliquary gains a second source-ingestion posture without changing native Archive ownership. In native mode, source turns remain durable Archive evidence. In external-derived mode, a host-owned transcript is observed through stable references and an asynchronous change feed; Reliquary persists only rebuildable index state and semantic derivations. Both modes share downstream Memory, Dream, Perception, vector, and Ego machinery where their evidence contracts permit it.

The implementation should prefer shared normalized source-processing inputs over host-specific forks. Hermes is the first adapter and validation target, not the schema authority for the feature.

## Ownership model

### Native mode

Existing native ownership remains unchanged:

```text
Warlock / native host
      |
      v
Reliquary InteractionRuntime
      |
      v
REL Archive owns source turns / conversation history
      |
      +--> Insomnia / Memories / vectors / Perception / Ego
```

The REL may own transcript/source bodies because the host has deliberately chosen Reliquary as the source-history authority.

### External-derived mode

For an external framework such as Hermes:

```text
external host
    |
    +--> canonical transcript
    |        |
    |        +--> durable change feed
    |                 |
    |                 v
    |        Reliquary external-source adapter
    |                 |
    |        ids / hashes / offsets / vectors /
    |        provenance refs / semantic state
    |                 |
    |                 v
    |              REL / PHY
    |
    +<-- hydrate(ref) -------------------+
    |
    +<-- search refs / memory / compaction proposals
```

The external host remains the sole authority for conversation bodies, authorization, routing, lifecycle, deletion, rewind, compaction commit, and backup.

Reliquary is allowed to hydrate source content transiently for indexing/inference. It must not persist a second canonical transcript body merely because it has seen that content.

## Governing invariants

1. **Native mode remains first-class.** Existing Archive-backed source ingestion and Warlock semantics are not removed or weakened.
2. **External mode is reference-backed.** Durable external-source state stores stable source identity, hashes, offsets, vectors, cursors, and semantic derivations, not full host transcript text.
3. **Derived state is rebuildable.** Loss of the external index lane can be repaired from host canonical history without reconstructing conversation bodies from REL data.
4. **Host acknowledgement is not required.** Reliquary indexing failure cannot block the host from committing a chat turn.
5. **No fake Archive provenance.** External messages must not be represented by invented Archive node IDs or synthetic Episode IDs merely to satisfy current REL-local APIs.
6. **No fake source ownership.** An external source reference remains externally owned even when it supports a REL or PHY Memory.
7. **Exact provenance survives without payload duplication.** A Memory may retain stable external message/conversation IDs and content hashes even when source text is absent locally.
8. **Content hashes fence derivation.** If the host message changes, the old derived chunk/vector/reference is stale even if the host reuses the same message ID.
9. **Deletes and rewinds invalidate derived source state, not semantic history by fiat.** Source disappearance changes evidence availability; it does not automatically erase a Memory unless the semantic owner policy says that Memory must be withdrawn.
10. **Cross-owner rules remain unchanged.** REL/PHY ownership, Dream, Perception, Ego, and privacy boundaries do not collapse merely because source evidence came from an external host.
11. **External mode never becomes a second host session database.** Routing, authorization, model state, approvals, billing, and host lifecycle metadata stay outside Reliquary.
12. **Compaction is proposal-based.** Reliquary may propose semantic compaction; only the canonical host commits transcript mutations.

## Required new concepts

The current implementation assumes Archive-backed source identity for many provenance paths. External-derived mode needs an explicit source abstraction rather than overloading Archive records.

### ExternalSourceNamespace

A stable host/source identity, for example:

```text
hermes:<profile-id>
codex:<workspace-or-account-id>
claude-code:<workspace-or-account-id>
```

The exact syntax is adapter-defined, but equality must be stable and explicit. A source namespace is not a REL owner ID.

### ExternalMessageRef

Candidate semantic identity:

```rust
pub struct ExternalMessageRef {
    pub source_namespace: String,
    pub conversation_id: String,
    pub message_id: String,
    pub content_hash: [u8; 32],
}
```

The host's native integer/string message identity is normalized without inventing Reliquary Archive IDs.

### ExternalChunkRef

Derived index rows may point at a range within one canonical external message:

```rust
pub struct ExternalChunkRef {
    pub message: ExternalMessageRef,
    pub start: u64,
    pub end: u64,
}
```

Offsets are defined by the host contract/adapter and must be validated against freshly hydrated canonical content before source text is exposed.

### External source cursor

Each adapter instance persists the last durably applied host change sequence together with source namespace and contract/version identity.

Cursor advancement must be atomic with the corresponding derived index mutation. A crash may cause replay; replay must be idempotent.

### Generalized Memory provenance

Current `MemorySourceRef` is explicitly REL-oriented: it contains owner-qualified Episode and Archive node identities. External-derived mode must not fabricate those fields.

Plan a backward-compatible generalized identifier-only provenance representation, conceptually:

```rust
enum SourceEvidenceRef {
    Reliquary(MemorySourceRef),
    External(ExternalMemorySourceRef),
}
```

where `ExternalMemorySourceRef` can retain the source namespace, conversation/message identities, content hashes, and any exact authority/grounding ranges needed by Insomnia.

Implementation must preserve decoding of current Memory records. The eventual storage-format revision must be documented and migration-tested; do not reinterpret existing `MemorySourceRef` bytes.

## External source adapter boundary

Add a provider-neutral adapter surface that Reliquary can implement against host APIs.

Conceptual operations:

```rust
trait ExternalConversationSource {
    fn source_namespace(&self) -> &str;
    fn feed_floor(&self) -> Result<u64, ExternalSourceError>;
    fn feed_watermark(&self) -> Result<u64, ExternalSourceError>;
    fn changes_after(
        &self,
        cursor: u64,
        limit: usize,
    ) -> Result<Vec<ExternalConversationChange>, ExternalSourceError>;
    fn snapshot_manifest(&self) -> Result<ExternalSnapshotManifest, ExternalSourceError>;
    fn hydrate(
        &self,
        refs: &[ExternalMessageRef],
    ) -> Result<Vec<HydratedExternalMessage>, ExternalSourceError>;
}
```

The exact transport may be in-process, plugin callback, local RPC, or another adapter-specific mechanism. Reliquary core should consume normalized source semantics, not Hermes-specific Python objects.

## Derived index storage

External-derived indexing needs durable state distinct from Archive source history.

Do **not** insert hydrated external message bodies into:

- Archive nodes;
- Archive content objects;
- Episodes;
- Fragments; or
- Echo source evidence

merely to reuse current native retrieval machinery.

The new lane should persist only what is necessary to rebuild/route semantic work, including:

- `ExternalMessageRef`;
- chunk offsets;
- content hashes;
- vector bindings;
- adapter/source namespace;
- index contract/version;
- indexed/reconciled state;
- source cursor/watermark; and
- provider-specific derived metadata that is not a copy of source text.

Physical vector storage may reuse `PackedVectorStore`. Source-to-row bindings should use a typed external-source binding rather than pretending an external chunk is an Archive `FragmentId`.

## Hydration and transient processing

Source text is fetched only when a bounded operation needs it.

Examples:

- embedding a newly changed message;
- answering a provenance/source drill-down;
- running Insomnia extraction over a bounded external turn window;
- building a compaction proposal; or
- validating a stale search reference.

Hydrated content is transient process memory unless it becomes a different legitimate semantic owner, such as a Memory body produced by Insomnia.

A Memory body is not considered duplicate transcript storage: it is a derived semantic proposition with its own ownership/lifecycle. The source transcript body itself remains external.

## Search contract

Reliquary external-source search returns source references plus scores/metadata, not source text.

Conceptually:

```text
query
  -> external derived vectors / routing
  -> ExternalChunkRef[]
  -> host/core hydration + authorization
  -> source text
```

For Hermes, the Hermes core performs the final authorization/hash/range validation before text reaches the caller.

Reliquary may separately return normal Memory/Entity/Observation results from REL/PHY. Those are Reliquary-owned semantic objects and are not hydrated from the host transcript.

## Insomnia in external-derived mode

Existing Insomnia is Archive/Episode-oriented. Reusing it by copying host conversations into Archive would violate the no-duplicate-transcript goal.

The future external path should therefore separate **semantic extraction input** from **Archive persistence**.

Target direction:

1. the adapter consumes host changes;
2. it groups bounded relevant external turns transiently;
3. canonical bodies are hydrated only for the processing window;
4. the existing extraction/classification machinery receives a normalized transient episode/view;
5. resulting Memories are published to REL/PHY through normal ownership routing;
6. provenance is stored as generalized external source references plus source chronology; and
7. hydrated source bodies are discarded.

Do not fork the actual proposition extraction semantics into a Hermes-specific Insomnia implementation. Extract a shared normalized processing input where necessary.

Native Archive-backed Insomnia continues using durable Episodes.

## Dream, Perception, and provenance

Derived Memories from external sources participate in Dream and Perception according to the same Memory authority rules as native-derived Memories.

Differences are limited to source recovery:

- source chronology comes from canonical host metadata captured/validated during processing;
- source evidence lookup may require external hydration;
- an unavailable host source can leave a Memory's source reference unresolved without invalidating the Memory automatically;
- content-hash mismatch marks source evidence stale and triggers reconsideration where the semantic subsystem requires it; and
- external source identity must not be treated as an owner-local corroborating REL Episode.

PHY privacy rules remain unchanged. An external reference carried into PHY remains identifier-only.

## Edits, rewind, deletion, and staleness

External change events must drive deterministic derived-index invalidation.

### Edit/content replacement

Same host message ID + new content hash:

- invalidate old chunks/vector bindings;
- index the new canonical content;
- retain historical derived semantic state only according to normal Memory revision/lifecycle policy;
- mark provenance evidence bound to the old hash stale.

### Rewind/inactive transition

- remove or deactivate source index rows no longer visible;
- stale old search refs;
- reconsider semantic observations only where policy requires;
- do not blindly delete independent Memories.

### Conversation deletion

- tombstone/remove external index rows for that conversation;
- preserve semantic objects whose owner policy allows them to outlive missing evidence;
- make provenance resolution report source unavailable/deleted rather than inventing a replacement source.

## Rebuild protocol

Rebuild must be normal operation.

The host supplies a snapshot manifest containing stable refs, hashes, visibility, and a feed watermark but no duplicate transcript database.

Reliquary:

1. creates a fresh derived index generation;
2. hydrates manifest refs in bounded batches;
3. verifies content hashes before indexing;
4. installs the completed generation atomically;
5. records the snapshot watermark; and
6. resumes incremental changes after that watermark.

If the adapter cursor falls behind the host feed floor, incremental processing stops and rebuild is required.

## Semantic compaction service

External-derived mode may provide Reliquary compaction without owning the transcript.

This should be a separate optional service from indexing.

Input:

- canonical conversation identity;
- host-provided snapshot/fingerprint;
- bounded hydrated transcript/context;
- optional Reliquary Memories/Entities/Observations/Ego context;
- host compaction constraints.

Output:

```text
CompactionProposal
  summary
  compacted source message refs
  preserved tail refs
  semantic/provenance metadata
  source fingerprint
```

The host validates the fingerprint/active set and performs its own normal canonical compaction mutation.

Reliquary never:

- marks host messages inactive directly;
- creates host child sessions;
- rewinds/deletes host history; or
- treats proposal generation as a durable transcript commit.

If Reliquary is unavailable, the host can use native compaction or defer according to host policy.

## Relationship to existing MemoryProvider-style integrations

An external framework may already have a turn-time memory provider hook.

For Reliquary derived-index mode:

- the durable source-ingestion authority is the host change feed, not a per-turn `sync_turn` callback;
- turn hooks may still provide recall, UI indicators, or bounded immediate semantic behaviour;
- the same completed turn must not be durably ingested once through a hook and again through the feed; and
- adapters should make replay/idempotency identity derive from host source/change identity.

This distinction is important for Hermes, whose current memory-provider API includes `sync_turn`.

# Phase tracker

| Phase | Status | Completion gate |
| --- | --- | --- |
| R0. External source contracts | Implemented on P5-G | Stable ref/cursor/hydration model accepted |
| R1. Reference-only derived index | Implemented on P5-G | Host messages index without durable text duplication |
| R2. Search + rebuild | Implemented on P5-G | Lost index rebuilds and returns validated refs |
| R3. External provenance + Insomnia input | Implemented on P5-G | Memories derive from transient host evidence without Archive copies |
| R4. Lifecycle invalidation | Implemented on P5-G | Edit/rewind/delete semantics are deterministic |
| R5. Hermes adapter | Planned after Hermes feed API | End-to-end async indexing works |
| R6. Semantic compaction proposals | Later | Hermes can consume proposals without storage ownership transfer |
| R7. Additional framework adapters | Later | Second adapter proves contract is not Hermes-specific |

## Implemented framework-neutral R0–R4

The P5-G external core implements normalized host contracts, synchronized reference-only source snapshots, hash-bound lexical/vector search and rebuild, external Memory provenance, shared transient Insomnia input and deterministic edit/visibility/delete invalidation. Current ownership, API, formats and bounds live in [Architecture](architecture.md), [Rust API](api.md), [Storage format](storage-format.md) and [Current limitations](current-limitations.md).

[External-core acceptance](multiplexing-p5g-external-acceptance-2026-10-08.md) records branch scope, replay/reopen/corruption/privacy/lifecycle evidence, recipient recovery and verification limits. Host-specific adapters, compaction proposals and external polling remain future work below.

## R5 — Hermes adapter

Build the adapter only after the Hermes contract is stable.

Responsibilities:

- consume Hermes feed floor/high-water/changes;
- hydrate stable message refs through Hermes core;
- map Hermes profile identity to one external source namespace;
- persist cursor atomically with Reliquary derived state;
- expose reference-returning search to Hermes;
- keep Hermes canonical text out of REL external-index storage; and
- coexist with normal Reliquary Memory/Perception functionality.

The adapter may live as a separate package/crate/module if that keeps Hermes-specific transport out of Reliquary core.

## R6 — Semantic compaction proposals

Add proposal generation after reference indexing and provenance are proven.

Use exact host snapshot identity and bounded hydration. Return a proposal; do not commit host transcript changes.

Quality tests should compare native Hermes compaction and Reliquary semantic compaction on retained decisions/constraints/provenance, while correctness tests focus on stale-proposal rejection and no lost turns.

## R7 — Additional framework adapters

A second host integration is the portability test.

The shared contract is successful only if a non-Hermes adapter can implement:

- stable source identity;
- change replay or rebuild;
- canonical hydration;
- reference-returning search; and
- optional proposal services

without adopting Hermes-specific lifecycle types.

## Cross-repository acceptance matrix

Hermes + Reliquary integration is not complete until all of these pass:

1. **Reliquary unavailable at Hermes startup:** Hermes chat/save/resume/rewind/delete/backup continue.
2. **Reliquary dies after Hermes commit:** on restart, exactly one derived entry exists for exactly one canonical message.
3. **Index destroyed:** rebuild from Hermes produces the same active source reference set.
4. **Message edited:** old content-hash refs stop resolving; new content indexes once.
5. **Message rewound/deleted:** old source refs no longer hydrate through Hermes.
6. **Profile isolation:** no REL external index for one Hermes profile can hydrate another profile's transcript.
7. **No duplicate source body:** serialized REL external-index state contains no canonical Hermes message text.
8. **Memory provenance:** derived Memory can identify exact Hermes source evidence even though that evidence body is absent from REL.
9. **Canonical-owner composition:** one submitted turn yields one Hermes canonical message, one feed mutation, and one Reliquary derived index update.
10. **Compaction proposal:** stale source fingerprint is rejected by Hermes; current proposal commits through Hermes and is then observed back through the feed.

## Definition of done

External-derived mode is complete when Reliquary can attach to a host-owned transcript, maintain a crash-safe rebuildable semantic/vector index without copying canonical transcript bodies, create normal REL/PHY semantic state with exact external provenance, survive source edits/deletes/rebuilds, return source references rather than unauthorized text, and optionally provide semantic compaction proposals while the host remains the sole conversation mutation authority.

Native Archive-backed Reliquary operation remains fully supported and semantically unchanged.

## Related docs

- [Roadmap](roadmap.md)
- [Architecture](architecture.md)
- [Architectural invariants](invariants.md)
- [Storage format](storage-format.md)
- [Rust API](api.md)
- [Operator integration guide](manual/integration-guide.md)
- [Perception subsystem plan](perception-subsystem-plan.md)
- [Chronos subsystem plan](chronos-subsystem-plan.md)

## Notes

Implemented R0–R4 facts have moved to canonical current-state documents. This plan retains host-specific adapters, proposal services and outstanding integration work; the acceptance record preserves the implemented core's evidence. The Hermes adapter must not become a hidden dependency of native Reliquary operation.
