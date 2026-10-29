# Import service crate extraction — acceptance evidence

**Specification:** [Import service crate extraction](import-service-crate-extraction-spec-2026-10-29.md) (spec commit `c5a16f0c`).
**Implementation:** `88a7d85ad0242d10c032451a144d20ddb15ace36` — *Extract native Import normalization and preparation into crate*.
**Branch:** `refactor/crate-first-restart`. **Date:** 2026-10-29.
**Acceptance baseline:** `b960e4ca` — 85/85 focused Root Import, 1,457 full Root passed with 5 ignored, CLI 32/32.

## Actual source ownership

`reliquary-import` is no longer a placeholder. It now owns schema-1 `NativeImport`, `ImportConversation`, `ImportMessage`, `ImportBranch`, `ImportArtifact`, canonical preparation and the four byte-only normalizers. These existing implementations moved:

| Root original before extraction | Compiled Import crate |
| --- | --- |
| `src/imports/chatgpt.rs` | `crates/reliquary-import/src/providers/chatgpt.rs` |
| `src/imports/claude.rs` | `crates/reliquary-import/src/providers/claude.rs` |
| `src/imports/hermes.rs` | `crates/reliquary-import/src/providers/hermes.rs` |
| `src/imports/providers.rs` | `crates/reliquary-import/src/providers/common.rs` |
| `src/imports/prepare.rs` | `crates/reliquary-import/src/prepare.rs` |
| DTO definitions formerly in `src/imports/contract.rs` | `crates/reliquary-import/src/lib.rs` |

A direct source-to-source diff against `c5a16f0c` verified the five moved implementation files retain the *same executable body*, with changes restricted to imports and cross-crate visibility. Root now has narrow forwarding functions and a preparation adapter and reexports the one lower DTO definition. Root `ImportError` retains existing public variants and display semantics and maps lower normalization/preparation errors at this boundary. There are no duplicate provider implementations, source DTO definitions or preparation algorithms.

**Owner boundary:** `src/imports/commit.rs`, `completion.rs`, `readiness.rs`, `epoch_scope.rs`, `epoch_store.rs`, `epoch_codec.rs`, `reconciliation.rs`, `reconciliation_interval.rs`, `reconciliation_replay.rs` and historical reconstruction remain Root/Cva owner operations. `reliquary-import-store` remains the existing durable epoch owner; Archive owns receipt and turn models. Nothing in the Import crate imports `Cva`, `Phylactery`, Container or Gateway. `cargo tree -p reliquary-import --depth 1 --offline` lists Archive as the **only** direct Reliquary crate dependency, plus Serde, Serde JSON, SHA-2 and time. Root depends downward on the compiled Import crate.

## Verification — observed, not projected

- `cargo check -p reliquary-import --locked --offline -j 1` — **passed**.
- `cargo test -p reliquary-import --locked --offline -j 1` — **4 crate provider tests, 1 actual external consumer test passed**. The consumer normalizes a real source and creates a deterministic Archive receipt, turn and attachment; doctests selected zero.
- `cargo test --lib imports:: --locked --offline -j 1 -- --test-threads=8` — **85 passed / 0 failed**, matching baseline; includes interrupted epoch recovery, source revisions, receipts, strict readiness, actual completion, complete-epoch reconciliation and evidence validation.
- `cargo check --workspace --all-targets --locked --offline -j 1` — **passed**.
- `cargo test --lib --locked --offline -j 1 -- --test-threads=8` — **1,457 passed / 0 failed / 5 ignored**, full Root regression (353.99 seconds for execution).
- `cargo test --manifest-path cli/Cargo.toml --offline -j 1` — **32/32 passed** while updating the excluded CLI's lockfile with the added local Import crate. First locked attempt correctly refused the stale CLI lockfile. After the minimal lockfile update, `cargo test --manifest-path cli/Cargo.toml --locked --offline -j 1` also **passed 32/32**.
- `cargo fmt --all -- --check` — **passed**.
- `git diff --cached --check` for the implementation commit — **passed**.
- All compilation/tests used offline locked dependencies except the one required offline lockfile refresh. Windows `TEMP`, `TMP` and `TMPDIR` pointed at a short repository-local path.

## Acceptance, scope and limitations

**Gates I0–I5:** Baseline captured; the single models/error boundary is compiled; all four existing normalizers and canonical preparation are migrated and used by Root; owner-authorized epoch completion/replay stays in Root; complete workspace, Root and CLI acceptance have passed. No format codec, persisted REL/PHY record, model workflow, source identity or public outcome was intentionally modified. No migration/legacy reader was introduced. Source-level algorithm parity and the import/recovery regression suite are the available behavioral proof; **no separate byte-by-byte promoted-corpus equivalence exercise was run**.

The approved crate-map dependency edges are maximum permissions, not dependencies to add; the actual physical graph contains only Archive under Import. The previous fresh Lexicon/Arcana graph scan timed out and the downstream Pitlord ownership graph had stale removed Context Engine paths. Those external tools have **not** supplied a fresh complete whole-repository architecture verdict for this cut. Direct Cargo dependency and source ownership checks are green; a stale graph is not presented as validated.

An unrelated pre-existing working-tree edit to `docs/graph-crate-extraction-evidence-2026-10-26.md` was preserved, not staged, not amended and not included in either Import commit.

**Result:** Existing Import normalization and canonical preparation now execute through `reliquary-import` as a real compiled crate. Root remains the sole authority for durable admission, source/Insomnia publication, historical Freshness settlement, strict completion and reconciliation. Implementation and acceptance are complete within the stated graph-tool limitation.

A subsequent workspace check during wrapper cleanup encountered a concurrent, unfinished History extraction (`reliquary_history` unresolved in `src/owner_lifecycle.rs`); this is separate from the recorded accepted implementation check above. The four provider forwarding functions now reside in `src/imports/mod.rs`, and the displaced Root provider files have been deleted.
