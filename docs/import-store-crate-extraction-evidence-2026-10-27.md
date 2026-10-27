# Import Store extraction evidence

## Purpose

Record the Import Store ownership cut and its verification.

## Overview

The lower crate owns current epoch values, codecs, identity and projection. The higher Import coordinator owns completion admission and the physical completion append.

## API inventory before relocation

`imports` is public. `ImportStart`, `ImportComplete`, `ImportWatermarks`, and `ImportEpochStatus` are exported through `imports::contract`. `Cva::open_import_epoch`, `import_epoch_status`, and `import_epoch_statuses` return owned values. Internally `commit.rs` borrows `open()` while completion and readiness consume owned status projections. The canonical structs can therefore be owned by the lower crate and re-exported unchanged; the lower store can still return a borrow from its sole projection. No persistent adapter cache is needed.

The completion coordinator reconstructs accepted physical records, validates receipt, readiness and settlement, then appends and synchronizes before publishing its projection. Owner open replays records, validates the owner, and checks receipts. Receipt digests must be computed above the storage boundary.

## Verification

- `cargo check -p reliquary-import-store --locked --offline` passed.
- `cargo test -p reliquary-import-store --locked --offline` passed eight tests, including frozen Start and Complete frames, owner-scoped ID, replay rejection and watermark checks.
- `cargo check --workspace --locked --offline`, `cargo fmt --all -- --check` and the detached CLI formatting and check commands passed.
- `cargo test --lib imports:: --locked --offline` passed 85 tests, including actual native import, settlement, completion, retry and reopen as well as reconciliation and sequential imports.
- `cargo test --manifest-path cli/Cargo.toml --locked --offline` passed 32 tests.
- `cargo tree -p reliquary-import-store --depth 1 --locked --offline` shows Container as the only direct Reliquary dependency.
- `ddocs check --indexes --links` passed. The changed-from documentation policy check reports missing standard sections in previously committed Dream, Ego and Import specification/evidence documents. The installed `pitlord` does not provide the required `docs` command.
- `cargo test --lib --locked --offline` passed with loopback access: 1,648 passed, 5 ignored. An earlier restricted import run was interrupted after a local HTTP embedding test failed; that test passed separately with loopback access.

## Notes

Dream Store was integrated as commit `71b64c10` before this work. The unrelated Graph evidence modification is outside this extraction.

## Related docs

- [Import Store specification](import-store-crate-extraction-spec-2026-10-27.md)
- [Native import](native-import.md)
- [Storage format](storage-format.md)
