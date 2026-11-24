# Temporal search acceptance on Windows

Parent index: [Acceptance evidence](./INDEX.md)

## Purpose

Verify the three REL temporal search paths and record a read-only comparison on the migrated 14-day and 28-day fixtures.

## Overview

The implementation branch starts from Core `4f1d53b3`. Source-time search applies a half-open window to accepted source timestamps. Mentioned-time search evaluates grounded Chronos evidence against the same window. Scope and evidence come from the authorized owner cut. [Temporal search specification](../planning/temporal-search-spec-2026-11-23.md)

## Verification

| Surface | Observed behavior |
| --- | --- |
| Passive Memory | A limit of one selected the dated lower-scoring result in lexical, hybrid and exact-semantic modes. The test covered an empty window result with a compatible profile, view-only access, and matching evidence. |
| Chronos evidence | Tests covered half-open bounds, an explicit offset change across Vancouver's 25-hour fall-back day, historical relative dates, coarse precision and deterministic fallback from a stale proof. |
| Agentic retrieval | The filter applied before the offer cap and during refinement and traversal. Recovery restored the original window, retained a newer active query's window, accepted the legacy manifest digest, and rejected mismatched delivered revisions and bodies. |
| Archive | The Root fixture used a two-turn fragment. An eligible turn with topical evidence produced a partial lexical hit. The window covering only surrounding context yielded no hit. Tests also covered gap dates and original per-turn reference times. |
| CLI and external consumer | CLI tests exercised complete RFC3339 flags, structured JSONL input and ordinary output compatibility. A fresh Root-only process reopened the synthetic G4 cohort and the CLI returned matching evidence and original source hashes. |

The locked offline workspace check passed. Test results were 458 workspace, 22 CLI and 16 Root-only consumer tests passed. The separate fresh-process G4 acceptance passed on PowerShell 7, including the ignored cohort-reopen test.

## Existing REL fixture comparison

The lexical topic was `project` with a limit of ten. For both fixtures the date window was May 15, 2026 UTC, selected from the first dated ordinary hit. Times are medians of three alternating fresh-process runs in the Windows debug build and include CLI startup and REL admission. The SHA-256 hashes of both REL files matched before and after. [14-day migration](owner-history-migration-other-pairs-2026-11-19.md) · [28-day migration](owner-history-migration-first28d-2026-11-18.md)

| REL | Basis | Median elapsed | Hits | Hits in ordinary top ten |
| --- | --- | ---: | ---: | ---: |
| First 14 days | Ordinary | 2,810 ms | 10 | 10 |
| First 14 days | Source | 2,823 ms | 8 | 3 |
| First 14 days | Mentioned | 3,287 ms | 0 | 0 |
| First 28 days | Ordinary | 9,550 ms | 10 | 10 |
| First 28 days | Source | 10,405 ms | 8 | 3 |
| First 28 days | Mentioned | 11,111 ms | 0 | 0 |

Each source-time hit included dated evidence. Five qualifying hits per fixture fell outside the ordinary top ten. The sampled date and topic returned zero grounded mentions in both fixtures.

## Related docs

- [Temporal search API](../public-product-api.md#temporal-search)
- [CLI reference](../reference/cli.md)
- [Development workflow](../development/workflow.md)

## Notes

The production dependency graph and the documentation policy checks passed. The Pitlord executable was unavailable for its separate policy-tool check. The local Arcana graph had no current snapshot.
