# P01 — public Rel/Phylactery API and consumer CLI acceptance

**Verdict: ACCEPTED** for the specified current-format 2.1 public product surface, trusted-host integration and REL-only operator CLI on `rebuild/clean-architecture`, 2026-11-06.

**Authoritative contract:** `Reliquary-crate-first/docs/architecture/p01-public-product-api-cli-implementation-spec-2026-11-06.md` (G3 and GM are satisfied prerequisites). The retained behavior map is `docs/p01-consumer-contract-map.md`; usage and intentional migration differences are in `docs/public-product-api.md`.

## Reviewed implementation and scope

| Slice | Result and evidence |
| --- | --- |
| P01-A consumer mapping | Reference `workspace_management.rs`, native import, historical payload, retrieval, multiplexing and CLI families mapped in `docs/p01-consumer-contract-map.md`; documented intentional retirement of `Cva`/CLI alias, PHY command family, developer simulation and unauthenticated provider CLI. |
| P01-B owner public API | Root `src/{lib,owner,error}.rs` exposes opaque `Rel` and `Phylactery` create/open/verify/info with current 2.1 native owner admission, typed error classification, stable identities and REL/PHY scope separation. Root has no original Root implementation and no mutable Container/store exports. |
| P01-C host admission | `src/{host,host_admission,host_instance}.rs` exports one trusted credential/grant gate over the accepted Gateway, with revocation, immutable grants and private mutable sessions. |
| P01-D management | `src/management.rs`, Application `management_workflow.rs`, Gateway `instance_management.rs` and owner-session `session/management.rs` implement bounded conversation/transcript pages, title updates, direct Memory/provenance/stats/health and quiesce/reopen. View grants cannot mutate and revoked credentials cannot continue. |
| P01-E history/maintenance | `src/{maintenance,maintenance_owner,maintenance_operations}.rs` exposes bounded historical cuts/proofs, policy rewrite, terminal Memory/Archive/Entity/Relationship prune, profile repair/retired vectors, unreachable storage reclamation and supported same-owner REL reconciliation through existing accepted GM workflows. Destination Dream admission replay is corrected in `rel_reconciliation_workflow.rs`. Unsupported divergence fails closed. |
| P01-F standalone CLI | Separate `cli/` package depends on **only** the public `reliquary-memory` library (plus JSON presentation); REL create/info/verify/history/rewrite/prune/repair/reclaim/reconcile use product operations. No `cva` alias or `phy` command; errors are nonzero and JSON. CLI argument parsing was corrected to reject unexpected, duplicated or missing-value options and to apply repeatable rewrite selections. |
| P01-G evidence and docs | External Cargo consumer depends only on Root; public integration suite and standalone CLI tests exercise distinct entry points. Public usage and exclusions documented. |

**Implementation review — architecture/standards:** The root has no Cva aliases, mutable store bag or local reimplementation of the semantic pipeline; source inspection confirms the public modules use the accepted Application/Gateway/owner-session contracts. The architecture checker remains at 23 packages and 76 approved production edges. Newly added public/CLI coordination modules were split into responsibility-local files under 300 lines. Existing lower-level warnings do not represent a new P01 dependency or behavior regression.

**Implementation review — specification/conformance:** The public API and CLI expose the selected retained 2.1 consumer operations and intentionally decline old 2.0/FRS07 runtime support, unauthenticated model/worker commands, arbitrary PHY/cross-owner merges and deferred semantic features. No unsupported operation is reported successful as a placeholder. GM-specified verified-copy and historical behavior remains owned by and tested through the accepted underlying workflows.

## Executed verification

Run in `C:\!bin\workspace\Reliquary-rebuild`, using the same `target-codex-check` directory:

- `python scripts/check_architecture.py` — **passed**: 23 packages; 76 production edges.
- `cargo fmt --all --check` — **passed** after source modularization.
- `cargo fmt --manifest-path cli/Cargo.toml --check` — **passed**.
- `cargo check --workspace --locked --offline -j 1 --target-dir target-codex-check` — **passed**.
- `cargo test --workspace --locked --offline -j 1 --target-dir target-codex-check -- --test-threads=1` — **passed** for all workspace crate, integration, public API and doc-test targets; six P01 public tests passed.
- `cargo test --manifest-path cli/Cargo.toml --locked --offline -j 1 --target-dir target-codex-check` — **passed**, seven CLI integration tests.
- `cargo check --manifest-path tests/external-consumer/Cargo.toml --locked --offline -j 1 --target-dir target-codex-check` — **passed** without direct imports of lower-layer packages.

Public integration tests demonstrate REL and PHY owner identity/kind enforcement; management privilege separation, revocation, provenance, quiesce and independent reopen; committed native conversation with bounded management transcript/title; non-mutating rewrite and exact-copy reconciliation with no-replace collision; retained Memory history and canonical historical proof after reopen. CLI regressions cover unexpected/duplicated options, invalid explicit Memory selection, no-output dry run, wrong kind, legacy command rejection, real reconciliation, history proof and immutable sources.

The independent GM acceptance tests continue to own detailed multi-domain safety, completed-import Dream/Insomnia/Freshness history and M03 conflict/failure verification; P01 does not replace or relax them.

## Explicit limits and next gate

P01 accepts a **native 2.1** library/CLI surface, not full product/corpus acceptance. G4 must run complete generated 2.1 product scenarios, including integrated consumer workflows. D01–D03 remain responsible for externally verified offline conversion of six legacy 2.0 owner fixtures and safe activation. G5 still owns measured performance and production cutover. Provider deployments, broad quality benchmarking, future Ego/Perception synthesis, arbitrary history merging, old-format runtime readers and a network daemon are not claimed.

The reference CLI's PHY and developer command families were not retained as product commands; the full private PHY **Rust API** remains supported. Revisit CLI scope only on documented consumer demand, not by silently importing the legacy command router.
