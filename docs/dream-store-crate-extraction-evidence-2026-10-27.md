# Dream Store crate extraction — implementation evidence

**Status:** Compiled integration and final verification complete (2026-10-27).

## Current boundary

`crates/reliquary-dream-store/` contains the `CVDREAM2` Begin codec, canonical
population digest, fixed-byte Memory identity, `CVDREAM1` journal projection,
and wire-exact accepted graph-cut proof value. Root uses these through
`src/freshness_dream_journal.rs`; the Begin and digest adapters are in
`src/freshness/dream_begin_codec.rs` and
`src/freshness/dream_population_digest.rs`. Root retains physical prefix
traversal, compact population reconstruction and authorization, Freshness
proof conversion, and higher Dream/History coordination.

The crate currently declares no direct Reliquary crate dependency. Its current
external dependencies are `sha2` and optional `serde_json` for test-oracle
support. No claim is made here that all History/import/rewrite callers or every
acceptance obligation in the specification has been completed.

## Verification results

The following final commands passed:

- `cargo check --workspace --locked --offline`
- `cargo fmt --all -- --check`
- `cargo test -p reliquary-dream-store --locked --offline` — 17 passed.
- `cargo test --lib dream --locked --offline` — 183 passed.
- `cargo test --lib --locked --offline` — 1,653 passed, 5 ignored, 0 failed (337.38 seconds).
- `cargo test --manifest-path cli/Cargo.toml --locked --offline` — 32 passed.
- `cargo tree -p reliquary-dream-store` — only runtime dependency is `sha2`.

Workspace check and formatting check passed. The final focused Dream and lower
crate runs were rerun after integration. An initial restricted
`cargo test --lib --locked --offline` run encountered loopback endpoint
failures and was interrupted; the elevated rerun above completed successfully.

## Scope notes

The full library suite, focused Dream suite, lower crate suite, CLI suite,
workspace check and formatting check passed. This evidence does not claim the
separate CVAFRS07 authoritative six-file corpus recovery gate; that remains an
independent Freshness gate. Broader physical replay, History/import/rewrite
contracts remain covered by their respective tests and evidence rather than
being inferred from this extraction report.

See the [implementation specification](dream-store-crate-extraction-spec-2026-10-27.md),
[architecture](architecture.md), [API](api.md), [maintainer map](maintainer-map.md),
[storage format](storage-format.md) and [Dream subsystem](subsystems/dream.md).
