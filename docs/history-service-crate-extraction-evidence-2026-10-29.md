# History service crate extraction evidence

Parent index: [Documentation index](INDEX.md)

Baseline: `5954545b66e83985bf49b44273ebe46bcc10b9a0`, branch `refactor/crate-first-restart`. The existing Graph evidence edit was left untouched. The working tree also contained unrelated Import changes during initial inspection; they were not included in this extraction.

`HistoricalPayload` moved from Root `src/owner_lifecycle.rs` to `crates/reliquary-history/src/lib.rs`, with the original Root path reexported. The evaluator selects the full-record, recognized pruned, and other-error branches and calls the owner's lifecycle or retained-revision callback only where required. REL `Cva::historical_memory_revision`, PHY `Phylactery::historical_memory_revision`, and the Entity and Relationship `historical_*_revision` methods invoke it. Owners retain the initial read, exact requested revision lookup, lifecycle, error translation, and mutations. Lifecycle value definitions remain in Root. History's sole direct dependency is Archive; no durable format, Import, Maintenance, replay or test-only fingerprint code moved.

The previous graph refresh timed out and Pitlord ownership evidence was stale; a fresh graph pass is not inferred from those earlier results.

Implementation commit: `26990c79` (the CLI lockfile was refreshed offline with only the local History dependency added).

## Verification

- Fresh committed baseline `5954545b`: `lifecycle_tests` 23 passed and `compact_prune_tests` 5 passed; the same named tests passed after the cut. An initial `historical_memory_revision` filter selected zero tests and is not counted as evidence.

- `cargo check -p reliquary-history --locked --offline -j 1` and `cargo test -p reliquary-history --locked --offline -j 1`: passed; 2 unit tests and 1 external consumer test. The evaluator tests cover active, tombstoned, pruned, missing retained structure, unrelated errors and callback laziness.
- `cargo test --test historical_payload_boundary --locked --offline -j 1`: 1 passed, proving the original public path is the same concrete type and all variants match exhaustively.
- `cargo check --workspace --all-targets --locked --offline -j 1`: passed. `cargo test --lib --locked --offline -j 1 -- --test-threads=8`: 1,457 passed, 5 ignored with unrestricted loopback and filesystem access. The restricted run had 14 environment-dependent failures; five local HTTP tests passed independently with loopback access, and all passed in the unrestricted full run.
- Focused `lifecycle_tests`: 23 passed, including Memory and Entity/Relationship tombstone and reopen, PHY retained pruning and rejected manifests. `publication_prune_tests`: 1 passed; `compact_prune_tests`: 5 passed, covering REL/PHY retained structures and proof rejection.
- `cargo check --manifest-path cli/Cargo.toml --offline -j 1` refreshed its local lock, then `cargo test --manifest-path cli/Cargo.toml --locked --offline -j 1`: 32 passed. Workspace and CLI formatting checks and `git diff --check` passed.
- `cargo tree -p reliquary-history --depth 1 --offline`: Archive is the only direct dependency. Source review confirmed all four consumers call the evaluator and retain original signatures, authorized owner reads, error variants, revision identity and retention evidence; no persistent state or production fingerprint, Import or Maintenance code moved.
- `python scripts/check_architecture.py --refresh`: the multiplexing and external ownership guard self-tests and production external ownership guard passed, but the Lexicon Rust scan timed out after 120 seconds. The installed Pitlord does not provide the documented `docs` subcommand. No fresh graph or Pitlord documentation-ownership pass is claimed.
- `ddocs check --indexes --links`: passed after index reconciliation. The shared docs-policy script failed while traversing an existing long path under `target`; it did not reach a policy result. The Graph evidence modification was not staged or changed by this work.
