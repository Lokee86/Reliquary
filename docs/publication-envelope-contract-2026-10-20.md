# Compact publication envelope — execution contract

Date: 2026-10-20
Status: Step 167 contract accepted against `b043494`; Memory runtime admission and ordinary writing accepted through Step 177. Offline Memory conversion and Entity/Relationship envelope codecs, consumers, ordinary writing and offline conversion are implemented and in verification. Archive contract Step 200 accepted against `2597c73` and body codecs Step 201 against `24572e4`; Archive replay/consumers/writing/conversion, Graph framing and final promotion remain pending.

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

## Entity envelope migration boundary

The Entity inventory contains 1,141 accepted version links across the six pinned inputs. Ordinary compact revisions still append a carrier, standalone global ticket and `CVAENTV1` link. Entity envelope migration therefore removes actual framing. Entity retains sole authority over global UUID, ID, revision, local sequence, mutation identity, description availability and merge/direct terminal semantics. Observation is not a separate persisted owner.

Entity bodies use `CVAENC02` for creations and `CVAEND02` for deltas. They preserve the corresponding `01` field order except for the omitted global integer, which comes exclusively from the envelope. Optional authenticated description proof retains its two digests and checksum; the checksum covers the new body bytes. Direct terminal bodies use `CVAETD02` with ID, revision and Entity version; merge terminal bodies use `CVAETM02` with the same fields followed by replacement ID. All integers remain little-endian. Terminal bodies retain no physical reference. Unknown Entity body versions, zero local/global coordinates, invalid base/revision/mask, malformed optional values, invalid proof lengths/checksums and trailing bytes reject. A body alone is inert; only a validated V2 envelope admits it. No new semantic clock or public delta API is introduced.

Creation/delta admission uses the existing Entity candidate validation and pruning authorization. Direct and merge tombstones use their existing distinct terminal checks, including replacement-chain semantics. Accepted local order stays dense even when reserved globals decrease. Standalone reservations retain their original timestamp and allocation slot. Converters place BindReserved envelopes at original version-link slots and remove accepted carriers only after retained references are accounted for; unlinked backing remains inert. Ordinary writers use allocation envelopes only after preflight validation, with retries consuming no bytes or clocks.

The concrete acceptance boundaries are:

| Responsibility | Existing concrete consumers | Required acceptance |
| --- | --- | --- |
| Codec | `entity_compact_codec`, `entity_codec`, `entity_compact_contract` | Literal fields, proof checksums, malformed/unknown versions, both terminals |
| Replay | `EntityOpenState::ingest/finish`, `EntityStore::replay_publication` | V2-only body admission; dense local ordering; exact terminal/retry rules |
| Historical projection | Entity historical fingerprint and whole-owner adapters | All local/global/time cuts, including reverse globals |
| Reconciliation/import | `cva_reconcile_entity`, `imports::reconciliation_interval`, completed-epoch inventory | Original binding slots/times and normalized evidence; strict unsupported-frame handling |
| Prune proof | `history_prune_manifest::owner_history_digest`, Entity description-proof/prune modules | Canonical comparison authority, exact retained descriptors and tamper rejection |
| Reclamation | `ReclamationScan`, `storage_reclamation::relocate_payload` | Accepted backing/proof references survive; orphan backing stays inert |
| Native copies | REL/PHY repack, `migration_rel`, `migration_phy`, semantic rewrite | Exact history for copies and explicit policy effects for rewrite |
| Ordinary publication | `entity_compact_publish`, direct and merge tombstone paths in `entity_store` | One envelope, preflight/no-op/retry atomicity, incomplete-tail recovery |
| Offline conversion | `storage_upgrade::entity`, staged-copy coordinator | Source identity/hash unchanged; owner/global/time/proof parity |

Implementation must close each boundary before writer cutover; the shared physical codec is not evidence that any of these consumers already supports Entity bodies. Ego has no removable global/link framing, so no Ego envelope is proposed. Archive and Graph applicability remains separately gated by exact inert-backing and local-history equivalence.

## Relationship envelope migration boundary

Relationship owns creation, delta and direct terminal semantics. It has no optional UUID or merge terminal. The pinned six-source corpus contains no accepted Relationship links; those inputs establish zero applicability rather than runtime codec coverage. Synthetic accepted histories must exercise every path.

Bodies use `CVARNC02` and `CVARND02`, preserving the `01` field order and all canonical participant/evidence collection operations, except that the global integer is supplied only by the envelope. Optional description proof and its checksum cover the new body bytes. `CVARTD02` stores Relationship ID, revision and local Relationship version as little-endian values. The envelope supplies its global coordinate. No body contains a carrier reference. Unknown family versions, zero coordinates, invalid revisions/masks/collections, malformed proof checksums, truncation and trailing bytes reject. Replay must preserve direct terminal authority and dense local sequence under reverse reserved globals.

The implementation gates remain independent: codec (`relationship_compact_codec` and values); replay (`RelationshipOpenState`); historical cuts/fingerprints; reconciliation range and completed-import inventory; authenticated description pruning and owner-history digest; reclamation liveness and relocation; REL/PHY native copies and explicit semantic rewrite policy; ordinary revision/direct terminal writers; and offline accepted-link conversion at original marker slots. Existing raw carriers remain inert unless accepted. Writer cutover waits for those consumers, and a zero-link authoritative corpus cannot substitute for adversarial synthetic parity.

## Archive envelope contract and repeated bindings

Archive has 32,357 accepted links in the three pinned REL files and zero in PHY. The [family inventory](../benchmarks/archive-publication-family-inventory-2026-10-20.json) resolves complete referenced carriers, excludes content backing/format/grouped owners, and records unchanged hashes. The corpus has no repeated references; synthetic repeated-binding coverage is mandatory because current Archive replay accepts them without applying the semantic record again.

Each applying Archive body has exactly one eight-byte family magic, then an explicit nonzero little-endian `archive_version: u64`, then the canonical semantic fields below. Global comes only from the physical envelope. Do not derive local sequence solely by counting envelopes, add another owner tag, nest an old carrier magic, or splice an encoded legacy carrier into a runtime body. Share actual field encoders/decoders under their existing concrete codec owners.

| Native body magic | Canonical fields following the common local coordinate | Concrete codec owner |
| --- | --- | --- |
| `CVANOD02` | timestamp, content ID, node/conversation/parent/role/principal fields of current Node | `archive_codec` |
| `CVABRC02` | canonical bool, branch/conversation/leaf IDs | `archive_codec` |
| `CVACON04` | conversation ID, optional title, active flag, policy, nonzero policy revision | `conversation_metadata_codec` |
| `CVACTM02` | conversation ID and exact node/branch/fragment/episode/file/metadata-version closure | `archive_codec` |
| `CVAFRA02` | fragment ID, conversation/start/end node IDs | `archive_codec` |
| `CVAEPI02` | episode ID, origin/boundary tags, source-through/finalized times, conversation/start/end node IDs | `episode_codec` |
| `CVAFIL02` | file/content IDs, byte length, filename, optional MIME | `archive_codec` |
| `CVATUR03` | current Node fields followed by the exact ordered attachment bundle | `turn_ingest_codec` |
| `CVAIMP02` | validated current ImportReceipt JSON model and exact source/revision fingerprints | `archive_import` |
| `CVAFME02` | file ID and Memory ID | `file_memory_link_codec` |

Legacy Node/turn absence of principal stays `None`; legacy metadata keeps the existing decoded inactive/policy/revision defaults. Conversion canonicalizes those model values into these native fields. It does not retain legacy body decoders inside new envelopes. A tombstone has one local coordinate in the common prefix and no duplicate semantic global; its model receives the envelope global and its explicit local, and existing exact closure validation remains mandatory. An IngestedTurn remains one atomic publication. Content `CVACONT1`, format `CVAAFMT2`, legacy links and grouped owner frames are never applying Archive bodies. Known unsupported versions, malformed flags/tags/lengths/strings, zero coordinates and trailing data reject.

`CVAARP02` is the separate Archive repeated-binding body: common explicit local followed by `first_applying_archive_version: u64`. It must name an earlier applying publication, never itself, a future local, a gap or another repeat. It consumes one local/global history cut and never reapplies the record. This retains the old accepted repeated-carrier behavior, including repeated terminal bindings. Historical/evidence readers resolve the first applying record with its original coordinates; current state keeps the original application. No new ancestry or clock is introduced. Offline conversion maps every repeated source reference to that first accepted local; it must not duplicate and reapply a semantic body. Native ordinary writers need not manufacture repeats.

Archive retains its existing dense local sequence and strictly increasing publication globals; physical reserved clocks do not authorize a decreasing Archive sequence. Prepare the next local coordinate before any semantic state mutation. New `ArchiveRecordVersion.record` references point to the envelope itself. Container reads remain raw: concrete Archive readers resolve applying/repeated publications explicitly. Raw content and unaccepted record-shaped backing stay inert. Only accepted carriers are removable during conversion, after every reference is accounted for.

The concrete consumer inventory is `ArchiveOpenState::ingest_version/apply_record`; `Archive::branch_at/record_version` and Archive/current historical materializers; `cva_reconcile_archive` ranges/receipts and import interval/evidence inventory; `history_prune_manifest::owner_history_digest`, Archive content/conversation pruning and terminal retention; `ReclamationScan` dependency liveness and physical relocation; REL attachment/native repack and rewrite/migration; and every `Archive::publish_record` caller (Node, Branch, metadata, tombstone, Fragment, Episode, File, IngestedTurn, ImportReceipt, FileMemoryLink). Each has an explicit gate in the linear plan. No writer cutover precedes these gates. Native copy must retain content dependencies, repeated anchors, IDs, all cuts and times; semantic rewrite keeps its explicit discard policy. Conversion binds at original marker slots with unchanged standalone reservations, canonical fields, proofs/receipts and source identity/hash/mtime. Per-family measured output bytes, including chunk framing, are required before any savings claim or promotion.

## Measurement and acceptance

Report actual before/after framed bytes per migrated owner, keeping grouped payloads and unclaimed tickets separate. An allocation header with time costs 25 bytes; an allocation without time or reserved-binding header costs 17 bytes; any predicted saving additionally depends on the domain body and removed link. These sizes are contract arithmetic, not measured output savings. Accept the runtime change only after literal/corruption tests, clock/recovery tests, owner historical parity and exact offline migration evidence pass. The existing [linear plan](minimal-lossless-history-storage-linear-plan-2026-10-19.md) owns progress and integration gates.

## Notes

Reserved binding is required by accepted delayed publication histories; it is not a legacy-file decoder. Global allocation and semantic publication remain separate facts. Broader owner migrations must expose their concrete failure domains before cutover.

## Related docs

- [Storage format](storage-format.md) — current admitted runtime records.
- [ADR 0003](decisions/0003-layered-version-clocks-and-local-ancestry.md) — clock and ownership policy.
- [Linear execution plan](minimal-lossless-history-storage-linear-plan-2026-10-19.md) — acceptance and integration order.
- [Execution evidence](minimal-lossless-history-storage-execution-2026-10-19.md) — measured fixtures and verification.
