# Agentic Memory retrieval Luna baseline — 2026-10-16

## Purpose

Freeze the Step 1 implementation baseline before agentic retrieval runtime changes.

## Overview

Step 1 is complete. Luna inspected the source and committed the initial attempt in `50ae1ff`; the primary executor repaired invocation/cache mechanics and collected the successful results below. No runtime implementation existed in the audited source.

## Snapshot

- Convergence branch: `feat/agentic-memory-retrieval` in `C:\!bin\workspace\Reliquary-agentic-retrieval`.
- Initial source HEAD: `596f5ceaac10b4a70ad7f8109dc5dc87cd92a018`. The subsequent `50ae1ff` baseline-note commit changed documentation only.
- Original `main` had exactly two unrelated dirty Ego documents; they were not copied into this isolated worktree and remain untouched.
- Refreshed Lexicon/Arcana snapshot: `sha256:0f00d82eb905356cedd4d2c27200817a415c1bf3625dc7f5f91e4c3ff5c872b8`.
- Arcana reports 74,737 nodes and 189,538 relationships, including 15,650 `calls` relationships. Source lookup correctly places `GatewayRuntime::search_visible_memories` in `runtime_host_gateway_semantic_search.rs`.
- Direct source search found no existing `AgenticMemory*` runtime/controller implementation.

## Ownership inspection

Passive gateway composition is in `src/runtime_inference/runtime_host_gateway_semantic_search.rs`; `runtime_host_memory_search.rs` is a shared-model re-export. Owner-local semantic retrieval lives in `src/retrieval/memory_retrieval.rs`, its index/build/vector modules, and `src/facade/cva_memory_retrieval.rs`. Complete lexical work will deepen `src/retrieval/lexical_index/memory.rs` and `lexical_search.rs`. Decision transport and routes are owned by `configured_decision.rs`, `configured_runtime.rs`, and `runtime_host_routes.rs`. Graph reads remain in `cva_graph.rs`. `freshness_access.rs` owns passive grades and the sole accepted-use receipt application.

## Commands and results

- Release-adapter `lexicon init --repo <convergence> --adapters <convergence>/.lexicon/release-adapters --languages rust`: passed, approximately nine minutes. A disposable packaged copy of the installed Rust release adapter avoided the slow debug invocation; only Rust was enabled. The adapter-root discovery sentinel is an empty `python` directory; no Python analysis was requested.
- `arcana sync --lexicon <convergence>/.lexicon --state <convergence>/.arcana`: passed, zero compatibility warnings.
- Arcana `stats` and `search_nodes` for `search_visible_memories`: passed and agreed with direct source placement.
- `cargo test --lib memory_retrieval -j 2`: 5 passed.
- `cargo test --lib model_switchboard -j 2`: 15 passed.
- `cargo test --test freshness_retrieval_boundary -j 2`: 3 passed.
- Documentation checks on the initial baseline record reported only the four pre-existing missing sections in `ego-status-decision-inventory-2026-10-14.md`; no retrieval finding was introduced.

## Failure and recovery

Delegated command windows first expired during analysis/building. A timed-out wrapper did not reliably terminate its child process; the primary executor inspected exact process identities before stopping the obsolete debug refresh. One test link then failed with LNK1181 because a generated `hashbrown` archive was absent despite existing fingerprints. `cargo clean -p hashbrown --target-dir <convergence>/target` removed only 21 generated cache files (3.4 MiB); the subsequent focused build and tests passed. No source workaround or dependency change was made.

## Related docs

- [Linear execution plan](agentic-memory-retrieval-linear-implementation-plan-2026-10-16.md)
- [Architecture](architecture.md)
- [Architectural invariants](invariants.md)
- [External Memory consumption](freshness-external-consumption-contract.md)

## Notes

This is baseline evidence, not acceptance of the agentic feature. The independent Luna AR2 route branch is preparation only until integrated in linear order. Generated tool state is disposable and uncommitted.
