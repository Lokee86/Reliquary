# Entity Resolution service extraction evidence

Parent index: [Documentation index](INDEX.md)

## Purpose

Record the implementation boundary and verification status of the [extraction specification](entity-resolution-service-crate-extraction-spec-2026-10-28.md).

## Overview

The implementation is integrated with the separately identified live-calibration gap below. `reliquary-entity-resolution` owns candidate value and ranking policy, query assembly, identity guards, fingerprints, resolver and admission inference, decision fast path, reconciliation pair classification, and audit collision calculations. Root gathers authorized evidence, configures routes, rechecks owner versions and commits accepted effects. `reliquary-perception-store` remains the durable owner.

## Verification

- `cargo check --workspace --all-targets --locked --offline -j 1`: passed.
- `cargo test -p reliquary-entity-resolution --locked --offline -j 1`: seven unit and two external-consumer tests passed.
- `cargo test -p reliquary-perception-store --locked --offline -j 1`: 93 tests passed.
- Root focused candidate, resolution, resolver, creation-guard, reconciliation, audit, lifecycle and principal groups passed. The resolution group included 63 tests and the disposable-owner bootstrap/reopen scenarios.
- Five configured decision route integration tests passed with local loopback access. In the restricted network environment they failed to reach their local test servers.
- `cargo test --manifest-path cli/Cargo.toml --locked --offline -j 1`: 32 passed with temporary files inside the repository.
- `cargo fmt --all -- --check`, `ddocs check --indexes --links`, and `git diff --check`: passed.
- Documentation policy using the Windows extended path reports the 28 pre-existing missing-section findings in unrelated extraction documents. The ordinary path hits the known Windows long-path failure.
- `cargo test --lib --locked --offline -j 1`: 1,470 passed, five ignored, with local loopback access. Three recovery tests exceeded 60 seconds but eventually passed.
- Architecture guard self-tests passed; Lexicon refresh was stopped after a lengthy scan without a fresh Arcana result. The installed Pitlord version does not support the repository's `docs` command, so documentation ownership enforcement was not verified.

## Limitations

The ignored 41-query live zero-Entity bootstrap requires a frozen local REL, and calibration files absent from this checkout. A model configuration exists in the original workspace, but its credentials were not verified. It has not been run. The promoted `CVAFRS07` corpus-recovery gate and agentic retrieval recall limitation remain unchanged.

## Related docs

- [Entity resolution state](entity-resolution-state.md)
- [Perception foundations](subsystems/perception.md)
- [Perception Store evidence](perception-store-crate-extraction-evidence-2026-10-26.md)

## Notes

This evidence does not claim completion of acceptance gates that were not run or did not finish.
