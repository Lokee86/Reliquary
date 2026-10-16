# Agentic Memory retrieval Luna baseline — 2026-10-16

## Purpose

Freeze the Step 1 implementation baseline before any agentic retrieval runtime work.

## Overview

This note records the source snapshot, ownership seams, refresh attempt, focused test attempt, and blockers.

## Related docs

- [Agentic Memory retrieval linear implementation plan](agentic-memory-retrieval-linear-implementation-plan-2026-10-16.md)
- [Architecture](architecture.md)
- [Architectural invariants](invariants.md)
- [External Memory consumption and Freshness](freshness-external-consumption-contract.md)

## Snapshot

- Branch: `feat/agentic-memory-retrieval`
- HEAD: `596f5ceaac10b4a70ad7f8109dc5dc87cd92a018` (`docs(retrieval): add linear agentic memory implementation plan`)
- Worktree: clean before this note; the unrelated dirty Ego documentation edits described by the plan were not present in this worktree and were not touched.
- Model assignment: Luna; Step 1 only.
- Agentic implementation: no `AgenticMemory*` source or test symbols were found under `src`, `tests`, or `cli`.

## Direct ownership inspection

- Passive Memory retrieval: `src/retrieval/memory_search.rs`, with facade ownership in `src/facade/cva_memory_retrieval.rs` and `src/facade/phylactery_memory_retrieval.rs`.
- `MemoryRetrievalIndex` and retrieval modes: `src/retrieval/memory_retrieval_model.rs`; owner facade entry points are the two files above.
- Lexical Memory indexing/search: `src/retrieval/lexical_index.rs`, `src/retrieval/lexical_search.rs`, and `src/memory/memory_routing_codec.rs`.
- Decision endpoint construction: `src/runtime_inference/configured_decision.rs`, currently entity-resolution-owned.
- Runtime route ownership: `src/runtime_inference/runtime_host_routes.rs`; configured construction is `src/runtime_inference/configured_runtime.rs`.
- Graph neighbors: `src/facade/cva_graph.rs` and `src/facade/phylactery_graph.rs`.
- Freshness reads and accepted-use writes: `src/runtime_inference/freshness_access.rs`, with boundary coverage in `tests/freshness_retrieval_boundary.rs` and owner contracts in `tests/freshness_owner_contract.rs`.

## Commands and results

- `python scripts/check_architecture.py --refresh` — not completed: the installed sibling Lexicon/Arcana refresh exceeded the orchestration limit without output.
- Direct retry: `lexicon.exe init --repo <this worktree> --adapters ..\\lexicon-arcana\\lexicon\\adapters --languages rust` — timed out after 114 seconds (exit 124); no child process remained afterward.
- Generated evidence: `.lexicon` contains only partial initialization/lock state; `.arcana/CURRENT` is absent. No graph snapshot ID is claimed.
- `cargo test --lib memory_retrieval -- --nocapture` — timed out after 114 seconds (exit 124) during compilation; no pass/fail result was emitted.
- No architecture-policy pass is claimed because the required refreshed Arcana snapshot is unavailable.

## Status and blockers

Step 1 is not complete: required Lexicon/Arcana evidence and a focused retrieval baseline result were not obtained within the available bounded runs. The blocker is tool/build duration, not a source failure. No runtime implementation or generated snapshot was committed. The next attempt should resume or reuse a completed local Lexicon build before rerunning Arcana sync and the focused tests.

## Notes

This is a baseline evidence record, not an implementation acceptance record. Direct source remains authoritative while generated graph evidence is unavailable.
