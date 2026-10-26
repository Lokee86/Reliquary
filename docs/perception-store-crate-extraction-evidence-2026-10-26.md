# Perception Store crate extraction evidence — 2026-10-26

Parent index: [Documentation index](INDEX.md)

## Purpose

Record implementation and verification of the
[Perception Store extraction specification](perception-store-crate-extraction-spec-2026-10-26.md)
on `refactor/crate-first-restart`, starting from `2519c3b`.

## Overview

`reliquary-perception-store` contains the canonical, separately mutable Entity,
mention-resolution and transitional Relationship stores. Container is its only
Reliquary dependency. Current codecs, replay, dense Entity/Relationship clocks,
terminal facts, retained structure and indexes moved with their owners.

Root REL/PHY facades retain cross-owner authorization, immutable Memory mention
binding, current/historical reference admission, principal provisioning, candidate
and inference policy, merge coordination and Graph effects. Product errors map
store-local terminal/payload facts back to the supported lifecycle vocabulary.
Memory evidence uses exact foreign wire values in the store and typed Memory Core
values at the product boundary. No additional persistent store, projection cache,
format reader, Observation synthesis or Perception service extraction is added.

The explicitly approved API adjustment is an owned
`Option<MemoryEntityResolution>` from `entity_resolution()`. Its argument and
record key remain Memory Core values. Principal ID and global UUID derivation
have one implementation in the store; policy still admits principal workflows
only where the product allows them. Creation-conflict heuristics remain in root.

## Verification

| Gate | Result |
| --- | --- |
| Perception Store locked/offline tests | 93 passed, including codec vectors, replay, retention and overflow |
| Detachable CLI locked/offline tests | 32 passed using its separate lockfile |
| Workspace locked/offline check and workspace/CLI formatting | Passed after the final boundary changes |
| Focused root Entity / Relationship / resolution | 216 / 46 / 76 passed |
| Focused root import | 168 passed, one retained-corpus test ignored, and two expensive prefix tests excluded |
| Focused root history | 77 passed; the high-cost scaling measurement was excluded |
| Broader root library | 1,713 passed, three external-source runtime tests failed, five ignored; four expensive history measurements were stopped before completion |
| Dependency audit | Container is the only direct Reliquary dependency; no displaced root store remains |
| Documentation policy and Archivist index/link checks | Passed; final recheck follows this evidence update |
| Pitlord | Policy syntax validates. Rust analysis timed out after 240 seconds, and a retry was stopped after more than seven minutes; no refreshed Arcana graph was produced, so `pitlord docs` could not run |

The three broader-suite failures are `external_completed_window_requires_fresh_message_authority_on_replay`,
`external_insomnia_routes_private_output_and_replays_without_inference`, and
`external_partial_phy_commit_resumes_frozen_plan_and_missing_receipt_fails_closed`.
They return `Conflict` in the external-source runtime; an isolated rerun of the
second reproduced the failure. Baseline comparison was inconclusive: the clean
baseline's isolated build did not reach the test executable within the bounded
attempt. The external-source runtime implementation itself is unchanged from
the extraction baseline. The stopped
long-running cases are the two all-byte-prefix historical recovery tests,
`canonical_history_scaling_measurement`, and
`long_owner_reopen_keeps_finite_receipts_and_exact_evicted_retry`. The five
ignored cases include the retained-corpus and frozen-fixture gates, a release
measurement, and two crash subprocess helpers.

## Documentation impact

- Inspected: the extraction specification, architecture, invariants, storage
  format, maintainer map, limitations, roadmap and relevant owner decisions.
- Updated: architecture, Rust API, maintainer map, crate map, extraction spec,
  this evidence and the documentation index.
- Not affected: stored format contracts, future Observation design and promoted
  semantic Freshness/Import recovery.
- Compliance check: repository documentation policy passes; final index/link
  and architecture results are recorded with verification.
- Known documentation gaps: baseline attribution for the external-source test
  failures and refreshed graph-based enforcement remain unverified.

## Architecture impact

- Standards added or changed: none; this applies the existing crate-first DAG.
- Ownership or boundary impact: three durable Perception stores now compile in
  the rank-2 owner package; product admission and decision policy stay above it.
- Pitlord enforcement impact: a separate `perception-store` source area and
  upward-dependency prohibition cover the extracted implementation.
- Other verification impact: owner-local tests moved to the crate; multi-owner,
  history, maintenance and service tests remain in root.
- Known architectural gaps: full graph-based enforcement depends on a successful
  refreshed analysis; the whole-product `CVAFRS07` recovery gate is separate.

## Related docs

- [Architecture](architecture.md)
- [Rust API](api.md)
- [Maintainer map](maintainer-map.md)
- [Current limitations](current-limitations.md)

## Notes

The pre-existing Graph evidence edit is excluded from implementation commits.
This extraction does not claim promoted six-fixture `CVAFRS07` corpus acceptance
or production benchmark results.
