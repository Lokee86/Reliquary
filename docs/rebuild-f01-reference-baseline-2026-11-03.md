# F01 — Freeze the implementation reference (2026-11-03)

Parent index: [Documentation index](INDEX.md)

**Status: COMPLETED and independently verified — 2026-11-03.** Execution evidence: `C:\!bin\workspace\reliquary-f01-evidence-2026-11-03.md`; separate verified dirty-state backup: `C:\!bin\workspace\Reliquary-reference-evidence\F01.2-f62b1fb4\evidence.json`. Detached/locked clean reference at `C:\!bin\workspace\Reliquary-reference`, pinned at `f62b1fb44ca7f0bd8351e3a30fb5d76211f85102`. Both `cargo metadata --format-version 1 --no-deps --locked` and `cargo check -p reliquary-container --locked --offline` passed with an independent target directory. Parent: [selective-reuse rebuild capability plan](architecture-rebuild-implementation-plan-2026-11-03.md), item `F01` in the [capability DAG](architecture/implementation-rebuild-capabilities-2026-11-03.json). **Goal:** make the existing implementation a *safe, reproducible source of code and behavioral evidence*, not a system that the rebuild must reproduce internally.

## Purpose

Pin the current implementation as a reference without changing its working tree, losing unrelated edits, copying generated build debris, or beginning the replacement prematurely. F01 is only reference establishment. **F02** chooses supported behavior; **F03** investigates the FRS07 corpus and identity goldens; **F04** creates the clean target architecture.

## Overview

F01 consists of **seven small actions**, each with an explicit result. The only mutations permitted at execution are creation of an independent **detached reference worktree** and an evidence record/backup outside the original working tree. No Rust code, working reference, data corpus, or target crate is changed. Where an operation would overwrite an existing path, **stop rather than force**.

### Observed baseline on 2026-11-03

- Primary candidate source: `C:\!bin\workspace\Reliquary-crate-first`, branch `refactor/crate-first-restart`, commit `eb74c8ccd1f9b2bafcf981be62b41adc4e58e731` **before this plan is committed**. Pin the exact HEAD again when executing F01; do not reuse the sample SHA blindly.
- `C:\!bin\workspace\Reliquary` is a separate `main` worktree at `f431401e40c3290f4df0662ac31ee7e5a6410977` (older, 2026-10-25). It can supply additional old implementation context, but is **not automatically the primary reference**.
- `Reliquary-baseline-audit-2026-10-25` and `Reliquary-phase5` are already registered worktrees. A stale/prunable `Reliquary-phase5/target/phase5-acceptance-checkout` registration exists. **Do not prune or repair it as part of F01.**
- One **uncommitted tracked file** exists in the candidate: `docs/graph-crate-extraction-evidence-2026-10-26.md` (25 inserted lines at inspection). This is user work; preserve its bytes and diff. Untracked `.t/` test/data scratch and the existing `target/` are also not to be cleaned.
- Git is `2.55.0.windows.3` and supports `git worktree add --orphan`. Neither `C:\!bin\workspace\Reliquary-reference` nor `C:\!bin\workspace\Reliquary-rebuild` existed at inspection time. Check again before any creation.
- The existing `target/` contains many `chronos-*` builds and has previously triggered long nested Windows paths. New worktrees must use **separate, short `CARGO_TARGET_DIR` values**, not re-use this tree or delete it.
- The `tests/current_format_boundary.rs` promoted six-file test is **ignored** and requires `RELIQUARY_CURRENT_CORPUS_ROOT`. Its correctness is F03's task, **not** a prerequisite to creating the reference snapshot.

## Seven execution actions

| ID | Single objective | Concrete action | Evidence / acceptance |
| --- | --- | --- | --- |
| **F01.1** | Identify source revision and existing worktrees | Read `git rev-parse HEAD`, `git branch --show-current`, `git worktree list --porcelain` and `git status --short`; record main and other worktree commits | One exact primary source SHA, reference path, common Git-dir path and known stale registration; no Git metadata cleanup |
| **F01.2** | Preserve the candidate's dirty state | Record tracked diff/stat and hash of the modified Graph document; save its binary-safe patch **outside** the working tree. List untracked scratch by top-level family only; do not copy or remove entire `target/` or `.t/` | Original tracked SHA/diff and patch are recoverable; after action candidate status and Graph file bytes unchanged |
| **F01.3** | Pin the implementation source | Choose the checked current crate-first HEAD as **primary code reference**, note optional `main` SHA as secondary, and create a compact manifest with source SHA, branch, toolchain, `Cargo.lock`, compiler/config files and fixture locations (not giant per-function mappings) | One plain-text manifest with immutable Git revision and source-corpus locations; no requirement to migrate any old Rust module |
| **F01.4** | Materialize a clean read-only reference checkout | Recheck that `C:\!bin\workspace\Reliquary-reference` is free; run `git worktree add --detach --lock --reason "Reliquary rebuild reference" C:\!bin\workspace\Reliquary-reference <pinned-sha>` from the existing Git repository. Treat it as read-only; `--lock` protects registration from pruning, **not files from edits** | Reference worktree HEAD is exactly the pinned SHA and `git status --porcelain` is empty; no `git worktree -f/-B` overwrites |
| **F01.5** | Verify independent reference build tooling | From the reference checkout, run `cargo metadata --format-version 1 --no-deps --locked`, then a minimal `cargo check -p reliquary-container --locked --offline` with its **own short external `CARGO_TARGET_DIR`**. If pinned dependencies or the existing build fail, capture exact logs/status without code changes | Toolchain/build availability is **proven or explicitly blocked**; no invented green baseline. Full algorithms/performance belong to F02 |
| **F01.6** | Reserve the rebuild isolation contract | Verify an unused `C:\!bin\workspace\Reliquary-rebuild` path, available `rebuild/clean-architecture` branch name and distinct short build-output directory. Record that **F04**, not F01, will execute `git worktree add --orphan -b rebuild/clean-architecture ...` to produce an **empty target tree** | The future clean-worktree creation is safe, but **not performed** in F01. No automatic checkout/copy of legacy implementation into the rebuild |
| **F01.7** | Recheck and close F01 | Compare original HEAD and modified-file SHA with F01.1–2; verify detached reference SHA, clean checkout, lock registration and manifests; record actual smoke-test results, blockers and downstream pointers | Original tree unchanged, reference separately pinned, rebuild path untouched, no commits staged in original, F02/F03/F04 may use the explicit evidence |

**Dependency order:** F01.1 → F01.2 → F01.3 → F01.4 → F01.5 → F01.7. F01.6 is independent after F01.1 and must finish before F01.7.

## Output and stop conditions

The implementation of F01 should produce **one small evidence note** (pinned SHAs, worktree paths, dirty-file patch/hash custody, toolchain, fixture location map, smoke-test result and reserved rebuild path), plus the detached reference checkout. Place the evidence note in the design repository *after* the source snapshot is pinned; commit only the note if desired, never the unrelated dirty Graph change. A pinned Git commit already fingerprints tracked source: **do not generate a 1,415-file transfer manifest**.

**Hard stops:** reference destination or branch exists; shared Git ref would be overwritten; dirty Graph document cannot be safely preserved; source/ref SHA changes unexpectedly; worktree HEAD mismatches the pin; command attempts cleanup, `git worktree prune`, broad checkout/reset, `cargo clean` or unrelated format repair. Treat the missing historical FRS07 admission as **a separately recorded F03 blocker**; it does not prevent F01 closure.

**Do not do in F01:** create replacement Rust crates, rewrite source, rename CVA→REL in code, repair FRS07, select every feature for retention, run expensive full benches, import the original tree into the new worktree, or clear old `target/`/scratch.

## Related docs

- [Capability-first build plan](architecture-rebuild-implementation-plan-2026-11-03.md)
- [Capability DAG, F01–F04](architecture/implementation-rebuild-capabilities-2026-11-03.json)
- [Current implementation inventory](current-system-inventory-2026-11-03.md)
- [Target crate design](dependency-architecture-and-crate-layout-2026-11-03.md)
- [Stable reference and cutover ADR 0049](decisions/0049-parallel-architecture-rebuild-and-verified-cutover.md)
- [Current-format test](../tests/current_format_boundary.rs)

## Notes

This is a **small setup step**, not an attempt to decompose the entire rebuild now. The new system is designed and implemented incrementally by its *own capabilities*; the reference provides selected source and behavioral oracles only. The separate reference worktree and independent build target were subsequently created and verified during F01 execution. The original `Reliquary-crate-first` working tree, its unrelated Graph edit, and ignored scratch/target directories were preserved; no clean rebuild worktree has been created yet.
