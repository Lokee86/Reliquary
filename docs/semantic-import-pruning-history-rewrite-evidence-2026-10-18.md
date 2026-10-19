# Semantic import, pruning and history rewrite — baseline evidence

Parent: [Documentation index](INDEX.md).

Status: **Complete.** This record preserves the original read-only baseline and subsequent implementation checkpoints, then closes with the Step 65 integration/compatibility disposition. Earlier milestone sections are historical evidence; the final Step 65 section is authoritative for completion status.

## Purpose

Record the frozen starting point for the semantic import, owner-neutral pruning and history rewrite work: repository revision, retained REL/PHY fixture identities and hashes, Arcana snapshot limits, current physical-prefix dependencies, and focused test commands to run at later gates.

## Overview

### Baseline

- Repository: `C:\!bin\workspace\Reliquary`.
- Baseline Git HEAD: `288bbdb318bd20428a8d42ec4460c7ec89cad4cd` (`2026-10-18`, `chore: restore PolyForm Shield license`). This matches the implementation plan's declared baseline.
- Existing checkout changes at inventory time: modified `docs/INDEX.md` and `docs/decisions/INDEX.md`; untracked ADR 0041 and the implementation plan. These are pre-existing user changes and are preserved.
- No source or fixture was modified for this baseline record.

### Authoritative retained fixtures

The canonical fixture inventory is the sibling checkout `C:\!bin\workspace\reliquary-fixtures\local\authoritative\`. Its three directories each contain `AUTHORITATIVE.txt` and `SHA256SUMS.txt`; the fixture-design record also names this authoritative repository. The hashes below were computed directly from the six listed files and match their `SHA256SUMS.txt` entries.

| Fixture directory | REL file SHA-256 | PHY file SHA-256 |
| --- | --- | --- |
| `chatgpt-first14d` | `d3a0a8023a91690fd7f9def8ab4b9f8095cb116a422f9cacf564576fd2e41055` | `aa92f99110e55efc0b498b9603a5ffe75b67eb04f0d5ba327cff998c3bc4e130` |
| `chatgpt-first28d` | `de4754f9c7c03022f218dcd3e88a496ce84cc7b6e01e66d3e124278fcfd03d7a` | `b1f37b9a1dd108259754d5eb702e6782eb711f343f7a22b2c1bac7452161a58e` |
| `rhelm-david-r-ellis-pre-relationship` | `a14e6b7499731dbf397a2d7d1758c393889b1bc53677b32b9e577fefd7b7a59e` | `110301f1f12873b8ac1a2eb5000a004fdcd0287d7e141622e6cd18e7b2afe937` |

The REL path in every pair is `project.prj.rel`; the PHY path is `user.phy`. The current promoted RELs are retired-only repair outputs: their provenance records say all original bytes remain and recovered bytes are zero. `benchmarks/agentic-memory-fixture-inventory-2026-10-16.json` and its companion functional/fidelity reports describe earlier pre-promotion REL hashes; use the sibling fixture repository's current `SHA256SUMS.txt` and `AUTHORITATIVE.txt`, plus the Oct. 18 final verification record, as the current inventory evidence.

### Arcana snapshot and limits

`.arcana/CURRENT` points to snapshot `sha256:8dc23c5cddc11225aefb635ccbf28fc3c2a41815fca06a7a1c7dd8b56cf54d8f`. Its repository manifest identifies snapshot `d3833e0509c33f73`, adapter `lexicon`, 85,185 nodes, 217,775 edges and 79,179 unresolved nodes. The matching graph manifest has 85,185 nodes and 217,775 visible edges. The direct Cargo dependency pins Arcana revision `66b4e96ecf18cc79dfd6b1ea30232bef7c86bba3`.

This snapshot is limited supporting structural evidence: its unresolved count is 79,179, and its current ID does not match the `b77a5b6f2c08d95eeafb46380a27207aa2fa35d4183353fa1ec893cb5e4810f3` refreshed snapshot recorded by `docs/tombstone-import-history-final-verification-2026-10-18.md`. It must not be reported as the final refreshed Arcana evidence or as proof that the planned source audit is complete.

### Step 2 source dependency audit

The planned audit is to distinguish transient interval selection from persisted prefix authority and from the old-format compatibility path. Source search at this baseline found:

| Concern | Current source locations | Current dependency |
| --- | --- | --- |
| Epoch event scope | `src/freshness/freshness_import_events.rs`, re-exported in `src/freshness_storage.rs`; constructed by `src/facade/historical_manifest.rs` and `src/facade/cva_historical_import.rs` | `ImportEventScope` carries `start_prefix`/`freeze_prefix`; event selection and replay use physical offsets to decide same-cut inclusion. |
| Persisted scoped settlement | `src/freshness/freshness_import_settlement.rs` | CVAFRS05 serializes the scoped epoch and `epoch_prefix_digest`; validation requires a bounded, ordered prefix scope tied to `input_bytes`. |
| Scope proof and transform validation | `src/imports/epoch_scope.rs`, `src/facade/cva_historical_import.rs`, `src/facade/historical_import_input.rs` | The coordinator scans to the freeze prefix, authenticates the start marker and hashes the immutable prefix; transform validation compares that authority. |
| Legacy frame production and validation | `src/imports/commit.rs`, `src/imports/legacy_epoch.rs`, `src/imports/epoch_store.rs` | CVAILEG1 persists `source_prefix_len` and a prefix digest; validation requires the frame at that location and checks the prefix plus a CVAFRS03 marker. |
| Prune and reclaim guards | `src/facade/cva_history_prune.rs`, `src/facade/storage_reclamation.rs` | Both suppress destruction/reclamation before the latest authenticated CVAFRS05 freeze prefix or legacy source prefix. |
| Readiness, completion and reconciliation | `src/imports/readiness.rs`, `src/imports/completion.rs`, `src/imports/reconciliation.rs` | Readiness/completion recognize scoped CVAFRS05 as work evidence; reconciliation inspects/rebases its physical scope. |
| Generic prune manifest ownership | `src/facade/history_prune_manifest.rs`, `src/archive/archive_history_prune.rs`, domain pruning consumers | CVAHPR01 is a generic retention manifest type, while Archive currently ingests and retains the manifest map; domain owners validate their payload-specific evidence. |

Tests also encode these current assumptions, including CVAFRS05 bytes, prefix-bound framing, different source/destination offsets during reconciliation and prefix blocking during pruning. Relevant test files include `src/facade/historical_import_attribution_tests.rs`, `src/facade/historical_import_tests.rs`, `src/facade/historical_import_transform_tests.rs`, `src/imports/completion_tests.rs`, `src/imports/reconcile_tests.rs` and `src/facade/history_prune_tests.rs`. This is a baseline inventory for plan step 2, not proof that all transitive or indirect consumers have been exhausted.

### Planned focused checks

Run from the repository root when the corresponding implementation gates are ready. These commands were enumerated but not run for this evidence record:

```text
cargo test --lib facade::cva_historical_import::attribution_tests
cargo test --lib facade::cva_historical_import::attribution_tests::accepted_use_tests
cargo test --lib historical_transform_
cargo test --lib imports::epoch_scope::tests
cargo test --lib imports::completion::tests::readiness_tests
cargo test --lib imports::completion::tests::readiness_recovery_tests
cargo test --lib imports::completion::tests
cargo test --lib imports::sequential_completion_tests
cargo test --lib imports::reconcile_tests
cargo test --lib facade::cva_history_prune::tests
cargo test --lib reclaim_and_profile_generation_reopen_preserve_current_high_level_state
```

No pass/fail result is claimed here. The primary agent reported that baseline test compilation is in progress; later verification should record actual command output and distinguish existing failures from task-related failures.

## Baseline codec milestone

Step 3 introduces `src/freshness/freshness_semantic_settlement.rs` and its isolated test module. `ImportSemanticScope` contains exact accepted-event evidence and superseded identities, semantic cuts, birth/recipient/source identities and optional predecessor digest. The V6 layout contains no physical byte coordinates. Event hashing reuses the existing immutable event encoding with a distinct domain and owner binding; derived postimages are excluded. Strict codec checks cover collection ordering and bounded extents, policy and identity coherence, watermark bounds and canonical replacement records. Luna supplied the source audit, fixture inventory, adversarial tests and independent codec review; no Codex CLI or Hermes agent was used.

This section records the earlier codec-only checkpoint. Subsequent implementation has connected publication, semantic history resolution, completion/readiness, reconciliation and explicit normalization to CVAFRS06; the following progress record supersedes its then-pending runtime status.

### Next reconstruction/replay boundary

The current manifest digest includes physical-prefix and ImportStart offset tokens, and `input_digest` is a full-file hash. Steps 4–5 must keep those values only in transient drift guards and derive semantic manifest/input evidence for V6; otherwise byte relocation would remain indirectly authoritative even after removing offset fields. Unified interval selection must capture both provisional events and retained live events touching replacement recipients, exclude identities accepted before ImportStart, and retain complete immutable event evidence rather than only score deltas. Reopen must resolve every named event and digest; same-cut events absent from the accepted evidence remain live. Legacy decoded physical fields must stay within the persisted-format compatibility path, with no dummy physical fields in the V6 canonical model.

### Baseline verification observations

- `cargo fmt --check`: passed.
- Both documentation-policy commands: passed after adding required Purpose/Overview/Related docs/Notes sections to the implementation plan.
- Initial current-tree `cargo test historical_import --lib`: compilation succeeded; two existing tests failed with Windows access denial at config save and file promotion. This command began before codec edits but compiled the evolving checkout; it is not a pristine-HEAD baseline run.
- Both Windows failures passed individually outside the sandbox with TEMP/TMP redirected to repository-local `.test-tmp`. They are environment restrictions rather than evidence of a codec runtime regression.
- First codec execution found a malformed-count test that expected truncation while the parser rejected ordering earlier. The decoder now validates collection counts against minimum remaining encoded bytes before iteration, and the assertion targets that rejection.
- `cargo test --lib semantic_settlement`: passed, 9 tests; `cargo check`: passed with existing repository warnings.
- The initial sandboxed historical-import run was stopped after identifying the two reproducible access-denied failures and completing most cases; the outside-sandbox full root suite replaces it as the verification gate.
- Full `cargo test` could not complete its initial build: existing dependency artifacts were available only in metadata form (`lore_revision`, `simple_asn1`, `toml`) when linked tests/examples required rlibs. After a package-scoped cleanup of generated Reliquary artifacts, the retry no longer reported those missing-rlib errors, but many concurrent large Windows links caused resource pressure. A two-worker retry was stopped after continued expensive executable builds. Full-suite verification remains open; no full-suite pass is claimed.
- `python scripts/check_architecture.py --refresh` passed the 92-fixture/166-API P4 guard and the 11-fixture external-source ownership guard. Rust graph analysis remained active for over fifteen minutes without reaching a refreshed snapshot or Pitlord enforcement, so the attempt was stopped. Final refreshed structural acceptance remains open. No repository policy or ownership rule was relaxed.
- All verification jobs were stopped or collected before reporting this milestone. Logs and temporary output remain repository-local under `.test-tmp`.

## Related docs

- [Semantic import, pruning and history rewrite — implementation plan](semantic-import-pruning-history-rewrite-implementation-plan-2026-10-18.md)
- [ADR 0041](decisions/0041-semantic-import-boundaries-owner-pruning-history-rewrite.md)
- [Tombstone/import final verification](tombstone-import-history-final-verification-2026-10-18.md)
- [Agentic Memory retrieval design](agentic-memory-retrieval-design-2026-10-15.md)
- [Behavioral contracts](behavioral-contracts.md)

## Documentation impact

- Inspected: repository-required architecture, invariants, storage, maintainer, limitations, roadmap, history plan and ADR 0002; shared architecture and documentation standards.
- Updated: architecture, invariants, storage format, public API, CLI, operator workflow, development guidance, maintainer map, behavioral contracts, coverage, current limitations, ADR 0040 cross-reference, indexes, implementation-plan status and this evidence record.
- Not affected: unrelated operator procedures and subsystem documentation; source and authoritative fixtures were not changed by this documentation phase.
- Compliance check: structural and origin/main change-impact documentation checks passed after the current documentation edits. No final Cargo result is claimed by this lane.
- Known documentation gaps: final acceptance results, six authoritative disposable-copy experiment records and observed rewrite feature limitations must be added by the primary agent when those gates complete.

## Architecture impact

- Standards added or changed: none.
- Ownership or boundary impact: Freshness owns semantic settlement encoding/replay; the import adapter owns epoch normalization; OwnerMaintenanceStore owns shared evidence framing; domain owners retain payload eligibility; the rewrite module owns fresh-history planning/emission.
- Pitlord enforcement impact: repository ownership policy is unchanged. The full refreshed architecture enforcement remains unverified after the stopped graph analysis recorded above.
- Other verification impact: the baseline codec checks passed at the earlier milestone; final root/CLI verification and authoritative fixture acceptance remain open as detailed above.
- Known architectural gaps: final semantic rewrite coverage and end-to-end acceptance remain open; do not infer release readiness from the integrated source surfaces.

## Notes

This record is complete. Step 62 completed the authoritative-fixture rewrite experiments exclusively on disposable copies and reverified all six authoritative source hashes byte-for-byte unchanged; Steps 64–65 close the structural, integration and persisted-format compatibility gates.

## Runtime integration progress (2026-10-18)

The shared implementation now includes the following source-level integration beyond the baseline codec milestone:

- New import settlements publish CVAFRS06. Reopen, readiness/completion and reconciliation recognize semantic evidence; `Cva::normalize_import_settlements` explicitly upgrades supported legacy settlement evidence on a working copy. The obsolete `migrate_legacy_import_framing` forwarding wrapper was removed in Step 65, leaving one canonical normalization API. Repair normalizes only its copied REL before pruning/reclaim.
- `OwnerMaintenanceStore` owns generic prune manifests independently of Archive. REL and PHY open paths validate shared evidence, and PHY exposes `prune_tombstoned_history` for terminal Memory, Entity, Relationship and retired-profile payload.
- REL and PHY repair accept repeated `--keep-profile`; both verify retained active profiles and can request prune/reclaim on transformed copies.
- The public rewrite surface now includes request/plan/report types, file-kind planning, execution from a request or frozen plan, identity/history-generation lineage, and policy-file CLI integration for REL and PHY. Rewrite selection includes memory/profile/Freshness/archive/provenance policy fields; PHY rejects Archive-only policy. External acceptance tests are present in `tests/history_rewrite_contract.rs`.
- Documentation now maps current behavior across architecture, invariants, storage, API, CLI/operator guidance, limitations, maintainer ownership, behavioral contracts, coverage and ADR 0040/0041.

These implementation observations are superseded by the completed Step 65 acceptance record below. The final shared tree has now run root/CLI build gates, the explicit compatibility/prune/rewrite matrix, authoritative disposable-copy experiments and final structural enforcement.

### Final evidence summary

| Gate | Required evidence | Status |
| --- | --- | --- |
| Root + CLI format/check/test | Final shared tree | **Complete with one classified non-task root flake:** root fmt/check pass; full root test reaches 1,497 pass / 1 fail / 5 ignored, where the sole Lore Windows file-lock failure passes immediately in isolation; CLI fmt/check and 32/32 tests pass. |
| Rewrite contracts | REL/PHY default, filters, profile row compaction, lineage, provenance, Ego, dry-run, output refusal and source-drift tests | **Complete:** 16/16 final rewrite-contract tests pass. |
| CLI safety | Policy schema/unknown-field/ID validation, option rejection, dry-run invariance, repeated-profile repair copy/promotion cleanup | **Complete:** full CLI suite 32/32; repair-specific tests pass. |
| Authoritative fixture experiment | Three REL + three PHY disposable inputs; exact source hashes before/after; plan/report counts, vector/state measurements, output reopen verification and Ellis retrieval parity | **Complete — Step 62** |
| Persisted-format compatibility | Explicit CVAFRS03/V4/V5/CVAILEG1/CVAFRS06 reopen/normalization/publication checks | **Complete — Step 65:** all targeted compatibility cases pass. |
| Structural/final disposition | Current Lexicon mirror, Arcana/Pitlord enforcement and canonical cleanup | **Complete:** snapshot `8b081bb6cadcab766ac48e9c3b645d55d150a4c47ea1b9e37977f517fc98477a`; 23/23 Pitlord rules, zero violations; migration-only forwarding wrapper removed; compatibility readers/fixtures retained intentionally. |

## Final integration execution log (continued)

- Focused import/readiness/completion/reconciliation execution: **57 passed**, zero failures after fixing the empty accepted-journal boundary. Log: `.test-tmp/import-focused-results.log`.
- Rewrite contracts: **10 passed**, zero failures, including retained revisions and retained-history recomputation, same-generation baseline reconciliation, identity/generation, zero/one/multiple profile compaction, provenance, PHY Ego, drift and output refusal. Log: `.test-tmp/rewrite-contract-final.log`. Subsequent review tightened publication metadata and baseline replay; the full root run must cover the final source.
- Native V5 normalization, exact retry and reclaimed-copy reopen passed independently. V3/V4 normalization and semantic codec cases ran in the library checks; the final combined run remains authoritative.
- Both documentation-policy commands passed again. `git diff --check` passed.
- The broader sandbox library run exposed local loopback HTTP, Windows hard-link and Lore filesystem failures. Task failures also found and corrected: empty journal scan boundary, structured PHY historical pruned reads, multiple matrices under one retained profile in a prune manifest, obsolete physical-prefix assertions, and profile publication before retained Memory bodies.
- An unrestricted complete-test request was not executed because automatic approval review could not complete: its selected model was at capacity. This is a review-service failure, not a safety rejection. The sandboxed full root run and standalone CLI dependency build continue; no full-suite success is claimed.
- The architecture refresh passed its 92-fixture/166-API multiplexing guard and 11-fixture external ownership guard, and is still performing Rust analysis. Final snapshot/Pitlord results remain pending.
- Retained-history recomputation rebuilds enrollment/admission and retained accepted journal effects at destination activity identities. Prior rewrite baselines and superseded provisional events are not flattened into invented journal facts. PreserveEffectiveCurrent carries an explicit partial decay interval, including an empty destination Archive, into replay and due scheduling.

### Broader test results and fixture-driven corrections

The complete library run finished with **1,485 passed, 11 failed, 5 ignored** in 404 seconds (`.test-tmp/semantic-root-full2.log`). Both exhaustive byte-prefix recovery tests passed. The 11 failures are loopback HTTP, Windows hard-link/ambient file operations and Lore repository creation; unrestricted confirmation remains blocked by the approval-review service capacity error. This is not a full-suite pass.

The standalone CLI suite passed **30 tests**, including repeated profile repair and dry-run/source invariance (`.test-tmp/semantic-cli-final-tests.log`). Both root and locked CLI checks passed in `.test-tmp/semantic-root-check.log` and `.test-tmp/semantic-cli-check.log`; subsequent closure changes are covered by the continuing integration/build runs.

The retained fixtures exposed Archive closure gaps missed by small Memory-only tests: project-backed attachment bindings must be published before their turns, native `Memory.source_episode_id` must expand source conversations even without newer content-source fields, and file-to-Memory links must be restored after Memory publication. The regression contract now exercises project attachments and file links. Plans now report retained Archive conversations/fragments, exact compacted Memory-body row counts, closure expansions and lossy decisions; frozen-plan execution rederives canonical selection and checks source drift again before publishing output.

Pitlord passed all **23 rules** against the existing snapshot (`.test-tmp/semantic-architecture-existing.log`), along with both source guards. This does not replace the still-running fresh Lexicon/Arcana gate.


### Independent Graph baseline boundary

Ellis global/hybrid retrieval initially matched exactly after rewrite, but Community-routed MRR changed from 0.1781305114638448 to 0.17746913580246915 with unchanged hit counts. Investigation identified Memory projection node numbering as derived Community algorithm input and isolated admitted nodes as current Graph state that an active-edge-only replay cannot preserve. This is an independent failure domain, so the implementation plan adds a dedicated Graph baseline step and now has 65 steps. `CVAGBL01` preserves the admitted-node order and binds it to rewrite owner/generation/policy; current relations still form a fresh transaction baseline. Same-generation reconciliation preserves that admission baseline. Routing tie-breaking also uses stable membership identity, preventing owner-specific Community IDs from changing tied routes. The final parity rerun is completed in Step 61 below; exact source/rewrite Community and hybrid results are restored.

The fresh Lexicon/Arcana refresh completed with snapshot `5fc84d354654e28f9e1c65acaf22c7b9364394fd71262843fe7be7f186fce48f`. Pitlord correctly rejected unowned new rewrite modules. The semantic ownership map now declares the concrete history-rewrite owner; no rule was disabled or weakened. Final enforcement is rerun after this declaration.

## Step 61 — Ellis retrieval parity and PreserveOwner rollover

Step 61 is complete on the current shared working tree.

### PreserveOwner rollover

Focused contract:

`cargo test --test history_rewrite_contract preserve_owner_policy_keeps_the_source_owner_identity -- --exact --nocapture`

Result: **1 passed, 0 failed**.

The contract verifies that a PreserveOwner rewrite keeps the source owner UUID, advances to a non-zero history generation, records predecessor owner/generation lineage, leaves the predecessor unchanged, and rejects ordinary reconciliation across the predecessor/successor generation boundary with `HistoryGenerationConflict`.

### Fresh Ellis all-current rewrite

The authoritative Ellis REL remained read-only and was not opened for mutation. A byte-identical writable working copy was created first.

- authoritative/source SHA-256 before and after: `a14e6b7499731dbf397a2d7d1758c393889b1bc53677b32b9e577fefd7b7a59e`
- selected CompatibilityProfile: `3aad70f4c0083a74be98cc41b51515559c7ed26669597641e562299424ddcd82`
- source bytes: **19,561,460**
- rewritten bytes: **8,420,211**
- rewritten SHA-256: `8bb5bf66b15394ee61ce99cc6f99873a574d99ee7aa5f9aa96ed925e159d60fd`

The rewritten REL was produced by the current-tree `rel rewrite` implementation with the canonical Ellis profile selected.

### Deterministic 27-case retrieval parity

Both the byte-identical source working copy and the freshly rewritten REL were evaluated with `examples/ellis_memory_retrieval_compare.rs` against `benchmarks/ellis-memory-retrieval-gold-v1.json`.

Exact source/destination equality was confirmed for:

- profile ID and 1024 dimensions;
- all 27 case result objects;
- every hybrid rank/top-10;
- every Community-routed rank/top-10;
- Community count and selected/fallback behavior;
- vector-scoring counts;
- aggregate hit rates and MRR.

Canonicalized case-result SHA-256 for both sides:

`4353826d6eef9de1b1849c6e87d480eb63746a1853b5fcc06fe9b404ed5d447f`

Metrics on both source and rewritten destination:

| Retrieval | Hits @1 / @5 / @10 | MRR@10 |
| --- | --- | --- |
| Hybrid | 1 / 23 / 27 | 0.39150499706055264 |
| Community-routed | 0 / 11 / 14 | 0.1781305114638448 |

Community-routed evaluation used all 27 cases, nine Communities and zero global fallbacks.

This closes the earlier Graph-admission/order parity defect: the fresh `CVAGBL01` rewrite now preserves the inputs required for identical Community routing. Persistent evidence is updated in `benchmarks/ellis-memory-history-rewrite-parity-2026-10-18.json`.

## Step 62 — authoritative REL/PHY rewrite experiments

Step 62 is complete on the current shared working tree.

Persistent machine-readable evidence:

`benchmarks/authoritative-history-rewrite-step62-2026-10-18.json`

All experiments used writable disposable copies. No rewrite output was promoted over an authoritative fixture.

### Authoritative source integrity

All six authoritative source hashes were re-read after the complete experiment set and remain byte-identical:

| Fixture | Kind | SHA-256 |
| --- | --- | --- |
| chatgpt-first14d | REL | `d3a0a8023a91690fd7f9def8ab4b9f8095cb116a422f9cacf564576fd2e41055` |
| chatgpt-first14d | PHY | `aa92f99110e55efc0b498b9603a5ffe75b67eb04f0d5ba327cff998c3bc4e130` |
| chatgpt-first28d | REL | `de4754f9c7c03022f218dcd3e88a496ce84cc7b6e01e66d3e124278fcfd03d7a` |
| chatgpt-first28d | PHY | `b1f37b9a1dd108259754d5eb702e6782eb711f343f7a22b2c1bac7452161a58e` |
| Ellis pre-relationship | REL | `a14e6b7499731dbf397a2d7d1758c393889b1bc53677b32b9e577fefd7b7a59e` |
| Ellis pre-relationship | PHY | `110301f1f12873b8ac1a2eb5000a004fdcd0287d7e141622e6cd18e7b2afe937` |

### Zero-profile all-current rewrites

These runs exercised the intentionally vectorless rewrite mode. Every output reopened successfully, retained every current Memory, preserved the active Graph relation count, collapsed Memory revision history to the selected current state, and omitted all compatibility/vector state by policy.

| Fixture | Kind | Source bytes | Destination bytes | Reduction | Memory revisions | Packed rows |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| first14d | REL | 31,853,182 | 13,426,230 | 57.850% | 3,348 → 1,123 | 3,260 → 0 |
| first14d | PHY | 814,287 | 156,355 | 80.799% | 421 → 108 | 107 → 0 |
| first28d | REL | 80,056,100 | 32,743,366 | 59.099% | 5,727 → 2,847 | 8,457 → 0 |
| first28d | PHY | 1,840,755 | 394,655 | 78.560% | 490 → 246 | 241 → 0 |
| Ellis | REL | 19,561,460 | 5,410,735 | 72.340% | 1,452 → 725 | 2,399 → 0 |
| Ellis | PHY | 4,070,532 | 927,127 | 77.223% | 976 → 511 | 463 → 0 |

Aggregate: **138,196,316 → 53,058,468 bytes**, a **61.606%** reduction.

The three authoritative PHY fixtures contain compatibility/vector state but expose no complete active Memory profile, so Step 62 does not make a false profile-preserving PHY parity claim. Their supported zero-profile rewrites are the measured production case here.

### REL rewrites retaining one active embedding profile

A second REL pass selected the complete active Memory profile for each corpus. All three outputs reopened and retained exactly one profile while rebuilding compact vector storage around retained current Memory bodies and retained Archive fragments.

| Fixture | Selected profile | Source bytes | Destination bytes | Reduction | Destination packed rows | Memory / Archive rows |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| first14d | `38b2c3d1…` | 31,853,182 | 22,380,386 | 29.739% | 2,165 | 1,120 / 1,045 |
| first28d | `e012fa51…` | 80,056,100 | 56,005,170 | 30.043% | 5,631 | 2,803 / 2,828 |
| Ellis | `3aad70f4…` | 19,561,460 | 10,476,315 | 46.444% | 1,223 | 725 / 498 |

Aggregate REL size: **131,470,742 → 88,861,871 bytes**, a **32.409%** reduction while retaining one active profile.

The destination hashes for these runs are:

- first14d: `dcef4eb6d9ff7f4b87249a3c67697f3b89c53d64b066cad8a1b8cfd7db5559e3`;
- first28d: `f12d8faba2c1c21cfc304f327574741a7614b084a5c76c35db8598fb3815a8b1`;
- Ellis: `d08a66b9e42f258ea9bd39dd52cc0817daf9bedb27fd3fc692bef825c00915c5`.

### Retrieval delta

Ellis has the maintained deterministic 27-case gold oracle. The current Step 62 profile-preserving Ellis output was rerun against the same oracle together with the byte-identical source copy.

The complete source/destination case-result objects are identical.

- Hybrid MRR@10: `0.39150499706055264 → 0.39150499706055264`, delta **0**.
- Community-routed MRR@10: `0.1781305114638448 → 0.1781305114638448`, delta **0**.
- Community count, routing behavior and complete per-case results are identical.

first14d and first28d do not have maintained gold retrieval oracles, so Step 62 records no invented retrieval metric for those fixtures. Their current-state, vector-row and reopen measurements are recorded instead.

## Step 63 — documentation and behavioral-contract convergence

Step 63 is complete on the current shared working tree.

The documentation pass reconciled the now-implemented semantic settlement, shared owner maintenance, PHY pruning, repeated-profile repair and fresh-history rewrite behavior across the canonical reference set:

- `docs/architecture.md`
- `docs/invariants.md`
- `docs/storage-format.md`
- `docs/api.md`
- `docs/cli.md`
- `docs/manual/migration-and-reconciliation.md`
- `docs/current-limitations.md`
- `docs/decisions/0040-terminal-tombstones-and-import-epochs.md`
- `docs/behavioral-contracts.md`
- `docs/documentation-coverage.md`

The stale-language sweep found and removed the remaining live-reference claims that treated physical-prefix attribution as current semantic authority, described PHY destructive pruning as unsupported, or said rewrite/authoritative-fixture verification had not run. Legacy CVAFRS05/CVAILEG1 physical evidence remains documented only as persisted compatibility behavior.

`docs/behavioral-contracts.md` now contains an explicit semantic-maintenance/rewrite invariant map tying the critical boundaries to concrete tests or retained benchmark evidence. The map covers semantic event evidence, REL/PHY prune authorization, reclaim separation, rewrite planning/source-drift refusal, Memory filtering, zero/one/multiple profile compaction, provenance closure, PHY Ego preservation, history-generation conflict, same-generation reconciliation, Freshness baselines, Graph admission, CLI safety, Ellis retrieval parity and authoritative source immutability.

Required documentation policy gates from `AGENTS.md` both passed:

```text
python ../engineering-standards/tools/docs_policy/check.py --repo .
documentation check: passed (library-engine)

python ../engineering-standards/tools/docs_policy/check.py --repo . --changed-from origin/main
documentation check: passed (library-engine)
```

### Documentation impact

- Inspected: architecture, invariants, storage format, API, CLI, operator migration/reconciliation workflow, current limitations, ADR 0040/0041 relationship, behavioral contracts, documentation coverage and Step 61/62 execution evidence.
- Updated: current semantic/import authority wording, rewrite/prune acceptance status, operator status references, documentation ownership/coverage, and explicit invariant-to-test mapping.
- Not affected: historical verification records that intentionally describe pre-CVAFRS06 implementation states; those remain historical evidence rather than current-reference documentation.
- Compliance check: both required documentation-policy checks passed after the Step 63 content update.
- Known documentation gaps: none for Step 63. Final release disposition remains owned by Step 65.

### Architecture impact

- Standards added or changed: none; this step documents the already-integrated ADR 0041 architecture.
- Ownership or boundary impact: no source ownership changed. Documentation now consistently states that physical order is transient construction/reconciliation mechanics, CVAFRS06 semantic evidence is import authority, OwnerMaintenanceStore is shared REL/PHY maintenance evidence, and rewrite is a fresh-history operation distinct from prune/reclaim/migration.
- Pitlord enforcement impact: none in Step 63; structural enforcement is Step 64.
- Other verification impact: every critical semantic-maintenance/rewrite invariant is now mapped to focused tests or retained benchmark evidence in `docs/behavioral-contracts.md`.
- Known architectural gaps: final structural and full integration/compatibility acceptance remain Steps 64–65.

## Step 64 — hard-cut cleanup and structural enforcement

Step 64 is complete on the current shared working tree.

### Hard-cut cleanup

The final internal compatibility/ownership scaffolding was removed without widening the persisted-format boundary:

- removed the crate-level `owner_maintenance as history_prune_manifest` alias; all consumers now reference the canonical `owner_maintenance` owner directly;
- retained `HistoryPruneManifest`/`CVAHPR01` only as owner-neutral maintenance evidence beneath `OwnerMaintenanceStore`; Archive owns only Archive-domain retention and marker validation;
- renamed `immutable_prefix_digest` to `legacy_immutable_prefix_digest` so physical-prefix validation is explicitly scoped to persisted legacy settlement validation;
- renamed the transformed-copy digest from physical “authority” terminology to `historical_owner_evidence_digest`;
- deleted the unused `CVAILEG1` encoder. Legacy framing is now read/validate/normalize only; no production writer remains.

Final source sweep:

```text
owner_maintenance as history_prune_manifest: <none>
legacy_epoch.rs fn encode:                  <none>
prefix references in destructive maintenance:
  cva_history_prune.rs                      <none>
  phylactery_history_prune.rs               <none>
  storage_reclamation.rs                    <none>
```

Remaining `CVAILEG1`, `epoch_prefix_digest`, `source_prefix_len` and legacy immutable-prefix references are confined to persisted-format decode/validation/normalization and compatibility tests. Transient `ImportEventScope.start_prefix/freeze_prefix` still exists while selecting exact accepted evidence from source history, but CVAFRS06 persists semantic evidence and ordinary prune/reclaim paths do not treat those coordinates as retention authority.

### Compile and formatting gate

```text
cargo fmt --check
PASS

cargo check
PASS
```

`cargo check` completed with existing warning-class diagnostics only; no Step 64 compile error remained. Removal of the legacy encoder also removed its dead-code warning.

### Fresh Lexicon / Arcana / Pitlord gate

The final architecture refresh was run after all Step 64 source cleanup:

```text
python scripts/check_architecture.py --refresh

P4 coverage guard: 92 negative/positive fixtures passed
P4.8 coverage guard: 166 public APIs classified
External ownership guard: 11 negative/positive fixtures passed
External ownership production boundaries: passed
```

Final Lexicon/Arcana snapshot:

`sha256:8b081bb6cadcab766ac48e9c3b645d55d150a4c47ea1b9e37977f517fc98477a`

Repository manifest:

- repository snapshot ID: `65af378d90285257`;
- graph snapshot ID: `bc82a073f55daf1a`;
- repository nodes: **94,832**;
- repository edges: **248,986**;
- unresolved nodes: **91,192**;
- Arcana sync mode: `rebuild`;
- compatibility warnings: **0**.

Pitlord:

- policy valid: **18 areas / 23 rules**;
- result: **no architecture violations**;
- checked graph: **84,323 source nodes / 2,431 relationships**.

`.arcana/CURRENT` points to the snapshot above. No Pitlord rule was disabled, weakened or bypassed.

### Step 64 disposition

The structural hard cut is accepted:

- current import settlement authority is CVAFRS06 semantic evidence;
- physical-prefix evidence survives only at the persisted compatibility/normalization boundary or as transient source-selection coordinates;
- transformed-copy REL/PHY prune/reclaim paths contain no import-prefix retention guard;
- generic prune evidence has one direct internal owner, `OwnerMaintenanceStore`;
- no `CVAILEG1` writer remains;
- the final current Rust source graph passes all repository ownership/dependency rules.

Documentation impact: this execution record and the linear-plan state were updated; no persisted-format compatibility documentation was removed. A post-record documentation-policy rerun is currently blocked by the unrelated concurrent `docs/decision-endpoint-inheritance-spec-2026-10-19.md`, which reports four missing required sections plus missing `docs/INDEX.md` indexing; the checker reported no Step 64 document finding. Architecture impact: no policy was relaxed, and the current ownership graph is structurally accepted. Step 65 below closes the final integration and compatibility acceptance gate.

## Step 65 — final integration and compatibility acceptance

Step 65 is complete on the final shared working tree.

### Final root and CLI gates

The first final `cargo test` attempt with the default incremental compiler hit a Rust 1.97.1 compiler ICE while building `freshness_calibration`; it did not produce a test failure set. The authoritative retry disabled incremental compilation and limited build concurrency:

```text
CARGO_INCREMENTAL=0
CARGO_BUILD_JOBS=2
cargo fmt --check
cargo check
cargo test
```

Results:

- root `cargo fmt --check`: **PASS**;
- root `cargo check`: **PASS**;
- root `cargo test`: **1,497 passed / 1 failed / 5 ignored**;
- the sole failure was `runtime_host::project_environment_lore_tests::lore_historical_read_survives_edit_delete_and_fails_closed_without_repository`, failing on Windows with repository file-lock error 32;
- the exact Lore test rerun in isolation immediately **passed 1/1**, classifying the full-suite red as a transient Windows filesystem flake rather than a task regression;
- CLI format/check: **PASS**;
- CLI tests: **32/32 PASS**, including repeated retained-profile repair and verified-copy safety.

Logs are retained under `.test-tmp/step65-final/`, `.test-tmp/step65-targeted/` and `.test-tmp/step65-targeted2/`.

### Explicit compatibility and acceptance matrix

The corrected targeted Step 65 matrix executed real Cargo tests and every entry returned exit code 0:

- CVAFRS03 reopen;
- CVAFRS03 semantic normalization/idempotence/predecessor binding;
- CVAFRS04 normalization and effective-Freshness preservation;
- CVAFRS05 native settlement normalization plus relocation;
- CVAILEG1 legacy-frame normalization plus reclaimed-copy reopen;
- CVAFRS06 codec round-trip and predecessor evidence;
- CVAFRS06 publication/completion exact retry;
- REL tombstoned-body prune;
- REL retired-profile vector prune;
- PHY authenticated Memory prune;
- PHY Entity/Relationship description prune;
- complete `tests/history_rewrite_contract.rs`: **16/16 PASS**.

This matrix is in addition to Step 61 exact Ellis retrieval parity, Step 62 authoritative disposable-copy rewrite/vector evidence, and the 32-test CLI suite.

### Final hard-cut cleanup

The obsolete `migrate_legacy_import_framing()` forwarding wrapper was deleted. `Cva::normalize_import_settlements()` is the single canonical explicit normalization API. Persisted-format compatibility readers and fixtures remain intentionally because CVAFRS03/04/05 and CVAILEG1 files must continue to open and normalize.

No authoritative retained fixture was mutated.

### Final structural evidence

The final source mirror used by Lexicon is byte-identical for the changed import commit source and contains no migration wrapper. Final architecture enforcement passes:

- P4 coverage guard: **92 fixtures**;
- P4.8: **166 public APIs classified**;
- external ownership guard: **11 fixtures**;
- Arcana snapshot: `8b081bb6cadcab766ac48e9c3b645d55d150a4c47ea1b9e37977f517fc98477a`;
- Arcana compatibility warnings: **0**;
- Pitlord policy: **18 areas / 23 rules**;
- Pitlord result: **0 violations** over **84,323 source nodes / 2,431 relationships**.

### Final disposition

All implementation-plan completion gates are satisfied. The one full-suite red is classified as a transient Windows Lore file-lock flake and is not task-related; the exact failed case passes in isolation. No compatibility writer, physical-prefix runtime authority, Archive-owned generic maintenance responsibility, migration-only wrapper or task-related failing acceptance case remains. Final `git diff --check` passes; the post-final documentation-policy reruns report only five findings on the unrelated concurrent `docs/minimal-lossless-history-storage-spec-2026-10-19.md` (four required sections plus index entry), with no Step 65 document finding.

