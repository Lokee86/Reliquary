# Compact publication envelope — execution contract

Date: 2026-10-20
Status: Step 167 contract accepted against `b043494`; runtime admission and writing remain pending subsequent explicit gates.

## Overview

Combine physical allocation/binding metadata with a concrete owner publication while preserving all accepted global/local/time cuts. This is an execution contract; runtime implementation and acceptance remain in the linear plan.

## Purpose

Reduce duplicate ticket/link framing without changing semantic ownership, deferred publication visibility or native import scope.

## Ownership and scope

[ADR 0003](decisions/0003-layered-version-clocks-and-local-ancestry.md) remains controlling. Container owns physical global ordering and transaction time. Memory, Entity, Relationship, Graph and Archive retain their local clocks, semantic admission and references. Ego has no global ticket; wrapping its local sequence would add bytes and is not justified. Existing Insomnia and external-source grouped publications remain atomic owner transactions with their current embedded ranges; do not turn their members into additional individual publications.

The [measured inventory](../benchmarks/publication-framing-byte-inventory-2026-10-20.json) distinguishes separate global tickets, domain links and grouped ranges. It does not establish that every owner should migrate. The four unclaimed standalone tickets remain allocated global positions; conversion must preserve them rather than closing gaps or inventing semantic publications.

## Physical wire contract

One Container chunk may carry one `CVAPUB01` envelope:

| Field | Encoding | Responsibility |
| --- | --- | --- |
| Magic | eight ASCII bytes `CVAPUB01` | Physical record version |
| Global version | little-endian u64, nonzero | Container global order |
| Clock tag | u8: 0 allocates without time; 1 allocates with time; 2 binds an earlier standalone reservation | Explicit physical allocation/binding mode |
| Transaction time | little-endian i64, present only for tag 1 | Exact source time, including zero/negative values |
| Owner body | remaining bytes, at least an eight-byte owner magic | Opaque to Container; interpreted by its concrete owner |

No owner tag, local clock, semantic ID, ObjectRef, body-length field or checksum is duplicated in this physical header. Existing Container length framing bounds the complete chunk. Unknown envelope versions or clock tags, truncated headers/timestamps, zero global versions and absent/short owner bodies reject. Tag 2 carries no duplicate timestamp: the retained standalone reservation owns that association. An envelope cannot directly contain another envelope. Codec validation alone does not grant a semantic publication: the concrete owner must recognize and admit the body.

Owner bodies used by a migrated domain must omit the global integer supplied by the envelope. Their versioned owner codecs retain local sequence, revision/delta base, stable identity, terminal state and physical references as applicable. They construct the same accepted semantic record using the envelope global coordinate. A legacy carrier plus its link is not silently reinterpreted as a new body. Each owner cutover must specify its own new body format and fail-closed version boundary before implementation.

## Allocation and deferred binding decision

Source ticket allocation and owner marker binding are separate physical events. Existing Memory and Relationship tests admit local successors bound to earlier reserved global positions, including local versions 1/2 at globals 2/1. Moving a binding to its ticket can cross an import interval boundary or introduce forward physical references. Preserve the original semantic binding slot and ticket chronology; do not reorder historical authority by global number.

Use one canonical envelope format with explicit physical clock modes. Tags 0/1 allocate the exact next global position at the envelope. Tag 2 binds a previously observed standalone reservation, allocates nothing and uses that reservation's original optional transaction time. A tag-2 binding to a future/nonexistent reservation, another envelope allocation or a grouped range rejects. A second envelope binding of the same reservation rejects. A semantic publication cannot also reuse that position through another owner; existing whole-owner global uniqueness remains required. Replay must explicitly track standalone reservation provenance and whether an envelope has consumed it; `global < head` or an entry in the timestamp map alone is insufficient. These are physical reservation semantics, not a V1 decoder or a second semantic authority.

Ordinary new writes use allocation tags. The initial offline converter uses tag 2 at the original accepted owner marker slot and retains its original standalone ticket. It resolves source accepted carrier facts through the owner, encodes the canonical body and relocates references. It may remove obsolete accepted carrier framing only after accounting for every retained reference; unmarked source backing stays inert. No adjacency optimization or ticket/publication reordering is part of this converter gate. Import physical proof normalization remains source-derived before offsets change: authenticate the original source prefix/receipt first, then use the existing relocated-proof flow to bind the transformed physical prefix, retaining the original authority rather than rewriting it.

## Clocks, append and recovery

Container observes allocation or reserved binding from the physical clock tag without reading owner fields. Allocated positions remain contiguous in physical allocation order; duplicate, missing, reversed and overflowing allocations reject under the existing clock rules. Deferred bindings retain their explicit earlier positions and are never silently treated as fresh allocations. Missing timestamps stay missing. Timestamp-cut queries retain contiguous-prefix and nonmonotonic-time behavior.

An ordinary owner prepares and validates its body before allocating/persisting the envelope. The physical append consumes one global position and returns its actual ObjectRef. Owner acceptance follows the existing append/sync boundary. No-op/retry decisions occur before append and consume no global position. A failed/partial tail must not advance admitted state; complete but semantically invalid owner bodies fail owner reopen. A tag-0/1 envelope must not also have a standalone ticket for its allocation; a tag-2 envelope requires that earlier reservation. Offline conversion supplies original coordinates and optional times without calling ordinary semantic publication APIs.

This makes the accepted publication self-contained with respect to its binding, while immutable referenced body/backing records keep their existing inert-until-accepted semantics. Physical copy/relocation must rewrite references through the concrete owner codec. Do not use transparent Container read unwrapping: raw bytes/ObjectRefs remain raw physical bytes, and semantic readers opt into the explicit envelope/body seam.

## Required owner gates

Memory's current writer includes creation, delta and tombstone paths. Its current replay binds compact carriers through `CVAMEMV1`; replacing that link touches independent consumers. Before ordinary Memory writer cutover, independently implement/verify:

1. Container envelope clock/timestamp admission and partial-tail behavior.
2. Memory body codecs, local sequence/delta-base/terminal admission and explicit V1/V2 boundary.
3. Historical Memory/global cuts, transaction times and grouped-publication coexistence.
4. Reconciliation range reads, import interval coordinates, completed-import evidence and strict unsupported-frame classification.
5. Native prune history digest/authenticated-source interpretation.
6. Reclamation liveness and concrete reference relocation.
7. REL/PHY repack/native copy and semantic migration behavior.
8. Ordinary creation/delta/tombstone writer/no-op behavior.
9. Offline conversion of every accepted direct publication, preserving original clocks/times and inert orphan backing.

The corresponding concrete consumers are `container_version::observe_version_payload_state`, `MemoryOpenState::ingest/finish`, `cva_reconcile_memory::read_memory_range`, `imports::reconciliation_interval::replay_frame`, completed-epoch `inventory`, `history_prune_manifest::owner_history_digest`, `ReclamationScan`, `storage_reclamation::relocate_payload`, and native repack/migration coordinators. Existing grouped Insomnia and external-source decoders remain their own atomic authorities. Other owner cutovers must inventory their actual consumers and insert explicit owner-specific rows before implementation; an envelope codec gate cannot stand in for those responsibilities.

## Measurement and acceptance

Report actual before/after framed bytes per migrated owner, keeping grouped payloads and unclaimed tickets separate. An allocation header with time costs 25 bytes; an allocation without time or reserved-binding header costs 17 bytes; any predicted saving additionally depends on the domain body and removed link. These sizes are contract arithmetic, not measured output savings. Accept the runtime change only after literal/corruption tests, clock/recovery tests, owner historical parity and exact offline migration evidence pass. The existing [linear plan](minimal-lossless-history-storage-linear-plan-2026-10-19.md) owns progress and integration gates.

## Notes

Reserved binding is required by accepted delayed publication histories; it is not a legacy-file decoder. Global allocation and semantic publication remain separate facts. Broader owner migrations must expose their concrete failure domains before cutover.

## Related docs

- [Storage format](storage-format.md) — current admitted runtime records.
- [ADR 0003](decisions/0003-layered-version-clocks-and-local-ancestry.md) — clock and ownership policy.
- [Linear execution plan](minimal-lossless-history-storage-linear-plan-2026-10-19.md) — acceptance and integration order.
- [Execution evidence](minimal-lossless-history-storage-execution-2026-10-19.md) — measured fixtures and verification.
