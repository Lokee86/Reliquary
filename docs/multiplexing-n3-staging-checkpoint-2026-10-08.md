# Multiplexing N3 staging checkpoint — 2026-10-08

Parent: [Next execution wave](multiplexing-phase5-next-parallel-execution-plan-2026-10-07.md)

## Purpose

Preserve accepted independent lane integration and identify the prerequisites for final Reliquary convergence and the locked Warlock dependency cut.

## Overview

Status: **N3 blocked; F-core/G-import staged, not final convergence acceptance.**

Branch: `feat/multiplexing-n3-staging`. Worktree: `C:\!bin\workspace\Reliquary-n3-staging`.
Base: `999fbf7d85c9f892a1bf979293588033bcc1484b`.
Staged source merge: `38754638f0e551bb3255142ef478c7bca55f20da`.

The branch contains F-core `13df1ff14ea8d7bfb1777289968ff7d5c87b3998` and G-import `99b9fa5e83f9c0052d5ceef0952a47275b3903cc`. The only merge conflict was the next-wave status table; both accepted lane descriptions and unique evidence records survive. No new Rust edits or final root export cut were made. The accepted N0 convergence branch stays at its original base pending the prerequisites below.

## Prerequisite audit

| Gate | Inspected current evidence | Required outcome |
| --- | --- | --- |
| E contract freeze and E5 | E `79a2d3424d821fed85ced6a833bb70e7c6677457`; its execution record expressly marks the adapter a draft and N1/N3 blocked | Complete durable session creation, original-token external cancellation, tool/artifact receipts, remaining lifecycle/diagnostic scenarios; freeze and accept the public consumer contract |
| G-external R0–R4 | `feat/multiplexing-p5-external` still points at N0 with uncommitted storage, runtime, Insomnia and ownership-policy changes; no accepted external-core commit | Finish owner-lane rebuild/privacy/lifecycle verification and commit exact framework-neutral source contract |
| Execution-only provider | C-close `a65ee92` records routing still requires durable pending-queue guarantees | B/E/P3 provide truthful execution-only readiness, stable-ID/digest reconciliation and cooperative cancellation |
| Exact task-root retrieval and durable attribution | Current C-close audit records selection-dependent search and missing accepted generation/source/file receipts | A/E/F/P3 supply exact authorized composition and receipt operations |
| Final C matrix and pins | C-close is open; production Reliquary pin `83b3ac87`, direct Arcana `66b4e96e` remain historical | After accepted N3, pin its exact published revision and matching Arcana, regenerate lock, verify one Arcana identity and run all C product gates |

F5 is optional at N3 and remains unimplemented. G3/G5 host-specific adapters remain separate visible tails; they are distinct from the unfinished G-external core.

E and G-external worktrees were inspected only. Their drafts were not promoted into accepted convergence or rewritten. Warlock pins and source remain unchanged. P6-H5 `b2b4bf4` is separately accepted on N0; its post-D port and measurements remain P6-R work.

## Verification

Root formatting and `cargo check --locked -j1` passed against merge `3875463`. The full `cargo test --locked -j1 -- --test-threads=4` attempt failed before assertions while compiling Lore: Windows error 112, insufficient disk space when building the static archive. It is not a test pass or a source-regression claim. The sequential matrix stopped there; CLI check/test did not run. Only 17 temporary Rust object files produced by this failed attempt (470,954,891 bytes) were removed; previous accepted test binaries, measurement reports, source and unrelated caches were preserved.

Independent gates passed: CLI formatting; P2/P3 enforced ownership; P4 coverage (42 fixtures, 149 APIs); management ownership (25 fixtures); Pitlord policy validation (16 areas, 19 rules); documentation structural/change-impact; and baseline whitespace. No merged-library test count is claimed. The archive example smoke was not executed because the required build could not finish.

A bounded `python scripts/check_architecture.py --refresh` attempt passed its embedded P4 guard, then timed out at 240 seconds during Python indexing after Rust analysis. Arcana synchronization and graph-based Pitlord enforcement did not complete. No stale graph is substituted.

Logs and command results are retained in `target/n3-staging-verification/`: `results.json`, `guard-results.json`, numbered build logs and guard logs. Matrix job: `job_uU9TKwxgvRKOn2lyVWXjbsPn`; guards: `job_hxiD4NuGz0g1V72r3NmBRn2l`; architecture: `job_XrrX7df2-z_kLSpP4RoQPJqy`. Root/CLI tests, archive smoke and fresh graph enforcement must be repeated with sufficient build space before acceptance.

## Resume sequence

1. Complete and accept E and G-external on their owning lanes, including the runtime owner seams requested by C.
2. Integrate those exact accepted commits into this staging branch, resolve the single final root export cut and DTO ownership, and repeat the complete library/CLI/guard matrix.
3. Advance the N0 convergence line only after that combined acceptance passes.
4. Pin Warlock to the exact accepted revision and align Arcana; run locked Rust/provider/admission/conversation fixtures, frontend build, dependency identity, source inventory and actual product acceptance.
5. N4 deletion remains gated by final C/E/F acceptance; N5/P6-R remains post-D.

## Documentation impact

- Inspected: next-wave/E/F/G acceptance records, C-close dependency audit, canonical architecture/invariants/storage/maintainer/limitations/roadmap/API and shared standards.
- Updated: this checkpoint, next-wave integration status and index; merged F/G canonical contract/evidence documentation.
- Not affected: historical N0 evidence, E/G-external drafts, Warlock source/pins, CLI command behavior and P6 historical measurements.
- Compliance check: structural and changed-from origin/main checks passed on the merged tree; final checkpoint checks passed before committing..
- Known documentation gaps: final E/G-external contracts and actual C/N3 acceptance remain open; no stronger completion claim is made.

## Architecture impact

- Standards added or changed: none.
- Ownership or boundary impact: F management composition and G normalization/Archive import ownership are preserved; no forwarding authority or duplicate runtime introduced.
- Pitlord enforcement impact: preserve F management rules and G native-import semantic rules.
- Other verification impact: merged root/CLI functional matrix, direct ownership/coverage checks and bounded architecture refresh.
- Known architectural gaps: the fresh architecture refresh timed out during Python indexing at 240 seconds; full functional matrix is unverified due to disk exhaustion; final E/G-external authority and consumer export integration remain unaccepted.

## Related docs

- [F-core execution](multiplexing-p5f-core-execution-2026-10-08.md)
- [G-import execution](multiplexing-p5g-import-execution-2026-10-08.md)
- [Architecture](architecture.md)
- [Current limitations](current-limitations.md)

## Notes

This checkpoint does not authorize D deletion, a final Warlock pin, or a Phase 5 completion claim.
