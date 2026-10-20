# Rebased compact-history freeze and format-retirement handoff — 2026-10-19

Parent: [Documentation index](INDEX.md).

## Purpose

Re-anchor the compatibility-retirement plan after the original compact-history stream advanced beyond the first freeze. Do not reset, revert or reinterpret independently accepted implementation work.

## Overview

**Current verified repository checkpoint:** `bafdaaee9abe7f7a74028044afa5d972fd38d099` (main, clean working tree at inspection), replacing the operational freeze point `39eea4a`. The earlier checkpoint remains an immutable historical reference.

**What advanced:** `fd61a1e` completed I1–I12; `c69f279` and `62d1bb2` record Step 87 measurements and Integration Gate C acceptance; `bafdaae` contains compact-field helper extraction corresponding to Step 88. The original linear plan still says **87/266 accepted** and identifies Step 88 next. The extraction commit alone does **not** prove every Step 88 acceptance requirement has passed. Verify the Step 88 gate and record it before marking Step 88 complete.

**Retirement state:** R1–R22 remain unimplemented and non-negotiable final acceptance gates. No canonical corpus promotion or runtime legacy-reader deletion is established by these four commits. The six authoritative V1.0 fixtures remain the retained migration baseline until a verified promoted replacement is recorded.

## Revised sequencing

1. Preserve the current `bafdaae` HEAD, acceptance evidence and clean working tree; do not reset to `39eea4a`.
2. Treat I1–I12 as **already completed**. Do not rerun the interruption as though it were pending or replicate its changes.
3. Resume with Step 88 **verification and evidence reconciliation**, since code exists but the step is not marked accepted. Apply a narrow corrective change only if the gate exposes a real defect.
4. Continue remaining original compact owner/domain steps from the confirmed frontier. Keep all V1 and legacy proof translation explicitly temporary and migration-only; do not create permanent compatibility contracts.
5. Once the complete V2 canonical domain conversion is capable of losslessly rewriting all retained owner histories, run R1–R22: fixture baseline, disposable migration, exact semantic/historical/byte-level verification, promoted corpus commit, removal of runtime V1 and CVAFRS03/04/05/CVAILEG1/2 compatibility, negative tests, final verification, code and document commits.
6. If a new format change occurs during the remaining implementation, inventory/migrate development data in the same change window and remove temporary machinery as soon as its last genuine migration consumer disappears. Do not defer that migration simply because the R-gates occur later.

## Invariants

- Only latest verified acceptance records authorize step completion.
- Nothing here changes the compact-history specification, the accepted historical semantics, or the full R1–R22 gates.
- The earlier freeze `39eea4a` is preserved as provenance; the **new operational checkpoint is `bafdaae`**.
- No implementation of R1–R22 has been performed by this rebase.
- Re-read the actual git status and HEAD immediately before implementation; this document is a snapshot, not a file lock.

## Related docs

- [Development format hard cut](development-format-hard-cut-interruption-spec-2026-10-19.md)
- [Format policy amendment](minimal-lossless-history-storage-format-policy-amendment-2026-10-19.md)
- [Original linear plan](minimal-lossless-history-storage-linear-plan-2026-10-19.md)
- [Interruption evidence](development-format-interruption-execution-2026-10-19.md)

## Notes

The prior freeze was not violated by lost work or a destructive merge. Further compact-history commits were made normally. This document aligns the next execution gate without discarding them.
