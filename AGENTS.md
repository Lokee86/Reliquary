# Reliquary clean rebuild — coding-agent instructions

This is the **replacement implementation worktree**, branch `rebuild/clean-architecture`. Work in this tree. Do not move old Root implementation wholesale or modify the pinned reference tree.

## Authoritative specification (in the adjacent design/source worktree)

- Execution plan: `C:\!bin\workspace\Reliquary-crate-first\docs\architecture-rebuild-implementation-plan-2026-11-03.md`
- Capability DAG and acceptance: `C:\!bin\workspace\Reliquary-crate-first\docs\architecture\implementation-rebuild-capabilities-2026-11-03.json`
- Storage, semantic services, runtime, and format: the Stage 1–5 documents linked by that plan.
- Current continuation evidence: `C:\!bin\workspace\Reliquary-crate-first\docs\rebuild-continuation-2026-11-04.md`
- **Executable package-edge contract in this worktree:** `architecture/target-crate-dag-2026-11-03.json`; guard: `scripts/check_architecture.py`.

When prose and executable package boundaries disagree, **do not invent a workaround**. Verify the relevant architectural decision and explicitly resolve the conflict before changing the approved graph. Do not silently change the DAG or relax its checker.

## Implementation invariants

- 23 packages, 76 approved production dependency edges, acyclic. Run `python scripts/check_architecture.py` for every accepted increment. Test-only dependencies are not production imports.
- Container owns physical format; eight durable stores own their state. Owner-session owns REL/PHY concrete stores, typed replay, owner authorization, and grouped writes. No writable store or Container escapes to higher layers.
- Semantic services produce bounded proposals/decisions; Application owns named publication and composition workflows; Gateway owns mounts, authenticated Instances, queue, host handoff/streams, and recovery. Gateway must not directly build or encode Archive/Memory store records or implement Application publication policy.
- Owner-session may expose **immutable, narrowly selected value types** to Application. It must not export mutable store handles, unrestricted store facades, or low-level codec construction for convenience.
- Normal runtime supports only REL 2.1. Old FRS07/2.0 handling belongs exclusively in the later isolated, independently verified offline converter.
- Preserve supported behavior, deterministic IDs, provenance, privacy, durable history, concurrency and recovery guarantees. Old internal APIs and compatibility shims are not mandatory.
- Prefer small, responsibility-local modules (target under 200 lines; avoid newly oversized 300+ line files). Split a specific unit if needed; **do not expand the 44-increment plan in advance**.

## Continuation and verification

The last verified architecture-correction checkpoint was **`95db1b41` (2026-11-04)**. This is not final acceptance. The capability tracker marks **F01–F04 and C01 complete; G1–G5 are not accepted**. Many other increments have partial implementations and passing package tests; those do **not** satisfy their dependency-gated acceptance.

For the next dependency-ready capability: read its actual acceptance, inspect relevant existing implementation, make the smallest cohesive change, compile and test behavior/reopen/negative cases, run the architecture guard, record evidence, and **commit the verified increment**. Split only the particular increment that proves too large; no new planning phase.

Verification from this worktree:

```powershell
python scripts/check_architecture.py
cargo fmt --all --check
cargo check --workspace --locked --offline -j 1
cargo test --workspace --locked --offline -j 1 -- --test-threads=1
```

Run CLI and documentation commands **only when their replacement artifacts actually exist**; their absence is unfinished work, not a reason to copy old binaries. Do not treat a passing workspace test run as G1/G2/G3/G4/G5 acceptance. The reference must remain untouched, and no unverified conversion or cutover is authorized.

Avoid multiplying Cargo target directories on Windows; C: build-cache exhaustion previously caused an unrelated linker/PDB failure. Do not delete user files, corpus, reference files, or another worktree's cache.
