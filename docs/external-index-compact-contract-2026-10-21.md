# Compact derived external index contract

Parent index: [Documentation index](INDEX.md)

## Purpose

ExternalIndexStore owns recoverable search/cursor cache state. Its persisted
revisions are not semantic history. External Memory publications (`CVAXMEM1` legacy, `CVAXMEM2` current),
their retry identity, evidence and accepted Memory effects remain authoritative
and are excluded from this optimization.

## Overview

The selected strategy is a self-contained binary current snapshot per source
namespace. The existing preparation pipeline already produces a complete
validated state and synchronizes it after profile/vector backing. Persisting an
incremental cache journal would introduce predecessor retention and recovery
dependencies without improving this ownership boundary. Older snapshots are
reclaimable; a snapshot remains usable after its predecessors are removed.

The ordinary writer retains the existing expected-revision check and sync
acknowledgement. Reopen selects the latest structurally valid snapshot in
physical order, independently of an earlier revision chain. Generation/revision,
feed cursor, last base/digest, rebuild flag, descriptors, vector routing and term
hashes are preserved. A valid snapshot can restore its namespace after a damaged
record; the damage flag continues to require rebuild for absent namespaces.
Canonical host authorization, hydration/hash checks and Memory publication
validation still decide whether evidence may produce semantic effects.

## Wire and recovery

`CVAXIDX2` has an eight-byte magic, one-byte kind, u64 little-endian body byte
length, body, and SHA-256 over the preceding bytes. Kind 0 is a snapshot. Kind 1
has an empty body and records a derived discard/rebuild barrier. Unknown kinds
or versions, invalid checksum, truncation, trailing bytes and invalid fields
discard cache state rather than invalidate canonical REL semantics.

A snapshot stores namespace once; contract/profile/dimensions, generation,
revision/cursor/last base/digest and rebuild flag follow. Counts and strings use
u32 lengths; integers are little-endian. Each message stores conversation/message
IDs, exact content hash, one of four existing roles, optional principal,
timestamp, visibility and chunks. Each chunk stores end byte position, matrix
hash, row and sorted term hashes. Its start is the preceding end (initially zero)
and its message reference comes from its descriptor. Existing validation already
requires those exact equalities. No host text is persisted in this cache.

The eight-MiB body bound, manifest/message/term bounds, strict flags, UTF-8 and
full-state validation apply. Counts are checked against remaining bytes before
allocation. Vector/profile backing validation remains the existing owner step.

## Reclamation and migration

Reclamation keeps the last current snapshot per namespace after the last
discard/damage barrier, plus that barrier. Superseded current snapshots require
no semantic prune marker or clock. Temporary histories containing legacy
`CVAXIDX1` records conservatively retain their revision chain; canonical offline
conversion removes that chain before runtime legacy retirement.

The private V1 converter reads the validated final derived projection, drops old
index revisions, and writes only surviving current snapshots plus an explicit
discard barrier when required. It preserves authoritative publication history through the existing Memory
carrier converter and leaves source bytes unchanged. Current index records reject in explicit V1 inspection and in
PHY owners.

## Related docs

- [Storage format](storage-format.md)
- [Architecture](architecture.md)
- [Linear execution plan](minimal-lossless-history-storage-linear-plan-2026-10-19.md)

## Notes

This is a disposable index format and current-state retention decision. It adds
no source authority, semantic event log, new public index API or generic mutable
storage framework. Legacy decoding is temporary and is removed after verified
canonical data promotion.
