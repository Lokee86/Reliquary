# Multiplexing P5-A local acceptance / integration handoff — 2026-10-06

Parent: [P5-A execution](multiplexing-phase5a-rel-consumers-2026-10-06.md). Branch: `feat/multiplexing-p5-rel-consumers`; worktree: `C:!binworkspaceReliquary-p5a-rel-consumers`; base: `c24b06273abbab383eb74780bf82cb5c0ceef0b5`.

## Implementation scope

A0: real legacy-to-canonical caller/API inventory in [P5-A matrix](multiplexing-p5a-consumer-matrix-2026-10-06.md), including concrete Warlock call sites, authority and P5-C/D handoff.

A1: added exact owner-granted branch-wide Archive result APIs. Existing exact transcript, Echo, dependency Archive index and page reads remain unchanged.

A2: added four owner-polymorphic knowledge mutations with separate REL ManageRel+post-sync P4 event and private PHY UseOwnPhy+sync paths. Legacy successor/retirement and relation-inheritance semantics preserved without a second owner or compatibility wrapper. Added owner-polymorphic provenance read/resolution with explicit source REL grant and final private owner revalidation. No PHY private bodies enter REL snapshots.

A3: current gateway already provides exact-owner project validation/checkpoint/ingest, status and quiesced reconciliation. No duplicate owner authority introduced.

A4: updated P4 public API classification guard, with negative/positive fixtures. Legacy files stay physically present for P5-D; root imports/exports are explicitly not changed. No Warlock or CLI edits, no worker owner modifications, no P6 benchmarks.

## Verification and gate record

Production code is compiled and guard-classified, but **P5-A runtime acceptance remains open** because dependency/test build failures prevented execution. Historical P3/P4 results do not count as new P5-A passes.

| Gate | Result |
| --- | --- |
| Cargo locked offline library check | **PASS**: `cargo check --offline --locked -j 1` compiled the new canonical files (warnings only); final rerun timed out rebuilding external dependencies after test-cache churn; original production check passed and the final change was a token revalidation at delivery |
| Cargo formatting | **PASS**: `cargo fmt --all -- --check` exit 0 |
| P4 public API/worker source guard | **PASS**: `python scripts/check_multiplexing_phase4_coverage.py --self-test` — 30 positive/negative fixtures, 120 classified public gateway APIs |
| Focused P5-A negative tests | **NOT RUN**: first `cargo test --lib p5a_` timed out compiling third-party dependencies; retry failed in external `lore-io` (missing `bytes` and `windows_sys` artifacts) |
| Shared-cache focused test retry | **NOT RUN**: failed in third-party `futures-core` with Rust sysroot `core` metadata-stub/prelude errors before Reliquary tests were built |
| Rust test-source type-check | **NOT COMPLETED**: separate `cargo check --tests` was stopped during long dependency compilation; no claim tests compile or pass |
| Existing gateway auth/project/semantic suite | **NOT RUN**; remains an acceptance gate |
| P2/P3/architecture, documentation structural tests | **NOT RUN**; P4 lexical guard passed but does not replace these gates |
| Git diff/commit | Verified only scoped P5-A paths; see branch commit; source tests remain unverified |

## Handoff and unresolved scope

P5-C must supply per-instance context and timestamp arguments in Warlock. P5-D must move the three public result type definitions out of legacy modules and remove legacy root wiring *after* upgraded consumers pass. Trusted project creation/adoption paths need a separate explicit convergence decision. No broad migrated-client or release acceptance is claimed here.

Documentation impact: inspected architecture/invariants, project/storage contracts, Phase 0B inventory, P4 publication, P5 parent and P5-A specs; updated this source-backed matrix and scoped acceptance. No canonical shared architecture or roadmap status is changed before final integration. Source classifications enforce API admission patterns but are not a substitute for runtime/negative testing.
## Verification status boundary

Do **not** mark Phase 5-A completely accepted until the new focused tests and existing gateway auth/project suites execute and pass with an intact Rust dependency cache. The failure traces concern external `lore-io` and `futures-core` compilation, not failing assertions; this is not evidence the new behavior is correct. The standalone production `cargo check` and source guard establish only compilation and classification. P5-C may inspect this branch/API matrix, but final integration should wait for those gates.