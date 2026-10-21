# Dream population and Memory first-publication prefix

Parent index: [Documentation index](INDEX.md)

## Purpose

This is a design contract for the Memory-local identity prefix and its use when
reconstructing a historical Dream population. The Memory metadata reader and private REL birth/cohort prefix projections implement these authority rules. Current Begin encoding/decoding, physical-prefix replay, ordinary writing and exact destination transport are implemented privately; acceptance and retirement remain tracked in the
[linear storage plan](minimal-lossless-history-storage-linear-plan-2026-10-19.md). The [storage format](storage-format.md) describes the current
transitional format and implementation status.

## Overview

Memory owns the accepted first-publication identity prefix. REL coordination combines that prefix with accepted birth/cohort evidence and the existing Dream source-global predicate. Physical compact encoding stores cut/digest authority only after exact population equality is proved; exceptions retain literal sets.

## Memory-local identity prefix

Let `head` be the accepted Memory-local version. A requested local cut `c` is
valid only when `0 <= c <= head`. Cut zero is the empty prefix. A cut beyond
the accepted head is invalid and must be reported as such; it must not be
clamped to the head. This is a Memory-local coordinate and must not be compared
numerically with a container global version.

For a valid cut, the identity prefix contains one entry for each Memory whose
accepted revision 1 is at or before `c`. Each entry carries the Memory identity
and the exact reserved global version stored on that first revision. The
prefix is ordered by the first revision's accepted Memory-local position. It
must be derived from accepted revision-1 records, across standalone, grouped,
and external-source births. In grouped publications, each Memory has its own
local version and its own reserved global version; a shared physical group
does not collapse those coordinates or change the order of its member rows.
Preserve each recorded global coordinate rather than deriving global order
from local order: accepted history can place them in different orders.

Later revisions do not create another identity entry or replace its first
global version. A later revision, archival state, or terminal tombstone also
does not remove an identity already present in the prefix. Conversely, a
revision-1 record after `c` is excluded even if its global coordinate is
earlier than that of a birth before `c`.

This result is an identity prefix only. It does not answer whether a Memory is
live, select its latest revision, resolve its body, or authorize it as a Dream
candidate. In particular, implementations must not substitute a current-live
set or current-body lookup for the accepted first-revision scan.

Pruning may make a retained record's body unavailable, but must not silently
erase the revision-1 identity and its accepted local/global coordinates from
this contract. If a source representation cannot establish those coordinates,
the caller must receive an explicit unavailable/unsupported result rather than
silently omitting that identity.

## Reconstructing a Dream population at Begin

The identity prefix is only one input to the historical Dream population.
Begin reconstruction must also apply the source-global fence captured for the
Dream source and require accepted birth/cohort evidence that lies in the
physical prefix ending at the accepted `physicalBegin` marker. The population
is therefore selected using both semantic coordinates and physical evidence:
the candidate's exact first global version must satisfy the source-global
fence, and its accepted birth proof must be visible by the Begin marker. A
Memory-local cut alone cannot establish either fact about the physical Begin
boundary, and current end-of-file birth/cohort evidence must not be treated as
if it were already visible there.

This distinction preserves cases where Memory-local and global order differ,
and excludes a sibling born in the same grouped cohort as the Dream source
when the accepted Begin rules do not admit that sibling. It also means that
matching identity or revision fields is not a substitute for accepted
birth/cohort evidence. The reconstructed Dream population remains a separate
historical selection; it does not change the identity-prefix rules above.

## Canonical population digest commitment

The private Dream population codec commits to a materialized set with
SHA-256 over the ASCII bytes `reliquary:dream-population:v1` and one NUL byte, followed by
the set cardinality as an unsigned 64-bit little-endian integer, followed by
the members' raw 32-byte Memory IDs in ascending byte order. The sorted set
representation gives each population one canonical byte sequence; duplicate
inputs have set semantics. Empty populations use the same domain and an
encoded zero count.

The digest covers membership only. Owner, source, Memory cut, and physical
Begin position remain separate authority fields and are not inputs to this
hash. The digest does not select, rank, or validate Memories. This is the
private codec commitment; implementing it does not itself add a wire writer or
replace an explicit population set as the authority.

## Accepted birth and cohort visibility

A Begin binds the physical prefix strictly before its own frame. Its Memory
cut is the accepted Memory-local head at that boundary, not the local position
preceding the source first revision. Select from that identity prefix using
`candidate.first_global < source.first_global`, excluding the source and its
accepted publication cohort. Reserved global values retain their original
meaning even when physical and Memory-local orders differ.

For direct publication, a matching pending Intent plus its accepted first
Memory publication already visible before Begin counts under the existing
recoverable birth rule. Consume records confirm the matching Intent identity;
they do not move birth visibility later when both prerequisite facts already
precede Begin. An Intent without its first Memory publication is insufficient.
A later Consume cannot make a Memory publication after Begin visible earlier.
ReplayBirth contributes only at its own physical position and requires an
accepted first Memory identity in the queried Memory prefix. Duplicate markers
retain set semantics. No end-of-file birth set can replace these queries.

Insomnia and external-source births and cohorts come from their own accepted
completion/publication records before Begin, with their existing member,
mutation and birth evidence validation. The whole-file cohort query cannot
establish earlier membership. Each source owner projects its own facts; REL
coordinates those projections and Memory's identity reader. Dream journal
framing does not acquire ownership of the other stores' semantic validation.

Prefix reconstruction occurs after source owner validation, using retained
physical positions or a second ordered scan. A later accepted proof never
retroactively expands an earlier Begin population. An unsupported reconstruction
must remain explicit: migration and destination transforms use the separately
planned explicit-set exception when the original exact set cannot be represented.
These rules do not add rejection conditions to existing explicit-set source
admission or replace its immutable set with an inferred one.

## Current Begin wire contract

The frozen target is a clock-neutral V2-container Begin payload with magic
`CVDREAM2`. It allocates no Container global ticket and adds no birth clock.
Encoder, decoder, owner replay and ordinary writer integration have separate
acceptance steps; ordinary writing now uses the validated current encoding.

All integers are unsigned little-endian. The shared fields occur in this order:

| Field | Encoding |
| --- | --- |
| Magic | Eight ASCII bytes `CVDREAM2` |
| Owner | Sixteen UUID bytes, matched to the containing owner at admission |
| Population variant | One byte: 0 CompactCut, 1 ExplicitSet |
| Pass identity | u32 byte length followed by nonempty UTF-8 bytes |
| Source Memory | Thirty-two raw identity bytes |
| Original REL turn | u64 |
| Semantic Graph baseline | u64 |

The first revision is implicitly 1, preserving the only accepted initial-pass
revision without storing a redundant integer. Initial roots are empty and
accepted settlement is absent; subsequent journal records retain those facts.
Pass identity bytes are preserved without normalization or a new length limit
beyond the existing u32 byte length. Canonical pass/source eligibility remains
an owner-admission check rather than a new codec restriction on legacy input.

Variant 0 appends a nonzero u64 Memory-local head cut and the 32-byte canonical
population digest. It stores no duplicate population count: the commitment
already includes cardinality. At materialization, the cut must equal the
Memory head immediately before this Begin, the source first publication must
exist in that prefix, and the exact reconstructed population must hash to the
stored digest. Framing alone cannot establish these cross-owner facts.

Variant 1 appends a u32 count and that many ascending, strictly unique raw
32-byte Memory IDs. The source must be absent. The explicit set is direct
population authority; it stores neither an inferred Memory cut nor a redundant
digest. It preserves manual/legacy subsets and transformed histories whose
exact original population is not representable at the destination prefix.
Do not replace it with a current population or tighten existing explicit-set
source admission. A later ordinary-writer gate may also select an empty literal
set when that is the smaller exact representation.

Encoders check u32 string/count representability before emission. Decoders
validate complete magic, known variant, nonempty UTF-8 identity, nonzero cut,
checked count/byte arithmetic, explicit-set ordering/uniqueness/source exclusion,
and exact end of payload. Truncation, trailing bytes and unknown versions/tags
reject. Decode is independent of Memory/birth materialization; it returns
population authority requiring the later owner checks, not an invented empty set.
No digest is a keyed authenticity claim.

Only Begin changes family. Existing `CVDREAM1` roots, settlement, acceptance,
Intent/Consume and ReplayBirth records retain their current framing and rules.
The old `CVDREAM1` explicit Begin decoder remains transitional until the
planned offline conversion and retirement gates; retirement must distinguish
that obsolete Begin variant from unchanged current journal kinds. A current
`CVDREAM2` Begin is not admitted in a V1 container. Source inspection and
conversion must leave source bytes unchanged.

## Notes

This contract adds no persisted clock and does not alter source admission rules. Runtime writing and offline conversion choose CompactCut only after exact prefix equality; unequal destination/legacy populations retain ExplicitSet. Empty populations use the smaller explicit form. Copy, prune and reclaim preserve prefix authority. Fresh-history rewrite intentionally discards old pass history under its existing policy; no obsolete cut is carried into the new generation. Final corpus promotion and legacy retirement remain separately gated.

## Related docs

- [Linear storage plan](minimal-lossless-history-storage-linear-plan-2026-10-19.md) — execution order and acceptance.
- [Storage format](storage-format.md) — current persisted records.
- [Behavioral contracts](behavioral-contracts.md) — legacy Dream reference cases.
