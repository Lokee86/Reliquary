# Insomnia Store extraction — implementation evidence

**Status:** Root store cutover implemented; product and CLI checks passed. Separate documentation-policy and corpus gates are noted below.

## Purpose

Record the current Insomnia Store cutover and verified acceptance results.

## Overview

`reliquary-insomnia-store` now owns fixed-byte work/attempt values, codecs, scheduling, lease/transition state, and reconstruction. Container is its only direct Reliquary dependency. The root product uses this store through `src/runtime_inference/insomnia/product_adapter.rs`, which converts public IDs and supplies Archive-authenticated ordering/state. Full `CVAINSC7`/`CVAINSC8` decoding, Memory/Freshness validation, and completion authoring remain in the root product. The approved owned `Cva::insomnia_work` API change and affected callers are implemented. Existing work, attempt, and completion formats are unchanged.

Verified during this implementation:

- `cargo check -p reliquary-insomnia-store --locked --offline`: passed.
- `cargo check -p reliquary-memory --locked --offline -j 1`: passed after the product cutover, per root-run verification.
- `cargo test -p reliquary-insomnia-store --locked --offline`: eleven tests passed, covering current work/attempt wire bytes, malformed attempt-count and foreign-family rejection, scheduling, transition/replay behavior, completion projection idempotence/conflicts and attempt progression, recovery precedence, and append asymmetry.
- `cargo fmt -p reliquary-insomnia-store -- --check`: passed.

- Focused root Insomnia tests: 75 passed; import/reconciliation tests: 90 passed; two completion authorization and conflict replay tests passed. These include owner reopen, grouped completion and retained compact completion cases.
- `cargo test --lib --locked --offline -j 1`: 1,658 passed, five ignored before the final codec family-version rejection change; that change is covered by the lower and focused Insomnia reruns. Local test servers required normal network access; the restricted run timed out on a local endpoint test.
- `cargo check --workspace --locked --offline -j 1` and `cargo fmt --all -- --check`: passed.
- `cargo test --manifest-path cli/Cargo.toml --locked --offline -j 1`: 32 passed, using repository-local temporary files.
- `ddocs check --indexes --links`: passed. Both shared documentation policy checks report four missing sections in the unrelated Ego Store specification. The installed `pitlord` does not provide the required `docs` command.

Reopen performs a bounded second frame scan after Archive and Memory reconstruction; the product validates each completion's Episode, local Memory references and Freshness birth proof before applying its projection in physical order. Completion writers preflight the lower transition before appending. CVAFRS07 recovery and the authoritative six-file corpus gate were not run and are not claimed here. The architecture evidence refresh was started but had not completed during verification; its guard self-tests passed before the Rust analysis began.

## Notes

Verification status reflects only commands listed above; pending gates do not imply failure or completion.

## Related docs

See the [extraction specification](insomnia-store-crate-extraction-spec-2026-10-26.md), [architecture](architecture.md), [maintainer map](maintainer-map.md), [storage format](storage-format.md), [Insomnia subsystem](subsystems/insomnia.md), and [Rust API](api.md).
