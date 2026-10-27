# External Index Store extraction evidence — 2026-10-27

Baseline: `d60e1695` on `refactor/crate-first-restart`. The unrelated Graph evidence modification was present before this work and is excluded.

The canonical descriptor, references, errors, index state, codec and Container installation now live in `reliquary-external-index-store`; the root re-exports those types. The lower crate's only internal dependency is Container. During reopen the root validates profiles and packed vectors, passing invalid source namespaces to the lower store. Rebuild, provider authorization, hydration and search remain in the root.

Borrowed-state inventory: `processing.rs` borrows `SourceState`; `index.rs` accepts `Option<&SourceState>`; root search and profile lookup borrow `SourceState`; lifecycle, maintenance and gateway use `external_index.sources`. The original concrete state types are re-exported, and root status conversion retains the existing public `ExternalIndexStatus` representation. Gateway authentication and hydration are unchanged. The root installation checks revision before writing profiles and matrices; the lower install checks it again before encoding, append, sync and publication.

Verification performed:

- `cargo check -p reliquary-external-index-store --locked --offline`: passed.
- `cargo test -p reliquary-external-index-store --locked --offline`: 2 passed, including fixed snapshot/discard bytes, truncation, corruption, CAS no-write and failed append invalidation.
- `cargo check --workspace --locked --offline`: passed.
- `cargo fmt --all -- --check`: passed.
- `cargo test --lib external_source --locked --offline`: 20 passed, covering codec shapes, publication, reopening, compaction, recovery, incremental updates and stale preparation.
- `cargo test --lib runtime_host::gateway::external_tests --locked --offline`: 7 passed with repository-local temporary paths; the default temporary-path run had 3 storage/conflict failures.
- `cargo test --manifest-path cli/Cargo.toml --offline`: 30 passed, 2 failed with Windows access denied saving configuration; the `--locked --offline` retry with repository-local `TEMP` and `TMP` passed all 32 tests.

Final independent acceptance of the in-progress extraction:
- `cargo test --lib --locked --offline -j 1` — **1,648 passed, 0 failed, 5 ignored**, 381.92 seconds.
- `cargo test -p reliquary-external-index-store --locked --offline -j 1` — **2 passed**.
- `cargo check --workspace --locked --offline -j 1` — passed.
- `cargo fmt --all -- --check` — passed.
- `cargo test --manifest-path cli/Cargo.toml --locked --offline -j 1` with repository-local `TEMP`/`TMP` — **32 passed**.
- `cargo test --lib external_source --locked --offline -j 1` — **20 passed**.
- `cargo test --lib runtime_host::gateway::external_tests --locked --offline -j 1` with repository-local `TEMP`/`TMP` — **7 passed**.
- `ddocs check --indexes --links` — passed after updating the generated documentation index to include this evidence and the three recent extraction specs.

The direct storage-error test exercises append failure using a read-only Container. A separate injected sync failure is not covered; the pre-existing append-then-sync error handling was moved unchanged. The `CVAFRS07` corpus and separate documentation-policy gates are outside this extraction and have not been claimed as complete.