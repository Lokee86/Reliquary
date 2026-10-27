# Owner Lifecycle crate extraction — specification

**Status:** Specified, implementation not begun (2026-10-27). **Target:** `crates/reliquary-owner-lifecycle`. **Prerequisites:** extracted lower stores integrated and committed, including External Index and the Maintenance Store decision. Correction-only compiled ownership cut.

## Intended responsibility
Owner Lifecycle coordinates **opening, validating and rebuilding one REL/PHY owner from already accepted Container evidence and lower owner-store APIs**. Unlike rank-2 stores, this crate is explicitly a **higher coordinator**: permitted dependencies are `reliquary-container`, `reliquary-archive`, `reliquary-memory-core`, `reliquary-echo`, `reliquary-graph`, and the actual extracted owner-store crates used by the implementation (Perception, Ego, Freshness, Vector, Insomnia, Dream, Import, External Index, Maintenance when real). Do not add a dependency just because the crate map permits it. No semantic processors, runtime gateway, Cva/root product, Import service, History service, Maintenance service or inference endpoints.

## Existing source crosswalk
Inventory `src/facade/cva_lifecycle.rs`, `cva_open_route.rs`, `cva_global_validation.rs`, `cva_vector_recovery.rs`, `cva_dream_journal_replay.rs`, `src/imports/epoch_scope.rs` and owner-specific replay helpers. Split by responsibility rather than moving entire facades:

- **Lower-owner reconstruction coordination:** physical frame traversal, owner identity and scope checks, orderly replay dispatch to compiled owner stores, accepted-prefix checks and cross-store *structural* integrity validation.
- **Remain in root / higher orchestration:** `Cva` construction and public API, semantic Archive-to-Insomnia policy, population selection, Memory authorization, Freshness scoring, Import settlement, Dream inference, repair, pruning, publishing, gateway grants and external provider operations.
- **History:** historical cut selection and proof interpretation remain read-only History composition; Owner Lifecycle exposes a bounded accepted-prefix reconstruction primitive rather than importing History.

## Mandatory design decisions before coding
1. **Inventory actual dependency paths first:** identify each root lifecycle operation's inputs, outputs and imports. The coordinator must not import the root `Cva` type or call root-only validations; design an owned `OpenedOwnerState` or direct tuple of concrete lower stores as necessary, with no generic service locator or cloned duplicate state.
2. **Single physical reader:** use Container's accepted frame/cursor semantics. Preserve physical order when different owners share a frame, and special replay sequences (Dream population prefix-before-Begin, Insomnia full completion authorization, grouped Memory birth and import epoch). **Do not replace** those higher semantic authorizations with shallow header recognition. Any reconstruction requiring unavailable higher services stays in the higher coordinator until a genuine narrow proof input seam can be established.
3. **Authority at the boundary:** lower store validates intrinsic records; Owner Lifecycle validates cross-owner structural references and accepted coordinates using owner read APIs; the root/high services retain semantic admission and conditional mutations. An object must never be treated as accepted merely because its encoded family magic matches.
4. **Error fidelity and APIs:** preserve native error distinctions, owner identity, fail-closed corruption behavior and current-only formats. Assess borrowed root types, profile IDs and references before moving. Root can consume owned lower-store outputs; no unsafe casts, permanent mirrored cache or surprise public break.
5. **Replay and side effects:** opening/history reading is passive; only explicitly existing repair and writer operations may mutate. Preserve REL/PHY parity, deterministic replay ordering and no new file format or background work.
6. **Risk of oversized cut:** slice by real owner replay groups and compile after each; keep coordinator small and avoid moving all `cva_lifecycle.rs` logic as one unit. If a slice requires a semantic service dependency, leave that slice in the appropriate higher crate and document the barrier, not a forbidden reverse edge.

## Bounded implementation checkpoints
1. Freeze baseline/HEAD, enumerate entire lifecycle call graph and constructor/API signatures. Record the replay order, special cases, existing golden owner fixtures and separate REL/PHY paths. Preserve unrelated Graph evidence modification.
2. Establish smallest compiled coordinator contract with Container/Archive/Memory/Echo primitives. Root still creates `Cva`; no public API break.
3. Move deterministic accepted frame routing and per-owner open/projection seams incrementally, using genuine lower store APIs. Explicitly handle Dream, Insomnia and Import authenticated replay ordering.
4. Move only cross-owner *structural* post-replay checks; retain semantic validation and mutation above. Eliminate displaced duplicates.
5. Verify repeated read-only reopen, malformed frame and foreign-owner rejection, grouped births, retained compact completions, Dream compact Begin, import start/complete, missing backing derived index, history cut and REL/PHY fixtures, plus fault/recovery ordering.
6. Run `cargo check -p reliquary-owner-lifecycle --locked --offline`, its tests, `cargo check --workspace --locked --offline`, `cargo fmt --all -- --check`, full root focused owner/replay/history suites, and `cargo test --manifest-path cli/Cargo.toml --locked --offline`. Document results in `docs/owner-lifecycle-crate-extraction-evidence-2026-10-27.md`; update architecture, maintainer map and storage format and commit only after real production cutover.

**Acceptance:** Root uses a compiled owner lifecycle path for proven portions, with no forbidden Cargo dependencies, duplicate state, lost validation, replay nondeterminism or changed current wire. An incomplete source cut must be reported as partial, not complete.

**Out of scope:** runtime/gateway extraction, generic DAG/ratchet tooling, repairs, migrations, new schemas, History or semantic service extraction, CVAFRS07 recovery claim absent its own execution.
