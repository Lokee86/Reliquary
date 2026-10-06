# Final multiplexing P3/P4 integration into main

Parent index: [Documentation index](INDEX.md)

## Purpose

Record integration of the accepted conversation and distribution lanes with current main, final verification, and removal of redundant multiplexing branches/worktrees.

## Overview

P3 input: a6cee63d53232f397519ca19b93f5db9351a8dee.
P4/convergence input: 963beee8dc68760ca7fb0e409d59ca7a3d9b273b.
Existing main input: 780aff5b391a36d062a0d6e02be0c7b768ba5cf1.
P3 merge: f0e567ef6a9fba14f5b31cd3bab80b47f8d478ba.
Combined merge: c3849e761197e35e74115e6686124378cf3f858e.
The final integration commit also contains the verified freshness call correction described below and this acceptance/status record.

The final integration preserves main's freshness implementation and the complete P3/P4 history. Main advances to the verified integration head; no remote push is included in this operation.

## Current behavior

P3.8 recovery/checkpoint integrity and coordinator hard-cut checks now coexist with P4's production REL feed and exact conversation stream binding. Existing distribution publication gates and recipient authorization remain intact. Recovery was extracted beneath the existing conversation owner, without losing distribution hooks or introducing a second journal, scheduler or subscriber owner.

Git merged Rust source automatically, but combined compilation exposed the P4 verified Insomnia commit path's older `commit_application` call. The integration passes the REL freshness store, Dream freshness journal, owner identity and accepted-turn count already used by main's prepared commit path, preserving grouped freshness birth semantics alongside verified personal publication. Documentation conflicts were resolved by retaining both feature sections and both change-policy rules. Current status references now distinguish the integrated P3/P4 implementation from remaining Phase 5 public-export/host-consumer migration. Historical subphase acceptance records retain their original scope and outcomes.

## Verification

| Gate | Combined result |
| --- | --- |
| Root formatting and locked offline check | Passed |
| Library tests | 1,085 passed |
| Integration tests | 128 passed across 18 executables |
| CLI formatting and locked offline check | Passed |
| CLI tests | 19 passed |
| P2 ownership guard | Passed; no gateway violations |
| P3 ownership guard | Passed; no violations |
| P3 guard fixtures | 7 passed |
| P4 coverage guard | 28 fixtures passed; 112 public APIs classified |
| Documentation structure and origin/main impact | Passed; repeated after final record/status edits |
| Git diff check | Passed |
| All-target/example compilation (`cargo test --no-run -j 1`) | Passed |
| Doctests | Passed; 0 tests present |
| Architecture refresh | Timed out in Lexicon indexing after 300 seconds; no current Arcana snapshot |

The first concurrent all-target compile exhausted the Windows paging file (OS error 1455), followed by metadata/dependency-artifact errors. The serial retry passed without source changes; unrestricted parallel compilation is not claimed. Library, integration and CLI tests were executed separately and passed as listed above.


Logs: C:\!bin\workspace\multiplexing-p34-final-verification.
Cargo used the existing isolated target at C:\!bin\workspace\multiplexing-p3-5-verification\target-lib, with locked offline dependencies and serial library/integration execution.

## Cleanup and preserved drafts

Cleanup removes the merged foundation, Phase 1, Phase 2, Phase 3, Phase 4 and convergence worktrees/branches, and the ancestor-only detached P4 acceptance worktree. Deletion uses ancestry checks and clean worktrees, after main contains their commits. The unrelated unmerged fix/warlock-owner-archive-fallback branch remains.

Three dirty worktrees were preserved in Git stashes before cleanup:

| Original worktree | Stable stash identity | Contents |
| --- | --- | --- |
| Phase 1 | b547f9ef1257f83c5a28f3545bcdb0054df9d181 | Uncommitted planning/index drafts |
| Phase 2 | c35e10d210879e6b693333fe6a363b1e01bbfefd | Parallel P3/P4 planning drafts |
| Detached P4 acceptance | b3d4e990ac2b75769dc8351ab903513525d9bf60 | Unfinished acceptance documentation and four extra tests |

These drafts are retained rather than treated as accepted implementation. In particular, the extra detached-worktree tests have no verification claim in this record. Stash hashes remain reachable independently of the removed feature branches. Historical acceptance documents and verification evidence are retained.

## Documentation impact

- Inspected: repository/shared engineering guidance, architecture, invariants, storage format, maintainer map, roadmap, limitations, coverage, P3/P4 execution and acceptance records, merge conflicts and dirty worktree drafts.
- Updated: combined documentation conflict resolutions, both docs-policy rule sets, this record, index, current P3/P4 status, roadmap, limitations and coverage.
- Not affected: frozen P34 shapes, persistence formats, remaining Phase 5 scope and unrelated unmerged fixes.
- Compliance check: final structural and origin/main change-impact documentation checks passed.
- Known documentation gaps: no new integration gap; historical subphase limits remain explicit.

## Architecture impact

- Standards added or changed: none; both existing change-policy rules are retained.
- Ownership or boundary impact: final P3 recovery and P4 distribution coexist under their existing owners; no new semantic authority.
- Pitlord enforcement impact: `python scripts/check_architecture.py --refresh` timed out after 300 seconds during Lexicon Rust/Python indexing. The P4 source-coverage preflight passed, but Arcana/Pitlord did not run. This remains a structural-analysis tool gap, not a passing architecture claim. A subsequent non-refresh check exited 1 because no current Arcana snapshot exists.
- Other verification impact: combined library/integration/CLI and P2/P3/P4 guards listed above.
- Known architectural gaps: external consumer/public-export migration remains Phase 5; any structural-analysis limitation is recorded above.

## Related docs

- [P3.9 acceptance](multiplexing-phase3-local-acceptance-2026-10-05.md)
- [P3.8 recovery](multiplexing-phase3-recovery-acceptance-2026-10-05.md)
- [P4.9 acceptance](multiplexing-phase4-acceptance-2026-10-05.md)
- [P4 production convergence](multiplexing-phase4-convergence-2026-10-05.md)
- [Parent plan](multiplexing-implementation-plan.md)

## Notes

Branch cleanup preserves committed history through main and uncommitted drafts through named stashes. This integration does not claim completion of the remaining multiplexing phases.
