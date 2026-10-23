# Reliquary Phase 3 worktree and branch audit

Date: 2026-10-23  
Status: Audited, **not cleaned**.  
Primary repository: `C:\!bin\workspace\Reliquary`  
HEAD: `084ba07` on `main`. Source evidence: `.verification-tmp/phase3-worktree-inventory-2026-10-23.json`, `.verification-tmp/phase3-worktree-content-comparison-2026-10-23.json`, and `.verification-tmp/phase3-branch-content-comparison-2026-10-23.json`.

## Purpose

Explain the 13 auxiliary worktrees, determine which Phase 3 work appears incorporated into main, identify potentially unpreserved changes, and specify cleanup order. This audit does **not** authorize branch deletion, force worktree removal or new implementation.

## Overview

This audit compares all auxiliary Git histories and worktree contents to accepted main, distinguishes integrated copies from potentially unpreserved dirty work, and defines a non-destructive cleanup sequence. No cleanup has occurred.

## Findings

- **14 worktrees total**: primary main and 13 auxiliary. **Eight auxiliary worktrees have uncommitted changes**; five are clean.
- **161 uncommitted file entries** in the auxiliary worktrees: **44 exact file copies of current main**, **98 different file versions**, **10 files absent from main**, and **9 deletions of paths still present on main**. Different bytes do not necessarily mean a missing feature; many differences are older drafts subsequently refactored in main.
- All eight dirty worktree HEADs are **ancestors of current main**, but their working copies contain edits layered over those old commits. Their contents therefore cannot be recovered merely by retaining the branch pointers.
- Five clean feature branches have **30 commits not in main ancestry** (including merge commits). All **19 non-merge commits** have exact subject-matched counterparts in main. Seven are patch-equivalent; twelve differ as patches but the added code largely appears in current main. This is consistent with selective cherry-picks/merge-forward/reworking, not an entirely separate set of unfinished features.
- Main is locally **199 commits ahead of `origin/main`** according to its tracking reference; no remote fetch or push was performed. Main itself has only a pre-existing `.gitignore` modification adding `.go-cache/`, and an untracked Phase 4 draft.

## Worktree-by-worktree assessment

| Worktree suffix | Dirty entries | Extra commits | Disposition |
| --- | ---: | ---: | --- |
| `phase3-agentic-receipt` | 0 | 1 | Low risk: patch-equivalent change in main |
| `phase3-birth-replay` | 43 | 0 | **Preserve for manual hunk review:** 33 differing paths, 3 old paths absent from main, 3 deletions, 4 exact copies |
| `phase3-completion-proof` | 18 | 0 | **Preserve for manual hunk review:** 15 differing paths and old Import completion file |
| `phase3-docs-final` | 22 | 0 | Low risk: 18 exact files; 4 older versions of docs/acceptance receipts |
| `phase3-epoch-proof` | 19 | 0 | **Preserve for manual hunk review:** 17 differing paths and 2 old paths absent from main |
| `phase3-events` | 0 | 15 | Moderate: 8 non-merge changes with matching commit subjects on main, 7 not patch-identical |
| `phase3-manifest` | 0 | 10 | Moderate: 6 non-merge changes with matching commit subjects on main, 5 not patch-identical |
| `phase3-obsolete-adapters` | 0 | 1 | Low risk: patch-equivalent in main |
| `phase3-phy-lifecycle` | 27 | 0 | Moderate: partly incorporated owner-lifecycle migration, older module removals |
| `phase3-project-replay` | 4 | 0 | Low risk: source additions present in main despite two differing file versions |
| `phase3-reconciliation` | 0 | 3 | Low risk: all 3 commits patch-equivalent in main |
| `phase3-selection` | 25 | 0 | Moderate: 13 exact copies; remaining extraction drafts and retired old module paths |
| `policy-coverage` | 3 | 0 | Low risk: older policy source; substantive added lines already exist in main |

## Why worktrees were left dirty

Parallel Phase 3 extraction used detached checkouts at earlier commits to reorganize import, replay, History, PHYL/REL lifecycle, external-index and pruning code. Equivalent source changes were integrated into main, but the parallel checkout changes were **not cleaned or marked as transferred**. Dirty working copies were retained alongside accepted main. After further source corrections, those older working copies naturally diverged from main.

For example, `phase3-birth-replay` and `phase3-epoch-proof` still define older helpers such as `validate_import_settlement_scope`, `import_epoch_scope_through`, or `validate_import_completion_settlements` that do not appear by those exact names in main. Current main instead uses `src/history/completion_proof.rs::validate_import_binding` and `completion_epochs` with coordinated Import scope processing. **Whether every old validation case is preserved is not proved by name matching.** Old tests and validation are therefore a preservation risk, not automatically a regression or useful code.

The older `phase3-selection` worktree has `external_source` index/storage changes that correspond to the present `external_index` extraction; `phase3-phy-lifecycle` has older PHYL profile module paths now placed in owner lifecycle. These are likely superseded drafts but require focused hunk review.

## Concrete cleanup sequence — NOT EXECUTED

1. Protect `main`, the untracked Phase 4 specification, `.gitignore`, and the 199 locally ahead commits. Validate remote status before any push; do not force-push.
2. **Archive all eight dirty worktrees in a verified, independent location**, including binary Git patches, index state, worktree HEAD, and every untracked file. Merely preserving branch refs cannot preserve dirty content.
3. Inspect the source differences in `birth-replay`, `completion-proof`, and `epoch-proof` against accepted current Import completion/History proof and related tests. Examine the remaining PHY lifecycle and selection hunk differences. Classify each as **already integrated, intentionally superseded, or genuinely missing**.
4. If a genuinely missing invariant is found, make a bounded fix against **current main** with a regression test. Do not merge an obsolete detached Phase 3 checkout wholesale.
5. After archival/verification, remove the five clean auxiliary worktrees and then the redundant dirty ones. Delete local branches only after their tips/histories are confirmed preserved or redundant.
6. Verify `git worktree list --porcelain`, branch inventory and `main` Git status afterward. Keep Phase 4 implementation and its untracked spec outside this cleanup.

## Confidence limits

This audit compared Git ancestry, patch IDs, matching commit subjects, file hashes, source-added lines, and symbols. It did not independently execute old worktree versions or prove exact functional equivalence of every changed source hunk. The distinct uncommitted source differences are **not yet safe to discard without verified archival**. No worktrees, branches or source files were removed or reset.

## Related docs

- [Phase 3 execution and acceptance](dependency-convergence-phase3-execution-2026-10-22.md)
- [Phase 1–3 dependency closure audit](dependency-convergence-phase1-3-closure-audit-2026-10-23.md)
- [Phase 3 to Phase 4 handoff](dependency-convergence-phase3-phase4-handoff-2026-10-23.md)

## Notes

Primary `main` is the only accepted Phase 3 integration checkpoint. Detached branches and worktrees are preservation candidates, not authorized feature implementation branches.