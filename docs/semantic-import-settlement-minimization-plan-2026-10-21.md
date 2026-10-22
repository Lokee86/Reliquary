# Semantic Import-Settlement Minimization — Follow-up Plan

Date: 2026-10-21
Status: Planned; not implemented.
Parent: [Roadmap](roadmap.md)
Related completed work: [Minimal lossless history storage](minimal-lossless-history-storage-linear-plan-2026-10-19.md), [ADR 0041](decisions/0041-semantic-import-boundaries-owner-pruning-history-rewrite.md), [Canonical migration results](canonical-history-migration-results-2026-10-21.md)

## Purpose

Correct the storage cost of the current semantic import-settlement proof without reopening the completed 377-step minimal-lossless-history hard cut.

## Overview

This is a bounded follow-up to the completed storage refactor. It audits the current `CVAFRS06` settlement for duplicated semantic state, replaces that duplication with the smallest independently valid semantic commitment, hard-cuts the development corpus to the corrected format, and remeasures storage. The completed 377-step plan remains closed historical evidence.

## Problem

The completed migration correctly removed permanent physical-prefix authority, but the replacement `CVAFRS06` settlement is substantially over-materialized in the retained corpus.

Across the six canonical fixtures:

- old `freshness_import_settlement`: **395,533 bytes**;
- current semantic settlement: **7,415,741 bytes**;
- increase: **7,020,208 bytes**;
- current representation is about **18.75×** the previous family size.

The old representation was artificially cheap because a small physical-prefix proof could pin unrelated bytes and prevent later pruning/relocation. Restoring that design is not acceptable. The correction is to keep semantic authority while removing duplicated semantic state from the settlement itself.

## Design requirement

The settlement must authenticate import attribution independently of physical file offsets and must survive eligible pruning, relocation, reclamation and repacking. It must not duplicate authoritative Freshness, Memory or import history merely to make validation self-contained.

The current representation must be challenged field by field, especially:

- per-event affected-recipient sets when accepted Freshness events already own the effects;
- repeated born/affected/cohort Memory-ID sets;
- complete settled `FreshnessRecord` postimages;
- repeated source/event identities that may be authenticated by a smaller stable commitment.

Historical cuts, exact identities, canonical digests/roots or compact membership commitments are acceptable only when the underlying authoritative members can still be deterministically resolved and validated. A digest cannot replace evidence that is otherwise unavailable.

## Execution plan

| Step | Work | Acceptance gate |
| ---: | --- | --- |
| 1 | Measure `CVAFRS06` byte contribution by field/collection across all six canonical fixtures and map every field to its authoritative source. | Field-by-field byte/authority table exists; each field is classified as irreducible authority, compact commitment or reconstructible duplication. |
| 2 | Design at least two materially different compact semantic-proof shapes and compare them on proof strength, locality, reconstruction cost, prune/relocation independence and byte cost. | Chosen design has a written reason for every persisted field and does not rely on physical-prefix authority. |
| 3 | Implement the corrected settlement codec and validation path behind the existing import-settlement owner boundary. | Focused synthetic tests prove exact attribution, corruption rejection, historical reconstruction, retry/completion and reconciliation behavior. |
| 4 | Exercise prune/reclaim/repack/rewrite interactions and prove that eligible unrelated payload remains removable without invalidating the settlement. | Existing maintenance semantics pass without restoring physical-offset dependencies. |
| 5 | Apply the normal development-format hard cut: migrate all six canonical fixtures to the corrected settlement format, verify exact semantic/history parity, then delete the superseded settlement decoder and temporary migration machinery. | Current runtime supports only the corrected format; no compatibility scaffolding remains. |
| 6 | Rerun the six-source historical oracle and import/prune/reclaim/rewrite/reconcile acceptance, then regenerate family and whole-file byte accounting. | Behavioral parity passes and final measurements separately report settlement savings, whole-file logical savings and ADR-0004 compression status. |

## Non-goals

- Do not reopen the completed minimal-lossless-history implementation plan.
- Do not restore physical prefix/offset authority.
- Do not weaken exact import attribution or corruption detection to chase a size target.
- Do not combine this work with ADR-0004 physical compression.
- Do not require an arbitrary percentage reduction; require elimination of reconstructible duplication and transparent measurement.

## Completion

This follow-up is complete only when every persisted settlement field has a concrete authority/authentication justification, all six canonical fixtures have been hard-cut to the corrected format, the old settlement runtime path is deleted, and full historical/maintenance parity plus new byte measurements are recorded.

## Related docs

- [Roadmap](roadmap.md)
- [Minimal lossless history storage — completed plan](minimal-lossless-history-storage-linear-plan-2026-10-19.md)
- [Canonical migration results](canonical-history-migration-results-2026-10-21.md)
- [ADR 0041](decisions/0041-semantic-import-boundaries-owner-pruning-history-rewrite.md)
- [Storage format](storage-format.md)

## Notes

The existing 5.235% whole-file reduction remains a valid measurement of the completed hard cut. This follow-up exists because that result exposed a separate proof-size regression; it does not retroactively invalidate the completed implementation or its historical acceptance evidence.
