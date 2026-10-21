# Dream population and Memory first-publication prefix

Parent index: [Documentation index](INDEX.md)

## Purpose

This is a design contract for the Memory-local identity prefix and its use when
reconstructing a historical Dream population. It specifies planned behavior
for the Memory-prefix and accepted-birth/cohort steps in the
[linear storage plan](minimal-lossless-history-storage-linear-plan-2026-10-19.md);
it does not claim that compact Memory history currently implements this
contract. The [storage format](storage-format.md) describes the current
transitional format and implementation status.

## Overview

Memory owns the accepted first-publication identity prefix. REL coordination combines that prefix with accepted birth/cohort evidence and the existing Dream source-global predicate. Physical compact encoding remains a separate execution gate.

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

## Notes

This contract adds no persisted clock and does not alter source admission rules. Runtime writer cutover, migration, destination exceptions and corpus measurements remain separate gates.

## Related docs

- [Linear storage plan](minimal-lossless-history-storage-linear-plan-2026-10-19.md) — execution order and acceptance.
- [Storage format](storage-format.md) — current persisted records.
- [Behavioral contracts](behavioral-contracts.md) — legacy Dream reference cases.
