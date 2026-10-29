# ADR 0031: Dream-derived community semantic names and user override

Parent index: [Architectural decisions](INDEX.md)

## Status

Accepted and implemented — 2026-09-05.

## Context

Reliquary already derives deterministic owner-local Community membership from the Memory Graph with Leiden scan-and-merge. Those structural IDs are useful for routing, but an opaque `CommunityId` does not tell a person or higher-level context system what the region is about.

Naming is semantic inference, so it does not belong inside Leiden. Conversely, keeping all semantic naming in Warlock would duplicate knowledge of Community membership, Memory vectors, and Dream model routing outside the subsystem that already owns Memory-Web interpretation.

A model also cannot recover useful human semantics directly from an embedding vector. The existing Community retrieval index already derives four sub-centroids per Community, however, and those centroids can deterministically select representative Memories whose actual text can be given to a model.

## Decision

Reliquary persists **Community semantic names** as clock-neutral metadata, with explicit provenance for Dream-generated and user-authored names.

- Leiden and scan-and-merge remain solely responsible for deterministic Community membership. Naming cannot add, remove, move, or relate Memories.
- Dream owns the bounded model inference that generates a semantic name.
- `CommunityStore` owns persistence and validation because the label is metadata keyed to one exact `CommunityId`.
- A semantic name is stored in a separate clock-neutral `CVACNAM1` record rather than inside `CommunitySnapshot`. Naming therefore does not advance Community generation, Graph version, or any semantic/global clock and does not invalidate a `MemoryRetrievalIndex`.
- The persisted name record is authored against one exact-membership `CommunityId`. Continuity across later membership changes is derived separately by the lineage policy in ADR 0032; exact Community identity itself does not change.
- Naming requires a current Community snapshot and one compatible Memory-vector profile.
- The existing four deterministic Community sub-centroids select up to eight representative Memory IDs. Dream targets the two nearest unique vectorized members per sub-centroid with stable Memory-ID tie-breaking; if centroid overlap leaves slots open, remaining vectorized members are admitted by best similarity to any Community sub-centroid before deterministic non-vector fallback.
- Raw centroid or Memory vectors are never sent to the model. Dream sends bounded title/content text from the selected representative Memories.
- The Dream naming contract is versioned independently of the Community clustering algorithm so a later prompt/schema/evidence-policy change can regenerate generated labels without recomputing membership.
- Names are owner-local for REL and PHY. Cross-owner naming is not introduced.
- `CommunitySemanticName.source` distinguishes `Dream` from `User`. User-authored names carry no Dream contract/evidence and are authoritative for that exact `CommunityId`; Dream must not schedule or overwrite them, including after a naming-contract upgrade.
- Name continuity across changed exact membership is governed by ADR 0032: user names follow clear lineage, while Dream names survive only while cumulative Jaccard similarity to their original naming baseline remains at least 0.750.
- Naming does not automatically trigger a full Leiden refresh. Community maintenance cadence remains separate because recomputing the complete partition after every Dream Graph mutation would be unnecessarily expensive.

## Consequences

Reliquary can expose human-readable semantic regions while preserving deterministic structural authority. Ego or Warlock can display or consume the names without needing to reproduce representative selection, provenance precedence, or model prompting.

Historical direct name records remain keyed to the exact Community membership against which they were authored. Current-name resolution may expose one through ADR 0032 lineage without rewriting historical records; ambiguous lineage or material Dream-name drift leaves the current Community unnamed until user or Dream naming supplies a new direct record.

Dream cannot name a Community until its snapshot is current and Memory vectors exist for the selected profile. A user may name a current Community without Dream evidence. Model failure leaves the Community unnamed unless a user name already exists and changes no structural or semantic state.

## Rejected alternatives

### Put model naming inside Leiden

Rejected. Leiden owns graph-structural partitioning and must remain deterministic and model-independent.

### Ask a model to interpret centroid vectors directly

Rejected. Embedding coordinates are not a portable human-semantic representation. Centroids are used only to select representative Memories; the model receives source text.

### Store the name inside `CommunitySnapshot`

Rejected. A model-derived annotation should not force a new partition generation or make retrieval indexes stale when membership is unchanged.

### Keep generated or user-authored Community naming entirely in Warlock

Rejected. Representative selection, Community identity, and the user-over-Dream suppression rule must remain co-located with the persisted exact-membership name record; otherwise the naming worker could overwrite a manual label when host presentation state is unavailable.

## Verification

`src/runtime_inference/dream_community_naming_tests.rs` protects REL/PHY naming, persistence, user-name suppression, representative membership, stale-snapshot rejection, invalid-output rejection, and no-op repeat behavior. `crates/reliquary-dream/tests/community_naming_consumer.rs` directly protects the two-representatives-per-sub-centroid / eight-Memory selection policy. Existing Community tests continue to protect deterministic membership and snapshot generation independently of naming.
