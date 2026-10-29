# Context Engine extraction evidence — 2026-10-28

The migration commit is the commit containing this evidence. Its baseline was `162c7063`; the unrelated Graph evidence worktree edit was excluded. The four production files moved from `src/context_engine/` to `crates/reliquary-context-engine/src/` have identical Git blob hashes to the baseline. Root directly re-exports the crate API. The nine existing tests moved to `crates/reliquary-context-engine/tests/behavior.rs`, with only their import adapted, so they exercise the external crate surface. The crate depends directly on Echo alone. Archive retains checkpoint persistence and the host retains source authorization and token-counter selection. Ego remains a placeholder.

The unchanged production blobs provide byte-level fidelity for prompts, budget and error behavior, checkpoint cuts, and Echo mapping; the migrated behavioral fixtures exercise these paths. No persistent format or runtime integration was changed.

## Verification

- Baseline Root context filter: 9 passed; Root `echo_tests`: 4 passed.
- `cargo check -p reliquary-context-engine --locked --offline -j 1` and `cargo test -p reliquary-context-engine --locked --offline -j 1`: 9 external behavioral tests passed.
- `cargo tree -p reliquary-context-engine --depth 1 --offline`: Echo is the only direct dependency.
- `cargo check --workspace --all-targets --locked --offline -j 1`: passed.
- Root `cargo test --lib --locked --offline -j 1 -- --test-threads=8`: 1,457 passed, 5 ignored; repository-local temporary files and loopback access were required.
- Root Echo regression and conversation compaction persistence filters: 4 passed each.
- CLI `cargo test --manifest-path cli/Cargo.toml --locked --offline -j 1`: 32 passed with repository-local temporary files.
- `cargo fmt --all -- --check`, `ddocs check --indexes --links`, and `git diff --check`: passed. Architecture coverage/ownership self-tests and policy validation passed. The pre-move graph passed the old policy, but the updated policy check reports deleted Root files from the stale snapshot as unowned. `python scripts/check_architecture.py --refresh` was attempted twice (including with broader access); its Lexicon Rust scan did not complete, so no refreshed graph result is claimed.

The promoted `CVAFRS07` corpus is an independent program gate and was not run for this extraction. A fresh Lexicon/Arcana graph and architecture check remain a tooling follow-up.
