# ADR 0042: Minimal lossless current history

Status: Accepted design, 2026-10-21; canonical promotion is complete and runtime retirement is implemented. Post-retirement acceptance remains tracked by the execution plan.

## Context

Repeated full postimages and separate publication carriers retain redundant
physical facts. Removing them must preserve the observable history of each
concrete owner, including rejected historical queries, exact timestamps,
transaction shape, repeat publications, ordered evidence and inert records.
Container framing cannot decide which semantic fields are reconstructible.

## Decision

Each semantic owner defines its compact record contract and replay rules.
Creation records retain literal facts; deltas inherit only from an admitted
same-owner predecessor with validated identity and revision. Tombstones retain
their terminal meaning. Shared framing supplies physical publication coordinates
without acquiring semantic dependency ownership.

Current native publication envelopes combine concrete bodies with global and
optional transaction-time coordinates. Owner-local versions remain explicit.
Reserved source versions and allocating ordinary writes use separate envelope
modes. Stable cross-owner identities and exact transaction order remain durable.

Dream Begin retains a validated physical Memory/birth-prefix reference and
literal processing identity; it does not regenerate a different historical
population from the latest state. Dream pairs use direct dense endpoint tables.
Interaction streams retain Begin, ordered UTF-8 suffixes and interruption state.
The disposable external index uses bounded checksummed snapshots and a discard
barrier. Entity Resolution uses CVAERS03 deltas; previously inert CVAERS02 bytes
remain inert inside exact checksummed CVAERL01 evidence carriers.

Accepted import settlements use standalone CVAFRS06 semantic evidence. Source
physical-prefix proofs validate during offline conversion; the resulting current
settlement does not depend on a legacy predecessor or CVAILEG transport frame.
Import completion changes only its derived settlement digest during conversion.

All retained development files must convert and pass owner history, maintenance
and source-integrity checks before promotion. Promotion updates the separate
fixture repository and manifests. Then remove V1 container admission, legacy
record dispatch, temporary normalization, upgrader APIs and CLI commands. Frozen
oracle data and current-format tests survive; a runtime compatibility route does
not. There is no approved legacy-consumer exception.

## Measurement decisions

Community reconstruction is rejected because identical algorithm inputs admit
different retained partitions. Membership and names remain exact. Historical
vector row reuse is deferred because the retained corpus has no repeated
same-profile generations; synthetic benefits alone do not establish a corpus
need. Derived checkpoints are deferred until a named latency target and workload
justify their complexity. Any future checkpoints remain disposable acceleration
state under the existing ownership invariant.

The six-file corpus saves 7,234,112 raw bytes, or 5.235%, including all metadata,
framing and standalone import-evidence growth. Compression remains a separate
ADR-0004 measurement. Fresh-process and warm reopens, lookup ages, migration
throughput and operating-system process memory peaks are reported separately;
an uncontrolled OS cache is not described as cold disk.

## Consequences

Replay requires explicit concrete owner state and strict predecessor admission.
Historical exactness takes precedence over a smaller result. Unknown authority
cannot be accepted through a generic residual-record bucket. The migration
window is temporary, and completion requires current-only acceptance after
deletion rather than successful conversion alone.

## Related docs

- [Migration results and evidence](../canonical-history-migration-results-2026-10-21.md)
- [Linear execution plan](../minimal-lossless-history-storage-linear-plan-2026-10-19.md)
- [Semantic import and maintenance decisions](0041-semantic-import-boundaries-owner-pruning-history-rewrite.md)
- [Storage format](../storage-format.md)
- [Version history](../version-history-plan.md)
