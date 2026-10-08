# Multiplexing P5-G-import execution — 2026-10-08

Parent: [P5-G specification](multiplexing-phase5g-interoperability-2026-10-06.md)

Wave: [Next execution plan](multiplexing-phase5-next-parallel-execution-plan-2026-10-07.md)

## Purpose

Record isolated G0–G2 native historical import work, exact source scope, verification and convergence handoff.

## Overview

Branch: `feat/multiplexing-p5g-import`. Base: `999fbf7d85c9f892a1bf979293588033bcc1484b`. Main and E/F/C worktrees are unchanged. [Native historical import](native-import.md) is the current contract owner.

G0 audited official provider export instructions and available Hermes/ACP source, recording concrete unsupported/unverified formats. G1 schema-1 namespace/message/revision/branch/artifact identities are implemented. G2 implements ChatGPT/Claude text profiles, Hermes full-session text JSONL and structured-agent interchange through one Archive/Insomnia native commit path. This is bounded supported-profile acceptance, not arbitrary real export or raw Codex rollout certification.

## Implementation and ownership

- Pure, byte-only adapters and versioned DTOs under `src/imports/`.
- One exclusive Cva commit composition; no mounted runtime, PHY path or second queue.
- Archive-versioned `CVAIMPT1` receipt evidence, validated cold rebuild, semantic replay and physical reclamation.
- Archive-owned branch-aware import/native-continuation Episode coverage, preserving shared-Node deduplication, requested origin/priority and existing packing.
- Immutable source revisions, explicit body-free tombstones/replacement refs, artifact byte hashes/source mappings, exact re-import and retry/reopen.
- Public golden profile tests and focused source/queue cut, preservation and history tests.
- One Pitlord source area plus rules forbidding I/O/publication authority in normalizers.

## Verification

N2 functional acceptance passed on the final source: 1,110 library tests, 133 integration tests across 19 targets and 19 CLI tests, with no failures or ignored tests. Root/CLI formatting and locked checks passed. Focused acceptance passed 11 import unit tests and five public profile tests. P2/P3 ownership guards passed; P4 passed 42 fixtures and classified 148 public APIs. Pitlord policy validation passed with 16 areas and 12 rules. Archive round-trip passed with three Nodes, two branches, two fragments, three content objects, four expanded references and 1,579 bytes.

Reproduce with `cargo fmt --all -- --check`, `cargo check --locked -j4`, `cargo test --locked -j4 -- --test-threads=4`, the equivalent formatting/locked check/test commands using `--manifest-path cli/Cargo.toml`, and `cargo run --locked -j4 --example archive_roundtrip -- <graph> <output>`. Accepted builds used an isolated external `CARGO_TARGET_DIR=C:\Users\archa\AppData\Local\P5GVerification\reliquary`, `CARGO_PROFILE_DEV_DEBUG=0`, `CARGO_PROFILE_TEST_DEBUG=0` and `CARGO_INCREMENTAL=0`; no manifest, lockfile or repository profile changes were required. Final matrix job: `job_YXIpYdG3WmamWH8Ievpt8CJ7`; focused job: `job_ePcyHLjzILJ6wutf57DYVaNs`; guard job: `job_USAYEnbln2nEr6lykBbdQLx4`.

Initial dependency compilation timed out after 240 seconds. The warmed retry exposed a private timestamp helper after adapter splitting; its visibility was repaired. Debug linking then hit Lore's MSVC PDB limit, and a narrower debug override exhausted disk space. Removing only this lane's generated target cache and rebuilding with the environment above resolved both tooling failures. A focused activity assertion was corrected from one to three canonical user/assistant Nodes. All final functional gates were rerun after the native-continuation repair; no source failure was waived.

Fresh `python scripts/check_architecture.py --refresh` (`job_OpkZuN37JZFpspxprcbxsD9J`) timed out at its 240-second bound while Lexicon was analyzing Python after Rust. Its embedded P4 checks passed, but Arcana synchronization and graph-based Pitlord checks did not complete. This is an explicit bounded tooling gap; no stale graph is accepted as fresh evidence. Direct ownership/coverage guards and policy validation passed.

Documentation structural and changed-from-`origin/main` checks passed, as did working-tree/index whitespace checks.

## Handoff and remaining gates

F5 can consume `reliquary_memory::imports` without creating an ad hoc codec. Mounted F5 still needs exact REL authorization, canonical P4 mutation admission and coordinator inventory refresh around the single commit operation. N3 owns final root exports and merged-library/CLI/consumer verification.

The official ChatGPT/Claude export instructions do not freeze JSON field schemas; no maintained real-account export fixture was available in this checkout. Supported v1 profiles use maintained synthetic fixtures and explicit shape validation. Missing artifact bytes, non-text/tool/lineage formats and arbitrary raw Codex rollouts remain explicit unsupported profiles. Hermes source is available at `7798df0a83721df7ff44b9ba023d56b85b351d1e`; unavailable source is not used to waive its live adapter.

G-external R0–R4, G3 ACP over E and G5 Hermes feed/hydration remain separate unfinished work. This lane does not complete all P5-G, F5, N3, D or Phase 5.

## Documentation impact

- Inspected: canonical architecture/invariants/storage/API/maintainer/limits/roadmap/coverage/contracts, G/next-wave plans, rollback/history decisions and shared standards.
- Updated: native contract, canonical branch-local ownership/API/format/limits/coverage/contracts routes, roadmap, indexes and lane evidence.
- Not affected: live E/P3 authority, PHY schema/privacy, model/configuration, CLI command surface and external-derived design.
- Compliance check: structural and changed-from-`origin/main` documentation policy checks passed.
- Known documentation gaps: actual export certification and mounted F5/external/live/raw-agent profiles remain listed above.

## Architecture impact

- Standards added or changed: none.
- Ownership or boundary impact: G normalizes/composes; Archive owns imported evidence and Episodes; Insomnia retains the only queue.
- Pitlord enforcement impact: native-import source area and normalizer authority/I/O exclusions.
- Other verification impact: golden profiles, branch/revision/delete/artifact/reopen/cut/preservation cases plus full existing gates.
- Known architectural gaps: mounted F5 admission and final convergence remain explicit; fresh Lexicon indexing exceeded the 240-second bound, so refreshed graph validation remains incomplete.

## Related docs

- [Native import](native-import.md)
- [Architecture](architecture.md)
- [Storage format](storage-format.md)
- [Current limitations](current-limitations.md)

## Notes

Receipt synchronization acknowledges the complete import batch; pre-receipt progress is resumable through the same normalized input.
