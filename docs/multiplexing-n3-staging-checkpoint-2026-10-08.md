# Multiplexing N3 staging checkpoint — 2026-10-08

Parent: [Next execution wave](multiplexing-phase5-next-parallel-execution-plan-2026-10-07.md)

## Purpose

Record the accepted E/F/G-import source integration and the remaining gates for final Reliquary convergence and the locked Warlock dependency cut.

## Overview

Status: **E5, F-core and G-import integrated and functionally verified on N3 staging; final N3 acceptance remains gated.**

Branch: `feat/multiplexing-n3-staging`. Worktree: `C:\!bin\workspace\Reliquary-n3-staging`.
Accepted N0 base: `999fbf7d85c9f892a1bf979293588033bcc1484b`.
Previous F/G merge: `38754638f0e551bb3255142ef478c7bca55f20da`.
Previous checkpoint: `737c8618d5524a6465187e637e5fb3ede667c66c`.

| Lane | Accepted source |
| --- | --- |
| E5 | `874851ac4c81fb9bb29f897fefe421c335fac7d4` |
| F-core | `13df1ff14ea8d7bfb1777289968ff7d5c87b3998` |
| G-import | `99b9fa5e83f9c0052d5ceef0952a47275b3903cc` |

E adds the accepted native/full-host-contract live adapter and narrow P3 cancellation/tool/artifact seams. F remains stateless management composition, and G-import retains pure normalization and its existing Archive/Insomnia publication owner. Gateway registers both E and F modules. The combined coverage guard preserves both factories and scans Gateway, management and all production live modules. Shared documentation preserves each lane's current contract rather than selecting one side of the merge.

The combined source introduces no duplicate semantic authority, format change, provider-contract substitute or compatibility forwarding. Named root exports remain the single convergence-owned cut. The N0 convergence branch and Warlock pins are unchanged by this staging import.

## Verification

| Gate | Result |
| --- | --- |
| Root formatting and locked check | Passed |
| Focused E scenarios | 17 executed and passed |
| Unrestricted root matrix | 1,138 unit and 136 integration tests passed; none ignored |
| CLI formatting, locked check and tests | Passed; 19 tests |
| P2/P3 enforced ownership and P3 fixtures | Passed; 7 fixtures |
| P4 coverage | Passed; 71 fixtures, 152 public APIs |
| Management guard | Passed; 25 fixtures |
| Pitlord policy validation | Passed; 16 areas, 19 rules |
| Archive roundtrip smoke | Passed; 6 nodes, 2 branches, 2 fragments and 8 expanded references preserved after reopen |
| Fresh architecture refresh | Incomplete; exit 124 after 240 seconds during Python indexing |

Logs are retained in `target/n3-e-import-verification/`. The source matrix was executed on the resolved E/F/G-import tree, with the shared verification target and stripped development/test debug information. Existing compiler/linker warnings remain. Focused E acceptance asserts that all 17 intended tests actually execute; a zero-match filter cannot satisfy this gate.

The fresh `python scripts/check_architecture.py --refresh` attempt passed its embedded P4 guard, then timed out after 240 seconds during Lexicon Python indexing. Its complete subprocess tree was terminated. Arcana synchronization and graph-based Pitlord enforcement did not execute; the policy validation and direct guards above do not substitute for fresh graph enforcement.

Both documentation policy modes and staged whitespace checks passed on the final documented tree.

## Previous attempt

The earlier F/G-only checkpoint passed formatting, locked check and direct guards, but its unrestricted tests stopped before assertions with Lore static-archive disk exhaustion. CLI check/tests and Archive smoke did not run. Its 240-second architecture attempt timed out during Python indexing. Those results remain historical evidence; the E/F/G-import matrix above supersedes that incomplete functional run. No stale graph is used as fresh enforcement evidence.

## Remaining convergence gates

| Gate | Current state | Required next outcome |
| --- | --- | --- |
| Named E/F root exports | E/F return types remain reachable through inferred public factory results; named root exports are pending | Perform one export cut and verify explicit downstream type imports |
| G-external R0–R4 | No accepted framework-neutral external-core commit imported | Finish rebuild/privacy/lifecycle acceptance on its owning lane and integrate its exact accepted commit |
| B/C execution-only provider | E tests the existing full ConversationExecution host contract; production Warlock does not advertise a durable pending queue | Resolve truthful execution-only readiness, stable ID/digest reconciliation and cooperative cancellation |
| Exact task-root retrieval and attribution | E provides native accepted generation/tool/file receipts; broader C composition still needs acceptance | Verify authorized C/A/F composition against the converged surface |
| Fresh architecture graph | fresh Lexicon indexing timed out at 240 seconds during Python analysis, before Arcana synchronization and graph-based Pitlord enforcement | Complete current Lexicon/Arcana indexing and Pitlord enforcement |
| Final C matrix and pins | C-close and historical Reliquary/Arcana pins remain unchanged | Advance accepted convergence, publish/pin the exact revision, align Arcana and run all C gates |

F5 selective import/export remains optional at N3. G3/G5 host-specific adapters remain separate tails, distinct from the unfinished G-external core. P6-H5 is separately accepted on N0; post-D port and measurements remain P6-R work.

## Resume sequence

1. Complete the single named export cut and G-external acceptance; integrate exact accepted owner commits.
2. Resolve B/C production provider conformance and fresh architecture enforcement, and run checks appropriate to the resulting source changes.
3. Advance the N0 convergence line only after combined acceptance.
4. Pin Warlock to the exact accepted Reliquary revision and matching Arcana; verify one Arcana identity, locked Rust/provider/admission/conversation fixtures, frontend build and actual product acceptance.
5. Keep D deletion gated by final C/E/F acceptance; P6-R remains post-D.

## Documentation impact

- Inspected: canonical architecture/invariants/storage/maintainer/limitations/roadmap/API, E/F/G acceptance records, next-wave plan, previous N3 checkpoint and shared standards.
- Updated: combined canonical lane sections through conflict resolution, this checkpoint and next-wave staging status.
- Not affected: historical N0 evidence, owner-lane acceptance evidence, G-external drafts, Warlock source/pins, CLI behavior and P6 historical measurements.
- Compliance check: structural and changed-from origin/main policy modes passed; staged whitespace check passed.
- Known documentation gaps: final named exports, G-external and actual C/N3 release acceptance remain open and explicit.

## Architecture impact

- Standards added or changed: none.
- Ownership or boundary impact: E recipient adaptation and narrow P3 validation coexist with F management composition and G normalization/Archive import ownership; existing P3/P4/A/B authorities remain canonical.
- Pitlord enforcement impact: preserve F management and G native-import policy rules; combine E/F source classification and module scans.
- Other verification impact: merged root/CLI functional matrix, focused E and public consumer recovery, P2/P3/P4/management guards and Archive roundtrip smoke.
- Known architectural gaps: fresh Lexicon indexing timed out at 240 seconds during Python analysis, before Arcana synchronization and graph-based Pitlord enforcement; named exports, G-external authority and production provider/consumer acceptance remain separate.

## Related docs

- [E5 acceptance](multiplexing-p5e-e5-acceptance-2026-10-08.md)
- [F-core execution](multiplexing-p5f-core-execution-2026-10-08.md)
- [G-import execution](multiplexing-p5g-import-execution-2026-10-08.md)
- [Architecture](architecture.md)
- [Current limitations](current-limitations.md)

## Notes

This staging import does not complete D deletion, final Warlock pinning or the Phase 5 release.
