# Reliquary rebuild — execution continuation checkpoint (2026-11-04)

Parent: [selective-reuse execution plan](architecture-rebuild-implementation-plan-2026-11-03.md) · [44 capability increments](architecture/implementation-rebuild-capabilities-2026-11-03.json).

## Exact active state

- **Implementation worktree:** `C:\!bin\workspace\Reliquary-rebuild`, branch `rebuild/clean-architecture`, verified correction commit **`95db1b412181c00ebc4f2c4292287fed93ec0cb4`**. This is a recovery/architecture checkpoint, not product acceptance.
- **Pinned independent reference:** `C:\!bin\workspace\Reliquary-reference` at `f62b1fb44ca7f0bd8351e3a30fb5d76211f85102`; do not modify it. Design documents and capability tracker remain in `C:\!bin\workspace\Reliquary-crate-first`; its unrelated pre-existing Graph evidence edit and scratch files must remain untouched.
- The **active worktree's `AGENTS.md`** is the short coding-agent entry point. Its `architecture/target-crate-dag-2026-11-03.json` is a byte-for-byte copy of this repository's approved target DAG, checked by `python scripts/check_architecture.py` from the rebuild root.
- Tracker status at this checkpoint: **F01–F04 and C01 completed; 29 increments in progress, 10 not started; none of G1–G5 accepted**. In-progress source implementations and green package tests must not be promoted to accepted capabilities without their full behavior and dependency evidence.

## Architecture correction completed

Commit `95db1b41` preserved previous in-progress Insomnia worker and verified-copy work and corrected architecture drift:

- Restored exactly **23 Rust packages and 76 approved production edges**, with no Cargo dependency cycles. Application's seven unplanned durable-domain production edges and Gateway's direct Archive edge are absent; test-only lower-library dependencies remain in `dev-dependencies`.
- Added automated **fail-closed** package edge and upper-layer direct-reference verification in `scripts/check_architecture.py`. Any proposed package graph change needs an explicit architecture decision first; never defeat the checker to unblock a call site.
- Kept owner-session as the single typed mutable owner. Application consumes selected immutable owner value types; grouped Memory record encoding stays at owner-session's persistence boundary. Gateway delegates native Archive publication to an Application workflow and retains authorization/queue scheduling/host responsibility.
- Split large Gateway queue and Instance modules, owner-session writer/replay/grouped-commit modules, and Insomnia store into focused responsibility modules. Many unrelated source files remain over 300 lines; that is **targeted cleanup debt**, not authorization for a new all-codebase relocation.
- Fixed an undeclared Insomnia `serde` dependency. No runtime 2.0 reader or replacement compatibility facade was introduced.

**Verified** on the corrected worktree: `python scripts/check_architecture.py` passed (23 packages, 76 production edges); `cargo fmt --all --check` passed; `cargo check --workspace --offline` passed; `cargo test --workspace --offline -j 1 -- --test-threads=1` passed including doctests, with no failing suite. The run required recovering 9.4 GB from **disposable alternate build target directories only** after low disk space caused a Windows PDB linker failure.

## How Terra should continue

Execute the **existing capability plan as written**; no new overarching phase or advance subdivision of its 44 increments. For each next concrete capability: inspect its acceptance and relevant existing code, complete it in the approved owner/service/workflow, prove read/write/reopen/privacy/crash and negative behavior where applicable, run the architecture guard and tests, document evidence, and commit. Break down only the individual increment that demonstrably cannot be implemented safely in one pass.

**Dependency semantics:** `depends_on` controls **acceptance**, not permission to inspect or provisionally implement code. Gateway H01–H03 and several semantic/application parts have partial code ahead of G1/G2 acceptance; **do not call them complete**. Concentrate first on completing/verifying prerequisite storage owner increments and **G1**, then semantic/application **G2**, Gateway recovery/host **G3**, public API/CLI **G4**, and independently verified converter/activation **G5**.

**Specific remaining blocker:** F03 recorded historic FRS07 read evidence but not independent six-file full-history/identity proof. Do not proceed with **D01** unless the authentic old-format source and its fingerprints are verified. This does **not** block synthetic REL 2.1 work.

**Mandatory checkpoint checks from `Reliquary-rebuild`:**

```powershell
python scripts/check_architecture.py
cargo fmt --all --check
cargo check --workspace --locked --offline -j 1
cargo test --workspace --locked --offline -j 1 -- --test-threads=1
```

The target CLI and documentation tree are **not yet built**. Only run CLI or replacement documentation acceptance commands after those artifacts exist, and record the absence as unfinished capability work rather than treating commands against the old source as proof. Preserve the clean rebuild branch and commit verified increments; never accumulate another huge uncommitted refactor. No new runtime-format compatibility or cutover before the required gates.

This checkpoint is a **status and execution clarification**, not a change to the Stage 1–5 target architecture or gate acceptance criteria.
