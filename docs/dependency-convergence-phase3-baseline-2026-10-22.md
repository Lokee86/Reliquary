# Dependency Convergence Phase 3 Baseline — 2026-10-22

Parent index: [Documentation index](INDEX.md)

## Purpose

Freeze the accepted Phase 2 source checkpoint, graph history, owner/consumer routing, canonical history corpus and executable starting checks for Phase 3. This is bounded before-image evidence; it does not claim that Phase 3 interfaces or implementation have been accepted.

## Overview

Phase 3 starts from accepted Phase 2 source commit `1380d0b` (`Separate Phase 2 durable owners and verify local acceptance`). The current branch tip is `b113bcf` (`Specify Phase 3 historical maintenance and import convergence hard cut`), which establishes the Phase 3 scope. The Phase 2 execution ledger reports all twelve gates accepted locally, including 2,142 tests across 26 targets, 32 CLI tests, and the six-source oracle. Those are prior receipts, not tests rerun by this baseline. The pre-existing `.gitignore` worktree modification is outside the phase. No Rust source or corpus file was changed for this inventory.

The Phase 2 source receipt is [`dependency-convergence-phase2-verification-2026-10-22.json`](../benchmarks/dependency-convergence-phase2-verification-2026-10-22.json), SHA-256 `ce129fe21d37a18884c7e514db1d2692133046919076eed41f798f668b5f2200`. It records 1,134 authored Rust files and manifest SHA-256 `a419d4bc400ebb99a8d38a228f3f27e66bb6a7f9c8ea85e70e24839344a8c9ad`, with the indexed source hashes. Root compared all 1,134 current source hashes to that receipt: **0 mismatches**. The separate working-tree `.gitignore` edit is not folded into that source evidence.

## Graph and dependency before-image

The original Phase 0 capture and classified findings remain immutable. The conceptual capture records source baseline `c43d33c136f560fdca2249619866e1acf9b7fdf3`, 19 coarse areas, and 4,609 initial classified findings. Its source graph remains at [`tools/pitlord/convergence-graph.json`](../tools/pitlord/convergence-graph.json); the Phase 0 source-capture blob SHA-256 is `a0deb5c2b011f0aa1d29125b3878d09bfecfc192c1ff72ab40236e1674e259d2`. Phase 2 retained the reviewed convergence delta without rebasing: 4,630 classified findings, 4,507 unchanged fingerprints, 118 stale fingerprints with dispositions, and 123 new or reidentified findings reviewed individually. The accepted 34-component DAG remains [`convergence-component-dag.json`](../tools/pitlord/convergence-component-dag.json), with 163 allowlisted edges. Phase 2's stabilized Arcana receipt is snapshot `sha256:f6ee9a86f878515af28ce0cb93d13a5454b2846e2b692697e5f6d4b3c19a8716`.

These records are distinct populations and must not be summed or treated as interchangeable: the 4,609 Phase 0 capture, the Phase 1 4,625 source-cleanup ledger, and the Phase 2 4,630 reviewed finding set each have separate scope and provenance. Phase 3 keeps the original graph and reviewed ledgers as the before-image. A clean DAG model or self-test does not assert that production source has already reached the target.

## Canonical REL/PHY history before-image

The immutable oracle is [`canonical-history-equivalence-2026-10-21.json`](../benchmarks/canonical-history-equivalence-2026-10-21.json), SHA-256 `d3c1cae685932927d30dd4a0b4189280b4acd0487397e89632022fff0c2b7d34`. Its Phase 2 rerun is independently receipted in the Phase 2 verification JSON above. The recorded run passed in 6,398.24 seconds with zero owner-domain mismatches and `authoritative_sources_unchanged: true`. The Phase 2 receipt records the exact source hashes and UTC modification times below; all six source hashes match the Phase 1 accepted files. Root rehashed the six external originals, verified their recorded sizes and hashes, and copied them to repo-local `.verification-tmp/phase3-before/`; each frozen copy matches its receipt. The machine-readable capture is [`dependency-convergence-phase3-baseline-2026-10-22.json`](../benchmarks/dependency-convergence-phase3-baseline-2026-10-22.json), SHA-256 `0e49c2bb71c19832fba173857fdc7afe6a24e21f0ae9e92ebd9c42ff07f692b1`. This operation read and copied canonical files but did not alter them.

| Canonical source | Bytes | SHA-256 | Recorded `last_write_utc` |
| --- | ---: | --- | --- |
| `chatgpt-first14d/project.prj.rel` | 30,742,210 | `7e452de7aadff26f5a891538bd8923fe7b499e680d0491499cba2ea187b4fd8d` | `2026-10-21T19:08:18.8580744Z` |
| `chatgpt-first14d/user.phy` | 675,300 | `7e690b4853afe0936702dc023f511dd7c78ffaae3e18e8e66f6ab1c6891a6c63` | `2026-10-21T19:08:18.9127663Z` |
| `chatgpt-first28d/project.prj.rel` | 76,542,131 | `1381fa663a41ebff3c69cb0418ff3f2369294195d10e36096bbec01cec82cf92` | `2026-10-21T19:08:19.0254778Z` |
| `chatgpt-first28d/user.phy` | 1,507,220 | `32970f3975bfcd6de0fcdb78df9c80c78b76787832f528c1b09fd75dfa4f501d` | `2026-10-21T19:08:19.1592491Z` |
| `rhelm-david-r-ellis-pre-relationship/project.prj.rel` | 18,315,280 | `dd7f0f0cf675901d456495a5ee93eb6c8884d793b4b9dd8b563a1ad4746043ab` | `2026-10-21T19:08:19.1948592Z` |
| `rhelm-david-r-ellis-pre-relationship/user.phy` | 3,180,063 | `58ac7872215e126749f024fc742828abc9a8a8f84775b39c0706d638392ce9e0` | `2026-10-21T19:08:56.9607511Z` |

The six canonical sources live in the external local fixture tree `../reliquary-fixtures/local/authoritative/`; the repo-local copies provide a phase-local frozen before-image. They are the before-format corpus for Phase 3's semantic comparison. Current runtime and V2 clock/proof behavior remain the Phase 2 accepted behavior. The originals were read and hashed, then copied without mutation; the frozen copies' hashes match.

The existing `CVAFRS06` family occupies 7,415,741 frame bytes across the six canonical files. A bounded read-only frame walk over the six frozen copies counted one settlement frame in each REL and none in the PHY files. The magic plus payload account for 7,415,717 bytes; the three 8-byte Container length prefixes account for the remaining 24 bytes. The former physical-prefix-dependent settlement family totaled 395,533 bytes. The 18.75× difference is the approved minimization target; it does not authorize restoring physical-prefix authority. This is family-level wire accounting, not a semantic oracle or the field-by-field audit required at P3.8.

| Canonical source | `CVAFRS06` count | Payload bytes | Frame bytes including 8-byte Container length |
| --- | ---: | ---: | ---: |
| `chatgpt-first14d/project.prj.rel` | 1 | 1,830,473 | 1,830,481 |
| `chatgpt-first14d/user.phy` | 0 | 0 | 0 |
| `chatgpt-first28d/project.prj.rel` | 1 | 4,330,845 | 4,330,853 |
| `chatgpt-first28d/user.phy` | 0 | 0 | 0 |
| `rhelm-david-r-ellis-pre-relationship/project.prj.rel` | 1 | 1,254,399 | 1,254,407 |
| `rhelm-david-r-ellis-pre-relationship/user.phy` | 0 | 0 | 0 |
| **Total** | **3** | **7,415,717** | **7,415,741** |

## Operation, consumer and owner routing

The accepted DAG makes History read-only, Maintenance the explicit mutation-policy owner, and Import the epoch-policy owner. The following routing preserves current composition while naming the target seam; it does not authorize new public or mounted behavior.

| Existing path / consumer | Current responsibility and consumers | Phase 3 target / constraints |
| --- | --- | --- |
| `facade/historical_global_{cuts,frontier}.rs`, `historical_cut_enumeration.rs` | Test-only historical cut enumerators: `lib.rs` includes these exact modules under `#[cfg(test)]`. They support test/oracle assertions and are not runtime consumers or production source edges. | Keep test-only; do not promote them into History runtime or count them as production violations. |
| `facade/owner_history_proof.rs` | Production source-history digest and prune-manifest verification; called by explicit REL/PHY history-prune code. It composes Container frames, Graph evidence and Maintenance manifests. | History owns read-only normalized source-history composition; Maintenance consumes that view for explicit validation. No History write/upcall. |
| `facade/historical_reconstruction.rs`, `cva_historical_import.rs`, `historical_import_base.rs`, `facade/cva_freshness*.rs` | Production historical Freshness reconstruction and settlement logic. Historical import freeze/build/commit and epoch settlement publication run through `cva_historical_import.rs`; reconstruction is an explicit offline API. | History prepares read facts; Import or the existing authorized offline operation publishes settlement. No writer, Freshness reinforcement, or provider call in History. |
| `facade/{cva,phylactery}_history_prune.rs`, `storage_reclamation.rs`, `semantic_maintenance.rs`, `maintenance/{store,manifest}.rs` | Public Cva/PHY prune and reclaim operations compose owner-local retention proofs and the maintenance manifest; CLI repair/reclaim commands use the public operations. | Maintenance owns explicit prune, reclaim, repair and rewrite authorization; stores retain their own proof and publication authority. No automatic open repair or semantic authoring for stores. |
| `rewrite/{model,plan,file,semantic_state,lineage}.rs` and facade rewrite adapters | Public planning and rewrite operations inspect, transform, publish and validate a separately written destination. | Existing behavior converges under Maintenance with the same identity modes and product contract. |
| `facade/cva_reconcile*.rs`, `imports/{reconciliation,reconciliation_replay}.rs` | Cva reconciliation consumes History/owner data; native import reconciliation consumes epoch markers and replays only existing supported owner payloads. | Maintenance owns generic divergent-history comparison/publication. Import keeps epoch-specific interpretation and bounded continuation authorization. No Import→Maintenance edge is allowed by the accepted DAG. |
| `imports/{epoch_store,epoch_model,epoch_codec,epoch_scope,readiness,completion}.rs` | Import epoch store owns durable start/complete state; public Cva import/readiness/completion paths derive exact owner evidence and bind/sync completion. | Import store owns epoch state; Import coordinator owns scoped admission, readiness, settlement, completion and recovery. Existing completion visibility and evidence requirements remain. |
| `freshness/{freshness_semantic_settlement,freshness_import_settlement}.rs` and Freshness replay | Freshness owns accepted event/proof state and current `CVAFRS06` encoding; Import completion binds the accepted epoch settlement. | Freshness owns the corrected compact settlement model and validation; Import builds and commits the epoch proof. Owners remain authority for their facts. |
| CLI, public API and Runtime/Gateway | CLI maintenance commands delegate to Cva/PHY repair, prune, reclaim and rewrite operations. Public Cva APIs expose history/rewrite/reconcile and native import/readiness/completion; Runtime/Gateway consumes public owner APIs for scheduling and reads. | Keep wrappers thin; Phase 3 introduces no new public surface. Mounted import/reconstruction remains unsupported; Runtime does not become store dependency or durable owner. |

### Accepted owner read and proof contracts (P3.0.03–.04)

This inventory names existing source-level contracts only. All methods below are current Phase 2 interfaces or already-existing operations; this table proposes no new durable owner or API.

| Owner / path | Existing methods or evidence | Authority and phase boundary |
| --- | --- | --- |
| Maintenance, `src/maintenance/store.rs` | `OwnerMaintenanceStore::{ingest,get,manifests,is_empty}` | Stores validated prune manifests by digest. It is owner-neutral proof storage, not mutation authorization. |
| Maintenance, `src/maintenance/manifest.rs` | `HistoryPruneManifest::{validate,encode,decode,digest,authorizes}`; typed `HistoryRetentionDescriptor`, `RetentionTarget`, `RetentionDomain`, `PrunedPayload` | Manifest validity, canonical bytes and target/payload bindings are local to the proof schema. Domain owners still establish whether a selected payload is pruneable. |
| Archive, `src/archive/archive_history_prune.rs` | `Archive::retention_proof(container, conversation_id, watermark, lengths)`, `pruned_content_markers()`, `node_content()` | Archive owns tombstone/retained-structure proof and its pruned-content markers; it does not decide cross-owner Memory validity. `archive_prune_admission.rs` composes its proof with Maintenance metadata. |
| Memory, `src/memory/memory_store.rs` | `MemoryStore::{retention_proof,retention_descriptor,records,pruned_body_markers,validate_provenance_fields,retained_body_bytes}`; existing `GroupedBodySpan` values for grouped replay | Memory owns body bytes/hash, provenance field shape, tombstone/topology and pruned-body evidence. The caller supplies accepted Archive/Insomnia/external facts before grouped publication. |
| Graph, `src/semantic_graph/graph_store_read.rs` | `GraphStore::{semantic_nodes,active_semantic_relations}` | Graph returns its accepted semantic nodes and active relations; lifecycle/admission callers validate foreign Memory/Entity references. `semantic_nodes` is production-visible after Phase 2. |
| Entity and Relationship, `src/perception/` | Existing `EntityStore::{contains,contains_historical}` and `EntityStore::records()`; `RelationshipStore::records()` and local participant/revision validation | Perception owners retain record/schema authority. Higher lifecycle/admission code checks cross-owner Memory/Entity identity and scope using the existing owner getters. |
| Freshness, `src/freshness/` | `FreshnessStore::{requested_import_event_entries,import_settlements,import_settlement,latest_import_settlement,records_at_cut}` | Freshness owns accepted event/proof identities, settlement values and cut reads. Import binds and commits epoch-specific evidence; exact event lookups do not transfer Freshness authority. |
| Import, `src/imports/epoch_store.rs` and `src/imports/commit.rs` | `ImportEpochStore::{ingest,append_start,open,status,statuses,validate_owner}`; public Cva operations `import_native`, `native_import_receipts`, `open_import_epoch`, `import_epoch_status`, `import_epoch_statuses` | ImportEpochStore owns start/complete records. The higher Import/Cva coordinator owns scoped admission, readiness, settlement and completion; completion append stays coordinator-private. |
| Import reconciliation, `src/imports/reconciliation.rs` | `reconcile_import_epochs` over accepted epoch/owner facts and existing supported physical replay operations | Import retains epoch-specific interpretation and authorized continuation. Generic divergent-history comparison/publication remains assigned to Maintenance. |

The production/test split follows the module declarations in `src/lib.rs` and direct source imports. In particular, historical cut enumeration and historical fingerprints in the test module group are `#[cfg(test)]` (with the opt-in current-corpus oracle gated by `all(test, feature = "history-oracle-fixtures")`). `owner_history_proof.rs`, `historical_reconstruction.rs`, `cva_historical_import.rs`, prune/reclaim/rewrite and Import coordinator modules are production paths. Do not infer production ownership edges from colocated `_tests.rs` files or from test-only modules.

The principal verification seams are `historical_current_corpus_tests`, `historical_global_frontier_tests`, `historical_cut_enumeration_tests`, `historical_matrix_*`, `historical_recovery_tests`, `history_prune_tests`, `cva_reconcile_*tests`, `imports/{reconcile_tests,reconciliation_*tests,completion_tests,readiness_recovery_tests}`, `freshness_semantic_settlement_tests`, `historical_import_*tests`, and `semantic_state_tests`. These are named source seams from the Phase 3 spec, not a claim that every filter was exercised in this baseline. Test-only fingerprint/oracle code must remain tagged as test-only in any source-edge review.

## Executable baseline and platform record

Cheap deterministic guards were run for this Phase 3 baseline:

| Command | Result |
| --- | --- |
| `python scripts/check_physical_boundary.py` | Passed: physical guards and 13 fixtures. |
| `python scripts/check_component_dag.py --self-test` | Passed: 34 components, 163 allowlisted edges, no cycles/upward dependencies; negative fixtures passed. |
| `python scripts/check_owner_boundaries.py --self-test` | Passed: owner-boundary checker fixtures. |
| `git diff --check` | Passed before this document was created; rerun after authoring. |
| `cargo check --locked` | Root baseline run passed in 20.24 seconds; 121 existing warnings and one cache hardlink warning were reported. |

The prior Phase 2 committed verification also records `cargo fmt --check`, locked Rust checks, all 26 feature-enabled Rust test targets (2,142 passed, 0 failed, 7 ignored), CLI format/check and 32 passing CLI tests, ten focused profile tests, the prepared graph roundtrip, and the separate canonical six-source oracle. The baseline record at [`dependency-convergence-phase2-baseline-verification-2026-10-22.md`](dependency-convergence-phase2-baseline-verification-2026-10-22.md) documents that the unprivileged sandbox library run encountered filesystem/process/provider failures, while a focused elevated lock reproduction passed and the elevated frozen suite passed 1,957/0/5. Phase 3's current source baseline must not claim those previous tests as a new rerun. Root review of this P3.0 evidence is pending; the current source hash reconciliation and six-file copy/hash capture have been completed, but acceptance is not self-declared here.

Phase 2 acceptance was executed on Windows. Unix lock, identity and publication behavior was source-reviewed, not executed. The ordinary sandbox's platform-sensitive test failures were resolved only in a separately recorded elevated local run; this is an environment limitation, not a product behavior change. The six-file full oracle took 6,398.24 seconds and is deliberately not repeated for this bounded inventory. The Phase 3 spec schedules focused owner checks during moves and the exhaustive oracle only after conversion freeze and at final acceptance.

`P3.0.07` is therefore bounded source/corpus evidence: the accepted component model and cheap physical/owner/DAG guards pass, but Phase 3 has not yet produced a refreshed source graph or a new full-suite/oracle run. The existing facade historical, reconciliation, prune/reclaim and rewrite composition paths are planned extraction scope, not newly classified violations. Any new or reidentified production edge must be reviewed against the immutable Phase 0/Phase 2 ledgers; test-only and analyzer findings remain separately identified.

## Related docs

- [Phase 3 specification](dependency-convergence-phase3-history-maintenance-import-spec-2026-10-22.md)
- [Phase 3 owner-contract handoff](dependency-convergence-phase3-handoff-2026-10-22.md)
- [Phase 2 execution ledger](dependency-convergence-phase2-execution-2026-10-22.md)
- [Phase 2 source inventory](dependency-convergence-phase2-baseline-2026-10-22.md)
- [Phase 2 baseline verification](dependency-convergence-phase2-baseline-verification-2026-10-22.md)
- [Accepted target DAG](dependency-convergence-target-dag-2026-10-21.md)
- [Semantic import-settlement minimization plan](semantic-import-settlement-minimization-plan-2026-10-21.md)

## Notes

This is a Phase 3 before-image only. It does not assert independent validation of every Phase 2 acceptance claim, certify source graph closure, run the exhaustive corpus scan in this turn, or modify source, format or canonical data.
