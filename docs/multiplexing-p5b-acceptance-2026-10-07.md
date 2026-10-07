# P5-B lane acceptance — 2026-10-07

Parent: [P5-B plan](multiplexing-phase5b-background-host-2026-10-06.md)

## Purpose
Record lane B implementation, actual verification and bounded handoffs without claiming whole P5/P6 convergence.

## Overview
Base: `c24b06273abbab383eb74780bf82cb5c0ceef0b5`. Branch: `feat/multiplexing-p5-background`. Worktree: `Reliquary-p5b-background`. Main, A's worktree and Warlock were preserved. B0–B4 source work is complete; final verification is recorded below. Consumer convergence, Warlock provider acceptance, legacy deletion and P6 release remain separate gates.

## Implemented source
- B0: [capability/worker ledger](multiplexing-p5b-capability-ledger-2026-10-06.md), exact C method/grant/durability table and D old-only deletion manifest.
- B1: exact REL/private PHY status, wake and background switches; trusted canonical route replacement; one worker set per mounted owner, independent of attachments; restart fencing and stale profile retirement; PHY vector profile and batch sync before publication.
- B2: only ConversationExecution has an executable host operation/receipt path. Insomnia, Dream, Embedding and ProjectEnvironment remain Native, including unrelated host outages; recovered debt remains Held. Stable ID/digest is checked before adapter I/O; unknown/untracked retry cannot dispatch.
- B3: focused REL and PHY blocked-provider tests prove owner reads progress and closed fences discard late results. Reopen invalidates old tokens and cannot double-start workers. A public staged cancellation test proves Running cancellation is cooperative, Unknown is Held without replay, and later real completion commits exactly once.
- B4: production source inventory found no ambient active REL/PHY or legacy host receiver in new background controls, canonical execution owners or capability router. Root wiring and legacy source deletion remain D's responsibility.

No algorithm redesign, disk schema change, new queue owner, legacy compatibility facade or Warlock adapter is introduced. Insomnia keeps its existing separate processing switch; the new REL switch covers Dream/Perception/vectors.

## Verification
Run on this worktree on 2026-10-07. Raw command logs are in ignored `target/p5b-verification/`.

| Command / evidence | Actual outcome |
| --- | --- |
| Baseline `cargo test --lib runtime_host::gateway -- --test-threads=4` | 236 passed |
| First implementation focused gateway rerun | 242 passed; two final cases added afterward and covered by final run below |
| `cargo fmt --check` and `cargo check` | Passed |
| Unrestricted parallel `cargo test -- --test-threads=4` | Build failed before tests: rustc Windows 0xc0000409 and missing dependency artifacts; serial rerun below passed |
| Final `cargo test -j1 -- --test-threads=4` | Passed: 1,093 library, 128 integration; zero doc tests, zero failures |
| `cargo check -j1 --all-targets` | Passed, including examples |
| CLI fmt/check/test with manifest; check/test `--locked` | Passed; 19 CLI tests |
| P2 and P3 ownership guards `--enforce` | Passed |
| P4 coverage guard `--self-test` | Passed; 37 fixtures, 120 public APIs classified; exact ephemeral native controls cannot contain authoritative CVA writes |
| Documentation structural and changed-from `origin/main` checks | Passed; structural and changed-file impact |
| `python scripts/check_architecture.py --refresh` | Timed out after 240 seconds during Lexicon init; Arcana sync and Pitlord check were not reached. Coverage guard passed first. Not green. |

The full functional run covers existing Insomnia/frozen publication/offline PHY/restart, host crash/lost acknowledgment/downgrade/dequeue races, independent lifecycle and Freshness characterization as well as eight new lane B tests. Fake host tests establish gateway mechanics; they do not certify a real external provider.

## Documentation impact
Inspected: architecture, API, invariants, behavioral contracts, storage format, maintainer map, limitations, coverage, roadmap, ADR 0037/0038 and phase plans.
Updated: canonical worker/capability/API truth, ephemeral control/privacy contracts, durability boundary, documentation routing, this acceptance and the capability ledger.
Not affected: semantic algorithms/formats, CLI behavior, external provider protocol; final roadmap and ADR singleton rewrite belongs to source convergence.
Compliance check: final documentation outcomes above.
Known documentation gaps: C provider acceptance and D final legacy removal/current-state rewrite remain pending.

## Architecture impact
Standards added or changed: no shared engineering standard changed.
Ownership or boundary impact: existing mounted RelRuntime/PhyRuntime own background control state; one finite gateway route owner; only executable host capability can acquire new Host authority.
Pitlord enforcement impact: policy unchanged; P4 source guard extended with finite native-control routes and positive/negative fixtures.
Other verification impact: eight new focused Rust tests, existing full suite and source/doc guards; architecture result above is not substituted by source checks.
Known architectural gaps: real host integrations for four declared stages do not exist; synchronous native endpoint calls lack per-call abort and can delay stop/join. Running work is not rollback.

## Related docs
- [Capability and worker ledger](multiplexing-p5b-capability-ledger-2026-10-06.md)
- [Parallel execution and convergence](multiplexing-phase5-parallel-execution-plan-2026-10-06.md)
- [P5-C consumer lane](multiplexing-phase5c-external-consumers-2026-10-06.md)
- [P5-D deletion lane](multiplexing-phase5d-legacy-deletion-2026-10-06.md)

## Notes
Lane B completion does not authorize blind host replay or imply P5/P6 completion. Convergence must use accepted A/B APIs and a verified C adapter, then perform D deletion and final P6 verification.
