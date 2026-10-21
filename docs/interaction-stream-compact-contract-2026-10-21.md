# Compact interaction stream contract

Parent index: [Documentation index](INDEX.md)

Status: behavior and wire contract for the stream acceptance steps; implementation
is tracked separately in the [linear plan](minimal-lossless-history-storage-linear-plan-2026-10-19.md).

## Purpose

Freeze the existing cumulative checkpoint state machine before replacing repeated
prefixes with metadata, exact suffixes and interruption markers. The public
`InteractionStreamRecord` and transcript behavior remain unchanged.

## Overview

InteractionStreamStore owns the latest accepted checkpoint per message ID.
Message ID, session ID, parent option, role and timestamp are immutable once a
message exists. Message/session IDs must be nonempty; existing parent values,
UTF-8 text and timestamps are preserved literally. Content is append-only.

| Previous state | Requested checkpoint | Existing rule |
| --- | --- | --- |
| Absent | Streaming or Interrupted | Accept complete metadata and any representable content, including empty |
| Streaming | Streaming | Same metadata and a content extension, including an identical checkpoint |
| Streaming | Interrupted | Same metadata and a content extension, including unchanged content |
| Interrupted | Interrupted | Same metadata and a content extension, including unchanged content |
| Interrupted | Streaming | Reject |
| Any existing state | Changed metadata or non-prefix text | Reject before persistence |

Interruption is irreversible but does not prohibit a later interrupted
checkpoint from extending content. The compact representation must preserve that
existing distinction. Completed messages belong to Archive turn publication;
the stream owner has no separate Complete status or completion clock.

## Target wire records

Every current stream payload starts with eight ASCII bytes `CVAISTR2`, followed
by a one-byte kind. Integers are little-endian. Strings use u32 byte lengths and
exact UTF-8 bytes. The existing u32 total-content representability bound remains.

| Kind | Fields after kind |
| --- | --- |
| 0 Begin | Message string; session string; parent presence byte and optional parent string; role byte (0 User, 1 Agent); i64 timestamp |
| 1 Append | Message string; u32 accepted content byte length; nonempty suffix string |
| 2 Interrupted | Message string; u32 accepted content byte length |

Begin creates empty Streaming content when absent. Repeated identical metadata
must not reset accepted text or resume an interrupted stream. Different metadata
rejects. Append requires an existing Begin, exact prior UTF-8 byte length, a
nonempty valid UTF-8 suffix and representable resulting length. It extends
content while preserving the current status. An exact Append retry is accepted
only when its resulting length and exact tail equal the current checkpoint;
older or conflicting prefixes fail. Interrupted requires an existing message
and its exact current byte length, then sets Interrupted. Exact marker retries
are inert. These byte positions are physical content coordinates, not clocks.

Unknown kinds/versions, malformed flags, invalid roles or UTF-8, forged lengths,
truncation and trailing bytes fail closed. Validate identity, metadata, base,
status and allocation bounds before changing the accepted projection.

## Writing and recovery

The owner first validates the requested full checkpoint against existing state.
A first checkpoint emits Begin, its nonempty initial suffix if any, and an
interruption marker if needed. Later checkpoints emit only a nonempty suffix
and/or a new interruption marker. Identical checkpoints need no new frame.
Displayed text is acknowledged only after the existing sync boundary.

Each complete physical prefix must remain replayable. A crash between first
Begin and Append may retain an unacknowledged empty stream; a crash after Append
but before Interrupted may retain the extended Streaming text. Neither case may
lose previously acknowledged text. An interrupted stream must never resume.
Truncated physical tails retain existing Container recovery policy; semantic
corruption is not repaired into accepted text.

Reconciliation still merges materialized latest checkpoints using immutable
metadata, compatible text prefixes, the longest text and interruption dominance.
Destination writing uses the compact owner. Temporary offline V1 conversion
replays each valid `CVAISTR1` checkpoint through that same owner, preserving its
exact text/status at the end of each converted checkpoint. Old repeated no-op
checkpoints are semantic stutters, not a new clock or retained event identity.

## Related docs

- [Storage format](storage-format.md) — current implementation status.
- [Architecture](architecture.md) — Archive and stream ownership.
- [Linear plan](minimal-lossless-history-storage-linear-plan-2026-10-19.md) — codecs, replay, migration and durability acceptance.

## Notes

This contract introduces no public delta API, semantic clock, generalized event
store or new completion authority. Legacy decoding and conversion are temporary
and must be removed after canonical development-data promotion.
