# Graph crate extraction evidence — 2026-10-26

Parent index: [Documentation index](INDEX.md)

## Purpose

Record the implementation and verification boundary for the
[Graph extraction specification](graph-crate-extraction-spec-2026-10-26.md).

## Overview

The implementation starts at `13016c4a`, with Echo already committed and a clean
working tree. Graph and Community source ownership moves to the compiled
`reliquary-graph` crate. Its only Reliquary dependency is Container; Arcana,
Leiden and SHA-256 retain their existing roles. No stored format is changed.

Root `GraphOwner` and `CommunityOwner` hold the canonical crate stores and
translate typed product values. They do not maintain a second topology,
relationship index, durable state or codec. `GraphReplay` composes already
rebuilt Memory/Entity facts with local Graph replay. Endpoint checks retain
input order and error precedence before optimistic conflicts or duplicate
checks. Historical references and current visibility remain separate.

Graph admission is an explicit foreign-fact contract: callers must derive
facts from the owning stores under the held owner/Container context. The
product seam performs those checks; constructing foreign identity bytes alone
never establishes record truth or grants access to a peer owner. Rewrite
baseline publication also requires admitted Memory facts, while authoritative
rewrite-genesis checks remain in root History/Maintenance.

## Verification

| Gate | Result |
| --- | --- |
| Graph crate locked/offline check and tests | Pass; 28 owner tests, including literal codecs, ordered replay, overflow, opaque IDs and historical-baseline admission |
| Root Graph filter | 104 pass, one existing corpus case ignored |
| Root Community / Dream / Perception / Ego filters | 25 / 186 / 3 / 53 pass |
| Current-format and rewrite integration contracts | 6 / 16 pass; promoted whole-corpus case ignored |
| Detachable CLI, separate lockfile | 32 pass |
| Workspace locked/offline build | Pass |
| Workspace / CLI formatting | Pass |
| Graph and root consumer Community benchmark compilation | Pass; high-cost benchmark not executed |
| Cargo dependency and displaced-owner audit | One Graph store and one Community store; Container is the only direct Reliquary dependency |
| Documentation policy and Archivist indexes/links | Pass |

The broader root library attempt reported 1,800 passes, 18 failures and five
ignored cases before interruption. Baseline executable comparisons reproduce
configured-runtime localhost transport failures, denied hard-link setup, the
stale Entity-audit expectation of automatic reopen repair, and an ambient Lore
repository-creation failure. Three Lore tests that failed under parallel runs
pass when rerun alone. No change to those unrelated runtime behaviors is made.
The separate historical filter reported 171 passes, two localhost embedding
failures, one transient Lore failure and one ignored retained-corpus case; the
Lore case passes alone.

The broad run did not finish these high-cost cases:
`historical_native_checkpoint_every_container_byte_prefix_reopens_and_retries`,
`historical_native_due_reopen_and_every_live_use_prefix_preserve_all_recipients`,
and `long_owner_reopen_keeps_finite_receipts_and_exact_evicted_retry`.
`canonical_history_scaling_measurement` did finish successfully. Two localhost
fixture tests also remained unfinished:
`codex_general_adapter_propagates_failed_incomplete_sse_response` and
`ready_general_adapter_propagates_malformed_structured_json`. These are not
counted as passing; focused historical runs explicitly skip the exhaustive
prefix cases and configured-runtime embedding fixture.

The architecture refresh's Python guards passed (92 multiplexing fixtures and
11 external-ownership fixtures), and the installed Pitlord validates the policy
(20 areas, 24 rules). Lexicon's Rust scan did not finish after roughly 20 minutes
and was interrupted; no fresh Arcana/Pitlord architecture pass is claimed.
Both installed Pitlord binaries reject the required `docs` subcommand, so that
specific enforcement gate remains unavailable.

The structural scan/merge benchmark is compiled but its high-cost measurement
is not part of this extraction gate. Ignored promoted-corpus and retained-corpus
cases remain separate; canonical CVAFRS07 Freshness/Import product recovery is
not claimed by Graph extraction. Test fixtures use disposable files.

## Related docs

- [Architecture](architecture.md#graph-and-community-owner-crate)
- [Maintainer map](maintainer-map.md)
- [Behavioral contracts](behavioral-contracts.md#compiled-graph-and-community-owner)
- [Storage format](storage-format.md)

## Review

- Standards/architecture axis: one canonical persistent owner, downward Cargo
  dependency, operation-scoped foreign facts and one-pass replay admission were
  inspected. The repeated Entity-history scan and missing baseline admission
  found in review were corrected.
- Specification/contract axis: current/historical admission, original error
  precedence, typed product identities, literal bytes and consumer seams were
  checked. Baseline admission now accepts retained historical Memory identities
  while keeping terminal nodes out of current projection; a regression test
  protects both acceptance and absence rejection. No remaining concrete product
  admission or ordering defect was identified. The verification limits above
  remain explicit; full corpus admission is not claimed.

## Documentation impact

- Inspected: architecture, invariants, format, API, limitations, roadmap,
  maintainer/coverage maps, governing ADRs and the extraction specification.
- Updated: Graph/Community ownership and API references, coverage, current ADR
  test paths, documentation index, crate map and extraction evidence.
- Not affected: persistent byte layouts, unrelated owner behavior and roadmap.
- Compliance check: shared documentation policy and Archivist links/indexes pass.
- Known documentation gaps: installed Pitlord has no `docs` command.

## Architecture impact

- Standards added or changed: no new architectural standard or format.
- Ownership or boundary impact: Graph/Community state is compiled below product
  lifecycle; Memory/Entity admission and rewrite authority remain above it.
- Pitlord enforcement impact: the semantic Graph area includes the crate;
  documentation coverage includes the crate and product admission seam.
- Other verification impact: locked/offline Cargo, owner and product tests plus
  direct source/dependency inspection protect the cut.
- Known architectural gaps: full refreshed Lexicon/Arcana enforcement did not
  finish; CVAFRS07 whole-product corpus recovery remains separate.

## Notes

Source and Cargo dependencies, persisted literal codec tests and product
consumer tests are the implementation authority. Architecture tooling is
supplemental evidence and does not substitute for working owner code.
