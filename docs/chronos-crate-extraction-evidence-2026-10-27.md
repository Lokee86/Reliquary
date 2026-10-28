# Chronos extraction verification — 2026-10-27

## Purpose

Record the independently verified extraction baseline and the redesign checks.

## Overview

Stage A checkpoint on `refactor/crate-first-restart`, based on commit `6b8aa5ab` (Owner Lifecycle extraction `8943749d`). The working tree already contained an in-progress Chronos extraction and an unrelated Graph evidence edit when verification began. The Graph edit was not changed.

The deterministic parser and tests were moved to `reliquary-chronos`; moved files were compared against HEAD. Except for the inference model, canonical verification and processing-epoch boundary, their blob contents matched. The pure inference request/response contract and canonical verification are in the lower crate; endpoint transport and Memory conversions remain above it. The old inference protocol remains in use at this checkpoint.

- `cargo test -p reliquary-chronos --locked --offline --target-dir target/chronos-gate-a -j 1`: 65 passed.
- `cargo check --workspace --locked --offline -j 1`: passed after correcting the Memory publication conversion.
- `cargo fmt --all -- --check`: passed.
- Existing root test binary, `chronos`: 7 passed; `temporal`: 22 passed, including REL/PHY temporal history and inference persistence; `dream_temporal`: 9 passed; `insomnia::temporal_tests`: 3 passed.
- The initial complete library run exposed Windows path-length failures with the longer repository-local temporary directory and restricted local networking. The previously failing external Insomnia replay test passed using the shorter repository-local `t` directory. The full library rerun with that directory and local networking available passed: 1,579 passed, 5 ignored, 0 failed.

This is extraction evidence only. It does not establish the deterministic-choice redesign or final acceptance.

## Candidate and route checks

The labeled six-case candidate fixture contains two context-free `biweekly` cases, a `semiweekly` case, an explicit date, a relative expression without a reference, and an unqualified season. The expected four alternatives for the two ambiguous cases are all generated (candidate recall 4/4); the other cases generate no alternatives (unsupported-candidate count 0). The simulated abstaining endpoint fixture expects two abstentions, zero wrong selections and two calls for the two ambiguous cases; the other four cases make no calls. These simulated results verify the contract and do not measure a decision model's accuracy.

The redesigned lower crate passed 71 tests, including distinct `bimonthly` alternatives and deterministic `semimonthly` parsing. The integrated workspace all-targets check and locked offline CLI check passed. The final library suite passed 1,577 tests with 5 ignored; the CLI suite passed 32 tests. The configured route test exercised both General Decision and constrained General over local simulated transports, and the backpressure test preserved its retry delay. The source-evidence replay check validates original spans and indication kind before accepting a stored conclusion.

The development-configuration audit searched tracked and unignored files as well as ignored local files, excluding build output, Git metadata, vendored sources and analysis copies. A binary-aware search found old route objects in ignored `.test-tmp` and `.verification-tmp` test outputs; these are generated test fixtures, not saved runtime configuration, and were left intact. No other saved development configuration containing the route was found. Source examples were migrated; an old configuration object is rejected rather than retained as an unknown extension.

## Related docs

- [Chronos specification](chronos-crate-extraction-spec-2026-10-27.md)
- [Chronos subsystem](subsystems/chronos.md)

## Notes

The unrelated Graph evidence change was preserved. The shared documentation-policy checks report 28 missing-section findings in seven existing non-Chronos extraction documents; those documents were not changed. `ddocs check --indexes --links` passed. The CVAFRS07 six-file gate was not run. Architecture refresh and documentation ownership enforcement are tracked separately from the functional regression results.
