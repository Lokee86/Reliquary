# Retained inert Entity Resolution evidence contract

Parent index: [Documentation index](INDEX.md)

Status: explicit correction308–314; corrected integration verification pending.

## Purpose

Preserve the frozen reader's observable semantics and every original byte of an
older corpus family without colliding with current evidence deltas.

## Overview

The frozen decoder at `444481a` recognizes only `CVAERS01`. The pinned14-day REL
also contains169 `CVAERS02` records, which that decoder does not project into
decisions, revisions, candidate indexes or readiness. The other five pinned
files contain none. The new delta format initially reused that existing tag;
the first complete migration stopped on its explicit V1 guard before producing
or promoting output. Synthetic legacy fixtures had not exposed this collision.

The [independent inventory](../benchmarks/retained-entity-resolution-family-2026-10-21.json)
checks every pinned hash before and after reading. The older records have117
95-byte payloads,36 410-byte payloads and16 63-byte payloads. The unresolved
payloads contain eight bytes beyond the current full-field model. Their meaning
is not inferred, and their contents are not discarded.

## Frozen semantic boundary

Original `CVAERS02` remains inert under explicit V1 inspection and the temporary
generic V1 route. The migration preserves that same zero projection effect.
It does not activate older decisions, invent a retry clock, canonicalize their
references or add semantic publications. Reactivation would require a separate
semantic policy decision; this is a physical history migration against the
frozen reader. `CVAERS01` decisions continue to materialize exactly as before.

Current real decisions/deltas use the distinct `CVAERS03` tag and the already
specified literal-first/masked-inheritance layout. The abandoned experimental
delta tag was never promoted into the retained corpus.

## Literal retained carrier

`CVAERL01` is owned by EntityResolutionStore, with exactly:

- eight-byte current retained-evidence magic;
- u32 little-endian original byte length, bounded to8MiB;
- complete original bytes starting with `CVAERS02`, including every unknown field;
-32-byte SHA-256 of all preceding carrier bytes.

The44 payload bytes of overhead and ordinary eight-byte framing are included in
raw accounting. This is a specific literal evidence variant, not a general
unknown-authority bucket or runtime legacy decoder. Reopen validates its length,
family and checksum, then makes no decision/revision/index change. No current
mutator writes an older source record or interprets its fields.

The offline converter emits one retained carrier at each original source slot
and records the relocation binding. Full original records compare byte-for-byte
in physical order in the six-source matrix. Completed-import comparison omits
both inert representations consistently; interval transport wraps a V1 source
record or copies an already-validated current retained record. No omitted field
is inherited into a destination decision. Ordinary reclamation retains this
history; fresh-history rewrite follows its existing deliberate history policy.

## Physical admission and retirement

Admission uses the actual read-only header probe, including generic opens:

| Record | V1 REL/PHY | V2 REL/PHY |
| --- | --- | --- |
| Raw retired `CVAERS02` | Inert inspection only | Reject |
| Current `CVAERS03` delta | Reject | Concrete owner replay |
| Current `CVAERL01` evidence | Reject | Validate; no projection effect |

Unknown authority outside this explicitly inventoried family still fails closed.
After verified promotion, remove the V1 route, raw retired-source admission and
one-time wrapping writer. The current checksum/evidence validator remains; it
provides no legacy decision interpretation or old container support.

## Related docs

- [Compact resolution contract](entity-resolution-compact-contract-2026-10-21.md)
- [Linear plan](minimal-lossless-history-storage-linear-plan-2026-10-19.md)
- [Converter audit](latest-offline-converter-coverage-2026-10-21.md)
- [Storage format](storage-format.md)

## Notes

The previous synthetic75-case evidence validates corrected deltas and retained
admission. The added import/literal-order regressions and complete corpus run
remain separate acceptance requirements. The frozen source files remain intact.
