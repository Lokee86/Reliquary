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

- `cargo test -p reliquary-insomnia --locked --offline -j 1`: 11 unit tests and one external-consumer test passed after evidence validation.
- Root Insomnia focused tests: 74 passed after evidence-response validation, with temporary files inside the repository.
- The Root Chronos-to-Insomnia error conversion regression test passed after restoring the public conversion.
- `cargo test -p reliquary-insomnia-store --locked --offline -j 1`: 11 tests
  passed at the pre-extraction baseline.

- `cargo check --workspace --all-targets --locked --offline -j 1` passed after the final evidence-boundary change.
- `cargo fmt --all -- --check` and `ddocs check --indexes --links` passed at the recorded checkpoint.
- The architecture guard passed before refresh, and the component map's 35 ranks and dependencies validate. Two refresh attempts were interrupted during Rust analysis, the second after more than 12 minutes; refreshed architecture evidence remains unverified. Pitlord's installed version does not support `pitlord docs`.
- The shared documentation-policy check failed with Windows error 3 on an existing path under `target`. On source-only snapshots, the current tree and committed baseline each produced the same 28 existing findings, with no new findings. The actual workspace check remains unverified.
- CLI tests passed 32/32 with repository-local temporary files. An earlier run using the system temporary directory failed two permission checks.
- A sandboxed full-library run reported failures and was interrupted after stalling. An isolated configured-route failure passed with local networking enabled. The full rerun with local networking and repository-local temporary files passed 1,465 tests with five ignored; it preceded the final response-reference bound and restored error conversion, which have separate focused coverage.

The unrelated dirty Graph evidence document was not changed as part of this extraction.

## Notes

These results describe only the listed commands and checkpoints; they do not
claim full extraction acceptance or current-format corpus verification.

## Related docs

- [Insomnia subsystem](subsystems/insomnia.md)
- [Insomnia Store extraction evidence](insomnia-store-crate-extraction-evidence-2026-10-26.md)
