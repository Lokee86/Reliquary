# Compact Entity Resolution history contract

Parent index: [Documentation index](INDEX.md)

Status: implemented current writer and replay for the Entity Resolution phase; execution
acceptance is recorded separately in the linear plan.

## Purpose

Preserve resolution decisions, candidate evidence, retry suppression, retention
and import readiness while avoiding repeated unchanged candidate/evidence
postimages. EntityResolutionStore remains the concrete owner in REL and PHY.

## Overview

The target is literal initial evidence followed by exact changed-field records.
All accepted decisions and retry/retention facts remain reconstructible in order.

### Field classification

| Persisted field | Classification and reason | Retention rule |
| --- | --- | --- |
| Memory ID, text field, start/end byte positions | Semantic mention identity; validated against historical routing/Memory evidence | Literal identity in every change; never reconstructed from current edited text |
| Resolution revision | Owner-local admission/retry order; reset after the existing tombstone operation | Preserve exact u32 value and contiguous live-key sequence |
| Updated timestamp | Decision/maintenance chronology | Preserve exact i64 per change |
| Status discriminator | Decision and pending/dormant lifecycle authority | Preserve each accepted transition |
| Resolved Entity ID and reason | Accepted decision/retarget evidence and readiness | Preserve literal values |
| Rejection reason | Terminal classification and rejection retention | Preserve literal reason |
| Candidate Entity IDs | Inputs to historical retry/retarget and candidate wake indexes | Preserve exact ordered set; inherit only byte-identical prior accepted values |
| Candidate-set fingerprint | Retry suppression and invalidation identity; retarget may explicitly zero it | Preserve exact bytes; never recompute from current Entities |
| Context fingerprint | Retry suppression identity | Preserve exact bytes |
| First-seen timestamp | Pending/dormant retention boundary | Preserve exact value, including any accepted change |
| Last-attempt timestamp | Retry-time validation and retention boundary | Preserve exact value |
| Attempt count | Retry evidence, bounded u16 counter | Preserve literal changed value; do not infer an increment |
| Dormant timestamp | Dormant lifecycle/retention boundary | Preserve exact value when present |

The pending-by-Entity reverse index and maintenance key set are reconstructed
projections and have no persisted authority to remove. Candidate IDs and both
fingerprints are not dispensable merely because they can resemble a snapshot.

## Existing behavior

Ordinary writes keep their current expected-revision, mention, live Entity,
terminal-state and retry checks. Identical resolved/rejected states and unchanged
retry fingerprints remain no-ops. A retry may wake Dormant to Pending, preserves
first-seen time and advances its literal attempt metadata. Entity retarget/repair
may replace IDs and zero the candidate fingerprint without inventing a new
attempt. Tombstones remove current status and reset that key's revision base.

Replay retains its existing structural admission contract: contiguous revisions,
valid status shapes and historical endpoint validation at owner finish. It does
not add stronger cross-transition/time restrictions than legacy replay accepted.
Candidate ordering, size, nonzero attempts and first/last/dormant ordering remain
the existing status constraints.

## Target wire

`CVAERS02` retains the stable mention key, u32 revision, i64 update timestamp and
existing status tags. Tombstone, Resolved and Rejected carry their literal status
fields. Pending and Dormant add an eight-bit changed-field mask:

| Bit | Field |
| ---: | --- |
| 0 | Candidate IDs (u8 count, then exact 32-byte IDs) |
| 1 | Reason tag |
| 2 | Candidate-set fingerprint |
| 3 | Context fingerprint |
| 4 | First-seen i64 |
| 5 | Last-attempt i64 |
| 6 | Attempt-count u16 |
| 7 | Dormant-at i64 (Dormant only) |

A first Pending/Dormant state carries every applicable field. Omitted fields
inherit only from the same mention's immediately preceding accepted Pending or
Dormant state. Pending forbids bit 7. Dormant requires a literal dormant time
unless the preceding state already has one. A status following a tombstone or a
non-pending state cannot inherit absent evidence. No approximate equality,
current-state recomputation, physical pointer or global clock is introduced.
Unknown versions/tags/masks, missing bases, malformed candidates, invalid reasons,
truncation, trailing bytes and revision conflicts reject before mutation.

The literal stable key deliberately avoids a separate physical ID registry:
single resolved/rejected histories retain their current size, while repeated
unresolved evidence gains from field deltas. Initial pending states pay one mask
byte. Raw measurements must include that cost and every frame.

## Consumers and transport

- `entity_resolution_store.rs` and its write/lifecycle children own replay,
  retry suppression, reverse indexes, maintenance and all ordinary/import writes.
- `imports/reconciliation.rs` currently hashes full resolution carriers as owner
  evidence and classifies them in completed-import intervals. It must materialize
  every source change in physical order before comparing canonical postimages.
- `imports/reconciliation_interval.rs` currently copies full resolution carriers
  into a staged destination owner. It must receive source materialized changes
  and encode them against destination prefix state; source deltas cannot inherit
  from an unrelated destination prefix.
- Current-state reconciliation still imports validated typed statuses through
  the resolution owner. Copy/prune/reclaim must retain the full ordered history.
- Temporary V1 conversion must preserve every accepted change, including literal
  tombstones and revision resets, through the same concrete writer. Genuine V1
  fixtures must restore full carriers rather than only changing file headers.

Normalized import evidence preserves its established canonical full-field byte
identity. Such comparison serialization is not an ordinary storage writer.
Temporary legacy decoding is removed only after verified canonical promotion.

## Related docs

- [Storage format](storage-format.md)
- [Architecture](architecture.md)
- [Linear plan](minimal-lossless-history-storage-linear-plan-2026-10-19.md)

## Notes

The two import consumers are separate execution steps before offline migration.
This contract removes only repeated physical fields; no accepted decision,
evidence, retry state or maintenance timestamp is classified away.
