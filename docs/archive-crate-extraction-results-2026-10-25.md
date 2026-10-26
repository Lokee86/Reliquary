# Archive crate extraction results

Parent index: [Documentation index](INDEX.md)

## Purpose

Record implementation and verification of the [Archive extraction specification](archive-crate-extraction-spec-2026-10-25.md) on `refactor/crate-first-restart`, starting at specification commit `3ce000c`.

## Overview

The product uses the source owner in `crates/reliquary-archive/`. Archive has exactly one internal Cargo dependency, `reliquary-container`, and no root dev dependency. The old `src/archive` owner paths and root path declarations are deleted. Echo remains under `src/echo`, interaction orchestration under `src/runtime_inference`, and search policy under `src/retrieval`.

Archive owns the existing codecs, replay, source indexes, receipts, Episodes, fragments, attachments, project bindings/correlations, compaction projection and durable stream records. Maintenance proof and lifecycle translation are root composition. File-to-Memory target checks run after both owners replay. Opaque target identities and stream-role values carry source facts without importing Memory or Runtime behavior. Public source iterators retain private indexes and avoid copying whole indexes into derived consumers.

## Verification

| Check | Result |
| --- | --- |
| Independent Archive locked/offline compilation | Passed |
| Archive locked/offline unit tests | 27 passed, including literal publication bytes, read-only replay, receipt retries, local/global cuts, pruned-content markers and private owner faults |
| Locked/offline workspace compilation | Passed |
| Focused product source/Import/History/runtime tests | 500 passed in two disjoint clean selections (375 source/Import/History/runtime and 125 additional conversation/tombstone tests); Lore attachment setup passed separately; the long Freshness history scaling test passed in the initial run |
| Detachable CLI locked/offline compilation | Passed |
| Detachable CLI tests | 32 passed (independently locked CLI manifest, shared repository-local build directory) |
| Formatting | Passed |
| Prepared graph `archive_roundtrip` | Passed: 5 Nodes, 2 Branches, 2 fragments, 5 content objects, 6 expanded references, 1,548 bytes |
| Documentation policy, full and changed-from `origin/main` | Passed |
| Archivist indexes/links | Passed after builds released Cargo file locks |
| Architecture refresh and Pitlord documentation ownership | Blocked: isolated offline Rust analysis was stopped after 15 minutes without producing `.arcana/CURRENT`; graph-dependent checks cannot run |

The focused product runs exercise create/reopen, historical cuts, tombstones/pruning, source receipts and revision conflicts, attachments, Episode coverage, import completion/reconciliation, compaction, interrupted streams and Runtime handoffs. Cargo dependency inspection and source searches confirm only the downward Container edge and one Archive implementation.

## Acceptance limits

Pitlord policy validation passed (20 owner areas, 24 rules). The refresh startup guards also passed: 92 multiplexing fixtures, 166 classified APIs and 11 external ownership fixtures. Cargo and direct source inspection establish the Archive edge independently of the unavailable graph. No graph or documentation-ownership pass is claimed.

The entire root suite was not run: 1,384 tests outside the focused selections remain unverified here, along with feature-gated corpus oracles. No authoritative six-file data was modified. CVAFRS07 semantic recovery and full promoted-corpus acceptance remain separately blocked as described in [current limitations](current-limitations.md#crate-first-recovery-boundary).

The Lore repository-creation test fails when run with other selected tests with `Lore repository creation failed with status -1`, and passes in a fresh test process. It is retained and verified in isolation; no Lore implementation was changed. One serial attempt also encountered disk-full errors; generated build caches were removed and affected source tests were rerun successfully.

## Follow-up correction and independent integration review (October 25)

The initial review treated Archive-local compaction chunk recycling as cross-owner Maintenance. That was incorrect: the existing `ConversationCompactionStore::put` may recycle/coalesce its own superseded chunks **during an explicitly requested, authorized Archive write**. Cross-owner pruning, retained-history proof and whole-file rewrite remain higher-level Maintenance. Ordinary REL/PHY opening stays byte-neutral and uses `finish_without_reclaim`.

- Removed unused, alternate open-time `ConversationCompactionOpenState::finish` and `finish_unrecovered`, `ConversationCompactionStore::recover` and `all_records`, and unused `InteractionStreamStore::apply`; preserved the active publication allocator and `free_and_coalesce`.
- Made three unused implementation modules private: `archive_history`, `archive_record_index` and `interaction_stream_codec`. Other legitimately consumed modules remain public pending actual consumer extractions, rather than adding transient wrappers.
- Independent verification: `cargo check --workspace --locked --offline` passed; Archive crate tests **27/27**; root `archive`-filtered tests **94/94**, root `conversation`-filtered tests **129/129**, root `imports::tests::` **19/19**, and independent CLI tests **32/32**. Filtered root selections may overlap and are not additive coverage counts.
- The broader `import`-filtered selection was stopped at the known expensive `historical_native_due_reopen_and_every_live_use_prefix_preserve_all_recipients` after numerous passing cases; **it has no final pass verdict**. The entire root suite and `CVAFRS07` promoted-corpus acceptance remain outside this scoped follow-up.
- Independently recovered build space by deleting only the inactive pre-refactor baseline audit's generated Cargo `target` directory (~14.2 GiB); the baseline source worktree, its Git state, original `main`, recovery bundles and authoritative corpus were not modified.

## Documentation impact

- Inspected: governing architecture, invariants, storage format, maintainer map, current limitations, roadmap, linked extraction/DAG specifications and shared engineering standards.
- Updated: architecture/API ownership, maintainer and coverage maps, current limitations, crate map, extraction specification, this verification record and documentation index.
- Not affected: persisted storage formats, ADR decisions, authoritative corpus and original main checkout.
- Compliance check: final results are listed above.
- Known documentation gaps: graph-dependent documentation ownership is unverified; no full-product or CVAFRS07 acceptance claim is made.

## Architecture impact

- Standards added or changed: no shared standards changed.
- Ownership or boundary impact: Archive source truth is independently compiled; cross-owner admission/proof remains above it; Echo remains independent.
- Pitlord enforcement impact: Archive source paths and ownership scope now cover the crate, Echo has a separate pending owner area, and Archive upward dependencies are forbidden.
- Other verification impact: owner codec/fault tests move into Archive; mixed-owner tests stay at root composition seams; CLI lock includes the actual crate.
- Known architectural gaps: refreshed graph enforcement is unverified because the local analysis did not complete; remaining owner crates and the CVAFRS07 recovery boundary are unchanged.

## Related docs

- [Architecture](architecture.md)
- [Maintainer map](maintainer-map.md)
- [Storage format](storage-format.md)
- [Archive extraction specification](archive-crate-extraction-spec-2026-10-25.md)

## Notes

Verification uses repository-local disposable fixtures and caches. Luna subagents handled bounded boundary cuts, consumer migrations and test/document moves. No connectors were used.
