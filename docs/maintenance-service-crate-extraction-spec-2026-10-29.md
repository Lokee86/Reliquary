# Maintenance service crate extraction — implementation specification

**Status:** Specified; not implemented (2026-10-29).
**Target:** `crates/reliquary-maintenance/`, currently a placeholder.
**Branch / baseline:** `refactor/crate-first-restart`, post-History verification HEAD `b0bca3cd` (recheck immediately before cutting).
**Scope:** Correction-only crate extraction of **existing** maintenance behavior. No new maintenance features, algorithms, durable format, history machinery or compatibility readers.

## 1. Audited boundary and extraction decision

The lower `reliquary-maintenance-store` **already owns** the canonical `CVAHPR01` prune-manifest codec and projection. Do not re-extract it or duplicate its manifest models. `reliquary-history` owns passive historical result classification; owner stores own retention descriptors and version evidence; `reliquary-container` owns physical frames, copy/sync/replacement mechanics.

Production maintenance currently remains across `src/facade/cva_history_prune.rs`, `phylactery_history_prune.rs`, `cva_archive_history_prune.rs`, `cva_description_prune.rs`, `memory_history_prune.rs`, `owner_maintenance.rs`, `history_prune_manifest.rs`, `cva_repack.rs`, `cva_principal_repack.rs`, `cva_reconcile*.rs`, `src/rewrite/{file,lineage,model,plan,semantic_state}.rs`, and existing storage-reclamation support. Not all of these files belong in Maintenance. Their ownership must be established **per executable function** before cutting.

| Existing code family | Destination / authority |
| --- | --- |
| `CVAHPR01` canonical encoding, parsing, projection, replay | Existing Maintenance Store; **leave intact** |
| Deterministic prune/rewrite selection, immutable planning, retention eligibility decisions derived exclusively from supplied owner facts, reusable relocation mapping and policy validation | **Candidate Maintenance service** only if executable, actual Root call sites are rewired |
| `Cva`/`Phylactery` source reads, writer leases, lifecycle authority, version-prefix validation, import-settlement gates, all cross-owner source fingerprints | **Root / respective owner**; pass already-authorized facts downward only |
| Output-file creation, object append, physical cursor, sync, reopen/compare, failed-output cleanup, publication or destructive mutation | **Root coordination using Container**; Maintenance must not own a second file or opaque owner session |
| Archive/Memory/Graph/Entity/Relationship/Vector-specific tombstone/retention validation and codecs | **Their respective store and Root owner APIs**; no copy in Maintenance |
| Historical Import / Freshness replay, semantic settlement, inference | **Import/Freshness/Root**, not Maintenance |
| Cross-owner reconcile admission, conflict resolution tied to owner state, merge/promotion transactions | **Root coordination** unless a narrow pure existing decision function can be moved with explicit evidence |
| `#[cfg(test)]` / `history-oracle-fixtures` historical fingerprint helpers | **Tests only**; never counted as production extraction |

**Minimum meaningful extraction:** at least one substantial **existing production** maintenance policy/planning implementation must execute through `reliquary-maintenance` from the real REL/PHY prune/rewrite/repack call paths. No empty crate, reexport-only wrapper, new generic framework, or toy relocation helper qualifies. The precise shared slices are a discovery outcome of M0 and must be enumerated in evidence; do not claim wholesale extraction of Root maintenance orchestration.

## 2. Non-negotiable invariants

1. **Pure or explicitly passive service boundary:** Maintenance takes owned/borrowed immutable facts and returns an existing plan/decision/result; no `Cva`, `Phylactery`, writer, `Container` handle, filesystem path operations, network/model call, implicit sync or mutation inside service policy. Root retains admission, append, reopen and commit.
2. **One canonical type per value:** relocate an existing internal plan/selection type only when needed for real compiled call sites; reexport the same type through existing Root public paths. No parallel projections or data caches, new manager traits, trait-object registries, or unnecessary cloning of whole owners.
3. **Preserve exact decisions:** terminal eligibility, explicitly selected targets, duplicate rejection, shared-body protection, vector and content protections, import-epoch/settlement refusal, owner identity, source watermark, prefix checks and manifest fingerprint/proof validation remain equivalent, including error ordering and distinct REL/PHY behavior.
4. **Preserve bytes and history:** ADR 0041/0042 current-only files, `CVAHPR01` wire bytes, payload relocation, version clocks, tombstones, compact historical evidence, original signatures, shrink/no-op policy, output rollback and reopened verification stay unchanged. No new durable records or migrations.
5. **Dependency direction:** service may depend **only on proven lower crate interfaces** actually needed. Existing Maintenance Store and owner-store read models are allowed only if the current crate map permits and the direct Cargo DAG stays acyclic; never depend on Root, Runtime Gateway, Import service, Dream/Freshness/Ego semantic services or a higher owner coordinator. Do not add every permitted dependency as scaffolding.
6. **Keep functions small:** prefer policy modules below 200 lines; never introduce a 300+ line multipurpose coordinator. Avoid manufactured abstraction layers and expanding tests far beyond boundaries and parity protection.
7. **Actual product integration:** after cutover the Root implementation delegates the selected real work downward and no longer retains a second copy of its algorithm. Tests must prove the production path calls the lower crate rather than merely importing it.

## 3. Implementation gates

| Gate | Bounded task | Required evidence |
| --- | --- | --- |
| **M0 — source crosswalk/baseline** | Recheck HEAD, status, current Cargo graph. Enumerate functions and call sites across prune, normal/fresh rewrite, project repack, reconcile, storage relocation and REL/PHY; classify *move*, *stay*, and *test-only*. Select **one or two coherent existing pure policy slices**, prioritizing real prune/rewrite paths. Capture named focused tests. | Function-to-owner table, no speculative extraction, unaffected pre-existing Graph document |
| **M1 — canonical policy model** | Move only the selected existing immutable plan/decision/value definitions and pure implementations into Maintenance. Reexport prior public types when moved. Compile lower crate before changing Root. | Actual existing algorithm present; identical outcomes/errors; no duplicated definition or upward dependency |
| **M2 — real REL/PHY wiring** | Replace original decisions in every applicable production caller with the lower implementation. Do not move owner mutation or copy loop to bypass authentication. Delete displaced duplicate bodies. | Source diff and tests prove true compiled use; all selected REL/PHY callers integrated |
| **M3 — narrow policy parity** | Test selected active/eligible, rejected/duplicate, shared-reference, missing-proof and idempotent/no-op cases at service edge, plus real owner-level replay/reopen/rollback where relevant. Use existing fixture inventory before adding tests. | At least one external consumer test and named nonzero production tests; error order preserved |
| **M4 — independence review** | Inspect direct Cargo tree, remaining Root helpers and all affected source signatures; verify Maintenance Store remains source of manifest bytes and History remains passive; no leaked filesystem authority or introduced compatibility path. | Source crosswalk and DAG evidence; unchanged external interfaces |
| **M5 — full acceptance/commit** | Run complete Root, workspace, CLI, formats, docs and architecture checks; record results and unresolved tooling limitations. Commit implementation and acceptance documentation, without unrelated files. | Compiled and accepted production extraction, not a placeholder |

**Stop rule:** If M0 finds no meaningful existing policy that can be moved without Root/owner backreach, **do not invent one**. Record the specific blocker and propose revising the planned crate allocation. If M1 reveals concrete incompatible borrowed IDs or upward dependency, stop, amend this spec from compiler/source evidence, and do not create a duplicate owner projection.

## 4. Verification

Capture baseline **before editing**. History's prior 1,457 Root passed / 5 ignored and 32 CLI passed are reference results, not new Maintenance acceptance.

Run at minimum:

- `cargo check -p reliquary-maintenance --locked --offline -j 1` and `cargo test -p reliquary-maintenance --locked --offline -j 1`, with an actual external product consumer.
- Existing nonzero focused Root cases for REL and PHY `prune`, `rewrite`, `history_rewrite_contract`, `repack`, tombstone/manifest proof, reopen and conflict/reconcile where selected paths are affected. Name filters and counts; zero selected does not count. Include normal and fresh rewrite, retained history, malformed proof, rejected targets, output refusal/rollback and preservation of unselected records.
- `cargo check --workspace --all-targets --locked --offline -j 1`; `cargo test --lib --locked --offline -j 1 -- --test-threads=8`; `cargo test --manifest-path cli/Cargo.toml --locked --offline -j 1`. If local CLI lock needs updating, refresh **offline**, then rerun locked.
- `cargo fmt --all -- --check`, CLI formatting, `git diff --check`, `cargo tree -p reliquary-maintenance --depth 1 --offline`, `ddocs check --indexes --links`, and existing architecture/ownership guards where operational. Fresh Lexicon/Arcana/Pitlord graph acceptance must not be inferred from stale snapshots or timed-out scans. Prefer short local writable TEMP/TMP/TMPDIR and `-j 1` to avoid previously observed Windows paging exhaustion.
- Compare original vs extracted path on real product cases, exact public signatures/errors, bytes/proofs and retention/owner identity. Never claim a CVAFRS07 corpus acceptance run without it.

Record selected exact source symbols, what remains in Root, baseline/implementation commits, test outputs, any environment-only failures, and unresolved architecture-tool limits in `docs/maintenance-service-crate-extraction-evidence-2026-10-29.md`. Update `docs/api.md`, `architecture.md`, `maintainer-map.md`, `crate-first-layout-spec-2026-10-25.md`, `docs/INDEX.md` and crate map **only** where actual ownership/dependencies changed.

**Acceptance:** at least one meaningful *already implemented* maintenance decision or plan is owned and invoked through the compiled service crate; no duplicated Root algorithm; REL/PHY and owner-authorized destructive semantics preserved; focused/full tests green. Completion does **not** imply all pruning, rewriting, reconciliation or Container storage code has moved from Root.

## References

[Maintenance Store evidence](maintenance-store-crate-extraction-evidence-2026-10-27.md); [History extraction](history-service-crate-extraction-spec-2026-10-29.md); [crate layout](crate-first-layout-spec-2026-10-25.md); [crate map](architecture/crate-map.json); [ADR 0041](decisions/0041-semantic-import-boundaries-owner-pruning-history-rewrite.md); [ADR 0042](decisions/0042-minimal-lossless-current-history.md).

**Specification only:** no production Rust, Cargo dependency or durable corpus changes were made.
