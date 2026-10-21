# Archive-vector generation row reuse decision

Parent index: [Documentation index](INDEX.md)

Status: defer the conditional new row/block format in this history migration.

## Purpose

Decide optional repeated-generation reuse from exact bytes, separating
compatibility-profile multiplicity from repeated same-profile history.

## Overview

`repeated_same_profile_archive_vector_generation_raw_measurement` adds eight
independent conversations and rebuilds one or two profiles after each addition.
It reopens every historical generation and compares literal matrix bytes. Old
fragments retain exact rows within each profile; different profiles use different
endpoint seeds and are counted separately.

The [synthetic artifact](../benchmarks/archive-vector-generation-duplication-2026-10-21.json)
records36 row occurrences, eight unique rows and28 unchanged same-profile
fragment occurrences for one profile. At384 dimensions current framed packed
matrices occupy55,872 bytes versus14,352 modeled with rows/manifests (74.31% less).
At12 dimensions it grows2,304 to2,448 (6.25%). Two profiles double both totals;
their multiplicity does not create additional same-profile reuse.

The model includes row magic, exact identity, schema and framing; matrix
manifests include identity, schema, count, references and framing. Existing
Archive-vector fragment mappings remain necessary and are excluded equally.
These are modeled candidate costs, not implemented-format or whole-file savings.

## Retained corpus evidence

The [six-file inventory](../benchmarks/archive-vector-authoritative-duplication-2026-10-21.json)
checks pinned SHA-256 before and after a read-only physical scan. Only published
generation references are followed; matrix identities and row shapes are
independently validated. Memory-vector matrices are excluded.

The14-day and28-day RELs each have one generation under one profile. Ellis has
two generations under two different profiles, one each. PHY files have none.
There are zero repeated same-profile fragment/row occurrences in accepted
generation history. Separate within-matrix/content duplication is present:
modeled28-day packed bytes fall11,804,744 to9,713,816; Ellis4,079,760 to3,965,246;
14-day grows4,325,448 to4,415,197. These separate opportunities are retained
explicitly in the report and are not described as repeated same-profile history.

## Decision and consequences

Defer row/block reuse for this migration. It removes no repeated same-profile
historical matrix in the retained corpus, introduces shared-backing lifecycle
dependencies and is not uniformly smaller. Whole-matrix content identity already
makes an exactly repeated build idempotent. Literal matrix storage retains
byte-distinct compatible embeddings and every historical generation.

The synthetic high-dimensional result is a concrete future optimization case.
Revisit with retained repeated same-profile generations containing unchanged
exact rows, or separately pursue measured within-matrix savings. That change
needs explicit PackedVectorStore codec/replay ownership, shared-row prune and
reclamation, materialized import transport, conversion and historical byte,
retrieval and corruption checks. The later converter audit must not absorb it.

Conditional identity implementation, storage, manifests, writer cutover,
conversion and new-format corruption checks are not applicable to this closed
deferral. There is no new wire family or production vector writer. Existing
generation, profile, packed-vector, retrieval and maintenance suites remain the
integration gate. The reproducible measurement test is retained.

## Related docs

- [Linear plan](minimal-lossless-history-storage-linear-plan-2026-10-19.md)
- [Storage format](storage-format.md)
- [Architecture](architecture.md)

## Notes

Compression is excluded. The decision does not merge profiles, recompute accepted
embeddings, remove old generations or equate vector identity with fragment text.
