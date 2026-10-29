# Insomnia service extraction evidence

Parent index: [Documentation index](INDEX.md)

## Purpose

Record the implementation boundary and scoped verification for the [Insomnia
service extraction](insomnia-service-crate-extraction-spec-2026-10-28.md).

## Overview

`reliquary-insomnia` owns immutable Episode input, the v3-4 authority ledger
and repair rules, fixed synthesis groups, metadata/ownership/wording stages,
candidate and provenance validation, Entity-mention enrichment, and the
single-round evidence request/response contract. `InsomniaEvidenceRound`
exposes only an immutable request slice. Before the final ledger pass, the
service checks result/request correspondence, duplicate identities, response
turn consistency, and the 64-turn/128 KiB limits.

Root authenticates Episode inputs, chooses configured endpoints, resolves and
hydrates Archive requests, binds owner/source and temporal evidence, publishes
Memory/Freshness effects, and runs work scheduling. `reliquary-insomnia-store`
remains the durable work/attempt owner. No persistent format or queue behavior
was changed.

## Verification recorded

- After module decomposition, `cargo test -p reliquary-insomnia --locked --offline -j 1`: 11 unit and three behavioral external-consumer tests passed. The external tests exercise an actual ledger-to-Memory candidate, one bounded evidence request/response round, and rejection of Archive fallback for external input.
- Root Insomnia focused tests: 75/75 passed with `cargo test --lib insomnia:: --locked --offline -j 1 -- --test-threads=1`. An earlier concurrent run completed 72 tests and timed out while three tests were pending; the serial rerun passed all 75.
- The Root Chronos-to-Insomnia error conversion regression test passed after restoring the public conversion.
- `cargo test -p reliquary-insomnia-store --locked --offline -j 1`: 11 passed against the final decomposed service workspace.

- `cargo check --workspace --all-targets --locked --offline -j 1`: passed against the final source tree.
- `cargo fmt --all -- --check`, `ddocs check --indexes --links`, and scoped `git diff --check`: passed after the module decomposition and documentation update.
- After decomposition, `python scripts/check_architecture.py` passed: deterministic ownership guards and Pitlord's 25 architecture rules reported no violations against available graph evidence (93,776 source nodes, 22,403 relationships). This is not a refreshed Arcana witness; that separate gate remains unverified. The installed Pitlord does not support `pitlord docs`.
- The shared documentation-policy check failed with Windows error 3 on an existing path under `target`. On source-only snapshots, the current tree and committed baseline each produced the same 28 existing findings, with no new findings. The actual workspace check remains unverified.
- `cargo test --manifest-path cli/Cargo.toml --locked --offline -j 1`: **32 passed**, zero failed, with repository-local temporary files. An earlier system-temp run had two environment permission failures.
- `cargo test --lib --locked --offline -j 1 -- --test-threads=8`: **1,466 passed, zero failed, five ignored**, against the final decomposed service and behavioral tests. An earlier serial broad-library attempt was intentionally stopped during expensive unrelated historical-import tests; only the complete eight-thread rerun is a full-library acceptance result.

The unrelated dirty Graph evidence document was not changed as part of this extraction.

## Post-extraction structural cleanup

The initial cut moved the semantic implementation but left three oversized service modules: `extraction.rs` (769 lines), `ledger.rs` (668), and `enrichment.rs` (363). The follow-on cleanup splits them by responsibility into nested execution, ledger schema/repair/parse and Entity-mention modules. Each of those three parent files and each new child module is under 200 lines; no production Insomnia service module exceeds 300 lines. The split preserved the existing prompts, contracts and candidate rules. Root continues to own Archive evidence hydration, owner authority, grouped publication, persistent completion and worker scheduling.

The three external-consumer tests call the compiled service's extraction and evidence APIs directly, rather than merely testing public constants. The broader current-format corpus acceptance remains a separate gate; a unit or integration suite cannot substitute for that corpus evidence.

## Notes

Results are reported by actual command and checkpoint. The promoted `CVAFRS07` corpus-recovery gate and model-driven full-corpus parity are still unverified. Do not interpret an interrupted broad Rust suite as passing.

## Related docs

- [Insomnia subsystem](subsystems/insomnia.md)
- [Insomnia Store extraction evidence](insomnia-store-crate-extraction-evidence-2026-10-26.md)
