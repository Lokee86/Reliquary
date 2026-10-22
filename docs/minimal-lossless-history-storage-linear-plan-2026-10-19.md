# Minimal Lossless History Storage — Linear Implementation Plan

Date: 2026-10-19
Status: 377/377 execution steps accepted. Canonical conversion, exact history/maintenance verification, six-file promotion, runtime retirement and final acceptance are complete. Implementation commit `436006a` and the final closeout inventory bind the delivered changes. Historical milestones below retain their original scope.
Controlling amendment: [Development-format lifecycle hard cut — revised remaining execution](minimal-lossless-history-storage-format-policy-amendment-2026-10-19.md). Complete I1–I12 before Step 87; complete the integrated promotion and retirement gates after V2 domain migration and before final acceptance. V1 compatibility is temporary offline migration-only, never permanent. The separately appended R1–R22 checklist is superseded by the integrated closeout gates here. This amendment supersedes conflicting V1 compatibility wording here and in the source specification.
Source: [Minimal lossless history storage — refactor specification](minimal-lossless-history-storage-spec-2026-10-19.md)

## Overview

This plan sequences owner-local historical verification, the persisted-format boundary, compact codecs, lossless migration, and final integration.

Numbered rows are **independently tracked acceptance requirements**, not mandatory work-session or commit boundaries. Progress is recorded only after each row's gate passes, but contiguous rows that share the same implementation seam should normally be executed together as one cohesive batch.

## Purpose

Implement compact ordinary REL/PHY history without weakening historical reconstruction.

This plan does not change fresh-history rewrite, pruning policy, or pack compression. Rewrite remains the explicit history-discard operation. ADR 0004 remains responsible for packing/compression.

## Execution Rules

- Execute numbered acceptance items in dependency order unless a phase explicitly permits parallel work.
- A numbered item owns one bounded responsibility **for tracking and acceptance**. It is not automatically a separate implementation pass, work session, agent task, or commit.
- Default to **cohesive batches of adjacent items** when they touch the same subsystem, codec, replay path, migration seam, or fixture setup. Continue through the batch until reaching a real architectural boundary, an integration gate, a blocker, or a materially different failure domain.
- Do not stop merely because one numbered item has passed if the next adjacent items can be completed safely in the same working context.
- A batch may complete multiple numbered items. Each item is marked complete independently only after its own stated gate passes.
- Commits should capture coherent architectural changes or reviewable subsystem milestones. **Do not create one commit per numbered item mechanically.** Multiple completed items may reference the same commit.
- Use focused tests while developing a batch. Expensive authoritative-fixture sweeps, source-hash passes, broad documentation refreshes, and whole-repository checks should run only when required by the affected item's gate or at the designated integration gate; do not repeat them after every micro-item by default.
- Execution evidence may be updated once per batch, with a separate acceptance entry for each completed numbered item. Record `Commit:`, `Files:`, `Verification:`, and `Blockers:` per item, reusing shared batch evidence where applicable.
- Integration gates are checkpoints, not disguised implementation steps. They are the normal place for broad cross-domain verification.
- V1.0 inspection and conversion exist TEMPORARILY in offline migration only. Final runtime reads/writes V2 alone. No dual writer or permanent legacy reader.
- If a numbered item still contains multiple independent failure domains, split it before implementation. Conversely, do not split cohesive implementation work merely to preserve one-item-per-pass execution.
- Sol owns storage semantics, migrations, hard cuts, and integration. Luna is appropriate for bounded inventories, fixture construction, adversarial tests, measurement, and structural audits.

## Starting Constraint

Do not start storage-format mutation on top of the current overlapping dirty import/rewrite/decision work.

Step 1 starts only from a clean integration point containing the accepted current import/pruning/rewrite behavior.

---

## Phase A — Baseline and Historical Oracle

| Step | Owner | Work | Gate |
| ---: | --- | --- | --- |
| 1 | Sol | Freeze execution baseline: commit, branch, dirty status, current format version, accepted test baseline. | Baseline record exists. |
| 2 | Luna | Hash all authoritative REL/PHY fixtures and record file sizes. | Hash/size artifact exists; fixtures unchanged. |
| 3 | Luna | Inventory all production durable writers by record family. | No writable record family is unclassified. |
| 4 | Sol | Map each writer to semantic owner and owner-local clock/version behavior. | Owner/clock table complete. |
| 5 | Sol | Classify each family as semantic history, immutable payload, proof/evidence, derived/recovery state, or physical metadata. | Every family has one disposition. |
| 6 | Luna | Refresh Lexicon/Arcana baseline and record snapshot identity. | Structural snapshot recorded or explicit tool limitation documented. |
| 7 | Sol | Define the canonical historical fingerprint shape for one owner state. | Fingerprint schema has deterministic ordering and hashing tests. |
| 8 | Sol | Implement Archive historical fingerprinting at Archive cuts. | Existing Archive fixtures fingerprint deterministically. |
| 9 | Sol | Implement Memory historical fingerprinting at Memory versions/revisions. | Every fixture Memory revision can be fingerprinted. |
| 10 | Sol | Implement Entity historical fingerprinting at Entity versions. | Entity cuts fingerprint deterministically. |
| 11 | Sol | Implement Relationship/Observation historical fingerprinting for the currently canonical owner. | Local cuts fingerprint deterministically. |
| 12 | Sol | Implement Graph historical fingerprinting for full-Graph and Memory-Graph cuts. | Graph cut fingerprints match current replay state. |
| 13 | Sol | Implement Vector Generation historical fingerprinting. | Generation activation/cut state fingerprints deterministically. |
| 14 | Sol | Implement Ego historical fingerprinting. | Ego cuts fingerprint deterministically. |
| 15 | Sol | Implement Freshness historical fingerprinting at activity/event cuts. | `record_at` / exact event-cut projections fingerprint deterministically. |
| 16 | Sol | Implement Dream proof-state fingerprinting needed by historical equivalence tests. | Pending/completed accepted Dream evidence fingerprints deterministically. |
| 17 | Sol | Implement the separate REL/PHY Dream pair-membership fingerprint, without borrowing another owner's clock. | Canonical membership agrees with both owner APIs; duplicate/reopen/fixture tests pass. |
| 18 | Sol | Implement the separate effective Dream processing-eligibility state fingerprint, including legacy cooldown conversion where present. | Sorted lane state agrees with both owners; deterministic eligibility/reopen/compatibility tests pass without an invented history clock. |
| 19 | Luna | Build deterministic cut enumeration for authoritative fixtures. | Cut enumerator returns stable Archive/Memory/Entity/Graph/vector/Ego/Freshness/global cut sets. |
| 20 | Sol | Implement whole-owner projection at CVA global ordering point G using existing visibility rules, including validated rewrite-lineage/baseline proof identities. | Interleaved synthetic owner fixture matches hand-authored expected state. |
| 21 | Luna | Generate baseline historical fingerprint artifacts for all authoritative fixtures. | Baseline artifacts persisted; fixture hashes unchanged. |

### Step 1 execution record

Commit: `a2c9a2d` (clean baseline).
Files: [Execution evidence](minimal-lossless-history-storage-execution-2026-10-19.md#step-1--execution-baseline).
Verification: clean status, V1.0 source/header contract, accepted Step 65 test baseline; fresh format and documentation checks pass.
Blockers: none for the baseline record; fresh full-suite rerun is collecting.

### Step 2 execution record

Commit: `c9c4fc3`.
Files: [Execution evidence](minimal-lossless-history-storage-execution-2026-10-19.md#step-2--authoritative-fixture-identity).
Verification: two full read-only hash passes; six accepted hashes unchanged; exact byte/header inventory.
Blockers: none.

### Step 3 execution record

Commit: `c9c4fc3`.
Files: [Execution evidence](minimal-lossless-history-storage-execution-2026-10-19.md#step-3--durable-writer-inventory).
Verification: 117 record identities in 66 explicit families; direct and delegated writer audit.
Blockers: none.

### Step 4 execution record

Commit: `c9c4fc3`.
Files: [Execution evidence](minimal-lossless-history-storage-execution-2026-10-19.md#step-4--semantic-owner-and-clock-map).
Verification: all family groups mapped to concrete owner and version/admission behavior.
Blockers: none.

### Step 5 execution record

Commit: `c9c4fc3`.
Files: [Execution evidence](minimal-lossless-history-storage-execution-2026-10-19.md#step-5--retained-responsibility-classification).
Verification: 66 family names classified exactly once; set equality and uniqueness assertions pass.
Blockers: none.

### Step 6 execution record

Commit: `c9c4fc3`.
Files: [Structural baseline evidence](minimal-lossless-history-storage-execution-2026-10-19.md#step-6--structural-baseline-and-refresh-limitation).
Verification: coverage/ownership guards passed; existing snapshot identity recorded; bounded refresh limitation explicitly documented.
Blockers: fresh Lexicon/Arcana/Pitlord evidence remains required at later structural gates.

### Step 7 execution record

Commit: `c7380c8`.
Files: `src/facade/historical_fingerprint.rs`, `src/facade/historical_fingerprint_tests.rs`, `src/lib.rs`; [contract](minimal-lossless-history-storage-execution-2026-10-19.md#step-7--canonical-fingerprint-contract).
Verification: all six focused tests pass in the real crate test binary and an independent source-module harness; Python SHA-256 golden agrees.
Blockers: none.

### Step 8 execution record

Commit: `4b2176e`.
Files: `src/facade/historical_archive_fingerprint*.rs`, test-only crate wiring, fixture feature, development/maintainer docs; [verification](minimal-lossless-history-storage-execution-2026-10-19.md#step-8--archive-historical-fingerprint).
Verification: three strengthened synthetic tests pass; authoritative fixture gate passes on all three RELs at four cuts each; all six source hashes unchanged.
Blockers: none.

### Step 9 execution record

Commit: `7a86dac`.
Files: `src/facade/historical_memory_fingerprint*.rs`, test-only crate wiring, development/maintainer docs; [verification](minimal-lossless-history-storage-execution-2026-10-19.md#step-9--memory-historical-fingerprint).
Verification: 3/3 checks pass; every one of 12,414 retained revisions across all six files fingerprints twice; latest memberships equal owner inventories; terminal/pruned/reopen synthetic checks pass; source hashes unchanged.
Blockers: none.

### Step 10 execution record

Commit: `ce26cdf`.
Files: `src/facade/historical_entity_fingerprint*.rs`, test-only crate wiring, maintainer/evidence docs; [verification](minimal-lossless-history-storage-execution-2026-10-19.md#step-10--entity-historical-fingerprint).
Verification: 3/3 checks pass; six fixture cut/membership comparisons, revision/merge/direct terminal/reopen and explicit pruned descriptions; 1,129 retained revisions and 12 terminal positions represented.
Blockers: none.

### Step 11 execution record

Commit: `89059e2`.
Files: `src/facade/historical_relationship_fingerprint*.rs`, test-only crate wiring, maintainer/evidence docs; [verification](minimal-lossless-history-storage-execution-2026-10-19.md#step-11--relationship-historical-fingerprint).
Verification: 3/3 checks pass; qualified participant/evidence revisions, terminal/reopen and pruned-summary behavior agree with owner APIs; all six empty fixture owners fingerprint deterministically; source hashes unchanged.
Blockers: none; nonempty coverage is synthetic because the authoritative corpus predates Relationship publication.

### Step 12 execution record

Commit: `146754d`.
Files: `src/facade/historical_graph_fingerprint*.rs`, test-only crate wiring, maintainer/evidence docs; [verification](minimal-lossless-history-storage-execution-2026-10-19.md#step-12--graph-historical-fingerprint).
Verification: final synthetic checks 3/3; strengthened fixture test passes on all six files with independent scans, exact current relation/node/clock replay agreement; source hashes unchanged. Independent first-publication fingerprint agrees.
Blockers: none; whole-owner lineage proof comparison is explicitly assigned to Step 20.

### Step 13 execution record

Commit: `19030b5`.
Files: `src/facade/historical_vector_generation_fingerprint*.rs`, test-only crate wiring, maintainer/evidence docs; [verification](minimal-lossless-history-storage-execution-2026-10-19.md#step-13--vector-generation-historical-fingerprint).
Verification: initial fixture/activation checks 2/2; final activation and authentic pruning checks 2/2. Three REL fixture heads 1/1/2, historical activation/public API equality, exact immutable closure, retirement, prune evidence and reopen; six source hashes unchanged.
Blockers: none.

### Step 14 execution record

Commit: `a1134bf`.
Files: `src/facade/historical_ego_fingerprint*.rs`, test-only crate wiring, maintainer/evidence docs; [verification](minimal-lossless-history-storage-execution-2026-10-19.md#step-14--ego-historical-fingerprint).
Verification: 4/4 checks pass; PHY/REL synthetic history/current getter/reopen comparisons, no-op clock stability, legacy identity equivalence and all six empty fixture owners; source hashes unchanged.
Blockers: none; nonempty evidence is synthetic because fixture Ego heads are all zero.

### Step 15 execution record

Commit: `dec0b97`.
Files: Freshness oracle adapter/serializers, owner test-only cut seam, shared pure import-scope reconstruction, normalization regression, test-only crate wiring and maintainer/evidence docs; [verification](minimal-lossless-history-storage-execution-2026-10-19.md#step-15--freshness-activity-and-exact-event-fingerprints).
Verification: authoritative fixture gate 1/1; final exact-cut synthetic 1/1; strengthened authentic normalization/reopen/reclaim/proof-rejection regression 1/1; normal library check and formatting pass; source hashes unchanged.
Blockers: none. Full public score-map parity is checked; single-record public API sampling is bounded on large fixtures.


### Step 16 execution record

Commit: `a715ad2`.
Files: `src/facade/historical_dream_proof_fingerprint*.rs`, `src/freshness_dream_journal.rs`, test-only crate wiring and shared maintainer/evidence docs.
Verification: Pending, accepted and completed proof lifecycle; empty versus bare settled marker; nonempty roots; repeat/reopen determinism on three REL fixtures. Shared focused run **17/17**, fixture gates **4/4** (151.25 seconds), normal library check and formatting pass.
Blockers: none for this item; whole-owner visibility remains Step 20.

### Step 17 execution record

Commit: `a715ad2`.
Files: `src/facade/historical_dream_pair_fingerprint*.rs`, test-only crate wiring and shared maintainer/evidence docs.
Verification: Canonical membership and reversed duplicate suppression on REL/PHY; repeat/API/reopen parity on all six fixtures. Shared focused run **17/17**, fixture gates **4/4** (151.25 seconds), normal library check and formatting pass.
Blockers: none for this item; whole-owner visibility remains Step 20.

### Step 18 execution record

Commit: `a715ad2`.
Files: `src/facade/historical_dream_processing_fingerprint*.rs`, test-only crate wiring and shared maintainer/evidence docs.
Verification: Deterministic eligibility and retry/reopen tests on REL/PHY; authentic V1/V2 cooldown conversion; all six fixture state projections agree (all lane sets empty). Shared focused run **17/17**, fixture gates **4/4** (151.25 seconds), normal library check and formatting pass.
Blockers: none for this item; whole-owner visibility remains Step 20.

### Step 19 execution record

Commit: `a715ad2`.
Files: `src/facade/historical_cut_enumeration*.rs`, `src/facade/historical_graph_fingerprint.rs`, test-only crate wiring and shared maintainer/evidence docs.
Verification: Both-owner hand-authored Graph [0,1,2,3] / MemoryGraph [0,1,3] gaps; complete cut-set repeat/reopen parity on all six fixtures. Shared focused run **17/17**, fixture gates **4/4** (151.25 seconds), normal library check and formatting pass.
Blockers: none for this item; whole-owner visibility remains Step 20.

### Step 20 execution record

Commit: `85ade1b`.
Files: `src/facade/historical_global_{cuts,frontier}*.rs`, `historical_whole_{owner,phy}_fingerprint*.rs`, test-only Graph baseline seam and crate wiring; [evidence](minimal-lossless-history-storage-execution-2026-10-19.md#whole-owner-and-baseline-batch--steps-2021).
Verification: final focused checks **7/7**; hand-authored REL/PHY interleaving, lifecycle visibility, actual grouped-publication interior cuts, fixed-source repeat/reopen, authentic rewrite identity and checksum-valid forgery rejection, typed missing-reference rejection.
Blockers: none. A global number alone does not freeze clock-neutral recovery appends; historical projections bind a fixed source. Incomplete reference closure is unavailable, not an invented historical state.

### Step 21 execution record

Commit: `85ade1b`.
Files: `src/facade/historical_baseline_artifacts.rs`, `benchmarks/historical-owner-baseline-2026-10-19.json`, test wiring and shared documentation.
Verification: generation gate **1/1** (278.72 seconds); all six source hashes unchanged; all **12,414** retained Memory revisions included in ordered digests; complete cut counts and explicitly sampled per-domain/global fingerprints persisted. Baseline contains 12 sampled unavailable Freshness event cuts and two unavailable REL global cuts, with explicit source reasons; all unexpected errors remain failures.
Blockers: none. Sampled cut fingerprints are not an exhaustive all-cut parity claim; exhaustive differential checks remain later acceptance items.

### Integration Gate A — Historical oracle

Run existing historical/lifecycle/reconciliation tests plus the new oracle on V1.0 files. Gate passes only if oracle output agrees with current public/internal historical APIs.

Verification: **passed**. The library/integration run passes **1,714 tests across 25 binaries** (five existing ignores), final new focused checks pass **7/7**, doctests pass (zero doctests), fixture APIs agree and all source hashes remain unchanged. Established source unavailability is retained explicitly in the baseline. Architecture indexing freshness is tracked separately in the execution evidence.

---

## Phase B — Container Version Boundary

| Step | Owner | Work | Gate |
| ---: | --- | --- | --- |
| 22 | Sol | Choose and encode the compact container format version. | New header round-trips. |
| 23 | Sol | Make old reader reject the compact version unless routed through new compact open logic. | Older-format path fails with typed unsupported-version error. |
| 24 | Sol | Separate conversation-compaction replay projection from physical recovery for V1 inspection. | Superseding/duplicate compactions project identically without freeing, coalescing, truncating or syncing source frames. |
| 25 | Sol | Preserve stored Entity-resolution history during V1 inspection rather than appending retired-reference repairs. | Stored revision and historical reference survive unchanged; V2 writable open still repairs canonical references. |
| 26 | Sol | Add explicit V1.0 read-only mode for REL. | V1.0 REL opens for inspection/migration without mutation. |
| 27 | Sol | Add explicit V1.0 read-only mode for PHY. | V1.0 PHY opens for inspection/migration without mutation. |
| 28 | Sol | Add typed `StorageFormatUpgradeRequired` mutation failure for V1.0 REL. | REL mutation fails before append. |
| 29 | Sol | Add typed `StorageFormatUpgradeRequired` mutation failure for V1.0 PHY. | PHY mutation fails before append. |
| 30 | Sol | Make new REL creation use the compact format version. | New REL reopens through compact path. |
| 31 | Sol | Make new PHY creation use the compact format version. | New PHY reopens through compact path. |


### Step 22 execution record

Commit: `29ea09a`.
Files: `src/container/container.rs`, `src/container/container_format_boundary_tests.rs`, [Storage format](storage-format.md#compact-container-version-boundary--staged).
Verification: V2.0 identity headers round-trip for REL and PHY, including exact header bytes and owner UUID; shared focused container verification recorded in execution evidence.
Blockers: none for header encoding. Ordinary creation remains V1 until the read-only/admission cutover in Steps 26–31.

### Step 23 execution record

Commit: `29ea09a`.
Files: `src/container/container_scan.rs`, `src/container/container_format_boundary_tests.rs`.
Verification: V1 route rejects V2 with typed `UnsupportedVersion` before identity/payload dispatch; explicit internal compact route accepts exactly V2.0 and rejects V1/unknown minor versions. Luna review found no actionable defect.
Blockers: none for strict route selection. Owner-level compact dispatch and version-preserving maintenance copies remain later cutover work.


### Step 24 execution record

Commit: `a822c23`.
Files: src/archive/conversation_compaction_store.rs; `src/facade/storage_format_boundary_tests.rs`.
Verification: Duplicate durable compactions project only the surviving generation; exact V1 source bytes remain unchanged while V2 writable reopen reclaims. Shared focused run **26/26**; broader evidence is recorded in the execution log.
Blockers: none for this acceptance item; explicit lossless upgrade execution remains Phase C.

### Step 25 execution record

Commit: `a822c23`.
Files: src/facade/cva_lifecycle.rs, src/facade/phylactery_lifecycle.rs; `src/facade/storage_format_boundary_tests.rs`.
Verification: Stored retired-Entity reference and resolution revision remain unchanged in V1; V2 writable reopen appends the existing canonical repair. Shared focused run **26/26**; broader evidence is recorded in the execution log.
Blockers: none for this acceptance item; explicit lossless upgrade execution remains Phase C.

### Step 26 execution record

Commit: `a822c23`.
Files: src/container/container_scan.rs, src/facade/cva_lifecycle.rs; `src/facade/storage_format_boundary_tests.rs`.
Verification: Explicit and automatic V1 REL opens use read-only handles; malformed V1 tails fail unchanged; V2 retains recovery. Shared focused run **26/26**; broader evidence is recorded in the execution log.
Blockers: none for this acceptance item; explicit lossless upgrade execution remains Phase C.

### Step 27 execution record

Commit: `a822c23`.
Files: src/container/container_scan.rs, src/facade/phylactery_lifecycle.rs; `src/facade/storage_format_boundary_tests.rs`.
Verification: Explicit V1 PHY inspection succeeds unchanged; strict V1 API rejects V2. Shared focused run **26/26**; broader evidence is recorded in the execution log.
Blockers: none for this acceptance item; explicit lossless upgrade execution remains Phase C.

### Step 28 execution record

Commit: `a822c23`.
Files: Concrete owner stores and REL facade writers; `src/facade/storage_format_boundary_tests.rs`.
Verification: Metadata no-op, exact Memory retry, empty Graph batch and exact Ego update return typed StorageFormatUpgradeRequired without resident/durable changes. Owner write scope cannot bypass the raw barrier. Shared focused run **26/26**; broader evidence is recorded in the execution log.
Blockers: none for this acceptance item; explicit lossless upgrade execution remains Phase C.

### Step 29 execution record

Commit: `a822c23`.
Files: Concrete owner stores and PHY facade writers; `src/facade/storage_format_boundary_tests.rs`.
Verification: Profile no-op and repeated writes return typed StorageFormatUpgradeRequired unchanged. Vector, Ego, Memory, Graph, processing and pair writers preflight format admission. Shared focused run **26/26**; broader evidence is recorded in the execution log.
Blockers: none for this acceptance item; explicit lossless upgrade execution remains Phase C.

### Step 30 execution record

Commit: `a822c23`.
Files: src/container/container.rs, src/facade/cva_lifecycle.rs; `src/facade/storage_format_boundary_tests.rs`.
Verification: New REL header is exactly V2.0; normal reopen succeeds and empty physical copies retain identity and UUID. Shared focused run **26/26**; broader evidence is recorded in the execution log.
Blockers: none for this acceptance item; explicit lossless upgrade execution remains Phase C.

### Step 31 execution record

Commit: `a822c23`.
Files: src/container/container.rs, src/facade/phylactery_lifecycle.rs; `src/facade/storage_format_boundary_tests.rs`.
Verification: New PHY header is exactly V2.0 and normal reopen succeeds. Shared focused run **26/26**; broader evidence is recorded in the execution log.
Blockers: none for this acceptance item; explicit lossless upgrade execution remains Phase C.


---

## Phase C — Lossless Upgrade Skeleton

| Step | Owner | Work | Gate |
| ---: | --- | --- | --- |
| 32 | Sol | Add transformed-copy upgrade request/result types and output-exists protection. | Minimal API tests pass. |
| 33 | Sol | Implement REL upgrade destination creation preserving owner UUID. | Minimal REL preserves owner identity. |
| 34 | Sol | Implement PHY upgrade destination creation preserving owner UUID. | Minimal PHY preserves owner identity. |
| 35 | Sol | Preserve CVA global-version numbers during transformed-copy upgrade. | Synthetic multi-version file retains exact global numbers. |
| 36 | Sol | Preserve transaction timestamps during upgrade. | Timestamp map compares exactly before/after. |
| 37 | Sol | Preserve owner-local version numbers during upgrade. | Synthetic interleaved owners retain exact local cuts. |
| 38 | Sol | Preserve accepted physical ordering required by existing visibility semantics without reissuing semantic history. | Synthetic ordering oracle matches source. |
| 39 | Sol | Add source-drift detection between upgrade planning and execution. | Changed source rejects before promotion. |
| 40 | Sol | Add partial-output cleanup/abort behavior. | Failed upgrade never leaves a promotable partial destination. |
| 41 | Luna | Add corruption, output-exists, abort, and drift regression fixtures. | All failure modes leave source untouched. |

### Step 32 execution record

Commit: `4652466`.
Files: src/storage_upgrade/model.rs; src/storage_upgrade/shell.rs.
Verification: Internal request/plan/result types; existing file, directory and source-alias outputs reject without overwrite. Shared Gate B run **34/34**; [execution evidence](minimal-lossless-history-storage-execution-2026-10-19.md#lossless-upgrade-shell).
Blockers: none for shell acceptance; semantic codec migration and public promotion remain later plan requirements.

### Step 33 execution record

Commit: `4652466`.
Files: src/storage_upgrade/shell.rs.
Verification: Minimal REL staging retains kind, owner UUID and V2 reopen; source bytes unchanged. Shared Gate B run **34/34**; [execution evidence](minimal-lossless-history-storage-execution-2026-10-19.md#lossless-upgrade-shell).
Blockers: none for shell acceptance; semantic codec migration and public promotion remain later plan requirements.

### Step 34 execution record

Commit: `4652466`.
Files: src/storage_upgrade/shell.rs.
Verification: Minimal PHY staging retains kind, owner UUID and V2 reopen; source bytes unchanged. Shared Gate B run **34/34**; [execution evidence](minimal-lossless-history-storage-execution-2026-10-19.md#lossless-upgrade-shell).
Blockers: none for shell acceptance; semantic codec migration and public promotion remain later plan requirements.

### Step 35 execution record

Commit: `4652466`.
Files: src/container/container_upgrade.rs.
Verification: Four exact global tickets survive physical copy and reopen; no allocation/rebase. Shared Gate B run **34/34**; [execution evidence](minimal-lossless-history-storage-execution-2026-10-19.md#lossless-upgrade-shell).
Blockers: none for shell acceptance; semantic codec migration and public promotion remain later plan requirements.

### Step 36 execution record

Commit: `4652466`.
Files: src/container/container_upgrade.rs.
Verification: Exact timestamp map retains missing, repeated and nonmonotone values. Shared Gate B run **34/34**; [execution evidence](minimal-lossless-history-storage-execution-2026-10-19.md#lossless-upgrade-shell).
Blockers: none for shell acceptance; semantic codec migration and public promotion remain later plan requirements.

### Step 37 execution record

Commit: `4652466`.
Files: src/storage_upgrade/tests.rs.
Verification: Interleaved Memory/Entity owner cuts and whole-owner fingerprints match at every retained synthetic global cut. Shared Gate B run **34/34**; [execution evidence](minimal-lossless-history-storage-execution-2026-10-19.md#lossless-upgrade-shell).
Blockers: none for shell acceptance; semantic codec migration and public promotion remain later plan requirements.

### Step 38 execution record

Commit: `4652466`.
Files: src/container/container_upgrade.rs.
Verification: Complete normalized-header digest and exact chunk ordering match; clock-neutral replay frontier matches at every synthetic cut. Shared Gate B run **34/34**; [execution evidence](minimal-lossless-history-storage-execution-2026-10-19.md#lossless-upgrade-shell).
Blockers: none for shell acceptance; semantic codec migration and public promotion remain later plan requirements.

### Step 39 execution record

Commit: `4652466`.
Files: src/storage_upgrade/shell.rs.
Verification: Same-size UUID drift before/during staging rejects; final source handle/path digest checks precede shell acceptance. Shared Gate B run **34/34**; [execution evidence](minimal-lossless-history-storage-execution-2026-10-19.md#lossless-upgrade-shell).
Blockers: none for shell acceptance; semantic codec migration and public promotion remain later plan requirements.

### Step 40 execution record

Commit: `4652466`.
Files: src/storage_upgrade/shell.rs.
Verification: Owned sibling guard removes partial file/directory on abort, error and drop; late output winner remains untouched. Shared Gate B run **34/34**; [execution evidence](minimal-lossless-history-storage-execution-2026-10-19.md#lossless-upgrade-shell).
Blockers: none for shell acceptance; semantic codec migration and public promotion remain later plan requirements.

### Step 41 execution record

Commit: `4652466`.
Files: src/storage_upgrade/tests.rs.
Verification: Eight shell regressions cover identity, ordering, historical cuts, truncated input, output aliases/collisions, abort and drift. Shared Gate B run **34/34**; [execution evidence](minimal-lossless-history-storage-execution-2026-10-19.md#lossless-upgrade-shell).
Blockers: none for shell acceptance; semantic codec migration and public promotion remain later plan requirements.

### Integration Gate B — Format boundary and upgrader shell

Run header/open/create/read-only/mutation-rejection/upgrade-shell tests for REL and PHY. No semantic owner codec is changed yet.

Acceptance: **34/34** focused tests pass (8 shell, 9 facade boundary, 4 header boundary, 13 existing container). Normal library check, formatting, both documentation-policy modes, current ownership guards and 23 retained-snapshot Pitlord rules pass. Fresh structural snapshot remains unavailable after a bounded refresh; no fresh evidence is claimed. The upgrader remains an internal non-promoting shell until owner migration/equivalence steps complete.

---

## Phase D — Memory Delta Contract

| Step | Owner | Work | Gate |
| ---: | --- | --- | --- |
| 42 | Sol | Inventory every Memory field and classify immutable vs mutable. | Field table complete. |
| 43 | Sol | Define optional-field clear vs unchanged semantics. | Table-driven contract tests pass. |
| 44 | Sol | Define collection-field delta semantics, if any. | Canonical operation rules documented/tested. |
| 45 | Sol | Freeze Memory creation record semantics. | Creation contract matches V1.0 first revision. |
| 46 | Sol | Freeze Memory delta record semantics. | Delta contract can represent every legal V1.0 later revision. |
| 47 | Sol | Define canonical Memory delta ordering/field-mask rules. | Equivalent changes encode identically. |

### Step 42 execution record

Commit: `4652466`.
Files: src/memory/memory_model.rs; refactor specification Memory field inventory; src/memory/memory_compact_contract_tests.rs.
Verification: Exact field-table coverage of all 21 MemoryDraft, 28 hydrated Memory and 26 MemoryRecord fields; no added created_at immutability. Shared contract run **7/7**; [execution evidence](minimal-lossless-history-storage-execution-2026-10-19.md#memory-delta-contract).
Blockers: none for contract acceptance; wire codecs, replay and writer cutover begin at Step 48.

### Step 43 execution record

Commit: `4652466`.
Files: src/memory/memory_compact_contract.rs; src/memory/memory_compact_contract_tests.rs.
Verification: Table-driven optional None/Some transitions distinguish unchanged, set and clear; actual source-node clear sets the correct mask. Shared contract run **7/7**; [execution evidence](minimal-lossless-history-storage-execution-2026-10-19.md#memory-delta-contract).
Blockers: none for contract acceptance; wire codecs, replay and writer cutover begin at Step 48.

### Step 44 execution record

Commit: `4652466`.
Files: src/memory/memory_compact_contract.rs; src/memory/memory_compact_contract_tests.rs.
Verification: Real temporal-inference resolution ordering triggers whole-value replacement; proof clear and unchanged remain distinct; routing stays independent. Shared contract run **7/7**; [execution evidence](minimal-lossless-history-storage-execution-2026-10-19.md#memory-delta-contract).
Blockers: none for contract acceptance; wire codecs, replay and writer cutover begin at Step 48.

### Step 45 execution record

Commit: `4652466`.
Files: src/memory/memory_compact_contract.rs; src/memory/memory_compact_contract_tests.rs.
Verification: Exact creation record and first-revision/nonzero coordinate guards; retained V1 reader accepts the same initial record. Shared contract run **7/7**; [execution evidence](minimal-lossless-history-storage-execution-2026-10-19.md#memory-delta-contract).
Blockers: none for contract acceptance; wire codecs, replay and writer cutover begin at Step 48.

### Step 46 execution record

Commit: `4652466`.
Files: src/memory/memory_compact_contract.rs; src/memory/memory_compact_contract_tests.rs.
Verification: Exact successor/identity/body/version/mutation envelope; metadata-empty publication and V1 later revision remain representable. Shared contract run **7/7**; [execution evidence](minimal-lossless-history-storage-execution-2026-10-19.md#memory-delta-contract).
Blockers: none for contract acceptance; wire codecs, replay and writer cutover begin at Step 48.

### Step 47 execution record

Commit: `4652466`.
Files: src/memory/memory_compact_contract.rs; src/memory/memory_compact_contract_tests.rs.
Verification: Twenty fixed ordered fields, exhaustive record pattern, reserved-bit rejection and identical canonical delta across change construction order. Shared contract run **7/7**; [execution evidence](minimal-lossless-history-storage-execution-2026-10-19.md#memory-delta-contract).
Blockers: none for contract acceptance; wire codecs, replay and writer cutover begin at Step 48.

## Phase E — Memory Codec and Replay

| Step | Owner | Work | Gate |
| ---: | --- | --- | --- |
| 48 | Sol | Implement compact Memory creation encoder. | Golden encode tests pass. |
| 49 | Sol | Implement compact Memory creation decoder/validation. | Round-trip + malformed/truncated tests pass. |
| 50 | Sol | Implement compact Memory delta encoder. | Golden encode tests pass. |
| 51 | Sol | Implement compact Memory delta decoder/validation. | Malformed/unknown/noncanonical tests fail closed. |
| 52 | Sol | Implement pure Memory delta application over prior accepted revision. | Valid delta materializes expected postimage. |
| 53 | Sol | Add validate-before-apply guard so invalid delta cannot mutate projection. | Negative tests prove prior state unchanged. |
| 54 | Sol | Teach compact Memory rebuild to replay creation + delta + tombstone. | Synthetic histories reopen identically. |
| 55 | Sol | Preserve exact mutation-ID retry/conflict behavior in compact replay. | Exact retry idempotent; conflicting payload rejects. |

### Step 48 execution record

Commit: `d5b8a79`.
Files: src/memory/memory_compact_codec.rs; neighboring compact codec/application/replay/retry tests.
Verification: Independent golden creation bytes pass; separate body identity and exact creation coordinates retained. Shared focused run **31/31**, existing Memory regressions **29/29**, CLI **32/32**; [execution evidence](minimal-lossless-history-storage-execution-2026-10-19.md#memory-codec-and-replay).
Blockers: none for this acceptance item; ordinary writer cutover and migration begin at Step 56.

### Step 49 execution record

Commit: `d5b8a79`.
Files: src/memory/memory_compact_codec.rs; neighboring compact codec/application/replay/retry tests.
Verification: Creation round-trip, every-byte truncation, bad magic/tags/UTF-8/lengths, empty mutation and zero coordinates reject. Shared focused run **31/31**, existing Memory regressions **29/29**, CLI **32/32**; [execution evidence](minimal-lossless-history-storage-execution-2026-10-19.md#memory-codec-and-replay).
Blockers: none for this acceptance item; ordinary writer cutover and migration begin at Step 56.

### Step 50 execution record

Commit: `d5b8a79`.
Files: src/memory/memory_compact_codec.rs; neighboring compact codec/application/replay/retry tests.
Verification: Independent golden delta bytes pass; only mask-selected values appear in fixed order. Shared focused run **31/31**, existing Memory regressions **29/29**, CLI **32/32**; [execution evidence](minimal-lossless-history-storage-execution-2026-10-19.md#memory-codec-and-replay).
Blockers: none for this acceptance item; ordinary writer cutover and migration begin at Step 56.

### Step 51 execution record

Commit: `d5b8a79`.
Files: src/memory/memory_compact_codec.rs; neighboring compact codec/application/replay/retry tests.
Verification: Malformed/truncated/unknown-mask/envelope inputs reject; prior-dependent redundant-operation canonicality is checked at application. Shared focused run **31/31**, existing Memory regressions **29/29**, CLI **32/32**; [execution evidence](minimal-lossless-history-storage-execution-2026-10-19.md#memory-codec-and-replay).
Blockers: none for this acceptance item; ordinary writer cutover and migration begin at Step 56.

### Step 52 execution record

Commit: `d5b8a79`.
Files: src/memory/memory_compact_apply.rs; neighboring compact codec/application/replay/retry tests.
Verification: All twenty changed fields, explicit optional clears and legitimate empty-mask publications materialize exact postimages. Shared focused run **31/31**, existing Memory regressions **29/29**, CLI **32/32**; [execution evidence](minimal-lossless-history-storage-execution-2026-10-19.md#memory-codec-and-replay).
Blockers: none for this acceptance item; ordinary writer cutover and migration begin at Step 56.

### Step 53 execution record

Commit: `d5b8a79`.
Files: src/memory/memory_compact_apply.rs; src/memory/memory_rebuild.rs; neighboring compact codec/application/replay/retry tests.
Verification: Invalid target/base/revision/coordinates/mutation, redundant fields and proof binding cannot replace prior accepted state. Shared focused run **31/31**, existing Memory regressions **29/29**, CLI **32/32**; [execution evidence](minimal-lossless-history-storage-execution-2026-10-19.md#memory-codec-and-replay).
Blockers: none for this acceptance item; ordinary writer cutover and migration begin at Step 56.

### Step 54 execution record

Commit: `d5b8a79`.
Files: src/memory/memory_rebuild.rs; src/memory/memory_store.rs; neighboring compact codec/application/replay/retry tests.
Verification: Creation/delta/tombstone history reopens; unversioned carriers inert; exact publication-marker binding and V1 rejection preserve source bytes; legacy late-ticket visibility matches. Shared focused run **31/31**, existing Memory regressions **29/29**, CLI **32/32**; [execution evidence](minimal-lossless-history-storage-execution-2026-10-19.md#memory-codec-and-replay).
Blockers: none for this acceptance item; ordinary writer cutover and migration begin at Step 56.

### Step 55 execution record

Commit: `d5b8a79`.
Files: src/memory/memory_compact_retry_tests.rs; neighboring compact codec/application/replay/retry tests.
Verification: Creation/latest-revision exact retries, mutation conflict, stale new revision and terminal retry preserve existing bytes, coordinates and retained revisions. Shared focused run **31/31**, existing Memory regressions **29/29**, CLI **32/32**; [execution evidence](minimal-lossless-history-storage-execution-2026-10-19.md#memory-codec-and-replay).
Blockers: none for this acceptance item; ordinary writer cutover and migration begin at Step 56.

### Step 56 execution record

Commit: `a7a485d`.
Files: src/memory/memory_store.rs; src/memory/memory_compact_writer_tests.rs.
Verification: Ordinary REL/PHY creation writes only compact creation carriers and reopens with identical coordinates. Shared focused compact tests **36/36**, import adapter tests **3/3**, existing regressions **61/61**, CLI **32/32**; root/CLI checks and formatting pass. See [execution evidence](minimal-lossless-history-storage-execution-2026-10-19.md#standalone-memory-writer-and-reader-cutover).
Blockers: none for this item; grouped/external publication and migration remain pending.

### Step 57 execution record

Commit: `a7a485d`.
Files: src/memory/memory_store.rs; src/memory/memory_compact_writer_tests.rs.
Verification: Ordinary revisions write only canonical changed fields; exact retry and conflict semantics survive reopen. Shared focused compact tests **36/36**, import adapter tests **3/3**, existing regressions **61/61**, CLI **32/32**; root/CLI checks and formatting pass. See [execution evidence](minimal-lossless-history-storage-execution-2026-10-19.md#standalone-memory-writer-and-reader-cutover).
Blockers: none for this item; grouped/external publication and migration remain pending.

### Step 58 execution record

Commit: `a7a485d`.
Files: src/memory/memory_compact_maintenance_tests.rs.
Verification: Existing tombstone markers remain terminal and retain publication coordinates through maintenance. Shared focused compact tests **36/36**, import adapter tests **3/3**, existing regressions **61/61**, CLI **32/32**; root/CLI checks and formatting pass. See [execution evidence](minimal-lossless-history-storage-execution-2026-10-19.md#standalone-memory-writer-and-reader-cutover).
Blockers: none for this item; grouped/external publication and migration remain pending.

### Step 59 execution record

Commit: `a7a485d`.
Files: src/facade/cva_reconcile_memory.rs.
Verification: REL reconciliation resolves accepted owner publications instead of decoding legacy carrier bytes. Shared focused compact tests **36/36**, import adapter tests **3/3**, existing regressions **61/61**, CLI **32/32**; root/CLI checks and formatting pass. See [execution evidence](minimal-lossless-history-storage-execution-2026-10-19.md#standalone-memory-writer-and-reader-cutover).
Blockers: none for this item; grouped/external publication and migration remain pending.

### Step 60 execution record

Commit: `a7a485d`.
Files: src/imports/reconciliation.rs; src/imports/reconciliation_compact_memory_tests.rs.
Verification: Completed-import evidence is normalized from accepted records; V1 and compact V2 evidence matches. Shared focused compact tests **36/36**, import adapter tests **3/3**, existing regressions **61/61**, CLI **32/32**; root/CLI checks and formatting pass. See [execution evidence](minimal-lossless-history-storage-execution-2026-10-19.md#standalone-memory-writer-and-reader-cutover).
Blockers: none for this item; grouped/external publication and migration remain pending.

### Step 61 execution record

Commit: `a7a485d`.
Files: src/imports/reconciliation.rs; src/imports/reconciliation_compact_memory_tests.rs.
Verification: Import interval admission recognizes strictly validated compact carriers and rejects malformed or unknown variants. Shared focused compact tests **36/36**, import adapter tests **3/3**, existing regressions **61/61**, CLI **32/32**; root/CLI checks and formatting pass. See [execution evidence](minimal-lossless-history-storage-execution-2026-10-19.md#standalone-memory-writer-and-reader-cutover).
Blockers: none for this item; grouped/external publication and migration remain pending.

### Step 62 execution record

Commit: `a7a485d`.
Files: src/facade/storage_reclamation_scan.rs; src/memory/memory_compact_maintenance_tests.rs.
Verification: REL/PHY reclamation preserves linked compact history and body closure while removing unversioned carriers and orphan bodies. Shared focused compact tests **36/36**, import adapter tests **3/3**, existing regressions **61/61**, CLI **32/32**; root/CLI checks and formatting pass. See [execution evidence](minimal-lossless-history-storage-execution-2026-10-19.md#standalone-memory-writer-and-reader-cutover).
Blockers: none for this item; grouped/external publication and migration remain pending.

### Step 63 execution record

Commit: `9996c28`.
Files: `src/runtime_inference/insomnia/completion.rs`; `src/runtime_inference/insomnia/completion_compact_legacy.rs`; `src/runtime_inference/insomnia/completion_compact_tests.rs`.
Verification: Atomic grouped/external publication tests pass **10/10**; existing Insomnia regressions pass **83/83**. See [execution evidence](minimal-lossless-history-storage-execution-2026-10-19.md#steps-63-86--grouped-publication-migration-and-import-proof-transport).
Blockers: none for this item; migration and integration acceptance are separate gates.

### Step 64 execution record

Commit: `9996c28`.
Files: `src/external_source/publication.rs`; `src/external_source/publication_compact_tests.rs`.
Verification: Compact external Memory publication/routing and atomic retry coverage pass in the **10/10** focused publication run; existing Insomnia regressions pass **83/83**.
Blockers: none for this item; end-to-end migration remains separately gated.

### Step 65 execution record

Commit: `9996c28`.
Files: `src/storage_upgrade/memory.rs`; `src/storage_upgrade/memory_tests.rs`; compact Memory writer/replay support.
Verification: V1 standalone creation migrates with identity, body and publication coordinates preserved; the migration-filter run passes **14/14** for REL/PHY histories.
Blockers: none for this item.

### Step 66 execution record

Commit: `9996c28`.
Files: `src/storage_upgrade/memory.rs`; `src/storage_upgrade/memory_tests.rs`; `src/memory/memory_rebuild.rs`.
Verification: V1 later revisions migrate to canonical compact deltas with historical parity; migration-filter run **14/14**.
Blockers: none for this item.

### Step 67 execution record

Commit: `9996c28`.
Files: `src/storage_upgrade/memory.rs`; `src/runtime_inference/insomnia/completion.rs`; `src/runtime_inference/insomnia/completion_compact_legacy.rs`; `src/runtime_inference/insomnia/completion_migration_tests.rs`.
Verification: Grouped histories preserve record IDs, bodies, clocks and exact accepted completion contents; atomic run **10/10**, migration run **14/14**.
Blockers: none for this item.

### Step 68 execution record

Commit: `9996c28`.
Files: `src/storage_upgrade/memory.rs`; `src/external_source/publication.rs`; `src/external_source/publication_compact_tests.rs`.
Verification: External publication and routing references migrate with owner coordinates and bodies preserved; migration run **14/14**.
Blockers: none for this item.

### Step 69 execution record

Commit: `9996c28`.
Files: `src/storage_upgrade/memory.rs`; `src/storage_upgrade/memory_tests.rs`; `src/memory/memory_rebuild.rs`.
Verification: Pre-tombstone revisions and terminal publication coordinates match across V1-to-compact migration; migration run **14/14**.
Blockers: none for this item.

### Step 70 execution record

Commit: `9996c28`.
Files: `src/storage_upgrade/memory_tests.rs`; `src/memory/memory_compact_replay_tests.rs`; `src/memory/memory_compact_apply_tests.rs`.
Verification: REL/PHY fixtures cover optional clears, supersession, archive/lifecycle changes, legal late-ticket coordinates and malformed histories; compact and migration tests pass **38/38** and **14/14**.
Blockers: none for the adversarial fixture gate.

### Step 71 execution record

Commit: `9996c28`.
Files: `src/storage_upgrade/memory_tests.rs`; `src/memory/memory_compact_retry_tests.rs`.
Verification: Creation/latest-revision retries, metadata conflicts, stale new mutations and terminal retry preserve records and coordinates; migration run **14/14**.
Blockers: none for the retry/conflict fixture gate.

### Step 72 execution record

Commit: `9996c28`.
Files: `src/imports/legacy_epoch_transport.rs`; `src/imports/epoch_store.rs`.
Verification: The relocated-proof contract keeps exact CVAILEG1 evidence and binds the transformed prefix independently; transport and legacy-epoch tests pass **6/6**.
Blockers: none for the transport contract.

### Step 73 execution record

Commit: `9996c28`.
Files: `src/imports/legacy_epoch_transport.rs`; `src/imports/mod.rs`.
Verification: Bounded CVAILEG2 encoding retains original evidence and binds owner/prefix/settlement data; focused legacy-epoch suite passes **6/6**.
Blockers: none for the encoder gate.

### Step 74 execution record

Commit: `9996c28`.
Files: `src/imports/legacy_epoch_transport.rs`; `src/imports/epoch_store.rs`.
Verification: Checksum, truncation, malformed nested frames, unknown versions and V1 rejection fail closed; focused legacy-epoch suite passes **6/6**.
Blockers: none for the decoder gate.

### Step 75 execution record

Commit: `9996c28`.
Files: `src/imports/epoch_store.rs`; `src/memory/memory_rebuild.rs`; `src/imports/legacy_epoch_transport.rs`.
Verification: V2 owner replay validates owner, transformed prefix, marker and settlement while historical evidence exposes the original proof; V1 remains read-only and rejects compact proof. Focused suite **6/6**.
Blockers: none for replay validation.

### Step 76 execution record

Commit: `9996c28`.
Files: `src/storage_upgrade/memory.rs`; `src/facade/historical_import_legacy_epoch_tests.rs`; `src/imports/legacy_epoch_transport.rs`.
Verification: The authoritative imported REL migration reopens with exact original proof and transformed-prefix binding; source bytes and semantic fingerprints match, with zero observed semantic mismatches.
Blockers: The six-file authoritative migration/measurement run is still in progress; this step's imported REL proof gate is satisfied.

### Step 77 execution record

Commit: `9996c28`.
Files: `src/facade/historical_import_legacy_epoch_tests.rs`; `src/facade/historical_import_legacy_epoch_version_tests.rs`.
Verification: Prefix, owner, marker, settlement, checksum, malformed framing and source-preservation attacks are covered; legacy-epoch suite passes **6/6**.
Blockers: none for the adversarial proof gate.

### Step 78 execution record

Commit: `9996c28`.
Files: `src/facade/cva_historical_import.rs`; `src/facade/historical_import_transform.rs`.
Verification: Read-only normalization preparation retains original proof bytes and does not append semantic versions or synthesize clocks; V3/V4/V5 legacy settlement tests pass within **6/6**.
Blockers: none for preparation.

### Step 79 execution record

Commit: `9996c28`.
Files: `src/facade/cva_historical_import.rs`; normalization and legacy settlement tests.
Verification: Normalization preparation is separated from persistence; repeated normalization is idempotent and preserves existing publication behavior. Legacy-epoch suite **6/6**.
Blockers: none for extraction.

### Step 80 execution record

Commit: `9996c28`.
Files: `src/facade/historical_import_legacy_epoch_version_tests.rs`; `src/facade/cva_historical_import.rs`.
Verification: CVAFRS03 normalization is prepared on the original source before relocation; exact evidence and canonical Freshness history survive migration and maintenance. Legacy-epoch suite **6/6**.
Blockers: none for this variant.

### Step 81 execution record

Commit: `9996c28`.
Files: `src/facade/historical_import_legacy_epoch_version_tests.rs`; `src/facade/cva_historical_import.rs`.
Verification: CVAFRS04 epoch scope and predecessor evidence survive migration without changing source bytes or semantic clocks; legacy-epoch suite **6/6**.
Blockers: none for this variant.

### Step 82 execution record

Commit: `9996c28`.
Files: `src/facade/historical_import_legacy_epoch_version_tests.rs`; `src/facade/cva_historical_import.rs`.
Verification: CVAFRS05 scoped event/source proof survives migration with matching canonical Freshness fingerprints and transaction times; legacy-epoch suite **6/6**.
Blockers: none for this variant.

### Step 83 execution record

Commit: `9996c28`.
Files: `src/facade/historical_import_legacy_epoch_tests.rs`; `src/facade/historical_import_legacy_epoch_version_tests.rs`; maintenance/rebind paths.
Verification: CVAFRS03/04/05 migration preserves source bytes, semantic fingerprints, global/local coordinates and transaction times; reclaim/repack/prune remain valid. Legacy-epoch suite **6/6**.
Blockers: none for the synthetic and migrated-owner equivalence gate; the whole-file measurement remains Step 87.

### Step 84 execution record

Commit: `9996c28`.
Files: `src/facade/storage_reclamation.rs`; REL/PHY reclaim, attachment/principal repack and prune paths.
Verification: Maintenance rebinds only the transported physical prefix; original CVAILEG1 receipt and semantic history remain unchanged. Legacy-epoch suite **6/6**.
Blockers: none for the covered maintenance paths.

### Step 85 execution record

Commit: `9996c28`.
Files: `src/facade/cva_historical_import.rs`; `src/imports/legacy_epoch_transport.rs`; historical-copy equivalence paths.
Verification: Historical evidence comparison unwraps and retains the exact original proof; maintenance validates the new physical binding separately. Legacy-epoch suite **6/6**.
Blockers: none for proof-equivalence handling.

### Step 86 execution record

Commit: `9996c28`.
Files: `src/facade/historical_import_legacy_epoch_tests.rs`; `src/facade/historical_import_legacy_epoch_version_tests.rs`.
Verification: Migrated-owner reclaim, attachment repack and prune preserve receipts, clocks and proof validation after earlier payloads are removed or changed; legacy-epoch suite **6/6**. Principal repack uses the same import-owned rebinding helper and is covered by the broader maintenance suite.
Blockers: none for the covered maintenance regression.

### Step 87 execution record

Commit: implementation `9996c28`; measurement artifact `c69f279`; policy interruption `fd61a1e`.
Files: `src/storage_upgrade/memory_authoritative_tests.rs`; `benchmarks/lossless-memory-migration-step87-2026-10-19.json`.
Verification: All six authoritative V1 sources pass every Memory cut/revision fingerprint, terminal state, transaction time and unchanged-source assertion; **zero mismatches**, optimized test **1/1**, 2288.21 seconds. Raw Memory carriers decrease by **1,146,938 bytes** in aggregate. Whole files increase by **6,269,151 bytes** because this temporary bridge retains old import proofs and adds semantic successors; per-file signed deltas are recorded without compression. See [execution evidence](minimal-lossless-history-storage-execution-2026-10-19.md#step-87-and-integration-gate-c).
Blockers: none for Memory measurement; canonical whole-corpus migration and reader retirement remain gated by R1–R22.

### Step 88 execution record

Commit: `bafdaae`.
Files: `src/compact_delta.rs`; `src/memory/memory_compact_contract.rs`; `src/lib.rs`; `tools/pitlord/semantic.json`.
Verification: Extracted only typed unchanged/set diff and known-mask mechanics; Memory field bits, order, tags and errors are unchanged. Focused compact Memory tests **38/38**; root/CLI format and checks pass; CLI **32/32**. No runtime/source-policy difference exists between the implementation and rebased checkpoint `14bcba3`. Current-source ownership: **987 Rust files, 20 areas, zero issues**. See [shared-mechanics evidence](minimal-lossless-history-storage-execution-2026-10-19.md#shared-delta-mechanics-steps-88-90).
Blockers: none; fresh enforcement for `bafdaae` passes all 23 rules across 20 areas and 91,596 source nodes (`.test-tmp/lossless-shared-delta-architecture-refresh.log`).

### Step 89 execution record

Commit: `bafdaae`.
Files: `src/compact_delta.rs`; `src/memory/memory_compact_apply.rs`; `src/memory/memory_compact_apply_tests.rs`.
Verification: Extracted pure candidate-field application. Envelope checks, exhaustive typed candidate construction and canonical re-derivation remain in Memory before owner admission. A generic semantic validation/candidate framework would add authority rather than reuse proven mechanics and was deliberately not introduced. Memory behavior is unchanged: **38/38**, root/CLI checks and CLI **32/32** pass; independent Luna review found no wire/validation change.
Blockers: none for this item; no generic semantic-owner abstraction is authorized.

### Step 90 execution record

Commit: `bafdaae`.
Files: `docs/architecture.md`; `docs/storage-format.md`; `docs/maintainer-map.md`.
Verification: Documentation explicitly retains owner-specific IDs, clocks, revisions, field order, wire tags, errors, terminal rules, canonical validation and publication. The helper imports no owner/storage types. Both documentation-policy modes pass (`.test-tmp/lossless-shared-delta-docs.log`).
Blockers: none for this documentation boundary; final format retirement remains gated by R1–R22.

### Step 91 execution record

Commit: `3cc6631`.
Files: `src/storage_upgrade/memory.rs`; `model.rs`; `prepruned_memory_tests.rs`; `memory_authoritative_tests.rs`.
Verification: Reproduced invalid staged reopen from unchanged maintenance-history proof. Typed unsupported-authority rejection now preserves genuine prepruned V1 source and cleans staging without a destination. Upgrade **13/13**, legacy proof **6/6**, six supported authoritative stages **1/1** with exact accepted byte counts and source hashes unchanged. See execution evidence for logs.
Blockers: none for fail-closed rejection; translating existing CVAHPR01 authority remains unsupported until explicit proof-equivalence work passes.

### Step 92 execution record

Commit: `3cc6631`.
Files: `src/perception/entity_compact_contract.rs`; `entity_compact_contract_tests.rs`; `entity_legacy_admission_tests.rs`; field-validation visibility; specification.
Verification: Actual V1 REL/PHY characterization **2/2**; pure owner contract **11/11**; broad Entity regressions **187/187**; root format/check and both documentation-policy modes pass. Contract preserves optional UUID changes, exact values, legal late tickets and six explicit mutable bits. Pruning evidence is explicit; file-level authorization is reserved for Step 101 before writer/migration cutover. Terminals remain separate.
Blockers: none for the representation contract; codecs, replay and authenticated file-level proof admission remain pending.

### Step 93 execution record

Commit: `ace0a33`.
Files: `src/perception/entity_compact_codec.rs`; `entity_compact_values.rs`; `entity_compact_codec_tests.rs`.
Verification: Independent golden creation framing, exact optional identity and signed timestamps, explicit prune evidence, checksums, truncation, tags, UTF-8, lengths/counts and trailing-byte rejection pass within the **220/220** Entity regression sweep (`.test-tmp/lossless-entity-codec-replay-import-regressions.log`). Root format/check pass. Six-byte CVAENC family avoids the existing Echo namespace; the Echo migration regression passes.
Blockers: none for the creation codec; file-level proof authentication is a later gate.

### Step 94 execution record

Commit: `ace0a33`.
Files: `src/perception/entity_compact_codec.rs`; `entity_compact_values.rs`; `entity_compact_codec_tests.rs`.
Verification: Independent golden delta, all 64 mutable-field masks, optional UUID transitions, empty mask, nonzero publication coordinates, unknown mask/version and checksummed corruption cases pass in the same **220/220** run. Independent Luna framing review found no material defect. Candidate-dependent semantic validation remains in owner application.
Blockers: none for delta framing.

### Step 95 execution record

Commit: `ace0a33`.
Files: `src/perception/entity_compact_apply.rs`; `entity_compact_apply_tests.rs`; `entity_rebuild.rs`; `entity_compact_rebuild_tests.rs`; owner lifecycle callers and replay lookup.
Verification: Canonical candidate re-derivation, all 64 field combinations, UUID clear/replace, interleaved Entity IDs, retained historical revisions, legal earlier global tickets, marker-coordinate mismatch, missing base, redundant Set, mutation conflict and terminal no-resurrection pass in **220/220**. Published compact carriers reject under V1; unpublished valid carriers remain inert. Existing REL/PHY legacy behavior remains covered.
Blockers: none for ordinary compact reconstruction; proof-bearing admission requires Step 101 and is not certified by this record.

## Phase F — Memory Writer Cutover and Migration

| Step | Owner | Work | Gate |
| ---: | --- | --- | --- |
| 56 | Sol | Hard-cut ordinary compact Memory first-publication writer to creation record. | No legacy full-record write on compact files. |
| 57 | Sol | Hard-cut ordinary compact Memory revision writer to delta record. | Later revisions emit only compact deltas. |
| 58 | Sol | Preserve compact Memory tombstone semantics. | Terminal behavior matches V1.0. |
| 59 | Sol | Replace REL reconciliation Memory carrier decoding with validated owner publication lookup. | Standalone creation/delta histories reconcile with exact semantic revisions and transaction times. |
| 60 | Sol | Normalize completed-import Memory evidence from accepted owner records rather than legacy carrier bytes. | Equivalent legacy/compact histories produce identical normalized import evidence. |
| 61 | Sol | Classify compact Memory creation/delta frames in import interval admission. | Valid compact source frames reach owner replay; malformed/unknown compact versions fail closed. |
| 62 | Sol | Teach shared REL/PHY storage reclamation compact Memory carrier liveness and body closure. | Published creation/delta chains and immutable bodies survive; unversioned carriers reclaim conservatively without historical loss. |
| 63 | Sol | Update grouped Insomnia Memory creation encoding inside its existing atomic completion record. | Grouped publication atomicity/retry tests pass. |
| 64 | Sol | Update external Memory publication encoding inside its existing atomic publication record. | External publication/routing tests pass. |
| 65 | Sol | Migrate V1.0 standalone Memory first revisions to compact creation. | Creation parity passes. |
| 66 | Sol | Migrate V1.0 standalone later Memory revisions to compact deltas. | Revision parity passes. |
| 67 | Sol | Migrate V1.0 grouped Insomnia Memory publications. | Grouped histories preserve versions/IDs/bodies. |
| 68 | Sol | Migrate V1.0 external Memory publications. | External histories preserve owner references/routing. |
| 69 | Sol | Migrate V1.0 Memory tombstones. | Historical pre-tombstone revisions and terminal state match. |
| 70 | Luna | Add Memory adversarial migration fixtures for optional clears, supersession, archive/lifecycle changes, retained late-ticket coordinates, and malformed deltas. | All valid histories match; invalid ones reject. |
| 71 | Luna | Add Memory retry/conflict migration fixtures. | Retry semantics identical. |
| 72 | Sol | Freeze relocated legacy-import proof contract retaining the exact original CVAILEG1 receipt and binding a transformed prefix separately. | Original evidence is immutable; physical transport is explicit. |
| 73 | Sol | Implement bounded relocated legacy-import proof encoder. | Exact framing/checksum tests pass. |
| 74 | Sol | Implement strict relocated legacy-import proof decoder. | Malformed, unknown and truncated frames reject. |
| 75 | Sol | Validate relocated legacy-import proofs during owner replay without changing exposed historical evidence. | V1 rejects compact proof; V2 validates owner, prefix, marker and settlement. |
| 76 | Sol | Bind the transformed prefix during lossless migration after validating the original source proof. | Authoritative imported REL reopens with original evidence unchanged. |
| 77 | Luna | Add adversarial prefix, owner, marker, settlement, checksum and source-preservation fixtures for relocated proofs. | Invalid bindings fail closed; valid migration retains exact evidence. |
| 78 | Sol | Freeze read-only semantic-normalization preparation for relocated legacy Freshness cuts, retaining original proof bytes. | No source write or fabricated epoch/clock. |
| 79 | Sol | Extract existing normalization preparation from append/publish mechanics. | Ordinary normalization behavior and idempotence remain unchanged. |
| 80 | Sol | Prepare and retain the CVAFRS03 semantic successor before physical migration. | Old physical cut is never interpreted against moved bytes. |
| 81 | Sol | Prepare and retain the CVAFRS04 semantic successor before physical migration. | Epoch scope and predecessor evidence survive. |
| 82 | Sol | Prepare and retain the CVAFRS05 semantic successor before physical migration. | Scoped source/event proof survives. |
| 83 | Luna | Verify migrated normalization equivalence, no new semantic versions/times, source preservation and maintenance across legacy settlement variants. | Canonical historical Freshness fingerprints match. |
| 84 | Sol | Rebind transported legacy-import physical prefixes during reclamation, attachment/principal repack and prune copies. | Changed-prefix copies reopen without altering original receipt. |
| 85 | Sol | Include transported original import evidence in historical copy-equivalence validation. | Proof bytes remain compared; physical binding is not semantic authority. |
| 86 | Luna | Add migrated-owner maintenance regression with earlier dropped/changed payloads. | Reclaim/repack/prune preserve receipts, clocks and proof validation. |
| 87 | Sol | Measure raw Memory-history bytes before/after compact migration, excluding compression. | Benchmark artifact records zero semantic mismatch and byte delta. |

### Integration Gate C — Memory reference owner

Run Memory lifecycle/history, Insomnia, external publication, imports, rewrite/prune/reclaim interaction, reconciliation, and historical-oracle suites.

Accepted for implementation `9996c28`: root library/integration **1,800 passed / 0 failed / 5 existing ignores across 25 binaries**; final CLI **32/32**; compact Memory **38/38**; legacy proof/normalization/maintenance **6/6**; six authoritative Memory histories **zero mismatches**. Fresh Lexicon/Arcana/Pitlord enforcement passes **23 rules**. Policy interruption I1–I12 is complete. This gate does not accept canonical corpus promotion or runtime retirement.

---

## Phase G — Shared Delta Mechanics

| Step | Owner | Work | Gate |
| ---: | --- | --- | --- |
| 88 | Sol | Extract only the proven field-mask/optional-clear codec helpers from Memory. | Memory tests remain green. |
| 89 | Sol | Extract only the proven validate-before-apply helper pattern. | Memory behavior unchanged. |
| 90 | Sol | Document which semantics remain owner-specific and must not enter the helper module. | No generic semantic-owner abstraction is introduced. |

---

## Pre-Entity converter proof guard

| Step | Owner | Work | Gate |
| ---: | --- | --- | --- |
| 91 | Sol | Reject existing untranslated maintenance-history proof authority before temporary Memory staging returns an invalid owner. | Valid prepruned V1 input receives typed CVAHPR01 rejection; source/final destination stay unchanged and owned staging is cleaned up; supported migration tests pass. |

## Phase H — Entity Compact History

A reproduced preexisting-maintenance-proof case added fail-closed temporary-converter Step 91. Entity consumer inventory added eight explicit adapters: reconciliation, import evidence/admission, backing liveness, retained-structure evidence, shared proof admission and REL/PHY pruning. Original contract/codec/replay Steps 91–94 are now 92–95; original writer 95 is 102; original 96–266 move to 105–275. Accepted Steps 1–90 are unchanged. I11 remains the dated pre-refinement mapping with this crosswalk.


| Step | Owner | Work | Gate |
| ---: | --- | --- | --- |
| 92 | Sol | Freeze Entity creation/delta/terminal field contract. | Contract covers all current Entity revisions. |
| 93 | Sol | Implement Entity creation codec. | Round-trip/corruption tests pass. |
| 94 | Sol | Implement Entity delta codec. | Field/evidence delta tests pass. |
| 95 | Sol | Implement Entity delta application/rebuild. | Historical revisions reconstruct exactly. |
| 96 | Sol | Replace REL Entity reconciliation carrier decoding with validated owner-publication lookup for compact revisions and both terminal forms. | Tail parity passes; pruned unavailable state never becomes authored placeholder text; unsupported proof fails typed. |
| 97 | Sol | Normalize completed-import Entity evidence from accepted owner records independently of compact carrier bytes. | Legacy/current publication evidence matches, including optional global identity and explicit unavailable descriptions. |
| 98 | Sol | Admit strictly validated compact Entity carriers in import-interval replay without broadening existing terminal admission policy. | Valid creation/delta evidence reaches owner replay; malformed and unknown variants reject. |
| 99 | Sol | Update shared REL/PHY reclamation Entity backing liveness for compact chains, both terminal forms and pruned revision evidence. | Published history/proof survives; unversioned carriers remain inert. |
| 100 | Sol | Separate Entity retained-description structure evidence from persisted revision-carrier encoding. | Existing authenticated descriptor identities remain stable; optional identity and pruned unavailable state are explicit. |
| 101 | Sol | Update shared REL/PHY Entity pruned-description and owner-history proof readers for canonical compact carriers. | Exact manifest authorization and original-carrier digest validation remain enforced; unsupported source proof rejects explicitly. |
| 102 | Sol | Hard-cut compact Entity writers to creation+delta while keeping merge/retirement/tombstone explicit. | Compact file emits no legacy postimage revisions. |
| 103 | Sol | Update REL Entity description pruning to reconstruct, redact and re-encode compact revision chains with authenticated prune evidence. | REL historical unavailability, retained structure and prune-manifest validation match. |
| 104 | Sol | Update PHY Entity description pruning to reconstruct, redact and re-encode compact revision chains with authenticated prune evidence. | PHY historical unavailability, retained structure and prune-manifest validation match. |
| 105 | Sol | Extend upgrader for Entity creation/revisions/terminal records. | V1.0→compact Entity oracle parity passes. |
| 106 | Luna | Add Entity adversarial evidence/add/remove/retry/merge/tombstone fixtures. | Invalid histories fail closed; valid histories match. |
| 107 | Sol | Measure raw Entity-history bytes before/after. | Size artifact recorded. |

---


## Entity concrete-consumer and migration acceptance

Steps 93–95 are revalidated against `73e88ae` after the unreleased ordinary-frame checksum refinement: exact EOF ordinary framing, proof-only checksum and all reconstruction gates pass in the final 231-test sweep.

### Step 96 execution record

Commit: `73e88ae` (earlier consumer implementation `ace0a33`).
Files: src/facade/cva_reconcile_entity.rs; src/perception/entity_replay.rs; reconciliation Entity tests.
Verification: Owner-publication lookup preserves exact records, optional identity, terminal form and transaction times; unavailable descriptions stay typed. Final focused regression **231/231** (`.test-tmp/lossless-entity-final-focused.log`); authoritative parity **1/1**, six inputs, zero mismatches, **189.50 seconds** (`.test-tmp/lossless-entity-authoritative-final.log`). Root format/check and both documentation modes pass.
Blockers: none for this gate; full canonical promotion, source-proof translation for unsupported authorities and runtime retirement remain integrated final obligations.

### Step 97 execution record

Commit: `73e88ae` (earlier consumer implementation `ace0a33`).
Files: src/imports/reconciliation.rs; src/imports/reconciliation_entity_compact_tests.rs; Entity retained publication lookup.
Verification: Legacy and compact normalized accepted evidence matches, including UUID transitions and explicit unavailable descriptions. Final focused regression **231/231** (`.test-tmp/lossless-entity-final-focused.log`); authoritative parity **1/1**, six inputs, zero mismatches, **189.50 seconds** (`.test-tmp/lossless-entity-authoritative-final.log`). Root format/check and both documentation modes pass.
Blockers: none for this gate; full canonical promotion, source-proof translation for unsupported authorities and runtime retirement remain integrated final obligations.

### Step 98 execution record

Commit: `73e88ae` (earlier consumer implementation `ace0a33`).
Files: src/imports/reconciliation_interval.rs; reconciliation Entity compact tests.
Verification: Strict creation/delta classification admits valid compact publications; malformed masks/versions reject and existing terminal interval policy is preserved. Final focused regression **231/231** (`.test-tmp/lossless-entity-final-focused.log`); authoritative parity **1/1**, six inputs, zero mismatches, **189.50 seconds** (`.test-tmp/lossless-entity-authoritative-final.log`). Root format/check and both documentation modes pass.
Blockers: none for this gate; full canonical promotion, source-proof translation for unsupported authorities and runtime retirement remain integrated final obligations.

### Step 99 execution record

Commit: `73e88ae` (earlier consumer implementation `ace0a33`).
Files: src/facade/storage_reclamation_scan.rs; src/facade/storage_reclamation.rs; compact Entity reclamation tests.
Verification: Both owners retain published compact chains and direct/merge terminals, drop inert orphans and preserve histories, coordinates and times. Final focused regression **231/231** (`.test-tmp/lossless-entity-final-focused.log`); authoritative parity **1/1**, six inputs, zero mismatches, **189.50 seconds** (`.test-tmp/lossless-entity-authoritative-final.log`). Root format/check and both documentation modes pass.
Blockers: none for this gate; full canonical promotion, source-proof translation for unsupported authorities and runtime retirement remain integrated final obligations.

### Step 100 execution record

Commit: `73e88ae` (earlier consumer implementation `ace0a33`).
Files: src/perception/entity_retained_structure.rs; REL/PHY history-prune descriptor builders.
Verification: Legacy Some-UUID digest input remains byte-identical; None identity, exact ordering and unavailable descriptions are explicit and carrier-independent. Final focused regression **231/231** (`.test-tmp/lossless-entity-final-focused.log`); authoritative parity **1/1**, six inputs, zero mismatches, **189.50 seconds** (`.test-tmp/lossless-entity-authoritative-final.log`). Root format/check and both documentation modes pass.
Blockers: none for this gate; full canonical promotion, source-proof translation for unsupported authorities and runtime retirement remain integrated final obligations.

### Step 101 execution record

Commit: `73e88ae` (earlier consumer implementation `ace0a33`).
Files: src/perception/entity_description_proof.rs; src/facade/history_prune_manifest.rs; REL/PHY lifecycle and history-prune validators.
Verification: Proof-bearing checksums, exact manifest/identity/revision coverage and original-carrier/source-history bindings pass; forged, missing, duplicate and detached proofs reject before recovery/repair can mutate source. Final focused regression **231/231** (`.test-tmp/lossless-entity-final-focused.log`); authoritative parity **1/1**, six inputs, zero mismatches, **189.50 seconds** (`.test-tmp/lossless-entity-authoritative-final.log`). Root format/check and both documentation modes pass.
Blockers: none for this gate; full canonical promotion, source-proof translation for unsupported authorities and runtime retirement remain integrated final obligations.

### Step 102 execution record

Commit: `73e88ae` (earlier consumer implementation `ace0a33`).
Files: src/perception/entity_compact_publish.rs; entity_store.rs; entity_replay.rs; entity_compact_writer_tests.rs.
Verification: Ordinary and exact replay publication emit compact creation/delta carriers only. Retry/no-append/no-clock behavior and both explicit terminal forms survive reopen. Final focused regression **231/231** (`.test-tmp/lossless-entity-final-focused.log`); authoritative parity **1/1**, six inputs, zero mismatches, **189.50 seconds** (`.test-tmp/lossless-entity-authoritative-final.log`). Root format/check and both documentation modes pass.
Blockers: none for this gate; full canonical promotion, source-proof translation for unsupported authorities and runtime retirement remain integrated final obligations.

### Step 103 execution record

Commit: `73e88ae` (earlier consumer implementation `ace0a33`).
Files: src/perception/entity_description_prune.rs; src/facade/cva_description_prune.rs; cva_history_prune.rs; entity_compact_prune_tests.rs.
Verification: REL pruning rewrites the accepted redacted chain with authenticated evidence, removes selected plaintext orphans, preserves descriptors/times/identities and retains the existing net-shrink policy. Final focused regression **231/231** (`.test-tmp/lossless-entity-final-focused.log`); authoritative parity **1/1**, six inputs, zero mismatches, **189.50 seconds** (`.test-tmp/lossless-entity-authoritative-final.log`). Root format/check and both documentation modes pass.
Blockers: none for this gate; full canonical promotion, source-proof translation for unsupported authorities and runtime retirement remain integrated final obligations.

### Step 104 execution record

Commit: `73e88ae` (earlier consumer implementation `ace0a33`).
Files: src/perception/entity_description_prune.rs; src/facade/phylactery_history_prune.rs; entity_compact_prune_tests.rs.
Verification: PHY pruning preserves exact structural history and unavailable-description state; proof tamper rejects with source bytes unchanged. Final focused regression **231/231** (`.test-tmp/lossless-entity-final-focused.log`); authoritative parity **1/1**, six inputs, zero mismatches, **189.50 seconds** (`.test-tmp/lossless-entity-authoritative-final.log`). Root format/check and both documentation modes pass.
Blockers: none for this gate; full canonical promotion, source-proof translation for unsupported authorities and runtime retirement remain integrated final obligations.

### Step 105 execution record

Commit: `73e88ae` (earlier consumer implementation `ace0a33`).
Files: src/storage_upgrade/entity.rs; entity_tests.rs; entity_authoritative_tests.rs; memory.rs; shell.rs.
Verification: Private V1 Memory-and-Entity staging preserves all six authoritative owner histories at every Entity-local cut, terminal descriptors, identities/heads and global transaction times; original SHA-256/bytes/size/mtime unchanged. Final focused regression **231/231** (`.test-tmp/lossless-entity-final-focused.log`); authoritative parity **1/1**, six inputs, zero mismatches, **189.50 seconds** (`.test-tmp/lossless-entity-authoritative-final.log`). Root format/check and both documentation modes pass.
Blockers: none for this gate; full canonical promotion, source-proof translation for unsupported authorities and runtime retirement remain integrated final obligations.

### Step 106 execution record

Commit: `73e88ae` (earlier consumer implementation `ace0a33`).
Files: src/perception/entity_compact_codec_tests.rs; entity_compact_writer_tests.rs; entity_replay_retry_tests.rs; src/facade/entity_compact_prune_tests.rs; storage_upgrade/entity_tests.rs.
Verification: Adversarial fields/masks, exact UUID transitions, late tickets, older mutation retries, merge/direct terminal distinctions, no resurrection, orphan/missing-base and forged proof cases pass. Final focused regression **231/231** (`.test-tmp/lossless-entity-final-focused.log`); authoritative parity **1/1**, six inputs, zero mismatches, **189.50 seconds** (`.test-tmp/lossless-entity-authoritative-final.log`). Root format/check and both documentation modes pass.
Blockers: none for this gate; full canonical promotion, source-proof translation for unsupported authorities and runtime retirement remain integrated final obligations.

### Step 107 execution record

Commit: `73e88ae` (earlier consumer implementation `ace0a33`).
Files: benchmarks/lossless-entity-migration-step107-2026-10-19.json; src/storage_upgrade/entity_authoritative_tests.rs.
Verification: Raw Entity carriers total 322,375 to 301,910 bytes (-20,465), without compression; whole-file +6,248,686 bytes separately attributed to combined conversion/import-proof transport. No corpus promotion. Final focused regression **231/231** (`.test-tmp/lossless-entity-final-focused.log`); authoritative parity **1/1**, six inputs, zero mismatches, **189.50 seconds** (`.test-tmp/lossless-entity-authoritative-final.log`). Root format/check and both documentation modes pass.
Blockers: none for this gate; full canonical promotion, source-proof translation for unsupported authorities and runtime retirement remain integrated final obligations.

## Phase I — Relationship / Observation Compact History

| Step | Owner | Work | Gate |
| ---: | --- | --- | --- |
| 108 | Sol | Resolve the canonical Relationship-vs-Observation target at execution time. | One target recorded; obsolete writer becomes migration-only if superseded. |
| 109 | Sol | Freeze creation/delta/terminal contract for the canonical target. | Contract covers all legal revisions. |
| 110 | Sol | Implement creation codec. | Round-trip/corruption tests pass. |
| 111 | Sol | Implement delta codec including participant/evidence operations. | Canonical collection-delta tests pass. |
| 112 | Sol | Implement delta application/rebuild. | Historical revisions reconstruct exactly. |
| 113 | Sol | Replace REL Relationship reconciliation carrier decoding with accepted owner-publication lookup for records, terminals and typed unavailable summaries. | Tail replay preserves exact accepted history, interval order, coordinates and transaction times. |
| 114 | Sol | Normalize completed-import Relationship evidence from accepted owner state, omitting only destination-rebased publication coordinates. | Legacy and compact evidence matches for participants/roles, evidence, summary availability, mutation and timestamps. |
| 115 | Sol | Admit strict compact Relationship families through the existing import-interval classifier and owner replay. | Accepted publications replay once; orphan carriers stay inert and malformed/unknown variants reject. |
| 116 | Sol | Update shared REL/PHY reclamation backing classification and relocation for compact Relationship history. | Published chains and terminals survive with exact references/history; missing backing rejects. |
| 117 | Sol | Separate canonical Relationship retained-structure evidence from persisted carrier encoding. | Legacy descriptor hash inputs remain byte-identical; compact and legacy structure compare equal. |
| 118 | Sol | Add strict compact Relationship prune-proof extraction and shared owner-history proof admission. | Exact manifest/ID/revision and original-carrier/source-history binding is enforced before recovery or repair; metadata alone grants no authority. |
| 119 | Sol | Hard-cut compact writer to creation+delta. | No compact legacy postimage writer remains. |
| 120 | Sol | Update REL Relationship description pruning to redact and re-encode accepted compact chains with authenticated evidence. | Historical summary unavailability and retained descriptors match; malformed, incomplete and cross-target proofs reject without source mutation. |
| 121 | Sol | Update PHY Relationship description pruning with the same owner-specific carrier and proof helpers. | PHY historical summary unavailability, descriptors, times and identities match; forged proof rejects before output mutation. |
| 122 | Sol | Extend upgrader for the canonical owner and any supported legacy input shape. | V1.0→compact parity passes. |
| 123 | Luna | Add participant/evidence/retry/terminal adversarial fixtures. | Invalid histories fail closed. |
| 124 | Sol | Measure raw history bytes before/after. | Size artifact recorded. |

---


### Step 108 execution record

Commit: `73e88ae` inspected source; decision recorded in the following plan/contract milestone.
Files: `src/perception/relationship_store.rs`; `relationship_model.rs`; `relationship_rebuild.rs`; `src/rewrite/semantic_state.rs`; `src/semantic_graph/graph_store_validation.rs`.
Verification: Relationship remains the only persisted candidate owner. Observation is explicitly not a separate persisted store; Graph rejects Observation nodes. Compact history therefore targets the existing Relationship owner and its stable references. No competing durable Observation writer exists to retire. Independent Luna inventory confirms the five mutable fields and concrete raw-carrier consumers; the eight explicit adapter gates are inserted before their cutovers.
Blockers: none for canonical target selection; if a concurrent canonical-owner change lands, stop and explicitly replan that ownership boundary.


### Step 109 execution record

Commit: `0650305`.
Files: `src/perception/` relationship_compact_contract.rs; contract/adversarial tests; relationship_v1_characterization_tests.rs.
Verification: All five mutable fields, exact V1 text/roles/reference ordering, fresh-mutation zero mask, late tickets, nonmonotonic update time within bounds and explicit terminal/proof semantics are frozen. Genuine V1 REL/PHY fixtures reopen unchanged. Cohesive Relationship regression **52/52**, zero failures (`.test-tmp/lossless-relationship-codec-rebuild-tests-final.log`); root format/check and both documentation modes pass. The initial run found two test-fixture precondition errors, corrected without weakening production validation.
Blockers: none for this gate. Published compact pruning evidence remains deliberately fail-closed until Step 118 authenticates the file-level proof reader; ordinary writer cutover and migration remain pending.

### Step 110 execution record

Commit: `0650305`.
Files: `src/perception/` relationship_compact_codec.rs; relationship_compact_values.rs; codec tests.
Verification: Independent creation golden, exact optional roles and bounded lists/UTF-8, every proper-prefix truncation, tags, unknown versions, ordinary EOF and proof-only checksums pass. Cohesive Relationship regression **52/52**, zero failures (`.test-tmp/lossless-relationship-codec-rebuild-tests-final.log`); root format/check and both documentation modes pass. The initial run found two test-fixture precondition errors, corrected without weakening production validation.
Blockers: none for this gate. Published compact pruning evidence remains deliberately fail-closed until Step 118 authenticates the file-level proof reader; ordinary writer cutover and migration remain pending.

### Step 111 execution record

Commit: `0650305`.
Files: `src/perception/` relationship_compact_codec.rs; relationship_compact_values.rs; codec/contract tests.
Verification: All 32 masks, exact participant role remove/add, evidence changes, zero mask, sorted unique disjoint operations, malformed/empty operation encoding, count bounds and reserved-bit rejection pass. Cohesive Relationship regression **52/52**, zero failures (`.test-tmp/lossless-relationship-codec-rebuild-tests-final.log`); root format/check and both documentation modes pass. The initial run found two test-fixture precondition errors, corrected without weakening production validation.
Blockers: none for this gate. Published compact pruning evidence remains deliberately fail-closed until Step 118 authenticates the file-level proof reader; ordinary writer cutover and migration remain pending.

### Step 112 execution record

Commit: `0650305`.
Files: `src/perception/` relationship_compact_apply.rs; relationship_rebuild.rs; relationship_store_read.rs; REL/PHY lifecycle callers; application/rebuild/owner tests.
Verification: Pure canonical candidate reconstruction and both real owner reopen paths preserve exact revisions/fields/clocks/times. Missing bases, marker mismatch, redundant sets, mutation conflicts and terminal resurrection reject; V1 published compact rejects and orphan carriers stay inert. Cohesive Relationship regression **52/52**, zero failures (`.test-tmp/lossless-relationship-codec-rebuild-tests-final.log`); root format/check and both documentation modes pass. The initial run found two test-fixture precondition errors, corrected without weakening production validation.
Blockers: none for this gate. Published compact pruning evidence remains deliberately fail-closed until Step 118 authenticates the file-level proof reader; ordinary writer cutover and migration remain pending.

## Phase J — Ego Compact History

| Step | Owner | Work | Gate |
| ---: | --- | --- | --- |
| 125 | Sol | Inventory mutable vs immutable fields for Identity, Anchor, Personality, and synthesis records. | Field table complete. |
| 126 | Luna | Measure repeated Ego text/payload bytes to decide whether owner-local payload identity is warranted. | Measurement-backed yes/no decision recorded. |
| 127 | Sol | Define Ego creation/delta semantics using the Step 126 decision. | Contract tests cover activation/deletion/state changes. |
| 128 | Sol | Implement Ego creation codec. | Round-trip/corruption tests pass. |
| 129 | Sol | Implement Ego delta codec. | Field-delta tests pass. |
| 130 | Sol | Implement Ego replay/materialization. | Historical Ego states reconstruct exactly. |
| 131 | Sol | Adapt Ego historical fingerprint scans to owner-materialized compact publications. | Every retained Ego cut preserves canonical publication and state fingerprints, including legacy Default Identity normalization. |
| 132 | Sol | Cut fresh-history rewrite Ego emission over to compact creation/activation records. | REL/PHY rewrite outputs reopen with exact selected Ego state and no retired full-postimage writer. |
| 133 | Sol | Verify opaque Ego compact chains through repack, reclamation and native REL/PHY pruning. | Maintenance preserves all retained Ego publications and historical cuts. |
| 134 | Sol | Preserve explicit safe rejection of unsupported Ego reconciliation and import intervals. | Compact Ego families cannot bypass current owner guards or silently disappear from imported intervals. |
| 135 | Sol | Hard-cut compact Ego writers. | No compact full-postimage revision writer remains except justified immutable payload records. |
| 136 | Sol | Extend upgrader for Ego history. | REL/PHY Ego parity passes. |
| 137 | Luna | Add activation/deletion/ownership adversarial fixtures. | REL/PHY restrictions and terminal semantics remain exact. |
| 138 | Sol | Measure raw Ego-history bytes before/after. | Size artifact recorded. |

### Integration Gate D — Postimage owners

Run Entity, canonical Relationship/Observation, Ego, lifecycle, reconciliation, REL/PHY ownership, and historical-oracle suites.

---

## Phase K — Graph Encoding Audit and Optimization

Steps 141–155 are conditional on Step 140 approving compact Graph IDs. A measured rejection records an explicit defer for each dependent gate; it does not trigger an unnecessary codec or consumer rewrite.

| Step | Owner | Work | Gate |
| ---: | --- | --- | --- |
| 139 | Luna | Produce byte-field inventory for current Graph mutation/batch/version records. | Semantic vs framing/identity bytes classified. |
| 140 | Sol | Decide whether Graph compact-ID dictionary/ordinal encoding is worthwhile from measured fixtures. | Measurement-backed yes/no decision recorded. |
| 141 | Sol | If justified, define the Graph local-ID table format and validation rules. | Corrupt/missing mappings fail closed. |
| 142 | Sol | If justified, implement Graph compact-ID encoding/decoding behind Graph codec boundary. | Decoded canonical mutations match existing semantics. |
| 143 | Sol | Materialize compact Graph ordinals at accepted publication markers through the Graph owner; retain V1 read-only/V2 compact format admission. | Carrier→mapping→marker, inert orphans, invalid links, mixed origins and exact REL/PHY Graph clocks remain equivalent. |
| 144 | Sol | Adapt full-Graph and MemoryGraph historical fingerprint readers to owner-resolved compact publications. | Every cut, transaction time, baseline membership and qualifying MemoryGraph position remains exact. |
| 145 | Sol | Adapt REL Graph reconciliation tail/range reads to the source owner's local mappings. | Divergent ordinal collisions resolve to original stable IDs; retries, origins and rebased transaction times remain exact. |
| 146 | Sol | Adapt Freshness Graph/proof readers that inspect Graph carriers. | Existing recipient/effect/proof derivation uses identical stable semantic relations from compact publications. |
| 147 | Sol | Adapt completed-import Graph evidence and interval attribution. | Accepted source Graph mutations preserve canonical evidence, order and scope independently of target ordinals. |
| 148 | Sol | Admit strict compact Graph families in import replay interval classification. | Valid single/batch carriers pass; malformed and unknown versions cannot be ignored or silently dropped. |
| 149 | Sol | Define and integrate Graph-owned canonical source-history digest binding for compact ordinal carriers in native prune proof admission. | Rechecksummed mapping changes to other valid IDs fail for REL/PHY; retained-source proofs bind semantic endpoints rather than only ordinal bytes. |
| 150 | Sol | Extend physical reclamation backing classification for compact Graph carriers; retain marker reference relocation. | Published carriers survive, unmarked backing is inert/reclaimable and mapping order/history remains intact. |
| 151 | Sol | Adapt PHY semantic migration's raw Graph carrier consumer. | Target Graph relations use source stable IDs with exact origins and owner-local clocks. |
| 152 | Sol | Verify compact Graph chains and mappings through native pruning and repack/reclaim copies. | REL/PHY retained Graph and MemoryGraph cuts, proof bindings, baselines and transaction times survive maintenance. |
| 153 | Sol | Hard-cut ordinary Graph mutation/batch writers to compact ordinal carriers through the Graph owner. | No ordinary full-ID mutation writer remains; no-op, retry, lifecycle and fresh-history rewrite behavior remain equivalent. |
| 154 | Sol | Extend upgrader for compact Graph encoding if Step 142 is implemented. | Graph historical oracle parity passes. |
| 155 | Luna | Add Graph mapping corruption/order/admission regressions if compact IDs are implemented. | Invalid mappings fail closed. |

---

## Phase L — Freshness Effect History

| Step | Owner | Work | Gate |
| ---: | --- | --- | --- |
| 156 | Luna | Produce field-level inventory of Freshness event payload: causal effects, proof evidence, and derived postimages. | Exact classification recorded. |
| 157 | Sol | Define compact Freshness event format without derived postimages, preserving all causal/proof fields. | Codec contract can represent current accepted events. |
| 158 | Sol | Implement compact Freshness event encoder/decoder. | Round-trip/corruption tests pass. |
| 159 | Sol | Implement replay that recomputes postimages from prior state + effects. | Valid events reproduce current postimages. |
| 160 | Sol | Port current forged/malformed-event validation so removal of persisted postimages does not weaken checks. | Equal-or-stronger negative-test matrix passes. |
| 161 | Sol | Hard-cut compact Freshness event writer to the new effect-only representation. | Compact files emit no event postimages. |
| 162 | Sol | Extend upgrader for V1.0 Freshness events while preserving exact accepted event identity/cuts/proofs. | All historical Freshness cuts match. |
| 163 | Sol | Compare canonical accepted-event membership and immutable event facts in completed-import reconciliation through the Freshness owner; retain coordinate normalization and legacy scope reconstruction. | Missing/extra accepted identities and changed causal facts conflict; legitimate turn/Graph-proof rebasing remains equivalent. |
| 164 | Luna | Add import-settlement and same-cut Freshness migration regressions. | CVAFRS06 proof semantics remain unchanged. |
| 165 | Sol | Measure raw Freshness-history bytes before/after. | Size artifact recorded. |

---

## Phase M — Publication Framing

| Step | Owner | Work | Gate |
| ---: | --- | --- | --- |
| 166 | Luna | Measure current global-ticket + domain-link/version framing by owner on authoritative fixtures. | Per-owner framing-byte artifact exists. |
| 167 | Sol | Define compact publication envelope semantics without changing ADR 0003 ownership/clocks. | Format contract distinguishes global order, transaction time, local version, payload/reference. |
| 168 | Sol | Implement envelope codec/validation. | Round-trip/corruption tests pass. |
| 169 | Sol | Integrate envelope allocation/reserved-binding observation with Container clocks and partial-tail recovery. | Exact next allocation, prior standalone reservation/time, duplicate/future/grouped binding rejection and timestamp-cut tests pass. |
| 170 | Sol | Define and implement Memory-owned envelope creation/delta/terminal body codecs with global coordinates supplied by the envelope. | Literal/round-trip/corruption tests preserve local clocks, revisions, delta bases, stable IDs and terminal facts. |
| 171 | Sol | Admit Memory envelope bodies through MemoryOpenState without changing grouped publication authority. | Dense local ordering, prior reserved globals, body/reference validation, V1/V2 boundary and REL/PHY reopen tests pass. |
| 172 | Sol | Verify historical Memory/global projections and grouped-publication coexistence for envelopes. | Every synthetic Memory/global/timestamp cut and existing atomic group semantics match. |
| 173 | Sol | Migrate Memory reconciliation ranges, import coordinates/evidence and strict interval classification to envelopes. | Exact publication/time capture, completed-import comparison and deferred binding across import boundaries pass. |
| 174 | Sol | Bind Memory envelope facts into native prune source-history proofs. | Real REL/PHY prune outputs reopen; changed envelope/body authority rejects authenticated proofs. |
| 175 | Sol | Migrate Memory reclamation liveness and physical reference relocation to envelopes. | Referenced backing survives, inert orphan backing remains nonauthoritative, relocated publications reopen exactly. |
| 176 | Sol | Verify Memory envelope REL/PHY repack, native copies and semantic migration. | Stable IDs, historical publications, clocks/times and required references survive each concrete copy path. |
| 177 | Sol | Hard-cut ordinary Memory creation/delta/tombstone writers to allocation envelopes. | One allocation envelope per direct publication; no-op/retry consumes no bytes or clocks; crash/reopen tests pass. |
| 178 | Sol | Extend upgrader for Memory publication-envelope conversion. | V1.0 Memory global/local cut parity passes. |
| 179 | Sol | Freeze Entity envelope contract and inventory concrete consumers. | Owner fields, two terminal kinds, proof checksum and every affected consumer have explicit decisions. |
| 180 | Sol | Implement Entity envelope creation/delta/direct-terminal/merge-terminal body codecs. | Literal/round-trip/corruption tests preserve local clocks, UUIDs, delta bases and authenticated description proof. |
| 181 | Sol | Admit Entity envelope bodies through EntityOpenState. | Dense local order, reverse reserved globals, terminal chains and V1/V2 boundary pass. |
| 182 | Sol | Verify Entity envelope historical and global projections. | All synthetic local/global/time cuts and direct/merge terminal facts match marker-bound history. |
| 183 | Sol | Migrate Entity reconciliation ranges and import interval/evidence consumers. | Original binding slot/time, completed-import evidence, strict classification and deferred bindings across boundaries pass. |
| 184 | Sol | Bind Entity envelope authority into native prune source-history proofs. | REL/PHY prune outputs reopen; exact proof/descriptor checks and authority tamper rejection pass. |
| 185 | Sol | Migrate Entity envelope reclamation liveness and reference relocation. | Published authority/proof survives and orphan backing remains inert; relocated outputs reopen. |
| 186 | Sol | Verify Entity envelope native copies, repack, rewrite and owner-ID migration. | Copy histories/times remain exact; rewrite retains its explicit selection policy. |
| 187 | Sol | Hard-cut ordinary Entity revision, direct and merge terminal writers to allocation envelopes. | One envelope per publication, validation before allocation, byte-stable retries and incomplete-tail recovery pass. |
| 188 | Sol | Extend offline upgrader for Entity publication envelopes. | Accepted V1 carrier facts retain marker slots/global/local/time/proof parity; source identity/hash unchanged. |
| 189 | Sol | Freeze the Relationship envelope contract and concrete consumer inventory; record zero accepted links in the six pinned fixtures separately from synthetic runtime applicability. | Contract retains stable participant/evidence owner IDs, roles, local revision/base, mutation identity, description proof and the single direct terminal lifecycle; framing arithmetic is not claimed as fixture savings. |
| 190 | Sol | Implement Relationship-owned envelope creation/delta/direct-terminal body codecs. | Literal/round-trip/known-mask, role clear, participant/evidence, proof-checksum, malformed/unknown version and trailing-byte tests pass. |
| 191 | Sol | Admit Relationship envelopes through RelationshipOpenState and concrete REL/PHY dispatch. | Dense local order, reverse reserved globals/times, terminal/retry rules, invalid bases, V1 boundary and foreign-body rejection pass. |
| 192 | Sol | Verify Relationship envelope historical/global projections. | Matched carrier/link and envelope owners agree at every local/global/time cut, including routed references, changed roles/evidence, interleaved owners and terminal state. |
| 193 | Sol | Migrate Relationship reconciliation ranges and import interval/evidence consumers. | Original binding slots/times, finite ranges, duplicate decisions, actual divergent reconciliation and real completed-import normalized evidence agree; unsupported frames reject. |
| 194 | Sol | Bind Relationship envelope facts into native prune source-history authority. | Every-watermark digest parity, actual REL/PHY prune reopen, exact retained descriptors and rechecksummed authority tamper/missing/detached/duplicate-proof rejection pass. |
| 195 | Sol | Migrate Relationship envelope reclamation and physical relocation. | Accepted inline facts/proofs and stable evidence references survive, inert backing remains nonauthoritative, malformed/foreign bodies reject, source bytes stay unchanged. |
| 196 | Sol | Verify Relationship native copies, repack, rewrite and supported owner-ID migration. | Native copies preserve all retained local cuts/times/terminal evidence; semantic rewrite and owner-ID migration retain their explicitly supported policies and limitations. |
| 197 | Sol | Hard-cut ordinary Relationship creation/delta/direct-terminal writers to allocation envelopes. | One envelope per publication; validation precedes append; retries/no-ops consume no bytes or clocks; incomplete-tail/reopen tests pass for REL/PHY. |
| 198 | Sol | Extend the offline upgrader for Relationship envelopes. | Synthetic V1 reverse-ticket and terminal histories preserve original marker slots, all local/global/time cuts, source identity/hash and inert backing; six pinned fixtures report their actual zero Relationship population. |
| 199 | Sol | Record the Ego envelope non-applicability decision from its actual local-only framing. | No global ticket/link exists to remove; do not add an invented global clock or envelope/converter. Existing Ego parity remains unchanged. |
| 200 | Sol | Freeze Archive envelope applicability and concrete per-record consumer contract. | Inventory every versioned ArchiveRecord family and its readers; exclude unversioned content/format and grouped owners; specify local clock, self-reference policy and exact inert-backing admission before selecting cutover. |
| 201 | Sol | Implement Archive publication body codecs for the selected versioned record families. | Local clock and every current semantic record field round-trip; unknown families/versions, unversioned content/format, truncated/invalid values and trailing bytes reject. |
| 202 | Sol | Admit Archive envelopes through ArchiveOpenState and REL dispatch. | Exactly one local publication per envelope, dense/reversed reserved-clock cases, self-contained accepted backing, terminal closure and V1/foreign-body boundaries pass. |
| 203 | Sol | Migrate and verify Archive historical/current readers at envelope references. | Every synthetic local/global/time cut and every versioned record family matches carrier/link history; branches, metadata, tombstones, content and provenance stay exact. |
| 204 | Sol | Migrate Archive reconciliation/import ranges, receipts and completed-epoch evidence. | Original publication interval slots, accepted source mapping/receipt identity, grouped coexistence and strict unsupported-frame handling pass in real completed imports and divergent reconciliation. |
| 205 | Sol | Bind Archive envelope facts into native prune/authenticated source-history proofs. | Exact every-watermark authority, conversation terminal descriptors and retained payload/reference evidence survive actual prune/reopen; valid-field authority tampering rejects. |
| 206 | Sol | Migrate Archive envelope reclamation liveness and reference relocation. | All versioned record-family content dependencies survive; shared backing remains live and unmarked backing inert; every relocated ObjectRef resolves and outputs reopen with exact history. |
| 207 | Sol | Verify Archive envelope native copies, attachment repack and semantic rewrite/migration. | Native copies retain all Archive cuts/backing bytes/IDs/times; semantic operations retain their explicit selection and identity policies without inventing source authority. |
| 208 | Sol | Hard-cut the inventoried Archive versioned writers to allocation envelopes. | Every concrete publisher validates before one append; exact retries/no-ops and clocks remain unchanged; all record-family reopen and incomplete-final-envelope recovery cases pass. |
| 209 | Sol | Extend the offline upgrader for Archive envelopes and measure actual framing. | Each accepted V1 link is replaced at its original slot with BindReserved; exact cuts/times/backing/proof, source hashes and six-fixture per-family framed-byte accounting pass before promotion. |
| 210 | Sol | Freeze Graph envelope applicability and concrete single/batch publication contract. | Inventory mapping, reader and proof consumers; preserve authoritative node ordinals, ordered transaction grouping and local Graph/MemoryGraph semantics; specify body and fail-closed format boundary. |
| 211 | Sol | Implement Graph-owned single/batch publication body codecs. | Literal/round-trip tests retain local clock and ordered ordinal mutation facts; malformed counts, flags, kinds/origins, zero coordinates, unknown versions and trailing bytes reject. |
| 212 | Sol | Admit Graph envelopes through GraphOpenState and concrete REL/PHY dispatch. | Existing mapping authority resolves each transaction once; dense local clocks, reversed reserved globals, missing/future mappings, invalid batches, V1/bare/foreign bodies and mutation-before-error cases pass. |
| 213 | Sol | Migrate and verify Graph and MemoryGraph historical/current readers at envelope references. | Every synthetic local/global/time cut, inactive fact and batch projection matches carrier/link history with exact stable-ID mutation ordering and mapping identity. |
| 214 | Sol | Migrate Graph reconciliation/import ranges and completed-epoch evidence. | Each envelope remains one original interval transaction with exact batch membership/order and normalized evidence; duplicate/conflict and malformed/unsupported/missing-mapping cases pass. |
| 215 | Sol | Bind Graph envelope facts into native source-history and downstream semantic proof consumers. | Canonical every-watermark digest, Freshness/Community proof references and actual unrelated-owner prune/reopen remain exact; altered transaction authority rejects. |
| 216 | Sol | Migrate Graph envelope reclamation and reference relocation. | Accepted ordinal transactions retain required mappings; inert mutation backing is nonauthoritative; relocated outputs retain exact ordered facts and reject dangling/foreign bodies. |
| 217 | Sol | Verify Graph envelope native copies, repack, rewrite and supported owner-ID migration. | Native copies preserve every Graph/MemoryGraph cut, raw mappings, ordered relations and times; semantic operations prove observable relation parity under explicit remapping policies. |
| 218 | Sol | Hard-cut ordinary Graph single/batch writers to allocation envelopes. | One global allocation per transaction, never per member; full preflight and no-op/retry behavior, REL/PHY reopen and incomplete-final-envelope recovery pass. |
| 219 | Sol | Extend the offline upgrader for Graph envelopes and measure actual framing. | Accepted V1 marker slots become BindReserved publications without flattening batches; all local/global/time cuts, exact mapping/backing/proof parity and six-fixture framed-byte accounting pass. |
| 220 | Luna | Audit converter, consumer, writer and measured framing coverage for every selected envelope owner; perform no implementation in this audit. | Owner-local/global/time/proof parity is complete; any missing responsibility receives an explicit owner-specific step before Gate E, with subsequent rows renumbered. |

### Supplemental Graph authority gate discovered before Gate E acceptance

| Step | Owner | Work | Gate |
| ---: | --- | --- | --- |
| 221 | Sol | Preserve exact Graph transaction authority when replaying a new completed import epoch over independently accepted duplicate relations. | Partial and full member overlap retain original batch shape/order and normalized epoch evidence; reconciling the merged result with the source is idempotent. Ordinary merge deduplication outside completed epoch authority remains unchanged. |

### Step 221 execution record

Commit: 9ce3476.
Files: src/semantic_graph/graph_store_write.rs; src/facade/cva_reconcile_graph.rs; src/imports/reconciliation_interval.rs; src/imports/reconciliation_graph_envelope_tests.rs.
Verification: Partial/full-overlap regressions first failed normalized epoch evidence, then both passed after import-scoped preservation. Ordered batch shape, time, unchanged source/left bytes and retry reconciliation are asserted. Broad library revalidation passes **2,014 tests**, five existing ignored, excluding three already-passed exhaustive sweeps and three new Dream reference fixtures outside this Graph gate. Root test compilation and architecture enforcement pass.
Blockers: none for this gate; compact Dream, promotion and runtime retirement remain separate future gates.

### Integration Gate E — Graph, Freshness, and publication framing

Run Graph, Freshness, version/timestamp, crash/recovery, reclamation/reconciliation, and historical-oracle suites. Confirm no new cross-owner semantic coupling.

Accepted against 9ce3476: broad library and focused import regressions pass; the preceding exhaustive recovery, 157 integration, 32 CLI and 192 historical-oracle tests remain valid. Refreshed architecture snapshot 4cc4320fdda5709a82b7f7a751a7ee3853248b44fd445e468eb6782d7754f694 passes all23 rules; same-owner supplement enforcement also passes. No owner boundary changed.

---

## Phase N — Dream First-Pass History

| Step | Owner | Work | Gate |
| ---: | --- | --- | --- |
| 222 | Sol | Freeze exact legacy Dream population semantics at Begin. | Reference cases preserve all durable historical births, source-global comparison and source-cohort exclusion; archived/tombstoned IDs and reversed local/global orders remain explicit. |
| 223 | Sol | Define the Memory first-publication identity prefix at a Memory-local cut. | The contract excludes current-liveness filtering and distinguishes first revision, later revisions, tombstones and grouped creation ranges. |
| 224 | Sol | Implement Memory first-publication population reconstruction at that local cut. | Every-cut reference sets pass for standalone/grouped births, later revisions, terminal state and reserved global orders. |
| 225 | Sol | Define accepted birth/cohort evidence visibility at the physical Dream Begin prefix. | Existing source admission rules remain unchanged; later evidence cannot enter the captured prefix and no new persisted birth clock is invented. |
| 226 | Sol | Implement prefix projection for direct Intent/accepted-first-Memory, Consume and ReplayBirth evidence. | Only accepted direct birth facts visible before Begin enter; recoverable pending Intent plus accepted first Memory, duplicate/retry and delayed-proof cases match the current capture rule. |
| 227 | Sol | Implement prefix projection for accepted Insomnia completion birth/cohort evidence. | Legacy/current grouped completions retain exact member sets and visibility; malformed or unaccepted completion evidence grants no birth. |
| 228 | Sol | Implement prefix projection for accepted external-source birth/cohort evidence. | Both accepted external formats preserve source cohort membership and prefix visibility independently of derived indexes. |
| 229 | Sol | Add a read-only bounded Container payload scan exposing the exact accepted global frontier at each physical frame. | Reuse existing version observation; allocations, reserved bindings, grouped ranges, clipped boundaries and malformed clocks are verified without writes, tail repair or owner semantics. |
| 230 | Sol | Implement REL Dream Begin coordination over Memory and accepted birth/cohort prefixes. | Replay resolves the materialized population at the Begin binding point without using end-of-file evidence or making the journal own other stores. |
| 231 | Sol | Define and implement the canonical Dream population digest. | Sorted stable identity bytes, domain separation, empty population and mutation/order-independent reference tests pass. |
| 232 | Sol | Freeze compact-cut and explicit-set-exception Dream Begin wire contracts. | Both variants preserve the existing pass identity and exact set authority; cut/digest/count limits and unsupported versions are explicit. |
| 233 | Sol | Implement current Dream Begin encoding for compact cuts and explicit-set exceptions. | Literal golden encodings preserve exact fields; empty and nonempty exception sets remain distinct from compact-cut authority. |
| 234 | Sol | Implement current Dream Begin decoding for both population variants. | Round-trip, truncation, trailing bytes, malformed counts/tags and unsupported-version rejection pass independently of materialization. |
| 235 | Sol | Integrate Dream journal replay and materialization for both population variants. | InitialDreamPass retains its materialized set API; old CVDREAM1 Begin and later roots/settlement records retain their existing semantics. |
| 236 | Sol | Validate Dream roots and eligibility against the reconstructed or explicit population. | Existing first-pass source, root subset, duplicate/no-link and retry rules remain exact; forged compact authority fails closed. |
| 237 | Sol | Update historical Dream proof and fingerprint consumers for population variants. | Every retained cut exposes the same pass population, roots, accepted graph proof and settled state as the explicit reference history. |
| 238 | Sol | Update runtime Begin capture to use the same Memory and birth/cohort prefix rule. | Runtime and reopen populations agree; candidate ranking does not narrow the durable birth set and later evidence cannot expand a saved Begin. |
| 239 | Sol | Update Dream reconciliation to rebase population authority exactly. | Destination-prefix representability is proved before compact emission; an unequal source set uses the explicit exception rather than changing roots. |
| 240 | Sol | Update import normalization and replay for Dream population authority. | Completed-epoch evidence and replay preserve the exact source set; destination cuts and exceptions are normalized without accepting later birth facts. |
| 241 | Sol | Update fresh-history rewrite to preserve retained Dream population authority. | Changed Memory/global order and retained population either reproduce the source set or emit an explicit exception under the existing rewrite policy. |
| 242 | Sol | Hard-cut ordinary compact Dream Begin writing after all readers and destination writers are ready. | Current writes stop persisting full sets when the cut is exact; valid legacy/manual/rebased populations retain the explicit exception path. |
| 243 | Sol | Derive a representable Memory cut for each V1 Dream Begin at its physical evidence prefix. | Representable legacy sets derive deterministic cuts despite reserved global order; unsupported prefix authority is explicit. |
| 244 | Sol | Prove V1 reconstructed population equality against the retained explicit set. | Exact stable-ID set and digest equality is required; no approximate population or fabricated birth/cohort evidence is accepted. |
| 245 | Sol | Emit explicit-set exceptions for unrepresentable V1 Dream Begin populations. | Arbitrary valid legacy subsets retain exact authority using the already-admitted exception codec and replay path. |
| 246 | Sol | Preserve Dream prefix authority across copy, prune and reclamation transforms. | Transforms retaining prefix facts preserve cuts; removed/reordered birth/cohort/first-publication evidence requires proved exactness or explicit-set rewriting. |
| 247 | Luna | Add wrong-cut, forged-digest, source/root-outside-population and cohort-evidence adversarial tests. | Malformed or later evidence fails closed without semantic mutation; both population variants retain strict authority boundaries. |
| 248 | Luna | Add reversed-clock, tombstone and destination-exception regressions. | Local/global counterexamples, proof-before/after-Begin cases and reconcile/import/rewrite fallback histories remain exact. |
| 249 | Sol | Measure Dream first-pass raw bytes before and after on pinned and populated synthetic histories. | Per-family bytes, exception applicability, exact population/proof parity and source-integrity evidence are recorded without zero-corpus savings claims. |

### Step 222 execution record

Commit: `37210c8`.
Files: `src/facade/freshness_dream_history_tests.rs; docs/behavioral-contracts.md`.
Verification: Four reference tests pass through runtime Begin: archived/terminal historical births, reversed local/global order in both directions and accepted grouped-cohort exclusion. The candidate-after-source-local case proves the source-predecessor local cut is insufficient. No production Dream behavior changed.
Blockers: none for this reference gate; compact reconstruction and encoding remain later steps.

### Step 223 execution record

Commit: `24d1c53`.
Files: `docs/dream-population-contract-2026-10-21.md`.
Verification: Contract reviewed against MemoryStore retained records and standalone/grouped publication coordinates. Zero/future cuts, first identity/global facts, later revisions, terminal state and body pruning are explicit. Actual Dream population additionally requires source-global and physical-prefix birth/cohort authority.
Blockers: none for this contract gate; Step224 implements the reader.


### Step 224 execution record

Implementation: `1aaf6cd`; final fixtures: `21b4ec9`.
Files: `src/memory/memory_store.rs; src/memory/memory_publication_replay_tests.rs; src/memory/memory_publication_prune_tests.rs; src/external_source/tests.rs`.
Verification: All22 Memory compact replay/copy/prune tests and the accepted external publication fixture pass. Every local cut preserves first identity/global facts despite later revision, tombstone and reversed global order; future cuts reject. Two-member grouped completion, both-owner body pruning/reopen and external births retain prefix membership without body resolution.
Blockers: none for this reader gate; no ordinary Dream writer changed.

### Step 225 execution record

Files: `docs/dream-population-contract-2026-10-21.md`.
Verification: Contract reviewed against Dream Intent/Consume/ReplayBirth, lifecycle replay, grouped Insomnia and external birth/cohort validation. Physical Begin excludes later evidence while matching pending Intent plus already accepted first Memory retains existing recoverable birth semantics. Source admission remains unchanged; unsupported compact derivation requires the planned exact-set exception.
Blockers: none for this contract gate; concrete source projections remain226–228.


### Step 226 execution record

Commit: `ccc0c94`.
Files: `src/freshness_dream_journal.rs; src/facade/cva_freshness.rs; src/facade/freshness_dream_history_tests.rs`.
Verification: Five direct-prefix cases pass, plus all six Dream-journal and four runtime population references. Broad revalidation passes2,024 tests with five existing ignored and three previously passed exhaustive sweeps filtered. Dream owns ordered Intent/Consume/ReplayBirth facts; REL correlates them with accepted first Memory identity/mutation at a validated local cut. Recoverable pending Intent, delayed Consume, unknown/future IDs, owner mismatch, recognized truncation, duplicate markers and post-consume re-intent preserve exact evidence. Consumed mutations remain monotonic across legacy retries. Existing source admission and ordinary writers are unchanged.
Blockers: none for this projection gate; Insomnia/external projections and physical Begin scan coordination remain227–229.


### Step 227 execution record

Commit: `99e7f04`.
Files: `src/facade/cva_freshness.rs; src/facade/cva_lifecycle.rs; src/facade/cva_freshness_insomnia_prefix_tests.rs; src/runtime_inference/insomnia/completion_migration_tests.rs`.
Verification: Two current-format prefix/validation cases and legacy V6-proof/V5-no-proof coverage pass; broad library revalidation passes2,027 tests, five existing ignored and three separately passed exhaustive sweeps filtered. Reopen and prefix projection share the existing policy/activity/first-mutation checks. Exact cohort identities and original local/global coordinates survive zero/partial/full local cuts; no-proof completions grant no birth. No EOF cohort query, body resolution or Dream-journal ownership was introduced.
Blockers: none for this projection gate; physical Begin selection remains229.


### Step 228 execution record

Commit: `ad45319`.
Files: `src/facade/cva_freshness.rs; src/facade/cva_lifecycle.rs; src/facade/external_birth_prefix_tests.rs`.
Verification: The populated two-member CVAXMEM1/CVAXMEM2 prefix fixture passes format parity, zero/partial/full/future cuts, absent proof, wrong activity/first mutation and malformed hash cases. Broad revalidation passes2,028 tests, five existing ignored and three separately verified exhaustive sweeps filtered. Lifecycle replay and prefix projection share the exact existing external acceptance checks; no derived index or body lookup grants authority.
Blockers: none for this projection gate; physical selection remains the separate bounded scan229 and REL coordinator230.


### Step 229 execution record

Commit: `aeb7a43`.
Files: `src/container/container.rs; src/container/publication_envelope_clock_tests.rs`.
Verification: Three new bounded-reader cases and all17 envelope-clock tests pass. The visitor reuses existing physical version observation for allocations, reverse reserved bindings and grouped ranges, preserves live clocks/times and bytes, rejects malformed clocks and V1 envelopes, and reads a valid prefix before a later truncated tail without repair. Mid-frame and full truncated boundaries fail without changing source bytes. No owner semantics entered Container.
Blockers: none for this physical reader gate; REL Begin coordination is230.


### Step 230 execution record

Commit: `944231e`.
Files: `src/facade/cva_dream_population.rs; cva_dream_population_tests.rs; cva_freshness.rs; external_birth_prefix_tests.rs; src/memory/memory_rebuild.rs; src/lib.rs`; architecture, maintainer and population contract docs.
Verification: All eight population/cohort module cases pass; broad revalidation passes2,036 tests, five existing ignored and three separately verified exhaustive sweeps filtered; root test compilation passes. A reusable REL prefix state replays Memory-local metadata through Container's bounded versioned visitor, retains only direct/grouped/external birth/cohort facts, checks the exact cut and source identity, excludes source siblings and applies strict first-global ordering. Tests cover late ReplayBirth, reversed global/local order, archived/terminal births, actual grouped/external cohorts, wrong cut/source and unchanged bytes. Existing validation APIs moved out of the large Freshness facade without a compatibility bridge. No body resolution, end-of-file birth membership, new clock or writer change was introduced.
Blockers: none for this coordinator gate; compact codec/replay/cutover remain232–242 and refreshed integration policy enforcement remains Gate F.


### Step 231 execution record

Commit: `99a9e73`.
Files: `src/freshness/dream_population_digest.rs; src/lib.rs; docs/dream-population-contract-2026-10-21.md`.
Verification: Two digest cases pass, covering independent Python SHA-256 literal vectors for empty/single/two-member sets, insertion order, set deduplication and changed membership. Domain is ASCII `reliquary:dream-population:v1` plus one NUL byte, then u64 little-endian count and ascending raw IDs. Initial reference vectors used the wrong domain terminator; independent regeneration corrected the test vectors without changing production hashing. Owner/source/cut authority stays separate.
Blockers: none for this commitment gate; no wire writer was introduced.

### Step 232 execution record

Commit: `e79cda6`.
Files: `docs/dream-population-contract-2026-10-21.md`.
Verification: Field layout reviewed against the legacy journal, canonical source admission and bounded population resolver. Both CVDREAM2 variants preserve common authority, implicit first revision1, exact set semantics and strict framing. CompactCut has only cut/digest; ExplicitSet retains sorted raw IDs without duplicate cut/digest. Non-Begin journal records remain unchanged; current Begin is V2-only and decoder/materializer integration is separately gated.
Blockers: none for this wire contract gate; encoder233 and decoder234 remain unimplemented.


### Step 233 execution record

Commit: `17844a3`.
Files: `src/freshness/dream_begin_codec.rs; dream_begin_codec_tests.rs; src/lib.rs`.
Verification: Four encoder cases pass. Independent fixed byte fixtures cover CompactCut (118 bytes), nonempty ExplicitSet (147 bytes) and empty ExplicitSet (82 bytes), preserving exact owner/pass/source/turn/graph fields and sorted set order. Empty identity, zero cut and source-containing exception reject before emission. String/count/capacity arithmetic is checked and allocation failure returns an error. Decoder, file replay and ordinary writers remain unchanged.
Blockers: none for this encoder gate; decoder234 is next.



### Step 234 execution record

Commit: `f810554`.
Files: `src/freshness/dream_begin_codec.rs; dream_begin_codec_tests.rs; src/lib.rs`.
Verification: All nine codec tests pass, including every proper prefix of independent byte fixtures, full-width round trips, unknown variants, trailing bytes, invalid identity, zero cut, duplicate/out-of-order/source-containing IDs and forged maximum lengths. Counts and remaining bytes are checked before collection allocation. Population reconstruction and file admission remain Step235.
Blockers: none for this decoder gate.



### Step 235 execution record

Commit: `d43d460`.
Files: `src/facade/cva_dream_journal_replay.rs; cva_lifecycle.rs; cva_dream_population_tests.rs; src/freshness_dream_journal.rs; src/container/container.rs; src/lib.rs`; architecture, maintainer and population contract docs.
Verification: Ten focused prefix/replay tests and2,052 broad units pass, with five existing ignored and three previously verified exhaustive sweeps filtered. Compact Begin checks exact pre-frame Memory head, source presence and digest; ExplicitSet remains literal. Mixed legacy/current Roots and settlement replay, EOF pending-intent recovery, wrong cut/digest/owner, unknown versions and V1 rejection preserve source bytes. Legacy-only first-scan admission remains unchanged; current replay uses one additional ordered scan of the held container after source validation. Formatting, structural and documentation-impact checks pass.
Blockers: none for replay; historical proof and destination writers retain their explicit later gates. Architecture refresh remains at integration Gate F; owner boundaries and policy are unchanged.


### Step 236 execution record

Commit: `c320fc6`.
Files: `src/facade/dream_population_authority_tests.rs; src/facade/cva_dream_population.rs`.
Verification: Canonical runtime identity/eligibility, immutable root subset, source exclusion, no-link and exact retry tests pass for both variants. Forged cut/digest/owner and later-root attempts reject without appending.
Blockers: none for this item; canonical promotion, runtime retirement and integration Gate F remain separately gated.

### Step 237 execution record

Commit: `c320fc6`.
Files: `src/facade/cva_dream_journal_replay.rs; cva_historical_dream.rs; dream_population_proof_tests.rs; src/freshness_dream_journal.rs`.
Verification: Legacy, compact and explicit histories have equal fingerprints and completed proofs at Begin, Roots, acceptance and settlement. Completed proofs replay actual physical authority; bare settled markers remain insufficient. All three pinned REL proof fingerprints and completed maps agree after conversion.
Blockers: none for this item; canonical promotion, runtime retirement and integration Gate F remain separately gated.

### Step 238 execution record

Commit: `c320fc6`.
Files: `src/runtime_inference/dream_freshness.rs; src/facade/dream_population_authority_tests.rs`.
Verification: Runtime capture uses the same physical Memory/birth/cohort prefix as reopen. Unknown ranking candidates neither grant authority nor narrow the birth set; later publication and retry preserve the saved pass. Reversed local/global and grouped sibling reference tests pass.
Blockers: none for this item; canonical promotion, runtime retirement and integration Gate F remain separately gated.

### Step 239 execution record

Commit: `c320fc6`.
Files: `src/facade/cva_dream_begin_transport.rs; src/imports/reconciliation_interval.rs; reconcile_tests.rs`.
Verification: Source Begin materializes at its original frame. Destination encoding proves exact set equality before CompactCut; a destination with an extra birth emits ExplicitSet and preserves original roots/population. Current non-import reconciliation retains its existing settled-identity policy.
Blockers: none for this item; canonical promotion, runtime retirement and integration Gate F remain separately gated.

### Step 240 execution record

Commit: `c320fc6`.
Files: `src/imports/reconciliation.rs; reconciliation_interval.rs; src/facade/historical_import_base.rs; src/freshness_dream_journal.rs`.
Verification: Completed-epoch normalization materializes source authority before zeroing destination coordinates. DirectBirthPrefix isolates pre-import birth evidence from pass roots. Equal-base retries retain existing coverage; extra-recipient repeat remains an explicit ImportEpoch conflict under independent settlement authority, documented in current limitations.
Blockers: none for this item; canonical promotion, runtime retirement and integration Gate F remain separately gated.

### Step 241 execution record

Commit: `c320fc6`.
Files: `src/facade/dream_population_authority_tests.rs; src/rewrite/file.rs (inspected); docs/dream-population-contract-2026-10-21.md`.
Verification: Fresh-history rewrite retains its existing intentional pass-history discard policy: both selected Memories reopen in the new owner/generation with no obsolete Begin/cut carried across. Source bytes remain unchanged. No new rewrite retention policy was introduced.
Blockers: none for this item; canonical promotion, runtime retirement and integration Gate F remain separately gated.

### Step 242 execution record

Commit: `c320fc6`.
Files: `src/facade/cva_freshness.rs; cva_dream_begin_transport.rs; dream_population_proof_tests.rs; docs/storage-format.md`.
Verification: Ordinary Begin writes CVDREAM2 after existing eligibility/retry validation. Exact nonempty populations use cut/digest; empty and unequal literal populations use ExplicitSet. Roots/acceptance/settlement framing remains unchanged; legacy reference construction is explicitly test-only.
Blockers: none for this item; canonical promotion, runtime retirement and integration Gate F remain separately gated.

### Step 243 execution record

Commit: `c320fc6`.
Files: `src/storage_upgrade/dream_begin.rs; memory.rs; mod.rs`.
Verification: The temporary graph-envelope stage scans V1 in physical order, deriving each Begin Memory head from accepted prefix publications without comparing local/global clocks or using later birth evidence. It creates no new birth/cohort fact or global ticket.
Blockers: none for this item; canonical promotion, runtime retirement and integration Gate F remain separately gated.

### Step 244 execution record

Commit: `c320fc6`.
Files: `src/storage_upgrade/dream_begin.rs; src/facade/dream_population_authority_tests.rs; src/storage_upgrade/dream_begin_authoritative_tests.rs`.
Verification: Full stable-ID set equality precedes CompactCut emission and digest commitment. Synthetic and all114 pinned Begins retain exact Dream proof state; both hash passes agree with the accepted fixture manifest.
Blockers: none for this item; canonical promotion, runtime retirement and integration Gate F remain separately gated.

### Step 245 execution record

Commit: `c320fc6`.
Files: `src/storage_upgrade/dream_begin.rs; src/facade/dream_population_authority_tests.rs`.
Verification: Manual subset and proof-after-Begin migration fixtures emit literal exceptions with the exact original population. They reopen without fabricated cuts or earlier proof visibility; source bytes remain unchanged.
Blockers: none for this item; canonical promotion, runtime retirement and integration Gate F remain separately gated.

### Step 246 execution record

Commit: `c320fc6`.
Files: `src/facade/dream_population_authority_tests.rs; docs/dream-population-contract-2026-10-21.md`.
Verification: Native copy, terminal-root body pruning and subsequent reclamation reopen the same compact pending pass and full original population. Existing first-publication metadata and physical birth facts survive these transforms; source bytes remain unchanged.
Blockers: none for this item; canonical promotion, runtime retirement and integration Gate F remain separately gated.

### Step 247 execution record

Commit: `c320fc6`.
Files: `src/facade/cva_dream_population_tests.rs; dream_population_authority_tests.rs; cva_freshness_insomnia_prefix_tests.rs; external_birth_prefix_tests.rs; src/freshness/dream_begin_codec_tests.rs`.
Verification: Wrong local cut, forged digest/owner, absent source, later roots, source-as-root, invalid grouped/external evidence and malformed framing fail closed. Existing direct/Insomnia/external cohort authority is unchanged.
Blockers: none for this item; canonical promotion, runtime retirement and integration Gate F remain separately gated.

### Step 248 execution record

Commit: `c320fc6`.
Files: `src/facade/dream_population_authority_tests.rs; dream_population_proof_tests.rs; freshness_dream_history_tests.rs; src/imports/reconcile_tests.rs`.
Verification: Reversed clocks, archived/terminal births, same-cohort exclusions, physical proof-before/after-Begin, destination exceptions and fresh-history policy regressions pass. Existing strict completed-import recipient authority remains explicit.
Blockers: none for this item; canonical promotion, runtime retirement and integration Gate F remain separately gated.

### Step 249 execution record

Commit: `c320fc6`.
Files: `benchmarks/dream-begin-storage-2026-10-21.json; benchmarks/dream-begin-authoritative-upgrade-2026-10-21.json; src/storage_upgrade/dream_begin_authoritative_tests.rs`.
Verification: Pinned REL payload bytes:14-day1,278,432 to8,208 (36 compact);28-day2,814,072 to7,068 (31 compact); Ellis1,060,536 to10,716 (47 compact). Total5,153,040 to25,992, with zero exceptions, exact proof parity and unchanged hashes. PHY has zero Begins and receives no savings claim. Populated synthetic0/1/2/64 cases also round-trip; compression is excluded.
Blockers: none for this item; canonical promotion, runtime retirement and integration Gate F remain separately gated.


---

## Phase O — Dream Pair History

| Step | Owner | Work | Gate |
| ---: | --- | --- | --- |
| 250 | Sol | Define validated owner-local Memory-ID table for Dream pair physical encoding. | Table format/validation tests pass. |
| 251 | Sol | Implement compact Dream pair encoder/decoder using local references. | Decoded canonical pair set matches existing semantics. |
| 252 | Sol | Hard-cut compact Dream pair writer. | No compact full-ID pair record writer remains. |
| 253 | Sol | Extend upgrader for V1.0 Dream pairs. | Pair-set parity passes. |
| 254 | Luna | Add missing/corrupt mapping and retry regressions. | Invalid mappings fail closed. |
| 255 | Sol | Measure Dream-pair raw bytes before/after. | Size artifact recorded. |

### Step 250 execution record

Commit: `d3e48a6`.
Files: `src/runtime_inference/dream_pair_history.rs; dream_pair_codec.rs; docs/storage-format.md`.
Verification: The per-file table uses dense nonzero u32 physical references and one stable Memory ID per reference. Exact mapping retry is inert; conflicting bindings, aliases, gaps, missing references and unknown endpoints reject. No semantic clock or cross-owner numeric identity was introduced.
Blockers: none for this item; final corpus promotion and legacy decoder retirement remain separately gated.

### Step 251 execution record

Commit: `d3e48a6`.
Files: `src/runtime_inference/dream_pair_codec.rs; dream_pair_history.rs; dream_pair_history_tests.rs`.
Verification: Independent fixed wire vectors, recognized truncation/trailing bytes, zero/self references and unknown versions pass. Resolved IDs must be in strict canonical order even when numeric references are reversed. Semantic pair fingerprints and unordered retry semantics match the original set.
Blockers: none for this item; final corpus promotion and legacy decoder retirement remain separately gated.

### Step 252 execution record

Commit: `d3e48a6`.
Files: `src/runtime_inference/dream_pair_history.rs; src/facade/cva_lifecycle.rs; phylactery_lifecycle.rs`.
Verification: Both owner writers emit only CVADRM01 mappings and CVADRP02 pair references; exact pair retries append nothing and allocate no global ticket. Both-owner reopen and existing divergent pair-union tests pass. V1 inspection rejects current mapping/pair records.
Blockers: none for this item; final corpus promotion and legacy decoder retirement remain separately gated.

### Step 253 execution record

Commit: `d3e48a6`.
Files: `src/storage_upgrade/memory.rs; dream_pair_authoritative_tests.rs; src/runtime_inference/insomnia/completion_migration_tests.rs`.
Verification: The private graph-envelope stage transports legacy full-ID memberships through DreamPairStore in physical order. Synthetic REL/PHY and all six pinned conversions reopen with identical pair fingerprints and unchanged source hashes; all three REL Dream proof fingerprints remain equal. Legacy fixture construction restores actual old carriers before V1 reheadering.
Blockers: none for this item; final corpus promotion and legacy decoder retirement remain separately gated.

### Step 254 execution record

Commit: `d3e48a6`.
Files: `src/runtime_inference/dream_pair_history_tests.rs; src/facade/historical_dream_pair_fingerprint_tests.rs`.
Verification: All11 focused cases pass: missing/aliased/conflicting/forward mappings, immutable metadata binding, exact retry, genuine V1/current rejection, both-owner writer/reopen, retry after only the first mapping is durable, and terminal payload prune/reclaim with retained pair identities. Invalid input cannot mutate accepted membership.
Blockers: none for this item; final corpus promotion and legacy decoder retirement remain separately gated.

### Step 255 execution record

Commit: `d3e48a6`.
Files: `benchmarks/dream-pair-storage-2026-10-21.json; src/storage_upgrade/dream_pair_authoritative_tests.rs; src/runtime_inference/dream_pair_history_tests.rs`.
Verification: All six pinned sources are unchanged.36,108 pairs use4,316 mapping rows: payload2,599,776 to767,632 bytes; framed2,888,640 to1,091,024 bytes (62.23% saved including tables). A zero-pair PHY has zero overhead/savings. Synthetic two-endpoint one-pair history grows80 to128 framed bytes; four-/sixteen-endpoint complete histories shrink480 to352 and9,600 to3,712. Compression is excluded.
Blockers: none for this item; final corpus promotion and legacy decoder retirement remain separately gated.

### Integration Gate F — Dream

Run Dream/Freshness/import historical suites plus oracle comparison on Dream-bearing fixtures.

Gate F passes against Dream implementations `c320fc6`/`d3e48a6` and the stream
checkpoint `bdca9b6`. The fresh scoped refresh completed in2,160.54 seconds with
snapshot `ee25c7365267a2dfbcadda0c01b390e45933e102d00beb10bef9a76554d28ee1`: all23
rules pass,107,943 source nodes,3,216 relationships, zero compatibility warnings.
All20 production Rust paths changed by the Dream/stream commits match that
snapshot's source mirror at `bdca9b6` after LF/CRLF normalization; none are missing
or mismatched. The92 P4 and11 ownership fixtures plus production boundaries
also pass. Earlier15-minute timeouts are superseded by this completed refresh.
Subsequent derived-index changes require their own Gate G evidence and are not
claimed to be covered by this earlier snapshot.


---

## Phase P — Interaction Stream

| Step | Owner | Work | Gate |
| ---: | --- | --- | --- |
| 256 | Sol | Freeze interaction stream state-machine semantics from current cumulative records. | Begin/append/interrupted transition table exists. |
| 257 | Sol | Define `StreamBegin` compact record. | Codec tests pass. |
| 258 | Sol | Define `StreamAppend` exact UTF-8 suffix record. | Codec tests pass. |
| 259 | Sol | Define terminal/interrupted compact record. | Codec tests pass. |
| 260 | Sol | Implement compact stream replay/materialization. | Every checkpoint reconstructs exact visible text/status. |
| 261 | Sol | Hard-cut compact stream writer to begin/append/terminal records. | No cumulative-prefix compact writer remains. |
| 262 | Sol | Update stream reconciliation for compact records. | Latest-state reconciliation matches current behavior. |
| 263 | Sol | Extend upgrader from cumulative V1.0 checkpoints to compact stream deltas. | Historical checkpoint parity passes. |
| 264 | Luna | Add long-stream linear-growth benchmark. | Growth is linear in appended text. |
| 265 | Luna | Add crash/reopen/interruption regressions. | Displayed-text durability invariant remains intact. |

---

### Step 256 execution record

Commit: `bdca9b6`.

Verification: The frozen transition table preserves metadata, exact append-only UTF-8 text, Interrupted-to-Interrupted growth, irreversible interruption and Archive-owned completion. The compact stream contract documents acknowledgement and crash boundaries.

Blockers: none for this execution item; fresh Gate F/stream architecture enforcement passes against `ee25c736`; canonical promotion and runtime retirement remain separately gated.

### Step 257 execution record

Commit: `bdca9b6`.

Verification: Independent literal Begin bytes round-trip; strict parent/role flags, negative timestamp, exact EOF and every recognizable truncated prefix are tested. Metadata retries cannot reset text or resume interruption.

Blockers: none for this execution item; fresh Gate F/stream architecture enforcement passes against `ee25c736`; canonical promotion and runtime retirement remain separately gated.

### Step 258 execution record

Commit: `bdca9b6`.

Verification: Independent literal Append bytes round-trip. Exact UTF-8 suffixes use checked u32 prior-byte coordinates; empty suffixes, overflow, invalid UTF-8, forward/stale/conflicting bases reject without projection mutation.

Blockers: none for this execution item; fresh Gate F/stream architecture enforcement passes against `ee25c736`; canonical promotion and runtime retirement remain separately gated.

### Step 259 execution record

Commit: `bdca9b6`.

Verification: Independent Interrupted marker bytes round-trip. Exact content position is required, repeated markers are inert, and interruption preserves subsequent interrupted text extensions. There is no invented Complete stream event.

Blockers: none for this execution item; fresh Gate F/stream architecture enforcement passes against `ee25c736`; canonical promotion and runtime retirement remain separately gated.

### Step 260 execution record

Commit: `bdca9b6`.

Verification: Every complete compact frame prefix materializes exact text/status; Begin-before-Append/Interrupted ordering, metadata retries, append retries, conflicting bases and unknown versions are covered.

Blockers: none for this execution item; fresh Gate F/stream architecture enforcement passes against `ee25c736`; canonical promotion and runtime retirement remain separately gated.

### Step 261 execution record

Commit: `bdca9b6`.

Verification: Ordinary checkpoint writing emits only Begin, nonempty exact suffixes and necessary interruption markers. Identical checkpoints append nothing; invalid checkpoints preserve file bytes. No cumulative writer or semantic reservation remains in this owner.

Blockers: none for this execution item; fresh Gate F/stream architecture enforcement passes against `ee25c736`; canonical promotion and runtime retirement remain separately gated.

### Step 262 execution record

Commit: `bdca9b6`.

Verification: Destination reconciliation preserves longest compatible text plus interruption dominance through the current owner. Both source files remain byte-identical; existing divergent runtime reconciliation also passes.

Blockers: none for this execution item; fresh Gate F/stream architecture enforcement passes against `ee25c736`; canonical promotion and runtime retirement remain separately gated.

### Step 263 execution record

Commit: `bdca9b6`.

Verification: The latest private converter replays each old cumulative checkpoint through the compact owner. Four historical checkpoint postimages preserve exact text/status and source bytes. Genuine V1 fixture construction restores legacy carriers; current stream records reject in V1 and PHY.

Blockers: none for this execution item; fresh Gate F/stream architecture enforcement passes against `ee25c736`; canonical promotion and runtime retirement remain separately gated.

### Step 264 execution record

Commit: `bdca9b6`.

Verification: The raw long-stream artifact measures 256 two-byte UTF-8 appends plus interruption: legacy 257 frames/79,925 framed bytes, current 258 frames/9,809 bytes. Growth is one metadata record plus linear suffix and marker cost; compression is excluded.

Blockers: none for this execution item; fresh Gate F/stream architecture enforcement passes against `ee25c736`; canonical promotion and runtime retirement remain separately gated.

### Step 265 execution record

Commit: `bdca9b6`.

Verification: Every torn byte tail following acknowledged UTF-8 text reopens retaining that text; every complete frame prefix has the expected status. Existing displayed-text sync, recovered interruption and completed-turn supersession runtime regressions pass.

Blockers: none for this execution item; fresh Gate F/stream architecture enforcement passes against `ee25c736`; canonical promotion and runtime retirement remain separately gated.

---

## Phase Q — External Derived Index

| Step | Owner | Work | Gate |
| ---: | --- | --- | --- |
| 266 | Sol | Freeze the semantic boundary: `CVAXIDX1` is derived state; `CVAXMEM1` remains authoritative publication. | Boundary documented in tests/code comments. |
| 267 | Sol | Decide current-state vs incremental derived-index persistence based on rebuild/write behavior. | One strategy recorded. |
| 268 | Sol | Implement compact ExternalIndex codec for the selected strategy. | Round-trip/current-state tests pass. |
| 269 | Sol | Implement compact ExternalIndex rebuild/current projection. | Lookup behavior matches V1.0. |
| 270 | Sol | Make superseded derived index storage reclaimable without semantic prune markers. | Reclaim tests pass. |
| 271 | Sol | Extend upgrader to migrate only the current required derived index state. | External Memory publication history remains untouched. |
| 272 | Luna | Add damaged-index rebuild/discard regressions. | Existing recovery contract remains intact. |

### Step 266 execution record

Commit: `444481a`.

Verification: The concrete boundary is frozen in the derived-index contract and codec comment: legacy CVAXIDX1/current CVAXIDX2 are disposable cache state; existing CVAXMEM1/CVAXMEM2 Memory publication/evidence remains authority. No semantic history is removed.

Blockers: none for this item; dedicated fresh Gate G architecture enforcement is collecting, and canonical promotion/retirement remain separately gated.

### Step 267 execution record

Commit: `444481a`.

Verification: A self-contained binary current snapshot per namespace was selected from the existing full-state preparation/sync behavior. Writer revision CAS remains; current replay does not require a retained predecessor chain. The explicit discard barrier preserves missing-namespace rebuild behavior.

Blockers: none for this item; dedicated fresh Gate G architecture enforcement is collecting, and canonical promotion/retirement remain separately gated.

### Step 268 execution record

Commit: `444481a`.

Verification: Independent binary offsets, every truncated prefix, checksum/length/kind/mask/UTF-8/count errors and all four roles round-trip/reject correctly. Namespace/message references and contiguous chunk starts reconstruct exactly under existing full-state bounds.

Blockers: none for this item; dedicated fresh Gate G architecture enforcement is collecting, and canonical promotion/retirement remain separately gated.

### Step 269 execution record

Commit: `444481a`.

Verification: All18 external-source tests pass, including reopen, edits, visibility/reactivation/delete, feed gaps, stale preparations, cursor recovery and exact lookup results. Existing profile/vector backing validation remains unchanged.

Blockers: none for this item; dedicated fresh Gate G architecture enforcement is collecting, and canonical promotion/retirement remain separately gated.

### Step 270 execution record

Commit: `444481a`.

Verification: Native reclamation removes two superseded current snapshots and keeps the latest lookup/status, global head and external Memory/evidence. Multiple namespaces and global damage barriers survive; source bytes remain unchanged. Temporary legacy chains are retained conservatively.

Blockers: none for this item; dedicated fresh Gate G architecture enforcement is collecting, and canonical promotion/retirement remain separately gated.

### Step 271 execution record

Commit: `444481a`.

Verification: The private V1 converter emits only surviving current index state plus a required discard barrier. Three legacy snapshots become one current snapshot. Accepted external publication history follows the already-existing CVAXMEM1-to-CVAXMEM2 converter unchanged, with exact current carrier/evidence checks and immutable sources.

Blockers: none for this item; dedicated fresh Gate G architecture enforcement is collecting, and canonical promotion/retirement remain separately gated.

### Step 272 execution record

Commit: `444481a`.

Verification: Damaged index/checksum/body/flags/counts/UTF-8, backing loss, discard/rebuild, stale CAS and interrupted preparation regressions pass. A damaged legacy derived tail converts to a rebuild barrier while accepted Memory authority stays readable; current index records reject in V1 and PHY.

Blockers: none for this item; dedicated fresh Gate G architecture enforcement is collecting, and canonical promotion/retirement remain separately gated.

### Integration Gate G — Operational/derived stores

Run interaction runtime, external-source index/publication, reconciliation, reclaim, and reopen suites.

---

## Phase R — Entity Resolution History

| Step | Owner | Work | Gate |
| ---: | --- | --- | --- |
| 273 | Luna | Classify every Entity Resolution persisted field as semantic proof, retry state, or reconstructible postimage/candidate material. | Field table complete. |
| 274 | Sol | Freeze compact Entity Resolution history contract from that classification. | Contract preserves every authoritative field. |
| 275 | Sol | Define initial candidate/evidence record. | Codec tests pass. |
| 276 | Sol | Define attempt/status delta record. | Codec tests pass. |
| 277 | Sol | Implement replay/materialization. | Accepted decisions/retries reconstruct exactly. |
| 278 | Sol | Hard-cut compact Entity Resolution writer. | No compact repeated full candidate snapshot remains unless classified authoritative. |
| 279 | Sol | Materialize Entity Resolution source postimages before normalized import comparison. | Equal histories have identical normalized evidence across full and delta carriers; source bytes remain unchanged. |
| 280 | Sol | Transport materialized Entity Resolution postimages through completed-import interval replay. | Destination deltas use destination prefix state; replay preserves source decisions/retries and existing conflicts. |
| 281 | Sol | Extend upgrader for V1.0 Entity Resolution history. | Decision/proof parity passes. |
| 282 | Luna | Add retry/conflict/import-readiness/adversarial resolution fixtures. | Existing Entity effects and readiness decisions remain identical. |
| 283 | Sol | Measure raw Entity Resolution history before/after. | Size artifact recorded. |

---

## Phase S — Community Proof Boundary

| Step | Owner | Work | Gate |
| ---: | --- | --- | --- |
| 284 | Luna | Identify every accepted proof that references Community generation/graph version/name. | Consumer inventory complete. |
| 285 | Sol | Define the exact reconstruction claim required to eliminate repeated membership snapshots. | Claim names graph cut + algorithm/policy version + any non-deterministic retained data. |
| 286 | Luna | Test that claim against authoritative/synthetic Community histories. | Exact yes/no evidence recorded. |
| 287 | Sol | If exact reconstruction is proven, define compact Community membership history; otherwise record explicit no-change decision. | One decision closed. |
| 288 | Sol | If compacting, implement Community codec/replay/writer. | Historical proof identity remains valid. |
| 289 | Sol | If compacting, extend upgrader. | Community oracle/proof parity passes. |
| 290 | Luna | If compacting, add semantic-name/non-determinism regressions. | Non-recomputable names/evidence remain retained. |
| 291 | Sol | If compacting, measure raw Community history before/after; otherwise mark measurement not required. | Evidence recorded. |

### Integration Gate H — Proof-boundary stores

Run Entity Resolution, Community, Dream/Freshness proof consumers, import readiness, and historical-oracle suites.

---

## Phase T — Archive Vector Generation Decision

| Step | Owner | Work | Gate |
| ---: | --- | --- | --- |
| 292 | Luna | Build repeated same-profile Archive-vector-generation workload. | Reproducible workload exists. |
| 293 | Luna | Measure unchanged exact vector-byte duplication across generations separately from compatibility-profile multiplicity. | Byte-duplication artifact exists. |
| 294 | Sol | Decide whether row/block reuse is justified by Step 293. | Explicit implement/defer decision recorded. |
| 295 | Sol | If justified, define immutable exact vector row/block identity preserving byte-distinct compatible embeddings. | Identity contract tests pass. |
| 296 | Sol | If justified, implement reusable row/block storage. | Exact vector bytes round-trip. |
| 297 | Sol | If justified, implement generation manifest/delta referencing row/block identities. | Historical generation materialization exact. |
| 298 | Sol | If justified, hard-cut compact Archive-vector generation writer. | No redundant full-matrix compact writer remains. |
| 299 | Sol | If justified, extend upgrader for historical generations. | Exact vector-byte/generation parity passes. |
| 300 | Luna | If justified, add retrieval and corruption regressions. | Retrieval parity and fail-closed behavior pass. |
| 301 | Sol | If justified, measure raw vector-generation bytes before/after; otherwise mark phase deferred. | Evidence recorded. |

### Integration Gate I — Vectors

Run profile, packed-vector, Memory-vector, Archive-vector, generation, retrieval, prune/reclaim, and rewrite-interaction suites.

---

## Phase U — Upgrade Coverage Completion

| Step | Owner | Work | Gate |
| ---: | --- | --- | --- |
| 302 | Luna | Re-run durable-family inventory against current source after all codec migrations. | No new/unhandled writer family appears. |
| 303 | Luna | Audit upgrade coverage for Archive-owned durable families after all earlier migrations. This step performs no implementation. | Either Archive coverage is complete or this plan is expanded with one explicit step per missing family before execution continues. |
| 304 | Luna | Audit upgrade coverage for import epoch and Historical Freshness settlement families. This step performs no implementation. | Either import-evidence coverage is complete or explicit per-family steps are inserted before execution continues. |
| 305 | Sol | Add/verify upgrader support for Echo records only. | Echo historical evidence parity passes. |
| 306 | Sol | Add/verify upgrader support for project-history correlation records only. | Project-history evidence parity passes. |
| 307 | Sol | Reject unknown/unsupported semantic authority explicitly during upgrade. | Unknown authority fails closed. |
| 308 | Sol | Freeze the discovered retained V1 CVAERS02 family contract against frozen-reader behavior and exact corpus byte inventory. | Older records remain inert; every original byte including unexplained trailing fields is retained; no semantic decision is reactivated. |
| 309 | Sol | Reserve CVAERS03 for current Entity Resolution deltas and update its codec/writer/comparison consumers and contract. | Current and retained V1 identities cannot collide; existing exact postimage vectors revalidate. |
| 310 | Sol | Define a concrete bounded checksummed retained-inert Entity Resolution evidence carrier. | Original CVAERS02 bytes round-trip exactly; malformed framing/hash/family reject. |
| 311 | Sol | Integrate retained-inert evidence validation into EntityResolutionStore replay without changing its visible decisions/revisions. | Both retained and historical source records have identical zero projection effect. |
| 312 | Sol | Convert retained V1 CVAERS02 bytes to that exact evidence carrier at their original physical slots. | Ordered original bytes and global coordinates survive; input bytes and timestamps remain unchanged. |
| 313 | Sol | Enforce resolution-family admission against actual container format for generic and explicit REL/PHY opens. | Raw retired V1 evidence, current deltas and retained current evidence cannot cross physical format routes. |
| 314 | Sol | Revalidate collision, import comparison/transport, readiness, reclaim and retained-corpus history with the corrected families. | Frozen semantic decisions remain unchanged and retained bytes remain exact; complete proof-store gate passes. |
| 315 | Sol | Convert each source-validated physical Historical Freshness settlement into standalone CVAFRS06 at its first accepted settlement slot; omit duplicate normalization carriers. | Semantic scope, every accepted record/event and historical cut remain exact; no predecessor digest remains. |
| 316 | Sol | Remove source-validated CVAILEG1/2 physical-prefix bridges from latest canonical output. | Initial semantic proof validates without either legacy epoch carrier; malformed source evidence still rejects before output. |
| 317 | Sol | Rebind only CVAIEPC1 derived settlement-byte digests to their validated canonical settlement. | Literal start, receipt, timestamps, watermarks and completion facts survive; completion replay and retry pass. |
| 318 | Sol | Verify standalone import-proof conversion for initial and native completed epochs, duplicate normalizations and damaged source proofs. | Exact historical Freshness parity, admission, readiness and source immutability pass before corpus promotion. |
| 319 | Sol | Reopen upgraded REL and validate all owner invariants. | REL validation passes. |
| 320 | Sol | Reopen upgraded PHY and validate all owner invariants. | PHY validation passes. |

---

## Phase V — Authoritative Fixture Migration (offline-only)

| Step | Owner | Work | Gate |
| ---: | --- | --- | --- |
| 321 | Luna | Upgrade disposable copy of 14-day REL. | Destination reopens; source hash unchanged. |
| 322 | Luna | Upgrade disposable copy of 14-day PHY. | Destination reopens; source hash unchanged. |
| 323 | Luna | Upgrade disposable copy of 28-day REL. | Destination reopens; source hash unchanged. |
| 324 | Luna | Upgrade disposable copy of 28-day PHY. | Destination reopens; source hash unchanged. |
| 325 | Luna | Upgrade disposable copy of Ellis REL. | Destination reopens; source hash unchanged. |
| 326 | Luna | Upgrade disposable copy of Ellis PHY. | Destination reopens; source hash unchanged. |
| 327 | Luna | Produce per-family raw-byte accounting for all six migrations. | Logical-history size report exists; compression excluded. |

---

## Phase W — Historical Equivalence Verification

| Step | Owner | Work | Gate |
| ---: | --- | --- | --- |
| 328 | Sol | Exhaustively compare Memory historical cuts source vs compact fixtures. | Zero mismatches. |
| 329 | Sol | Exhaustively compare Archive historical cuts where tractable. | Zero mismatches. |
| 330 | Sol | Exhaustively compare Entity and Relationship/Observation cuts. | Zero mismatches. |
| 331 | Sol | Exhaustively compare Graph cuts. | Zero mismatches. |
| 332 | Sol | Exhaustively compare Ego cuts. | Zero mismatches. |
| 333 | Sol | Exhaustively compare Freshness activity/event cuts. | Zero mismatches. |
| 334 | Sol | Compare vector-generation cuts and exact vector bytes. | Zero mismatches. |
| 335 | Sol | Compare Dream proof state. | Zero mismatches. |
| 336 | Sol | Compare global ordering points exhaustively where tractable and deterministically sample only where documented runtime makes exhaustive comparison unreasonable. | Zero mismatches; sampling rationale recorded if used. |

---

## Phase X — Maintenance and Reconciliation Verification

| Step | Owner | Work | Gate |
| ---: | --- | --- | --- |

| 337 | Sol | Verify compact-to-compact reconciliation against the frozen V1 semantic decision oracle. | Current-only reconciliation agrees with recorded baseline; no dual-format runtime path. |

| 338 | Sol | Run compact REL prune tests. | Maintenance behavior unchanged. |
| 339 | Sol | Run compact PHY prune tests. | Maintenance behavior unchanged. |
| 340 | Sol | Run compact reclaim tests. | Reclamation preserves semantic history. |
| 341 | Sol | Run compact repair tests. | Repair behavior unchanged. |
| 342 | Sol | Run fresh-history rewrite from compact sources. | Rewrite semantics unchanged. |
| 343 | Sol | Reopen all maintenance outputs and rerun relevant oracle checks. | Zero semantic mismatch beyond explicit maintenance policy. |

---

## Phase Y — Performance and Checkpoint Decision

| Step | Owner | Work | Gate |
| ---: | --- | --- | --- |
| 344 | Luna | Measure cold reopen old vs compact fixtures. | Results recorded. |
| 345 | Luna | Measure warm reopen old vs compact fixtures. | Results recorded. |
| 346 | Luna | Measure recent, middle and oldest historical lookup on frozen V1 and canonical V2 fixtures. | Three distinct cut-age metrics recorded; no runtime V1 reader retained. |


| 347 | Luna | Measure upgrade throughput. | Results recorded. |
| 348 | Luna | Measure peak migration memory. | Results recorded. |
| 349 | Sol | Decide whether replay cost justifies derived checkpoints. | Implement/defer decision recorded from measurements. |
| 350 | Sol | If justified, specify checkpoint semantics under `version-history-plan.md`. | Checkpoint is explicitly disposable/non-authoritative. |
| 351 | Sol | If justified, implement checkpoint generation/load path. | Deleting checkpoints and rebuilding yields identical state. |
| 352 | Sol | If justified, add checkpoint corruption/rebuild tests. | Corruption cannot alter authority. |

---

**Mandatory dependency:** finish canonical owner converters, verify historical equivalence, promote the retained development corpus, then delete legacy readers and one-time converters; rerun complete acceptance after deletion. These gates replace the appended R1–R22 plan rather than adding another workstream.

## Phase Z — Architecture Closeout

| Step | Owner | Work | Gate |
| ---: | --- | --- | --- |
| 353 | Sol | Write compact-history ADR capturing only hard-to-reverse decisions. | ADR reviewed against implementation. |
| 354 | Sol | Update ADR index. | Navigation correct. |
| 355 | Luna | Update `storage-format.md` with compact record families and migration boundary. | Format doc matches source. |
| 356 | Luna | Update `architecture.md` and `invariants.md`. | Ownership/history invariants current. |
| 357 | Luna | Update `current-limitations.md` and `maintainer-map.md`. | Operational limitations/navigation current. |
| 358 | Luna | Update affected API/development docs and docs index. | Documentation compliance passes. |
| 359 | Luna | Refresh Lexicon/Arcana from final source. | Fresh snapshot available or explicit bounded limitation recorded. |
| 360 | Sol | Promote all verified canonical V2 development fixtures; update SHA-256 manifests and commit fixture repository. | All retained inputs migrated and validated before promotion; hash and commit recorded. |
| 361 | Sol | Remove V1 runtime container reader and dispatch. | Only V2 opens normally; old headers yield typed unsupported error. |
| 362 | Sol | Remove CVAFRS03/04/05 normalization and CVAILEG1/2 epoch/prefix runtime compatibility. | No reachable legacy codec; CVAFRS06 semantic proof remains valid. |
| 363 | Sol | Remove temporary upgrader APIs, CLI commands, translation codecs and obsolete tests after promotion. | No migration-only code reachable; negative tests retained. |
| 364 | Sol | Entity: prune selected bare current creation/delta bodies; replace retired-format orphan fixtures with current bodies and verify ownership, clocks and proofs. | Owner-specific current maintenance acceptance passes. |
| 365 | Sol | Relationship: prune selected bare current creation/delta bodies; replace retired-format orphan fixtures with current bodies and verify ownership, clocks and proofs. | Owner-specific current maintenance acceptance passes. |
| 366 | Sol | Memory: reclaim only unpublished current native bodies; replace retired-format orphan fixtures with current bodies and verify ownership, clocks and proofs. | Owner-specific current maintenance acceptance passes. |
| 367 | Sol | Entity: reclaim only unpublished current native bodies; replace retired-format orphan fixtures with current bodies and verify ownership, clocks and proofs. | Owner-specific current maintenance acceptance passes. |
| 368 | Sol | Relationship: reclaim only unpublished current native bodies; replace retired-format orphan fixtures with current bodies and verify ownership, clocks and proofs. | Owner-specific current maintenance acceptance passes. |
| 369 | Sol | Archive: reclaim only unpublished current native bodies; replace retired-format orphan fixtures with current bodies and verify ownership, clocks and proofs. | Owner-specific current maintenance acceptance passes. |
| 370 | Sol | Graph: reclaim only unpublished current native bodies; replace retired-format orphan fixtures with current bodies and verify ownership, clocks and proofs. | Owner-specific current maintenance acceptance passes. |
| 371 | Sol | Audit legacy references/mixed writes, verify owner boundaries and rerun root, CLI, history, prune/rewrite, reconcile, Ellis, structural and docs acceptance. | Current-format-only post-retirement verification passes. |

### Final Integration Gate

Run:

- formatting;
- compile/check;
- full applicable library tests;
- integration tests;
- CLI tests;
- authoritative fixture upgrade/reopen;
- historical equivalence matrix;
- reconciliation/maintenance matrix;
- performance/size evidence.

Gate passes with zero task-related failures or with precisely documented environmental limits that do not hide correctness failures.

---

## Final Closeout Steps

| Step | Owner | Work | Gate |
| ---: | --- | --- | --- |
| 372 | Sol | Record final implementation commits and changed-file inventory in this plan. | Execution record complete. |
| 373 | Sol | Record actual raw logical-history savings by family and by fixture. | Logical savings report complete. |
| 374 | Sol | Record ADR-0004 compression results separately from logical-history savings. | No conflation of compression and logical compaction. |
| 375 | Sol | Record every measurement-gated defer decision (Community/vector/checkpoints/etc.). | Deferred work has explicit evidence/reason. |
| 376 | Sol | Update spec status to match shipped architecture and remaining limits. | Spec and implementation agree. |
| 377 | Sol | Mark complete only after canonical fixture promotion, legacy decoder and upgrader deletion, and post-retirement acceptance. | Final hard-cut acceptance complete. |

## Integration Gates

This plan intentionally separates implementation steps from integration gates:

- Gate A — historical oracle
- Gate B — format boundary/upgrader shell
- Gate C — Memory reference owner
- Gate D — remaining postimage owners
- Gate E — Graph/Freshness/publication framing
- Gate F — Dream
- Gate G — operational/derived stores
- Gate H — proof-boundary stores
- Gate I — vectors
- Final gate — complete repository acceptance

A gate may execute many tests, but it is not an implementation step and does not hide unfinished work.

## Verification Priorities

Repeatedly verify:

1. historical state equality, not just current-state equality;
2. stable semantic identity across upgrade;
3. exact owner-local and global version coordinates;
4. transaction-time preservation;
5. crash/retry atomicity;
6. reconciliation equivalence;
7. no implicit history deletion;
8. no new global semantic ancestry;
9. no derived/recovery state accidentally elevated to authority;
10. logical-history savings reported independently from compression.

## Related docs

- [Refactor specification](minimal-lossless-history-storage-spec-2026-10-19.md)
- [Storage format](storage-format.md)
- [Versioning and historical cuts](version-history-plan.md)

## Notes

The numbered tables are an acceptance ledger. They make unfinished responsibilities visible; they do not prescribe how many implementation passes, agents, commits, or test invocations are required.

The unnumbered integration gates are broad acceptance checkpoints and natural batch boundaries.

## Tracking

An earlier decomposition contained **294 execution steps**; the later execution crosswalks below record subsequent owner-specific expansions. The initial 243-step expansion split the previous 74-step version into independent schema, codec, replay, writer, migration, proof, fixture, and measurement responsibilities. Execution discovery added explicit Dream pair-membership and processing-eligibility oracle steps before whole-owner integration; they are separate from accepted first-pass proof and must not be absorbed into the whole-owner gate. V1 open-path discovery added separate conversation-compaction and Entity-resolution read-only projection checks before owner lifecycle cutover; their physical reclamation and durable revision-repair failure domains require independent evidence.

Standalone Memory writer cutover exposed four legacy-carrier readers with separate owners/failure domains: REL reconciliation, completed-import evidence, import interval admission and shared physical reclamation. These are explicit Steps 59-62; former Steps 59-247 move to 63-251. They may not be absorbed into the Memory integration gate.

Steps 63–86 are accepted against `9996c28`; Step 87 and Integration Gate C passed against the six-file artifact `c69f279`. Steps 88–90 passed against `bafdaae`, including refreshed architecture enforcement. Steps 91–92 passed against `3cc6631`; Steps 93–95 passed against `ace0a33`. Steps 96–107 passed against `73e88ae`. The accepted range is 1–107; remaining owner converters and integrated final promotion/retirement gates still gate final acceptance.

The number is not a target or an estimate of required work sessions. It is the consequence of making acceptance/failure domains explicit.

A healthy execution run is expected to close several adjacent items when they share one implementation seam. The plan should therefore be read as **294 acceptance checks grouped into phases**, not isolated mini-projects.

If any one item still requires multiple independent implementation passes, split it before starting. If several adjacent items are naturally one coherent implementation pass, execute them together and mark each gate independently. Do not preserve numbering conventions at the expense of efficient, bounded execution.
Authoritative Memory migration exposed the legacy `CVAILEG1` import receipt's binding to original physical prefix bytes. Steps 72-77 now explicitly own its contract, encoder, decoder, replay validation, migration binding and adversarial verification before Memory measurement/Gate C. Former Steps 72-251 move to 78-257. The later post-migration import-family audit remains required and does not substitute for these execution steps.

Steps 78-80 explicitly cover transported import-proof maintenance and evidence equivalence discovered during review. Former Steps 78-257 move to 81-260; ordinary maintenance cannot copy a stale physical-prefix binding.

Legacy settlement normalization still reads original physical cuts. Steps 78-83 explicitly extract read-only source normalization and cover CVAFRS03/04/05 before relocation and maintenance. Former Steps 78-260 move to 84-266. The source proof remains retained; canonical semantic normalization supplies the existing validated successor rather than rewriting physical evidence.
Entity discovery added eight explicit adapter/evidence responsibilities and a reproduced maintenance-proof failure added one fail-closed converter gate, increasing the pre-consolidation plan from 266 to 275 (subsequently reduced to 271). Accepted Steps 1–90 are unchanged; the Phase H crosswalk preserves pending original work.


## Replan crosswalk

Accepted steps 1–90 and their evidence are unchanged. Former future 238 (redundant legacy reconciliation), 240 (runtime cross-format reconciliation), and 250–251 (duplicated age-specific benchmark setup) are removed or merged. Former 265–269 are rewritten as actual fixture promotion, decoder/upgrader deletion and post-deletion verification, not mere audits. All pending rows are renumbered in order; final step 271. The former R1–R22 are no longer a separate execution stream. No source implementation or corpus migration is implied.

## Relationship execution-discovery crosswalk

After Entity Step 107, the current source still persists Relationship; Observation has no separate durable owner and Graph Observation nodes are rejected. Eight concrete consumers have independent failure domains and cannot be hidden in the writer or converter row. New Steps 113–118 cover reconciliation, import evidence, interval admission, reclamation, retained structure and shared proof readers. Former writer Step 113 becomes 119; new Steps 120–121 cover native REL/PHY pruning. Former converter/adversarial/measurement 114–116 become 122–124; former 117–271 move to 125–279. Accepted Steps 1–107 are unchanged. Historical crosswalk counts above describe their dated replans; the current total is 279. Integrated corpus promotion and runtime retirement retain all consolidated former R1–R22 requirements.

## Ego execution-discovery crosswalk

Ego source inspection found four concrete failure domains outside ordinary replay/writing: the independent historical fingerprint scanner, fresh-history rewrite emission, maintenance preservation, and existing unsupported reconciliation/import boundaries. Steps 131–134 make these explicit before ordinary writer cutover. Former Steps 131–279 move to 135–283; Steps 1–130 remain unchanged. Earlier crosswalk totals retain their historical meaning; the current total is 283. These gates do not add Ego reconciliation support or change rewrite history-discard policy.

## Relationship and Ego inventory acceptance — Steps 113–126

Shared batch verification: implementation `41eaa0a`; 73 focused Relationship tests, 1,917 executed root tests (five existing opt-in ignores), 32 CLI tests, root/CLI format/check, six-file authoritative conversion and raw byte evidence. Architecture passes all 23 rules at snapshot `17e404f7eb4de346ba75f1a1861c60030df4ae9f4932479280680e4ac315bc91` (95,098 source nodes/3,124 relationships). Changed Relationship implementation hashes match committed bytes after LF/CRLF normalization; root/export module registrations are independently inspected. Documentation checks are rerun after this ledger update.

### Step 113 execution record

Commit: `41eaa0a`.
Files: `src/perception/relationship_replay.rs; src/facade/cva_reconcile_relationship.rs`.
Verification: Accepted coordinate lookup, exact replay, retries, terminal and typed unavailable tests pass. Shared batch evidence above and the detailed execution document apply.
Blockers: none for this item; later owner conversion, canonical promotion and runtime retirement remain pending.

### Step 114 execution record

Commit: `41eaa0a`.
Files: `src/perception/relationship_replay.rs; src/imports/reconciliation.rs`.
Verification: Normalized source evidence preserves accepted fields except rebased coordinates; legacy/compact evidence parity passes. Shared batch evidence above and the detailed execution document apply.
Blockers: none for this item; later owner conversion, canonical promotion and runtime retirement remain pending.

### Step 115 execution record

Commit: `41eaa0a`.
Files: `src/imports/reconciliation.rs; reconciliation_relationship_compact_tests.rs`.
Verification: Strict creation/delta/terminal admission and malformed/unknown-family rejection pass. Shared batch evidence above and the detailed execution document apply.
Blockers: none for this item; later owner conversion, canonical promotion and runtime retirement remain pending.

### Step 116 execution record

Commit: `41eaa0a`.
Files: `src/facade/storage_reclamation_scan.rs; storage_reclamation_relationship_compact_tests.rs`.
Verification: Published compact and terminal backing survive relocation/reopen; orphans remain inert. Shared batch evidence above and the detailed execution document apply.
Blockers: none for this item; later owner conversion, canonical promotion and runtime retirement remain pending.

### Step 117 execution record

Commit: `41eaa0a`.
Files: `src/perception/relationship_retained_structure.rs`.
Verification: Canonical retained-structure hash equals the prior summary-redacted schema independently of carrier encoding. Shared batch evidence above and the detailed execution document apply.
Blockers: none for this item; later owner conversion, canonical promotion and runtime retirement remain pending.

### Step 118 execution record

Commit: `41eaa0a`.
Files: `src/perception/relationship_description_proof.rs; src/facade/cva_description_prune.rs; phylactery_history_prune.rs`.
Verification: Strict proof extraction, exact membership/source binding and seven rechecksummed tamper cases for REL/PHY pass. Shared batch evidence above and the detailed execution document apply.
Blockers: none for this item; later owner conversion, canonical promotion and runtime retirement remain pending.

### Step 119 execution record

Commit: `41eaa0a`.
Files: `src/perception/relationship_compact_publish.rs; relationship_store.rs`.
Verification: Ordinary creation/delta publication shares validated owner replay seam; no ordinary legacy postimage writer remains. Shared batch evidence above and the detailed execution document apply.
Blockers: none for this item; later owner conversion, canonical promotion and runtime retirement remain pending.

### Step 120 execution record

Commit: `41eaa0a`.
Files: `src/perception/relationship_description_prune.rs; src/facade/cva_history_prune.rs`.
Verification: Native REL pruning emits authenticated redacted chains and removes selected orphan carriers; reopen/maintenance pass. Shared batch evidence above and the detailed execution document apply.
Blockers: none for this item; later owner conversion, canonical promotion and runtime retirement remain pending.

### Step 121 execution record

Commit: `41eaa0a`.
Files: `src/facade/phylactery_history_prune.rs`.
Verification: Native PHY pruning preserves exact retained structure, terminal history and authenticated unavailable descriptions. Shared batch evidence above and the detailed execution document apply.
Blockers: none for this item; later owner conversion, canonical promotion and runtime retirement remain pending.

### Step 122 execution record

Commit: `41eaa0a`.
Files: `src/storage_upgrade/relationship.rs; memory.rs; shell.rs`.
Verification: Nonempty REL/PHY conversion and all six pinned V1 conversions preserve exact history, UUID, ticket times and source bytes/hash/mtime. Shared batch evidence above and the detailed execution document apply.
Blockers: none for this item; later owner conversion, canonical promotion and runtime retirement remain pending.

### Step 123 execution record

Commit: `41eaa0a`.
Files: `src/perception/relationship_replay_tests.rs; src/facade/relationship_compact_prune_tests.rs`.
Verification: Participants/evidence remove-and-add, retry/orphan/terminal/unavailable and proof-tamper fixtures pass within 73 focused tests. Shared batch evidence above and the detailed execution document apply.
Blockers: none for this item; later owner conversion, canonical promotion and runtime retirement remain pending.

### Step 124 execution record

Commit: `41eaa0a`.
Files: `benchmarks/relationship-history-storage-2026-10-20.json`.
Verification: Six pinned inputs explicitly contain zero Relationship records; nonempty synthetic REL/PHY each save 28 raw bytes (689 to 661). Shared batch evidence above and the detailed execution document apply.
Blockers: none for this item; later owner conversion, canonical promotion and runtime retirement remain pending.

### Step 125 execution record

Commit: `41eaa0a`.
Files: `docs/minimal-lossless-history-storage-execution-2026-10-19.md`.
Verification: Identity/Anchor/Personality/synthesis stable identity, mutable fields, terminal state, ownership and independent clocks inventoried. Shared batch evidence above and the detailed execution document apply.
Blockers: none for this item; later owner conversion, canonical promotion and runtime retirement remain pending.

### Step 126 execution record

Commit: `41eaa0a`.
Files: `scripts/measure_ego_history.py; scripts/test_measure_ego_history.py; benchmarks/ego-payload-repetition-2026-10-20.json`.
Verification: Six immutable inputs contain zero Ego records; six independent parser/projection tests pass. No owner-local payload table warranted for this rollout. Shared batch evidence above and the detailed execution document apply.
Blockers: none for this item; later owner conversion, canonical promotion and runtime retirement remain pending.

## Ego numbered acceptance — Steps 127–138

Implementation milestone `b10815d`. Focused codec/contract/replay/consumer checks pass; final corrected public writer/rewrite tests pass within the running full root test suite. Six-source authoritative conversion passed in `.test-tmp/ego-writer-migration-focused.log`; nonempty conversion passed in `ego-writer-migration-final.log`. Root/CLI format and compilation checks pass. Integration Gate D remains **pending** until the complete root/CLI runs and current-source architecture enforcement finish; these numbered records do not claim the integration gate or final rollout complete. Detailed evidence is in the execution document.

### Step 127 execution record

Commit: `b10815d`.
Files: `src/ego/ego_compact_contract.rs; ego_compact_contract_tests.rs`.
Verification: Nine contract tests preserve activation, terminal state, admitted repeated publications, backwards source cuts and owner restrictions.
Blockers: none for this numbered gate; broad Integration Gate D, later canonical promotion and runtime retirement remain pending.

### Step 128 execution record

Commit: `b10815d`.
Files: `src/ego/ego_compact_codec.rs; ego_compact_creation_codec_tests.rs`.
Verification: Seven literal/roundtrip/truncation/corruption creation/terminal tests pass for all four kinds; implicit revision and exact strings validated.
Blockers: none for this numbered gate; broad Integration Gate D, later canonical promotion and runtime retirement remain pending.

### Step 129 execution record

Commit: `b10815d`.
Files: `src/ego/ego_compact_delta_codec.rs; ego_compact_delta_codec_tests.rs`.
Verification: Six field-mask/literal/truncation/corruption tests pass; zero-mask publication and metadata-only deltas are retained.
Blockers: none for this numbered gate; broad Integration Gate D, later canonical promotion and runtime retirement remain pending.

### Step 130 execution record

Commit: `b10815d`.
Files: `src/ego/ego_compact_apply.rs; ego_store_compact.rs; ego_compact_replay_tests.rs`.
Verification: Five replay tests preserve exact materialized records across interleaved objects, legacy bases, activation and terminals; invalid bases fail.
Blockers: none for this numbered gate; broad Integration Gate D, later canonical promotion and runtime retirement remain pending.

### Step 131 execution record

Commit: `b10815d`.
Files: `src/facade/historical_ego_fingerprint.rs; historical_ego_compact_tests.rs`.
Verification: Both REL and PHY legacy/compact histories produce equal canonical fingerprints and projected fields at every cut.
Blockers: none for this numbered gate; broad Integration Gate D, later canonical promotion and runtime retirement remain pending.

### Step 132 execution record

Commit: `b10815d`.
Files: `src/rewrite/semantic_state.rs; src/ego/ego_compact_writer_tests.rs`.
Verification: Public REL/PHY fresh-history rewrite test passes in full root run; exact selected state, reset revisions, independent activation and existing cache policy remain.
Blockers: none for this numbered gate; broad Integration Gate D, later canonical promotion and runtime retirement remain pending.

### Step 133 execution record

Commit: `b10815d`.
Files: `src/facade/ego_compact_maintenance_tests.rs`.
Verification: Both owner histories retain every Ego fingerprint through native prune/reclaim/reopen; REL principal/attachment repacks also preserve cuts.
Blockers: none for this numbered gate; broad Integration Gate D, later canonical promotion and runtime retirement remain pending.

### Step 134 execution record

Commit: `b10815d`.
Files: `src/imports/reconciliation_ego_compact_tests.rs; src/facade/cva_reconcile_ego_compact_tests.rs`.
Verification: All compact families and independent activation remain explicitly unsupported in import intervals; divergent reconciliation rejects nonempty compact Ego without output publication.
Blockers: none for this numbered gate; broad Integration Gate D, later canonical promotion and runtime retirement remain pending.

### Step 135 execution record

Commit: `b10815d`.
Files: `src/ego/ego_store.rs; ego_store_compact.rs; ego_compact_writer_tests.rs`.
Verification: Ordinary REL/PHY writer tests pass in full root run: compact creation/delta/terminal, minimal activation, exact reopened state, small metadata deltas and no-op byte/clock stability.
Blockers: none for this numbered gate; broad Integration Gate D, later canonical promotion and runtime retirement remain pending.

### Step 136 execution record

Commit: `b10815d`.
Files: `src/storage_upgrade/ego.rs; memory.rs; shell.rs; ego_tests.rs; ego_authoritative_tests.rs`.
Verification: Nonempty REL/PHY every-cut parity and four malformed-source cleanup cases pass; six immutable authoritative V1 conversions pass with exact owner/clock/source parity.
Blockers: none for this numbered gate; broad Integration Gate D, later canonical promotion and runtime retirement remain pending.

### Step 137 execution record

Commit: `b10815d`.
Files: `src/ego/ego_compact_contract_tests.rs; ego_compact_replay_tests.rs; ego_compact_writer_tests.rs; src/storage_upgrade/ego_tests.rs`.
Verification: Activation/deletion/owner/sequence/revision/source-watermark and terminal resurrection cases preserve current admitted history and public lifecycle restrictions.
Blockers: none for this numbered gate; broad Integration Gate D, later canonical promotion and runtime retirement remain pending.

### Step 138 execution record

Commit: `b10815d`.
Files: `benchmarks/ego-history-storage-2026-10-20.json`.
Verification: Six retained files explicitly report zero Ego records. Synthetic raw payload bytes 256→200 REL and 656→555 PHY; framed bytes 296→240 and 760→659. No compression or corpus saving inferred.
Blockers: none for this numbered gate; broad Integration Gate D, later canonical promotion and runtime retirement remain pending.

## Graph execution-discovery crosswalk

The measured ordinal opportunity exposes eleven concrete consumer/admission responsibilities beyond a pure codec: owner marker resolution, historical cuts, reconciliation, Freshness Graph reads, import evidence, import classification, retained proof binding, reclamation, PHY migration, maintenance preservation and ordinary writer cutover. These become Steps 143–153. Former pending Steps 143–283 move to 154–294; accepted Steps 1–138 are unchanged. The mapping/source-history proof gate is necessary because the former direct carrier hash embedded stable IDs, whereas an ordinal-only carrier would otherwise omit its interpretation. Earlier crosswalk totals describe their dated revisions; the current total is 294. No Graph runtime implementation or Gate D acceptance is claimed by this replan.

## Integration Gate D acceptance

Gate D passes against Ego implementation `b10815d`: full root **1,956 passed, zero failures, five existing opt-in ignores across 26 suites**; **32 CLI tests**; root/CLI format and compile/check; six-source authoritative conversion; nonempty REL/PHY historical/migration, reconciliation, native prune/repack/reclaim and lifecycle coverage. Refreshed architecture snapshot `14b14fc52a55a56b1d69809d18cf708415f16706ab2562f6e36bcdd1f21bb739` passes all **23 rules**, **96,374 source nodes**, **3,142 relationships**, no compatibility warnings. All 28 changed production/registration Rust paths in `b10815d` match snapshot content identities after LF/CRLF normalization; two test fixture files changed assertions/comments after snapshot capture and passed the complete test run. The later unregistered Freshness preparation test is outside the Ego milestone snapshot; direct ownership audit includes it with zero missing/overlapping ownership. Detailed logs and limits are in the execution document. No canonical corpus promotion or runtime retirement is claimed.

### Step 139 execution record

Commit: `e9b36e9`.
Files: `scripts/measure_graph_history.py`, `scripts/test_measure_graph_history.py`, `benchmarks/graph-history-byte-inventory-2026-10-20.json`.
Verification: six independent parser/projection tests pass; six pinned source hashes unchanged. Mutation/batch/mapping/version fields classified independently of framing. Payload total 7,562,328 bytes; Container-framed total 8,349,616. Ordinal capacity, full dictionary framing, typed Memory widths and family-specific batch strides are validated.
Blockers: none for this inventory; dictionary projections remain hypothetical until actual encoding is measured.

### Step 140 execution record

Commit: `e9b36e9` (measurement evidence); owner/source review and decision recorded here.
Files: Graph byte inventory; `src/semantic_graph/graph_store.rs`, `graph_store_write.rs`, `graph_rebuild.rs`; this execution ledger.
Verification: **yes, compact endpoint IDs are warranted**. The conservative four-byte projection saves 3,918,649 bytes across all six fixtures even while charging an additional framed typed dictionary. Reuse the already-persisted dense Graph-owned u32 mappings rather than adding a second ID authority. Retain stable semantic IDs in all public/historical views; use fixed u32 ordinals to preserve the existing mapping range and checked exhaustion, without adaptive-width rollover. Actual byte savings, map interpretation/proof binding, every-cut parity and hard-cut retirement remain separate gates.
Blockers: none for the measured decision; Step 141 contract and Steps 142–155 must pass before this becomes ordinary persisted mutation encoding.

### Step 141 execution record

Commit: `864efe9`.
Files: `src/semantic_graph/graph_compact_contract.rs; graph_store.rs; docs/storage-format.md`.
Verification: Fixed u32 endpoint ordinals reuse the existing dense Graph mappings; zero is valid, semantic kind/32-byte identity remains stable, and missing/non-dense/duplicate mappings fail closed. The integrated adjacent compact regression run passes **240/240** with zero failures/ignores (`.test-tmp/graph-writer-compact-regressions.log`); the pre-cutover run passes 236/236.
Blockers: none for this numbered gate; Graph converter Step 154, broad Integration Gate E, canonical promotion and runtime retirement remain pending.

### Step 142 execution record

Commit: `864efe9`.
Files: `src/semantic_graph/graph_compact_codec.rs; graph_compact_codec_tests.rs`.
Verification: Independent literal single/batch fixtures cover all relation kinds, every origin, zero/max ordinals, repeated values, malformed prefixes/counts/flags, unknown versions and exact trailing-byte rejection. The integrated adjacent compact regression run passes **240/240** with zero failures/ignores (`.test-tmp/graph-writer-compact-regressions.log`); the pre-cutover run passes 236/236.
Blockers: none for this numbered gate; Graph converter Step 154, broad Integration Gate E, canonical promotion and runtime retirement remain pending.

### Step 143 execution record

Commit: `864efe9`.
Files: `src/semantic_graph/graph_rebuild.rs; graph_store_compact.rs; src/facade/{cva,phylactery}_lifecycle.rs`.
Verification: REL/PHY replay resolves ordinals at the accepted marker, admits carrier→mapping→marker, preserves raw repeated transactions/mixed origins/clocks, rejects dangling/repeated markers, and enforces published compact V2 admission while keeping valid unmarked carriers inert. The integrated adjacent compact regression run passes **240/240** with zero failures/ignores (`.test-tmp/graph-writer-compact-regressions.log`); the pre-cutover run passes 236/236.
Blockers: none for this numbered gate; Graph converter Step 154, broad Integration Gate E, canonical promotion and runtime retirement remain pending.

### Step 144 execution record

Commit: `864efe9`.
Files: `src/facade/historical_graph_fingerprint.rs; src/semantic_graph/graph_compact_rebuild_tests.rs; graph_compact_writer_tests.rs`.
Verification: Full Graph and qualifying MemoryGraph cuts retain exact stable relation facts, versions and transaction times; isolated rewrite-baseline membership remains available at Graph cut zero. The integrated adjacent compact regression run passes **240/240** with zero failures/ignores (`.test-tmp/graph-writer-compact-regressions.log`); the pre-cutover run passes 236/236.
Blockers: none for this numbered gate; Graph converter Step 154, broad Integration Gate E, canonical promotion and runtime retirement remain pending.

### Step 145 execution record

Commit: `864efe9`.
Files: `src/facade/cva_reconcile_graph.rs; graph_compact_reconcile_tests.rs`.
Verification: Two independent source owners assign the same ordinal to different stable IDs; tail replay resolves through each source and preserves target semantics and times. Existing mixed-origin reconciliation rejection remains unchanged. The integrated adjacent compact regression run passes **240/240** with zero failures/ignores (`.test-tmp/graph-writer-compact-regressions.log`); the pre-cutover run passes 236/236.
Blockers: none for this numbered gate; Graph converter Step 154, broad Integration Gate E, canonical promotion and runtime retirement remain pending.

### Step 146 execution record

Commit: `864efe9`.
Files: `src/facade/cva_freshness.rs; graph_compact_freshness_tests.rs`.
Verification: Dream Graph snapshot/evidence reads resolve compact endpoints at the pinned accepted cut, retain origin/active/candidate filters, and exclude later compact publications from the accepted proof. The integrated adjacent compact regression run passes **240/240** with zero failures/ignores (`.test-tmp/graph-writer-compact-regressions.log`); the pre-cutover run passes 236/236.
Blockers: none for this numbered gate; Graph converter Step 154, broad Integration Gate E, canonical promotion and runtime retirement remain pending.

### Step 147 execution record

Commit: `864efe9`.
Files: `src/imports/reconciliation_interval.rs; reconciliation_graph_compact_tests.rs`.
Supplement accepted against `7017965`: final review found raw carrier bytes in completed-epoch Graph authority comparison after coordinate extraction had passed. Canonical stable-ID evidence now preserves shape/order across formats and source mapping assignments. The full root library run and three focused fixture-enabled import tests pass, including identical ordinal bytes naming different stable edges.
Verification: Shared and retained completed-interval coordinate extraction resolves compact backing through its source owner; source Graph cut 2 rebases to destination cut 3 without assuming physical or local-coordinate equality. The integrated adjacent compact regression run passes **240/240** with zero failures/ignores (`.test-tmp/graph-writer-compact-regressions.log`); the pre-cutover run passes 236/236.
Blockers: none for this numbered gate; Graph converter Step 154, broad Integration Gate E, canonical promotion and runtime retirement remain pending.

### Step 148 execution record

Commit: `864efe9`.
Files: `src/imports/reconciliation.rs; reconciliation_graph_compact_tests.rs`.
Verification: Strict import-interval classification admits valid compact Graph carriers and rejects malformed or unknown versions rather than absorbing them as opaque replay work. The integrated adjacent compact regression run passes **240/240** with zero failures/ignores (`.test-tmp/graph-writer-compact-regressions.log`); the pre-cutover run passes 236/236.
Blockers: none for this numbered gate; Graph converter Step 154, broad Integration Gate E, canonical promotion and runtime retirement remain pending.

### Step 149 execution record

Commit: `864efe9`.
Files: `src/semantic_graph/graph_store_compact.rs; src/facade/{history_prune_manifest,owner_maintenance,cva_lifecycle,phylactery_history_prune,graph_compact_proof_tests}.rs`.
Verification: Graph-owned canonical carrier digests bind ordered stable endpoint IDs, kind, active and origin. Native REL/PHY prune reopen succeeds; valid mapping substitution fails maintenance proof. Legacy full-ID digest matches an independent pre-change golden calculation. Orphans, unused later mappings and post-watermark markers remain excluded. REL admission validates after Graph materialization. The integrated adjacent compact regression run passes **240/240** with zero failures/ignores (`.test-tmp/graph-writer-compact-regressions.log`); the pre-cutover run passes 236/236.
Blockers: none for this numbered gate; Graph converter Step 154, broad Integration Gate E, canonical promotion and runtime retirement remain pending.

### Step 150 execution record

Commit: `864efe9`.
Files: `src/facade/storage_reclamation_scan.rs; storage_reclamation.rs; graph_compact_maintenance_tests.rs`.
Verification: Reclamation identifies compact backing carriers, drops proven unmarked payloads, preserves published ones and relocates the unchanged Graph version links. All Graph/MemoryGraph cuts, mapping order and times survive REL/PHY reclaim. The integrated adjacent compact regression run passes **240/240** with zero failures/ignores (`.test-tmp/graph-writer-compact-regressions.log`); the pre-cutover run passes 236/236.
Blockers: none for this numbered gate; Graph converter Step 154, broad Integration Gate E, canonical promotion and runtime retirement remain pending.

### Step 151 execution record

Commit: `864efe9`.
Files: `src/facade/migration_phy.rs; graph_compact_maintenance_tests.rs`.
Verification: PHY semantic migration resolves stable IDs from the source Graph owner. A legacy source without owner UUID uses an independent cumulative source-state reference; every migrated cut, global/local coordinate and transaction time agrees. The integrated adjacent compact regression run passes **240/240** with zero failures/ignores (`.test-tmp/graph-writer-compact-regressions.log`); the pre-cutover run passes 236/236.
Blockers: none for this numbered gate; Graph converter Step 154, broad Integration Gate E, canonical promotion and runtime retirement remain pending.

### Step 152 execution record

Commit: `864efe9`.
Files: `src/facade/graph_compact_maintenance_tests.rs; graph_compact_proof_tests.rs`.
Verification: Native REL/PHY prune/reclaim and REL principal/attachment repacks preserve compact Graph interpretation; native repacks compare every Graph/MemoryGraph cut and exact publication times. Table bytes remain owner data, not a second dictionary authority. The integrated adjacent compact regression run passes **240/240** with zero failures/ignores (`.test-tmp/graph-writer-compact-regressions.log`); the pre-cutover run passes 236/236.
Blockers: none for this numbered gate; Graph converter Step 154, broad Integration Gate E, canonical promotion and runtime retirement remain pending.

### Step 153 execution record

Commit: `864efe9`.
Files: `src/semantic_graph/graph_store_write.rs; graph_compact_writer_tests.rs`.
Verification: Ordinary REL/PHY Memory-only and typed single/batch writes emit CVAGCM01/CVAGCB01 after mappings. Reopen state/clocks/times and exact no-op byte stability pass; fresh-history rewrite emits compact Graph and preserves isolated Graph-zero baseline membership. The integrated adjacent compact regression run passes **240/240** with zero failures/ignores (`.test-tmp/graph-writer-compact-regressions.log`); the pre-cutover run passes 236/236.
Blockers: none for this numbered gate; Graph converter Step 154, broad Integration Gate E, canonical promotion and runtime retirement remain pending.


## Freshness accepted-event authority discovery crosswalk

Read-only Step 163 preparation found that completed-import reconciliation compares normalized interval events but omits the settlement accepted-event map. Owner admission validates each listed event, while a map can omit an event or name an accepted event outside the physical interval. A separate Sol implementation gate is required before the regression-only row: new Step 163 derives the accepted identities and immutable event facts from the existing Freshness owner, compares canonical normalized evidence, and preserves legitimate turn/Graph-proof rebasing. Former pending Steps 163–294 become 164–295; existing Steps 1–162 retain their identities. Raw payload digests cannot be compared directly because their accepted-turn/proof coordinates legitimately rebase. No second Freshness authority or new effect codec is authorized by this discovery. The current plan contains 295 consecutive numbered execution rows.

### Step 154 execution record

Commit: `7017965`.
Files: `src/storage_upgrade/{graph,graph_tests,graph_authoritative_tests,memory,model,shell,mod}.rs`; `benchmarks/graph-history-storage-2026-10-20.json`.
Verification: Three upgrade tests pass, including six pinned immutable V1 REL/PHY sources. All 46,075 accepted Graph publications retain ordered stable mutations, global/local coordinates, mapping bytes and transaction times; deterministic replay establishes every-prefix equality, and synthetic REL/PHY fixtures additionally compare every cut fingerprint. Orphans remain inert, malformed published histories reject with cleanup and no source changes. Actual all-Graph payload bytes 7,562,328→3,465,986; Container-framed bytes 8,349,616→4,253,274; saving 4,096,342 bytes without compression or another dictionary. Whole-file savings from other owner converters are not attributed to Graph. All source bytes/hashes/lengths/mtimes remain unchanged.
Blockers: none for this gate; corpus promotion, final integration/architecture refresh and converter/runtime retirement remain pending.

### Step 155 execution record

Commits: `864efe9`, `7017965`.
Files: Graph compact codec/rebuild/owner/proof/reconciliation/maintenance/writer tests and private upgrade tests.
Verification: Mapping density/uniqueness, zero/max ordinals, missing/prior/future mappings, dangling/repeated markers, inert malformed-order orphans, typed same-byte identities, retirement, mixed origins, repeated raw transactions, invalid flags/counts/versions, source mapping collisions, V1/V2 admission and rechecksummed valid mapping substitution are covered. Full root **1,988 passed across 26 suites**, zero failures and five existing opt-in ignores (`.test-tmp/graph-full-tests.log`), including the renewed Step 147 collision regression. Three targeted fixture-enabled import comparison tests pass. Root and CLI format/check pass, and all 32 CLI tests pass (`.test-tmp/graph-{format,check,cli-format,cli-check,cli-tests}.log`). The broad architecture gate remains Integration Gate E.
Blockers: none for the adversarial gate; final integration, canonical corpus promotion and runtime retirement remain pending.


## Publication envelope consumer expansion crosswalk

Step 167 review confirms distinct Container allocation/reserved-binding semantics and Memory codec, replay, historical, reconciliation/import, authenticated-proof, reclamation, native-copy and writer failure domains. New Step 169 owns physical clock/recovery integration; former Memory writer Step 169 becomes explicit Steps 170–177. Former pending Steps 170–295 become 178–303. Steps 1–168 retain their identities; Integration Gate E now follows Step 184. Existing earlier crosswalk totals remain dated checkpoints. The plan has 303 consecutive execution rows. [Envelope contract](publication-envelope-contract-2026-10-20.md) records the reserved-binding decision and source import-boundary constraints. Before another owner migration, inventory its actual consumers and insert explicit owner-specific steps rather than absorbing work into an umbrella row.


### Step 156 execution record

Commit: `b043494`.
Files: Freshness field inventory and concrete consumer audit in execution evidence.
Verification: Every event identity/order/effect/proof/provenance/semantic field is accepted authority; per-event postimages are derived. Birth/admission/settlement records remain accepted metadata/proof.
Blockers: none for this numbered gate; envelope runtime implementation, broad Integration Gate E, canonical promotion and compatibility retirement remain pending.

### Step 157 execution record

Commit: `b043494`.
Files: docs/storage-format.md; src/freshness_storage.rs.
Verification: Existing CVAFRS02 already expresses every required accepted fact without postimages. Reused the current format instead of inventing another carrier; independent literal wire test passes.
Blockers: none for this numbered gate; envelope runtime implementation, broad Integration Gate E, canonical promotion and compatibility retirement remain pending.

### Step 158 execution record

Commit: `b043494`.
Files: src/freshness/freshness_effect_storage_contract_tests.rs; src/freshness_storage.rs.
Verification: Existing encoder/decoder verified by exact literal semantic bytes, complete decoded fields, truncation/trailing/producer/option/owner/version rejection; all four contract tests pass.
Blockers: none for this numbered gate; envelope runtime implementation, broad Integration Gate E, canonical promotion and compatibility retirement remain pending.

### Step 159 execution record

Commit: `b043494`.
Files: src/freshness_storage.rs and effect-storage contract tests.
Verification: Canonical prior-state-plus-effect reduction reproduces exact postimages; decoded events carry no postimages. Adjacent scoring/order/replay regressions pass.
Blockers: none for this numbered gate; envelope runtime implementation, broad Integration Gate E, canonical promotion and compatibility retirement remain pending.

### Step 160 execution record

Commit: `b043494`.
Files: Freshness owner, semantic settlement, forgery and contract tests.
Verification: 154 adjacent tests pass; the unchanged long-owner retry test passed in the earlier run. Forged candidate postimages reject before omission; malformed wire, provenance, policy, identity, duplicate/conflict and semantic source rules remain enforced.
Blockers: none for this numbered gate; envelope runtime implementation, broad Integration Gate E, canonical promotion and compatibility retirement remain pending.

### Step 161 execution record

Commit: `b043494`.
Files: src/freshness_storage.rs; effects-only contract and synthetic migration tests.
Verification: Ordinary event encoding already writes only accepted effects/proof/evidence. Literal bytes prove derived record maps cannot alter the payload; durable accepted-use events reopen and reconstruct observable same-turn postimages.
Blockers: none for this numbered gate; envelope runtime implementation, broad Integration Gate E, canonical promotion and compatibility retirement remain pending.

### Step 162 execution record

Commit: `b043494`.
Files: src/storage_upgrade/freshness_{synthetic,authoritative}_tests.rs; src/storage_upgrade/mod.rs.
Verification: Two migration tests pass (317.25s), including six hash-pinned V1 owners and a nonempty legacy-Memory/two-event fixture. Exact retained facts, supported cuts, event-cut availability, proofs and source-derived settlement normalization agree. Source bytes/hashes/lengths/mtimes are unchanged; no promotion.
Blockers: none for this numbered gate; envelope runtime implementation, broad Integration Gate E, canonical promotion and compatibility retirement remain pending.

### Step 163 execution record

Commit: `b043494`.
Files: src/freshness/freshness_import_events.rs; src/imports/reconciliation.rs; reconciliation_freshness_authority_tests.rs.
Verification: Freshness owner resolves canonical scope accepted IDs, expands batches and rejects missing/conflicting events. Completed epochs compare normalized immutable event maps including outside-interval facts. Four focused tests pass, including real settled import inventory; legitimate coordinate rebasing remains equivalent.
Blockers: none for this numbered gate; envelope runtime implementation, broad Integration Gate E, canonical promotion and compatibility retirement remain pending.

### Step 164 execution record

Commit: `b043494`.
Files: Freshness authority/synthetic/authoritative migration regressions.
Verification: Real CVAFRS06 normalization bytes match source-derived successors. Two same-turn synthetic event cuts preserve exact facts and derived scores. All 111 retained intra-settlement event cuts return the same documented unavailable outcome; no successful retained event-cut fingerprints are claimed.
Blockers: none for this numbered gate; envelope runtime implementation, broad Integration Gate E, canonical promotion and compatibility retirement remain pending.

### Step 165 execution record

Commit: `b043494`.
Files: benchmarks/freshness-history-storage-2026-10-20.json.
Verification: 111 events/148,708 effects retained. All CVAFRS02 journal payload bytes 5,098,875→5,098,875; framed bytes 5,100,635→5,100,635; zero journal saving because effects-only encoding already exists. Actual CVAFRS06 normalization adds 7,415,813 payload bytes, separately attributed. Policy receipts are outside journal totals.
Blockers: none for this numbered gate; envelope runtime implementation, broad Integration Gate E, canonical promotion and compatibility retirement remain pending.

### Step 166 execution record

Commit: `b043494`.
Files: scripts/measure_publication_framing.py; parser tests; benchmarks/publication-framing-byte-inventory-2026-10-20.json.
Verification: Nine parser/accounting tests pass; six before/after source hashes match. Exact-ID/range attribution partitions 87,296 global tickets once (four unclaimed, none shared), owner links and 1,943 grouped ranges/4,695 global IDs. Per-owner ticket/link/framing field bytes and explicit absent families are recorded; no projected savings claimed.
Blockers: none for this numbered gate; envelope runtime implementation, broad Integration Gate E, canonical promotion and compatibility retirement remain pending.

### Step 167 execution record

Commit: `b043494`.
Files: docs/publication-envelope-contract-2026-10-20.md; expanded linear-plan rows169–177.
Verification: Parent review accepts explicit allocation tags and prior-standalone-reservation binding, original marker/import slots and source-authenticated proof relocation. Container stays physical; each owner retains semantic admission/local clocks. Characterized reverse-global Memory/Relationship histories remain representable. No runtime codec/writer is accepted by this contract gate.
Blockers: none for this numbered gate; envelope runtime implementation, broad Integration Gate E, canonical promotion and compatibility retirement remain pending.

### Step 168 execution record

Commit: `e438360`.
Files: `src/container/publication_envelope.rs; publication_envelope_tests.rs`.
Verification: Five independent literal codec tests cover all allocation/reserved tags, optional signed timestamp extremes, structural maximum global, opaque borrowed body bytes, zero/short/nested/unknown/truncated frames and exact round-trip. All 29 envelope-filter tests pass; no semantic owner registry or ordinary writer cutover is introduced.
Blockers: none for this codec gate; owner body/readers/writer/converter and final integration remain pending.

### Step 169 execution record

Commit: `e438360`.
Files: `src/container/{container_scan,container_version,publication_envelope_clock_tests}.rs; src/facade/historical_global_frontier.rs`.
Verification: Fourteen new clock/recovery tests pass. Container tracks standalone reservation provenance and consumes each envelope binding once; grouped/direct/future/repeated bindings reject. Exact allocations/times and physical global frontiers, V1 barriers, malformed-complete rejection, every partial allocation/binding frame prefix, failed validation, write fences and overflow pass. Five adjacent filters pass 318 unique tests with zero failures/ignores; root/CLI format/check and 32 CLI tests pass. Both documentation modes pass. Existing ordinary semantic writer shapes remain unchanged.
Blockers: none for the mechanical gate; Memory body admission/consumers, other owner decisions, final integration/architecture refresh and retirement remain pending.

### Step 170 execution record

Commit: `032530b`.
Files: `src/memory/memory_publication_codec{,_tests}.rs; memory_compact_codec.rs`.
Verification: Seven independent codec tests pass for CVAMC002 creation, CVAMD002 delta and CVAMT002 terminal bodies. Outer global context is not duplicated; local/revision/identity/field mask and exact values remain owner-bound. Literal wire, optional clears/empty deltas, corruption/truncation/unknown versions and input immutability pass.
Blockers: none for this body-codec gate; ordinary writer/converter and downstream consumer gates remain pending.

### Step 171 execution record

Commit: `032530b`.
Files: `src/memory/{memory_rebuild,memory_store,memory_publication_replay_tests,memory_rebuild_overflow_tests}.rs`.
Verification: Canonical owner candidate/terminal validation applies envelope bodies exactly once. Five replay/admission regressions pass, including actual REL/PHY reopen, reverse reserved globals, missing backing, invalid local/base state without mutation, raw 002/V1 rejection and unknown body failure. Local-version overflow is checked before insertion/removal and the atomicity regression passes. Adjacent Memory suite: 278 passed, zero failures, one existing opt-in ignore; initial localhost restriction resolved by permitted mock-server rerun. Root/CLI format/check, 32 CLI tests and both documentation modes pass.
Blockers: none for owner admission; import/proof/copy/writer/converter and final integration remain pending.

### Step 172 execution record

Commit: `032530b`.
Files: `src/memory/memory_publication_replay_tests.rs`; existing grouped and historical regressions.
Verification: Every local cut of marker/envelope REL/PHY fixtures agrees, including reverse-global records, exact optional times and terminal state. Real grouped Insomnia plus envelope REL compares every local/global cut and timestamp. All seven focused replay/history tests, six external grouped publication regressions and fourteen Insomnia completion tests pass; existing grouped whole-owner/interior-cut cases pass in the 278-test adjacent run.
Blockers: none for this historical gate; downstream consumers/writers/conversion and final integration remain pending.


### Step 173 execution record

Commit: `53e603e`.
Files: `src/facade/cva_reconcile_memory.rs; src/imports/reconciliation{,_interval,_memory_envelope_tests,_freshness_authority_tests}.rs; Memory publication coordinate helper.`
Verification: Five focused cases and fifty adjacent reconciliation tests pass. Direct interval replay and divergent reconciliation retain source times; real completed-import evidence includes an envelope revision; strict malformed/foreign body rejection passes. Deferred reserved bindings are captured at their physical publication interval. Terminal replay retains its preexisting unsupported revision-API restriction.
Blockers: none for this numbered gate; offline Memory envelope conversion, later owner framing, Integration Gate E/source refresh and final promotion/retirement remain pending.

### Step 174 execution record

Commit: `53e603e`.
Files: `src/facade/history_prune_manifest.rs; src/memory/memory_publication_codec.rs; memory_publication_prune_tests.rs.`
Verification: Canonical comparison-only marker/carrier evidence agrees at every global watermark with reverse-global legacy marker history. Actual REL/PHY native prune outputs reopen, removed Memory bodies resolve as unavailable, and a valid creation mutation-ID change rejects the authenticated output. Source bytes remain unchanged.
Blockers: none for this numbered gate; offline Memory envelope conversion, later owner framing, Integration Gate E/source refresh and final promotion/retirement remain pending.

### Step 175 execution record

Commit: `53e603e`.
Files: `src/facade/storage_reclamation{,_scan,_memory_publication_tests}.rs.`
Verification: Actual REL/PHY reclamation retains creation/delta/terminal authority and hydrated historical bodies, exact records/times and byte-identical envelopes. One inert orphan carrier/body is reclaimed without source changes. Strict malformed and unsupported owner envelope bodies reject; existing marker reference relocation remains supported.
Blockers: none for this numbered gate; offline Memory envelope conversion, later owner framing, Integration Gate E/source refresh and final promotion/retirement remain pending.

### Step 176 execution record

Commit: `53e603e`.
Files: `src/memory/memory_publication_copy_tests.rs; existing history rewrite, native maintenance and migration suites.`
Verification: Native REL attachment repack preserves all Memory-local fingerprints/current membership, revision payloads and global times. REL/PHY current-selection semantic rewrites retain the selected identity/body semantics; ownerless PHY migration retains live envelope revisions and source times. Existing AllRetained rewrite contracts and native REL/PHY maintenance paths pass in the complete root run; intentional fresh-history coordinate remapping is not claimed as source-history preservation.
Blockers: none for this numbered gate; offline Memory envelope conversion, later owner framing, Integration Gate E/source refresh and final promotion/retirement remain pending.

### Step 177 execution record

Commit: `53e603e`.
Files: `src/memory/memory_store.rs; memory_publication_writer_tests.rs; compact writer/maintenance tests; explicit legacy terminal/completion fixtures.`
Verification: Direct creation/delta/terminal publication emits one allocation envelope with no separate ticket/MemoryVersion/001 carrier. Exact mutation and terminal retries consume no bytes or clocks. Actual REL/PHY reopen and every incomplete final-envelope frame prefix pass. Complete root run: 2,045 passed, zero failures, five existing ignores; root/CLI format/check, 32 CLI tests and both documentation modes pass.
Blockers: none for this numbered gate; offline Memory envelope conversion, later owner framing, Integration Gate E/source refresh and final promotion/retirement remain pending.


## Entity envelope consumer expansion — 2026-10-20

The Step 178 review inventories Entity before cutover. Former umbrella Step 179 becomes explicit Steps 179–188 for contract, body codec, replay, historical projections, reconciliation/import, authenticated prune proof, reclamation, native copies, ordinary writer and offline conversion. Former pending Steps 180–303 become 189–312. Earlier accepted rows keep their identities; earlier dated crosswalks remain historical records. Integration Gate E follows the renamed envelope coverage audit at Step 193. Relationship, Archive and Graph must receive equivalent concrete consumer treatment if selected; their umbrella rows do not authorize absorbing independent failure domains. The [envelope contract](publication-envelope-contract-2026-10-20.md#entity-envelope-migration-boundary) names the actual seams and wire decisions.


## Remaining envelope consumer expansion — 2026-10-20

The owner inventory confirms real removable framing for Relationship, Archive and Graph; zero Relationship links in the six pinned inputs is not an absence of a live owner. Former Steps 189, 191 and 192 become ten independently gated owner-specific rows each. Relationship is now 189–198, Ego no-cutover decision 199, Archive 200–209, Graph 210–219, and the final coverage audit 220. Integration Gate E follows Step 220; former 194–312 become 221–339. Earlier accepted rows retain their identities and earlier dated crosswalks remain historical. Each selected owner must preserve its own existing lifecycle: Relationship has one direct tombstone and no Entity-style UUID or replacement chain. Archive content/format frames and atomic grouped owners are not ordinary versioned Archive publications. The vector reuse decision now points to the actual unchanged-vector-byte measurement at Step 273, rather than a stale Entity Resolution row. Structural acceptance requires 339 consecutive execution rows.


## Envelope owner batch acceptance — Steps 178–199 (2026-10-20)

Implementation: `ca60b5c`. Focused envelope regression 91/91; Relationship 98/98 and Entity 281/281 before writer cutover. New Relationship writers 4/4. Full unit sweep: 1,963 passed and 5 ignored, with one obsolete destination-wire assertion subsequently corrected and verified 1/1 against the same production code. Root/feature compilation, root/CLI formatting, 32 CLI tests and both documentation modes pass. The broader repository command is running against this implementation snapshot; Integration Gate E remains pending after Archive/Graph gates.

### Step 178 accepted — Memory offline envelope conversion

Commit: `ca60b5c`.
Files: src/storage_upgrade/memory_envelope.rs; memory_envelope_tests.rs; memory_envelope_authoritative_tests.rs; historical_import_memory_envelope_upgrade_tests.rs.
Verification: 13 focused tests pass; genuine V1 proof-boundary reservation and post-epoch binding pass. Six pinned sources preserve every local cut/global frontier, payload/routing, times, bytes/SHA-256/mtime in 1,178.86 seconds. Measured standalone framing is separate from grouped payloads.
Blockers: None for this gate; CVAHPR01 remains an explicit typed unsupported source and canonical promotion is pending.

### Step 179 accepted — Entity envelope contract

Commit: `ca60b5c`.
Files: docs/publication-envelope-contract-2026-10-20.md.
Verification: Four concrete body schemas, all consumers, proof authority and direct/merge distinctions inventoried and frozen.
Blockers: None.

### Step 180 accepted — Entity envelope codecs

Commit: `ca60b5c`.
Files: src/perception/entity_publication_codec.rs; entity_publication_codec_tests.rs.
Verification: Literal global omission, UUID transitions, empty/full deltas, both terminals, checksums, unknown masks/versions, truncation and trailing-byte rejection pass.
Blockers: None.

### Step 181 accepted — Entity envelope replay

Commit: `ca60b5c`.
Files: src/perception/entity_rebuild.rs; entity_store.rs; facade REL/PHY dispatch.
Verification: REL/PHY admission, dense local sequence, reverse reservations, bases, terminal chains and V1 boundary pass. Overflow rejection leaves all semantic indexes unchanged.
Blockers: None.

### Step 182 accepted — Entity historical projections

Commit: `ca60b5c`.
Files: src/perception/entity_publication_history_tests.rs.
Verification: Matched marker/envelope histories agree at all synthetic local/global cuts and transaction times, including both terminal forms.
Blockers: None.

### Step 183 accepted — Entity reconciliation and import evidence

Commit: `ca60b5c`.
Files: src/facade/cva_reconcile_entity.rs; src/imports/reconciliation.rs; reconciliation_freshness_authority_tests.rs.
Verification: Finite physical ranges, reversed ticket times, duplicate replay, actual divergence, both terminals and normalized evidence from real completed imports pass.
Blockers: None.

### Step 184 accepted — Entity native prune proof

Commit: `ca60b5c`.
Files: src/perception/entity_description_proof.rs; entity_description_prune.rs; src/facade/history_prune_manifest.rs; entity_compact_prune_tests.rs.
Verification: Every-watermark canonical authority, mixed raw/envelope source hashes, actual REL/PHY prune/reopen, exact retained descriptors, missing/detached/duplicate and rechecksummed proof tampering pass.
Blockers: None.

### Step 185 accepted — Entity reclamation and relocation

Commit: `ca60b5c`.
Files: src/facade/storage_reclamation_scan.rs; storage_reclamation.rs; storage_reclamation_entity_publication_tests.rs.
Verification: Accepted inline authority/proofs/terminals remain live; inert backing is reclaimed; exact envelope bytes, cuts and times survive supported native outputs. Malformed/foreign bodies reject.
Blockers: None.

### Step 186 accepted — Entity native copies and semantic policies

Commit: `ca60b5c`.
Files: src/perception/entity_publication_copy_tests.rs; docs/current-limitations.md.
Verification: Native REL attachment repack preserves every retained cut/time/terminal. Semantic rewrite follows selection; supported live PHY owner-ID migration preserves revisions/times.
Blockers: None for supported policies. Existing owner-ID migration terminal limitation is explicit and must not be used for canonical promotion.

### Step 187 accepted — Entity ordinary writer cutover

Commit: `ca60b5c`.
Files: src/perception/entity_compact_publish.rs; entity_store.rs; entity_compact_writer_tests.rs.
Verification: Creation, delta, direct and merge terminal use one allocation envelope. Preflight, retries, reopen and incomplete-tail recovery pass. Genuine V1 boundary fixtures now construct legacy terminal authority explicitly.
Blockers: None.

### Step 188 accepted — Entity offline conversion

Commit: `ca60b5c`.
Files: src/storage_upgrade/entity_envelope.rs; entity_envelope_tests.rs; entity_envelope_authoritative_tests.rs.
Verification: Synthetic reserved marker slots, orphan retention, invalid authority and both terminals pass. Six pinned files pass all Entity cuts/times/identities/terminals and unchanged hashes in 125.07 seconds. Accepted framed bytes saved: 64,772; reserved tickets excluded.
Blockers: None for this gate; final canonical promotion remains pending.

### Step 189 accepted — Relationship contract and inventory

Commit: `ca60b5c`.
Files: docs/publication-envelope-contract-2026-10-20.md.
Verification: Creation/delta/direct terminal contract retains stable refs, roles, evidence, mutation identity and local sequence. No invented UUID/merge lifecycle; zero pinned population distinguished from populated synthetic coverage.
Blockers: None.

### Step 190 accepted — Relationship envelope codecs

Commit: `ca60b5c`.
Files: src/perception/relationship_publication_codec.rs; relationship_publication_codec_tests.rs.
Verification: Literal global omission, exact fields/collections, zero masks, prefix/version/mask/trailing errors and proof-body checksum tampering pass.
Blockers: None.

### Step 191 accepted — Relationship replay and dispatch

Commit: `ca60b5c`.
Files: src/perception/relationship_rebuild.rs; relationship_store.rs; facade REL/PHY lifecycle.
Verification: Both facades, reverse reservations/times, dense local order, missing/duplicate/gapped bases, resurrection and V1 boundaries pass. Rebuild overflow is atomic.
Blockers: None.

### Step 192 accepted — Relationship historical projections

Commit: `ca60b5c`.
Files: src/perception/relationship_publication_history_tests.rs.
Verification: Paired marker/envelope REL/PHY histories compare every local cut, owner-global mapping, physical global whole-owner fingerprint and timestamp with interleaved Memory and role/evidence changes.
Blockers: None.

### Step 193 accepted — Relationship reconciliation and imports

Commit: `ca60b5c`.
Files: src/facade/cva_reconcile_relationship.rs; cva_reconcile_relationship_envelope_tests.rs; src/imports/reconciliation.rs; reconciliation_freshness_authority_tests.rs.
Verification: Physical exclusive ranges, source times, duplicates, actual branch divergence and direct terminal replay pass. Genuine completed-import evidence and strict foreign/malformed classification pass. Legacy-tail destination assertions now inspect new envelopes.
Blockers: None.

### Step 194 accepted — Relationship prune authority

Commit: `ca60b5c`.
Files: src/perception/relationship_description_proof.rs; relationship_description_prune.rs; relationship_publication_prune_tests.rs; relationship_publication_legacy_prune_tests.rs; src/facade/relationship_compact_prune_tests.rs.
Verification: Every-watermark authority, mixed raw/envelope carrier hashes, actual REL/PHY pruning/reopen, retained descriptors and valid-field proof tamper/missing/detached/duplicate rejection pass.
Blockers: None.

### Step 195 accepted — Relationship reclamation

Commit: `ca60b5c`.
Files: src/facade/storage_reclamation_scan.rs; storage_reclamation.rs; storage_reclamation_relationship_publication_tests.rs.
Verification: Orphan-only reclamation, exact inline envelope retention, stable refs, terminal/cut/time parity and malformed/foreign scan/relocation rejection pass.
Blockers: None.

### Step 196 accepted — Relationship native copies and rewrite

Commit: `ca60b5c`.
Files: src/perception/relationship_publication_copy_tests.rs.
Verification: REL attachment repack and PHY reclaim preserve exact envelope bytes, all retained cuts/times and terminals. Rewrite tests explicitly select the Memory closure and reset retained relationships to a baseline.
Blockers: None for supported policies; existing owner-ID migration terminal policy is unchanged and excluded from promotion.

### Step 197 accepted — Relationship ordinary writer cutover

Commit: `ca60b5c`.
Files: src/perception/relationship_compact_publish.rs; relationship_store.rs; relationship_publication_writer_tests.rs.
Verification: Four REL/PHY writer/recovery tests pass: one allocation envelope, no naked carriers/links/tickets, byte/clock-stable retries, invalid preflight and truncated final terminal recovery.
Blockers: None.

### Step 198 accepted — Relationship offline conversion

Commit: `ca60b5c`.
Files: src/storage_upgrade/relationship_envelope.rs; relationship_envelope_tests.rs; relationship_envelope_authoritative_tests.rs.
Verification: Synthetic V1 reverse slots, zero masks, terminal history, inert orphans and invalid source cleanup pass. All six pinned sources stage/reopen with zero Relationship links/envelopes, unchanged bytes/hash/mtime and no final publication in 183.67 seconds.
Blockers: None for this gate; final canonical promotion remains pending.

### Step 199 accepted — Ego envelope non-applicability

Commit: `ca60b5c`.
Files: src/ego/ego_codec.rs; ego_compact_codec.rs; ego_store.rs; docs/publication-envelope-contract-2026-10-20.md.
Verification: Source audit finds no allocation/global publication link in Ego; lifecycle global coordinates remain None. Existing Ego regressions pass in the unit sweep. No invented global clock or envelope was added.
Blockers: None.


### Step 200 accepted — Archive body contract and concrete consumers

Commit: `2597c73`.
Files: `docs/publication-envelope-contract-2026-10-20.md`; `benchmarks/archive-publication-family-inventory-2026-10-20.json`.
Verification: Ten applying-family schemas and the separate repeated-binding body retain one explicit local coordinate; physical global comes only from the envelope. Current semantic field helpers, legacy default preservation, terminal closure, inert content/backing, exact repeat/no-reapply behavior and every concrete reader/proof/copy/writer/converter seam are inventoried. Complete-frame source inventory counts 32,357 accepted links with unchanged six-source hashes; synthetic repeats remain explicitly required because no pinned repeat exists. Peer review and both documentation modes pass with zero findings.
Blockers: None for the contract. Codec/admission/consumer/writer/conversion gates 201–209 remain unaccepted.


### Step 201 accepted — Archive publication body codecs

Commit: `24572e4`.
Files: `src/archive/archive_publication_codec.rs`; shared Archive semantic field codecs; three publication codec test modules; `docs/storage-format.md`.
Verification: All 15 focused body tests pass, including independent literal bytes, every applying family, repeat framing, envelope-supplied tombstone global, metadata legacy defaults and V1 turn principal canonicalization. All 106 Archive-filtered regressions pass. Test compilation and diff checks pass. Valid standalone codec bytes remain unchanged; malformed publication versions, flags, identity, truncation and trailing fields reject.
Blockers: None for body codecs. Replay, consumers, writers and migration remain separate unaccepted gates.
Documentation impact: Native body layout and retained legacy defaults recorded in storage-format.
Architecture impact: Archive owns body interpretation; shared semantic field helpers avoid nested standalone framing. No Container semantic dispatch added.


### Step 202 accepted — Archive native envelope admission

Commit: `ed7e21a`.
Files: `src/archive/archive_rebuild.rs`; `src/facade/cva_lifecycle.rs`; `src/archive/archive_publication_replay_tests.rs`.
Verification: 108 Archive-filtered regressions pass. New cases cover mixed legacy/native REL reopen, explicit dense local and strictly increasing owner globals under reserved bindings, rejection without consuming cuts, first-applying repeat anchors and no repeat reapplication, and tombstone envelope-global substitution with exact closure rejection. Existing physical envelope tests retain the V1 boundary. Checked next-local arithmetic precedes owner mutation. Historical resolution implementation is included but Step 203's broader reader parity remains unaccepted.
Blockers: None for admission. Reader parity and downstream mutation consumers remain pending.
Documentation impact: Admission and pending consumer gates recorded here and in the execution ledger.
Architecture impact: REL dispatch delegates body interpretation to ArchiveOpenState; physical envelopes remain Container-owned.


### Step 203 accepted — Archive readers at envelope references

Commit: `9e153aa`.
Files: archive_history.rs; historical_archive_fingerprint.rs; archive_publication_reader_tests.rs.
Verification: All ten applying families and a repeated binding match standalone evidence at every retained local/global/time cut; current Node, Branch, metadata, Fragment, Episode, File, import receipt, File/Memory and attachment indexes match. Historical branch state does not roll back at an older repeated binding.
Blockers: None for this gate; ordinary Archive writers and offline conversion remain pending.
Documentation impact: Concrete consumer and proof behavior recorded in storage-format and this ledger.
Architecture impact: Archive retains semantic body ownership; concrete consumers resolve it explicitly and Container remains physical framing only.


### Step 204 accepted — Archive reconciliation and import evidence

Commit: `9e153aa`.
Files: cva_reconcile_archive.rs; imports/reconciliation.rs; reconciliation_archive_envelope_tests.rs.
Verification: 86 import and 50 reconciliation regressions pass. Native reserved-binding interval replay retains timestamp -123 and normalized evidence; repeated source bindings do not reapply older mutations. Every applying family has carrier/envelope evidence parity.
Blockers: None for this gate; ordinary Archive writers and offline conversion remain pending.
Documentation impact: Concrete consumer and proof behavior recorded in storage-format and this ledger.
Architecture impact: Archive retains semantic body ownership; concrete consumers resolve it explicitly and Container remains physical framing only.


### Step 205 accepted — Archive pruning and proof identities

Commit: `9e153aa`.
Files: archive_history_prune.rs; cva_history_prune.rs; history_prune_manifest.rs.
Verification: All eleven synthetic cuts have identical owner history digests across carrier/link and native forms, including a repeat. Terminal descriptors match. Native Node/tombstone payload pruning reopens with bound proof, exact cuts/times and retained structure. Seven pruning regressions pass. Legacy standalone descriptor bytes remain preserved; unsupported V1 proof conversion is still rejected by the offline authority boundary.
Blockers: None for this gate; ordinary Archive writers and offline conversion remain pending.
Documentation impact: Concrete consumer and proof behavior recorded in storage-format and this ledger.
Architecture impact: Archive retains semantic body ownership; concrete consumers resolve it explicitly and Container remains physical framing only.


### Step 206 accepted — Archive reclamation dependencies and relocation

Commit: `9e153aa`.
Files: storage_reclamation_scan.rs; storage_reclamation.rs; archive_publication_replay_tests.rs.
Verification: Native Node content survives reclamation while an orphan content object and unpublished carrier are removed. Input bytes, accepted cuts and transaction times remain exact; output reopens. Seventeen reclamation regressions pass. Every native applying family passes byte-preserving relocation validation.
Blockers: None for this gate; ordinary Archive writers and offline conversion remain pending.
Documentation impact: Concrete consumer and proof behavior recorded in storage-format and this ledger.
Architecture impact: Archive retains semantic body ownership; concrete consumers resolve it explicitly and Container remains physical framing only.


### Step 207 accepted — Archive native copies and principal migration

Commit: `9e153aa`.
Files: project_file_attachment_tests.rs; cva_principal_repack.rs; archive_publication_replay_tests.rs.
Verification: Native File and repeat bodies survive attachment repack byte-for-byte with exact cuts/times and stable source attachments. Native Node principal backfill reopens with unchanged cuts/times; atomic native turn backfill preserves attachments and reserved clock header, retries unchanged and rejects conflicting principal. Three principal repack regressions and all 111 Archive-filtered tests pass.
Blockers: None for this gate; ordinary Archive writers and offline conversion remain pending.
Documentation impact: Concrete consumer and proof behavior recorded in storage-format and this ledger.
Architecture impact: Archive retains semantic body ownership; concrete consumers resolve it explicitly and Container remains physical framing only.


### Step 208 accepted — Ordinary Archive envelope writers

Commit: `2801100`.
Files: Archive family writer stores; `archive_history.rs`; `archive_publication_writer_tests.rs`; `imports/epoch_scope.rs`; explicit legacy fixtures.
Verification: 113 Archive, 86 import, 50 reconciliation and 30 upgrade tests pass. Every ordinary writer uses the prepared native publication path. Node/turn/file preflight rejects local exhaustion before content backing is appended; Node/Branch/File retries leave physical chunks unchanged. Actual writer frames contain one allocation envelope per publication and no old semantic carrier/link/ticket. Reopen preserves clocks, ordered turn attachments and semantic indexes. Import Episode scope is additionally revalidated through accepted native/legacy publications; repeated bindings do not enroll an old Episode again. The V1 freshness fixture now constructs explicit legacy Archive records instead of relabeling current envelope writes.
Blockers: None for ordinary writers. Offline conversion and measured corpus parity209 remain pending.
Documentation impact: Storage-format and maintainer map name the prepared writer boundary and accepted-publication Episode scope.
Architecture impact: Archive prepares semantic bodies and commits local state after Container appends the physical envelope; shared backing stays outside the publication.


### Step 209 accepted — Archive offline conversion and actual framing

Commit: `ea26adc`.
Files: `src/storage_upgrade/archive_envelope.rs`; synthetic and authoritative Archive conversion tests; shared staged upgrade transport; `benchmarks/archive-envelope-migration-2026-10-20.json`.
Verification: All six pinned sources pass in 169.26 seconds, preserving all 32,357 accepted Archive cuts, exact source-global and time associations, canonical body fields, current indexes, content dependencies, inert carriers and source hashes/bytes/mtimes. All ten applying families plus a repeated binding have populated synthetic conversion coverage; invalid history returns no stage, and CVAHPR01 remains typed UnsupportedAuthority. Actual Archive carrier/link framed bytes fall from 7,597,309 to 6,937,773 (659,536 bytes saved), excluding reservations, other owners and grouped payloads. No whole-container claim or destination promotion is made. Supplemental writer global-exhaustion preflight and legacy fixture owner filtering are verified; broad unit sweep passes 1,988 tests, five ignored, with the three long sweeps explicitly deferred to Gate E.
Blockers: None for Archive conversion. Graph envelope gates210–219 and integrated promotion/retirement remain pending.
Documentation impact: Per-family measured byte accounting, applicability and exact parity evidence are recorded in the tracked measurement, storage-format and current limitations.
Architecture impact: Archive owns admitted V1 fact conversion; shared transport places BindReserved bodies at original link slots and retains physical reservations. No runtime V1 authority is synthesized.


### Step 210 accepted — Graph single/batch envelope contract

Commit: `6946ce4`.
Files: `docs/publication-envelope-contract-2026-10-20.md`; `benchmarks/graph-publication-family-inventory-2026-10-20.json`.
Verification: Complete-frame inventory covers all six pinned sources and 46,075 accepted single/batch links with no repeated refs or recognized orphan mutation carriers. Contract freezes explicit Graph-local and ordered ordinal rows, single/one-member-batch distinction, per-mutation origins, authoritative prior mappings, separate format/baseline controls, Graph/MemoryGraph cuts and every concrete reader/proof/copy/writer/converter seam. Source-code review corrects the inventory prose assumption about uniform origins: replay admits per-row-valid mixed origins; semantic reconciliation/migration retain their existing rejection. Documentation impact/structure checks pass with zero findings.
Blockers: None for the contract. Execution gates211–219 remain unaccepted.
Documentation impact: Current body layouts, authority and concrete failure gates frozen in the envelope contract.
Architecture impact: Graph keeps ordinal-table and semantic transaction authority; Container continues to own physical allocation, reservation and time.


### Step 211 accepted — Graph single/batch publication body codecs

Commit: `b939ff2`.
Files: `src/semantic_graph/graph_publication_codec.rs`; `graph_publication_codec_tests.rs`; `graph_compact_codec.rs`; `docs/storage-format.md`.
Verification: All 11 compact Graph codec tests pass, including five native-publication tests. Literal single/batch bytes retain local and ordered ordinal fields without global duplication; one-member batches and per-row origins survive. All ten kind codes, three origins and both active flags round-trip. Zero clocks, invalid flags/kinds/origins/counts, unsupported old/native versions, controls, truncation and trailing bytes reject. Coordinate adaptation retains old link targets and uses the envelope itself for native references. Test compilation passes.
Blockers: None for codecs. Graph admission/consumer/writer/converter gates212–219 remain pending.
Documentation impact: Native Graph body fields and separate admission/writer gates added to storage-format.
Architecture impact: Graph-owned child codec shares actual ordinal row field helpers; no Container semantic decoder or nested standalone framing added.


### Step 212 accepted — Atomic Graph envelope admission in REL/PHY

Commit: `aea161e`.
Files: `graph_rebuild.rs`; `graph_store.rs`; REL/PHY lifecycle dispatch; Graph publication replay and overflow tests.
Verification: 105 publication-filtered tests and 117 Graph-filtered regressions pass (one existing Graph test ignored). Native single and mixed-origin batch transactions reopen through both owners exactly once with the original time and Graph/MemoryGraph cuts. Missing/future mappings, duplicate keys, invalid endpoint/origin contracts, local gaps, reversed/duplicate reserved globals and future global claims reject without semantic mutation. Native use rejects Format1; bare bodies and repeated links reject, while foreign bodies are not admitted as Graph publications. Rebuilt local overflow installs no transaction, MemoryGraph cut or mutation count; pending old carriers are consumed only after successful admission.
Blockers: None for owner admission. Concrete historical/proof/copy readers remain unaccepted at213–217; ordinary writers remain unchanged until218.
Documentation impact: Admission ownership, exact-once PHY routing and preflight behavior recorded in storage-format.
Architecture impact: Concrete owner routes delegate to GraphOpenState; Container remains unaware of Graph body semantics.


### Step 213 accepted — Graph and MemoryGraph historical/current readers

Commit: `e5ec250`.
Files: `graph_store_compact.rs`; `historical_graph_fingerprint.rs` and tests; `cva_freshness.rs`; `docs/storage-format.md`.
Verification: All four historical Graph tests and 118 Graph regressions pass (one existing ignored). Populated carrier/native parity covers all seven local cuts including zero, sparse MemoryGraph positions [0,1,4,5,6], every global and optional-time cut, raw mapping/baseline identity and exact stable-ID mutation order. The fixture includes ordinary and one-member batches, mixed origins, Entity/Principal-only gaps, deactivation, lifecycle exclusion and a nonempty rewrite baseline. Current semantic topology matches retained replay. Concrete Dream graph-window readers use the same owner coordinate/body adapters; downstream proof integration remains its separate215 gate.
Blockers: None for these readers; reconciliation/import, proofs, reclamation/copies, ordinary writer and converter gates214–219 remain pending.
Documentation impact: Concrete native reader semantics and physical Container boundary recorded in storage-format.
Architecture impact: GraphStore resolves native ordinal bodies; concrete historical readers consume accepted owner coordinates without adding a generic Container decoder.


### Step 214 accepted — Graph reconciliation and import transaction evidence

Commit: `bfb20d1`.
Verification: Test compilation and formatting pass. Both native interval tests, 56 reconciliation tests and 99 Graph regressions pass. Native source coordinates preserve exact mutation order, single versus one-member batch shape, optional transaction time and normalized stable-ID evidence. Duplicate replay is a byte-identical no-op; mixed-origin replay preserves the existing rejection policy before destination mutation. Completed-epoch inventory and interval coordinates use the Graph owner adapter.
Blockers: None for this gate; downstream proofs215 remain pending.
Documentation impact: Acceptance ledger updated.
Architecture impact: Graph owns body interpretation and replay shape; Container remains physical.


### Steps 215–217 accepted — Native Graph proofs, reclamation and semantic copies

Commit: `c0a6b0f`.
Verification: 107 Graph-focused tests and 2,009 broad units pass (five existing ignored; three long integration sweeps remain assigned to Gate E). Test compilation and formatting pass. Native source-history digests match compact history at every watermark and bind transaction shape, ordered fields and stable endpoint identities. REL/PHY prune/reopen rejects valid-but-substituted ordinal mappings. Accepted Dream graph cuts, committed roots and exact Community generation/version/membership survive native batches, later publications and unrelated Memory prune/reopen. Reclamation retains exact native frames/mappings and drops unpublished carriers; malformed/truncated/trailing/foreign bodies reject. Native REL/PHY reclaim, principal/attachment repacks, fresh-history rewrite/follow-up writing and PHY owner-ID migration pass. Migration retains single versus one-member batch shape, original order and optional transaction times; mixed-origin replay rejects before output creation.
Blockers: None for these consumer gates. Ordinary writers218, offline Graph conversion219, coverage audit220 and integration Gate E remain pending; this is not final runtime retirement.
Documentation impact: Storage-format documents Graph import normalization, semantic proof identity and native relocation/reclamation. Structural/change-impact policy verification is recorded with the execution checkpoint.
Architecture impact: Graph continues to own ordinals/body semantics. Concrete proof and maintenance consumers call Graph adapters; no Container-level semantic framework or ownership change. Full refreshed architecture enforcement remains an integration Gate E requirement.


### Step 218 accepted — Ordinary Graph allocation-envelope writers

Commit: `308e725`.
Verification: 108 Graph writer/consumer tests pass, including every byte prefix of the final native publication followed by reopen/retry. The subsequent 110-test Graph run includes both synthetic converter tests; 2,012 broad units pass, five existing ignored and three long sweeps assigned to Gate E. Native REL/PHY writers and fresh rewrites emit one allocating Graph envelope per transaction, preserving ordering, origins, local/global/time cuts, no-op byte stability and single/batch shape. Validation, revision/duplicate checks, body/count/ordinal planning and successor checks precede writes. New mapping/format state installs only after envelope append; transaction state and clocks commit at the same point. Legacy migration fixtures explicitly translate current Graph publications into old carriers/links before header downgrade.
Blockers: None for ordinary writers. Offline Graph conversion219 and coverage220 remain unaccepted until their separate measurements/audit complete.
Documentation impact: Storage-format records direct native writer commit sequencing and incomplete-tail recovery.
Architecture impact: Concrete Graph writer retains ordinal ownership and calls the physical Container allocation-envelope API; no per-member tickets or whole-store clone.


### Steps 219–220 accepted — Graph envelope conversion, framing and owner coverage audit

Implementation: `e1e4d9e`; six-source measurement: `0113e78`.
Verification: 110 Graph tests and 2,012 broad units pass. Explicit V1 REL/PHY fixtures cover single, one-member batch and ordered mixed-origin batches, reserved gaps, optional times, inactive and repeated postimages, raw mappings, inert carriers and invalid-history cleanup. Six pinned conversions pass in 288.52 seconds with unchanged source bytes/hashes/mtimes and no published output. Ordered accepted facts, Graph/MemoryGraph cuts, mapping bytes and all ticket times remain exact. Across 46,075 accepted Graph publications, native envelope frames total 2,867,407 bytes versus 3,927,132 compact carrier/link framed bytes: 1,059,725 saved. Actual legacy source carrier/link frames total 8,023,474 bytes; reservations, mappings and other owners are excluded from these Graph totals. See the tracked Graph envelope migration JSON.
Audit220 (Luna, read-only): Memory, Entity, Relationship, Archive and Graph each have concrete codec/admission, consumer/proof/copy, writer/converter and measured-framing evidence. Ego is explicitly local-only and not applicable; grouped Memory and external/Insomnia publications are outside the selected envelope set. No hidden remaining family or missing valid-envelope consumer was found. Graph219 measurement is now complete; no new owner-specific execution row is required by this audit.
Blockers: Integration Gate E remains in progress pending refreshed architecture enforcement. The 192-test historical-oracle run passes with one existing ignored. Legacy Archive integration fixtures were corrected in `cb95e38` without weakening their contract checks; the exhaustive checkpoint rerun passes. Runtime retirement remains a later gate.
Documentation impact: Plan/execution ledger and Graph measurement artifact updated; integration acceptance is explicitly separate.
Architecture impact: The offline Graph-owned converter translates accepted link slots with BindReserved, preserving mappings and reservations. Shared transport only composes concrete owner plans. No new cross-owner semantic authority.


### Dream population discovery crosswalk — before Step221 execution

Read-only preparation found responsibilities hidden in the former first-pass block: Memory-local order differs from reserved global order; accepted direct, Insomnia and external birth/cohort evidence has its own physical visibility; and reconciliation, imports, rewrite and pruning can change destination-prefix representability. Exact legacy sets include archived and tombstoned births. The existing journal cannot derive these facts from Memory current IDs or end-of-file birth evidence.

Former Steps221–231 are now27 explicit owner-specific rows221–247. The current codec has one encoder and one decoder for both cut and explicit-exception variants, avoiding artificial per-variant implementation rows. Exception read/write/replay and all destination consumers precede ordinary writer cutover. Memory prefix, source-specific birth admission, REL coordination, pure digest, consumer validation, migration equality and maintenance preservation remain independent gates. No Phase N implementation has started while Integration Gate E is pending.

Former later rows232–339 become248–355; the vector-byte measurement formerly273 is now289 and its reuse decision points there. Accepted rows1–220 retain their identities, and earlier crosswalks remain historical. The current structural requirement is355 consecutive unique execution rows; this adds16 real responsibilities to the prior339-row plan, with integration gates still unnumbered.


### Completed-import Graph authority discovery crosswalk

A final edge-case audit found that ordinary semantic member deduplication cannot be reused blindly inside a new completed import interval. Replaying source batch[A,B] over an independently accepted A can emit[B], while completed-epoch authority still binds[A,B]; full overlap can omit the publication entirely. Source epoch authority must retain its original ordered transaction facts, and repeat reconciliation must not conflict with that same accepted epoch.

This is explicit supplemental Graph Step221 before Integration Gate E acceptance. Former unexecuted rows221–355 shift to222–356; accepted1–220 remain unchanged. The Dream block is222–248 and the vector-byte measurement is290. The current structural requirement is356 consecutive unique execution rows. Existing semantic merge deduplication outside completed epoch authority is a separate policy and is not silently broadened or removed.


## Physical-prefix scan dependency — 2026-10-21

REL coordination needs the exact accepted global frontier while replaying Memory-local metadata before Begin. The existing bounded visitor is read-only but lacks that frontier; the full versioned scanner can repair a later truncated tail and is not a bounded read-only query. Add explicit Container Step229 for a thin bounded visitor reusing existing version observation, with independent physical-clock and unchanged-source tests. Former future229–356 shift to230–357; accepted1–228 remain unchanged. The Dream block ends249 and vector unchanged-byte measurement becomes291. The current structural requirement is357 consecutive unique rows; integration gates remain unnumbered.


### Entity Resolution import-consumer expansion (2026-10-21)

Discovery after stream acceptance found two independent consumers of full
resolution carriers: normalized completed-import owner evidence and ordered
interval replay. Delta inheritance must be materialized in source order for
comparison, then re-encoded against destination state for transport. These are
separate failure domains from owner replay and offline V1 conversion. New
279–280 make them explicit before implementation; former pending279–357 become
281–359. Accepted1–265 are unchanged. The current plan has359 contiguous numbered
execution rows; integration gates remain unnumbered.


## Proof-store and vector-decision acceptance — 2026-10-21

Entity Resolution and Community implementation/evidence commit: `ddde79a`.
Vector workload and measured deferral are recorded in their dedicated decision and artifacts.

### Step 273 execution record

Verification: Every persisted field is classified in the committed Entity Resolution contract; candidate IDs, fingerprints, timing and attempt counts stay authoritative.

Blockers: none for this item. Canonical corpus equivalence, promotion and legacy retirement remain separately gated.

### Step 274 execution record

Verification: Frozen literal-first/evidence-inheritance contract preserves every accepted field and names source comparison versus destination transport.

Blockers: none for this item. Canonical corpus equivalence, promotion and legacy retirement remain separately gated.

### Step 275 execution record

Verification: Independent initial Pending vector carries every required field; initial Dormant includes dormant time. Truncation and illegal field/status/count/reason checks pass.

Blockers: none for this item. Canonical corpus equivalence, promotion and legacy retirement remain separately gated.

### Step 276 execution record

Verification: Attempt-only delta has an independently asserted73-byte vector. All evidence fields change literally; status transitions and tombstones round-trip.

Blockers: none for this item. Canonical corpus equivalence, promotion and legacy retirement remain separately gated.

### Step 277 execution record

Verification: Every accepted Pending/Dormant/wake/retarget/terminal/tombstone/revision-reset postimage equals its full-field baseline. Missing or terminal inheritance bases reject.

Blockers: none for this item. Canonical corpus equivalence, promotion and legacy retirement remain separately gated.

### Step 278 execution record

Verification: Ordinary put, tombstone and import-current writes use CVAERS02. Validation precedes append; replay and failed/conflicting changes preserve projections.

Blockers: none for this item. Canonical corpus equivalence, promotion and legacy retirement remain separately gated.

### Step 279 execution record

Verification: Completed-import owner evidence materializes full source changes in physical order and uses the established complete comparison carrier; omission bytes never become identity.

Blockers: none for this item. Canonical corpus equivalence, promotion and legacy retirement remain separately gated.

### Step 280 execution record

Verification: Dedicated interval test starts with different destination evidence under the same key/revision; re-encoded output exactly matches the source postimage without source mutation.

Blockers: none for this item. Canonical corpus equivalence, promotion and legacy retirement remain separately gated.

### Step 281 execution record

Verification: Genuine V1 REL conversion preserves every resolution change and final state, then native reclamation preserves those postimages. Current resolution carriers explicitly reject in V1 REL and PHY.

Blockers: none for this item. Canonical corpus equivalence, promotion and legacy retirement remain separately gated.

### Step 282 execution record

Verification: All71 initial focused cases and nine latest delta-specific cases pass, including the two added inheritance/V1 boundary checks. Full root2,102 cases pass with all three exhaustive recovery cases and no filtered cases.

Blockers: none for this item. Canonical corpus equivalence, promotion and legacy retirement remain separately gated.

### Step 283 execution record

Verification: Raw64-attempt/two-candidate workload including every frame measures13,952 legacy bytes versus7,338 current bytes. Initial payload grows210 to211; no whole-file or compression claim.

Blockers: none for this item. Canonical corpus equivalence, promotion and legacy retirement remain separately gated.

### Step 284 execution record

Verification: Committed Community decision inventories freshness proof cuts, Dream roots, lineage/split/merge/name inheritance, semantic-name evidence, import normalization and historical manifests.

Blockers: none for this item. Canonical corpus equivalence, promotion and legacy retirement remain separately gated.

### Step 285 execution record

Verification: Exact reconstruction would require unique membership/name evidence from the recorded graph cut, algorithm, seed, resolution, quality and retained nondeterministic authority.

Blockers: none for this item. Canonical corpus equivalence, promotion and legacy retirement remain separately gated.

### Step 286 execution record

Verification: Independent complete PHY copies and a durable regression accept different one-versus-three-community partitions with identical recorded coordinates. The uniqueness claim is disproven.

Blockers: none for this item. Canonical corpus equivalence, promotion and legacy retirement remain separately gated.

### Step 287 execution record

Verification: Explicit no-change Community decision retains membership snapshots and non-recomputable names. No production Community codec changes.

Blockers: none for this item. Canonical corpus equivalence, promotion and legacy retirement remain separately gated.

### Step 288 execution record

Verification: Not applicable under the explicit no-change decision; codec/replay/writer remain unchanged.

Blockers: none for this item. Canonical corpus equivalence, promotion and legacy retirement remain separately gated.

### Step 289 execution record

Verification: Not applicable under the no-change decision; existing literal membership/name transport remains required.

Blockers: none for this item. Canonical corpus equivalence, promotion and legacy retirement remain separately gated.

### Step 290 execution record

Verification: No new compact-name format exists. The uniqueness counterexample plus existing algorithm1/name-lineage/Dream/freshness regressions pass in the root suite.

Blockers: none for this item. Canonical corpus equivalence, promotion and legacy retirement remain separately gated.

### Step 291 execution record

Verification: Raw Community compaction measurement is not required for the rejected reconstruction strategy; exact counterexample artifact is retained.

Blockers: none for this item. Canonical corpus equivalence, promotion and legacy retirement remain separately gated.

### Step 292 execution record

Verification: Durable reproducible workload builds eight generations under one or two profiles, with independent immutable conversations and12/384 dimensions; every historical matrix reopens byte-exact.

Blockers: none for this item. Canonical corpus equivalence, promotion and legacy retirement remain separately gated.

### Step 293 execution record

Verification: Synthetic artifact separates28 same-profile unchanged rows from profile multiplicity. Six pinned sources are independently scanned with before/after hashes, published references, shape and matrix-identity checks; none has repeated generations within a profile.

Blockers: none for this item. Canonical corpus equivalence, promotion and legacy retirement remain separately gated.

### Step 294 execution record

Verification: Explicit measured deferral: high-dimensional synthetic row reuse saves74.31%, tiny rows grow6.25%; retained corpus has zero same-profile historical repeats. Separate within-matrix savings and growth are reported without relabeling them historical reuse.

Blockers: none for this item. Canonical corpus equivalence, promotion and legacy retirement remain separately gated.

### Step 295 execution record

Verification: Not applicable under the explicit row/block deferral; no new immutable identity is introduced.

Blockers: none for this item. Canonical corpus equivalence, promotion and legacy retirement remain separately gated.

### Step 296 execution record

Verification: Not applicable under the deferral; existing exact packed-matrix storage remains.

Blockers: none for this item. Canonical corpus equivalence, promotion and legacy retirement remain separately gated.

### Step 297 execution record

Verification: Not applicable under the deferral; no generation manifest/delta family is introduced.

Blockers: none for this item. Canonical corpus equivalence, promotion and legacy retirement remain separately gated.

### Step 298 execution record

Verification: Not applicable under the deferral; existing idempotent whole-matrix content identity remains.

Blockers: none for this item. Canonical corpus equivalence, promotion and legacy retirement remain separately gated.

### Step 299 execution record

Verification: Not applicable under the deferral; no new vector format requires conversion.

Blockers: none for this item. Canonical corpus equivalence, promotion and legacy retirement remain separately gated.

### Step 300 execution record

Verification: No new wire format exists; existing profile, packed/Memory/Archive vector, generation, retrieval and maintenance regressions pass in the complete root run.

Blockers: none for this item. Canonical corpus equivalence, promotion and legacy retirement remain separately gated.

### Step 301 execution record

Verification: Phase deferred explicitly in the vector reuse decision. Both synthetic and authoritative raw-byte models are retained, with modeled metadata/framing and compression excluded.

Blockers: none for this item. Canonical corpus equivalence, promotion and legacy retirement remain separately gated.

Integration Gate H: complete root verification passes2,102 library cases, five existing ignored, zero filtered; all157 integration cases and doctests pass. The latest nine delta-specific cases pass. The intermediate broader run hit one Windows sharing violation in unrelated Lore scratch deletion; its isolated rerun and the complete root rerun pass. Gate I: all unchanged vector/profile/retrieval/maintenance suites and the exact synthetic workload pass. No new vector format is claimed.


## Retained Entity Resolution family correction — 2026-10-21

The latest six-source matrix stopped before output on the first REL. Its pinned
V1 bytes already contain CVAERS02, a family the frozen source decoder (CVAERS01
only) treats as inert. The proposed current delta tag accidentally collided with
it. Pending retained records are eight bytes longer than the current full-field
model. No source was changed or promoted.

New308–314 separate inventory/contract, distinct current tag, bounded literal
evidence codec, replay, offline transport, physical admission and parity. Existing
future308–359 shift to315–366; accepted1–301 remain numbered unchanged. Prior
synthetic Entity Resolution acceptance requires revalidation with the corrected
current tag. Gate H is pending that revalidation; its earlier2,102-test run is
preserved as historical evidence, not substituted for the corrected source.

The contract follows the frozen reader's observable behavior: original CVAERS02
records remain inert. Their complete original bytes are retained as concrete
owner evidence, rather than inferred as active decisions or interpreted as a new
clock. Unknown authority outside this specifically inventoried family still
fails closed. Reactivating those older decisions would be a separate semantic
policy change outside this physical history migration. Integration gates remain
unnumbered; the structural requirement is366 consecutive unique execution rows.


### Source audit correction — standalone import evidence

The Step 304 audit found a real remaining dependency: the existing normalization
retains CVAFRS03–05 as authenticated predecessors, and CVAILEG2 still binds the
old physical prefix. Those outputs cannot support legacy reader deletion.
Steps 315–318 explicitly convert this owner evidence before corpus acceptance;
previous future315–366 become319–370. Accepted1–301 are unchanged. Integration
gates remain unnumbered. No corpus promotion or retirement is accepted yet.


### Upgrade coverage and retained resolution acceptance — Steps302–313

Accepted against `d51d872`: current inventory164 identities in68 families; Archive audit; import audit identifying and explicitly scheduling the standalone-proof gap; literal Echo and genuine PRJCOR01 conversion parity; fail-closed unknown-family admission; frozen CVAERS02 zero-effect contract; current CVAERS03; exact CVAERL01 byte preservation and bounded checksum validation; both generic and explicit REL/PHY admission. Corrected core verification passes2,107 cases (five existing ignored);76 focused resolution/import cases pass. Corrected architecture capture `aec4d1958c962d3b2a5d500996c4a9a3b28877ee4c419b4e8fd9d78b6b25bddb` passes23 rules with zero compatibility warnings. Both documentation-policy modes pass on the pre-commit byte-verified1,808-file snapshot. Step314 corpus proof, standalone import315–318, promotion and retirement are still outstanding. No later step is accepted by this checkpoint.


### Accepted Steps314–327 — canonical conversion and owner admission

Standalone import conversion is committed as `f03c583`. Every physical source proof validates before its legacy predecessor/epoch binding is removed; native completion rebinds only the derived settlement-byte digest. All2,110 core cases and157 integration cases pass, including exhaustive recovery and focused damaged-source/native retry regressions.

All six pinned sources now have admitted disposable V2 copies with exact source hashes/modification times, owner UUIDs and owner heads preserved. The [conversion results](canonical-history-migration-results-2026-10-21.md) link per-file hashes and framed-byte inventories, concrete-family accounting including envelope overhead, and independent exact resolution history for8,134 active postimages plus169 inert original records. Total raw bytes138,196,316 become130,962,204:7,234,112 saved (5.235%), with compression excluded and standalone import-proof growth fully charged.

This accepts corrected-resolution314, explicit import315–318, REL/PHY admission319–320, six disposable conversions321–326 and accounting327. Exhaustive remaining-owner history328–336, reconciliation/maintenance, final measurements, authoritative promotion and reader/upgrader retirement remain pending. No output has been promoted. The standalone-import architecture refresh is collecting; the prior corrected-resolution architecture snapshot is not substituted for it.


The standalone-import architecture capture subsequently passed: snapshot `dc54dee47978dbf0eaf046cf6fe06a364a9106e5e7b789e6caee103924ece48e`,23 rules,109,407 nodes/4,184 relationships and zero compatibility warnings. All15 affected production paths match its source mirror after line-ending normalization; the added cfg(test) performance declaration and later test-only oracle changes are explicitly separated in the binding artifact. The six original inputs and six metadata files are preserved by exact hash before any future promotion; the fixture repository remains unmodified by this continuation.


### Accepted Steps328–336 — complete six-source history comparison

The optimized canonical matrix completed all six source/output pairs in2,338.55seconds with zero owner-domain mismatches and unchanged authoritative sources. The permanent [history oracle](../benchmarks/canonical-history-equivalence-2026-10-21.json) records every Memory, Entity, Relationship, Graph, MemoryGraph and Ego cut; every Freshness activity cut and accepted Event coordinate; every vector generation; Dream pair/processing/proof identities; and exhaustive global ordering with transaction times. Resolution postimages and inert literal evidence are independently covered by Step314.

Archive compares every ordered publication, origin, dependencies, global/time coordinates and orphan behavior, with exhaustive publication-prefix digests. Whole Archive snapshots and whole-owner global projections use recorded representative cuts where the measured cost of all snapshots is prohibitive; exhaustive physical publications, owner histories and global ordering are retained. Cached test-only inputs are cross-checked against ordinary queries at initial/middle/head cuts, without changing runtime behavior. Public maintenance, promotion and retirement remain pending.


### Measurement decisions for Steps344–352

The [performance artifact](../benchmarks/canonical-history-performance-2026-10-21.json) records18 isolated optimized processes: six source and six current reopen/lookup cases plus six migrations. Step344 uses a fresh-process first-open definition; the OS file cache is uncontrolled, so no cold-disk result is claimed. Step345 records three warm reopens. Step346 records preparation separately and three repetitions each at oldest/middle/recent Memory cuts, with equal fingerprints in every source/output pair. Steps347–348 record planning/staging/total throughput and Windows lifetime peak resident/committed bytes sampled every5ms; all positive measurements include fixed harness overhead.

Steps349–352 close with measured deferral: the largest warm reopen is approximately7.9seconds, and measured preloaded Memory queries remain below one second. No named latency budget requires a checkpoint layer in this change. Revisit against an explicit workload/target; no checkpoint format, load path or corruption fallback is introduced. Community reconstruction remains rejected by the uniqueness counterexample and historical vector row reuse remains deferred by zero repeated same-profile corpus generations. Measurements and conditional decisions are complete, but the sequential accepted counter remains336 until maintenance337–343 closes.

ADR0042 records the owner-specific compact-history design and final hard-cut obligations. Its status explicitly leaves promotion and retirement pending; adding the ADR does not satisfy final acceptance.


### Accepted Steps337–359 — maintenance, measurement and pre-promotion documentation

All six public-client cases complete in[the acceptance artifact](../benchmarks/canonical-history-public-acceptance-2026-10-21.json). Frozen/current semantic comparison classifies identical, extension and divergence identically for all three RELs. Current identical/extension reconciliation outputs are byte-for-byte the selected source, including exact appended Echo evidence. All three divergences refuse before output with the same historical-checkpoint rebasing decision as the independently replayed frozen V1 policy. This accepts337 without adding a dual-format reconciliation path.

Steps338–340:53 focused current compact prune cases pass, covering REL/PHY Memory, Entity/Relationship descriptions, vectors, topology, invalid proof and authorization behavior. The six retained files contain no terminal Memory target; empty prune requests correctly refuse before creating output. All six reclaimed outputs are byte-for-byte identical to their canonical inputs, so the entire already-verified history oracle is retained, not merely current heads.

Steps341–343: all six files recover partial frame-header and partial frame-body tails to exact original bytes and identical public snapshots. Every default fresh-history rewrite reopens, emits exactly planner-selected IDs and preserves complete selected current Memory facts after normalizing only the intentionally new revision/global/local coordinates. All original inputs remain unchanged. Output paths and hashes are retained in[repair/rewrite evidence](../benchmarks/canonical-history-repair-rewrite-2026-10-21.json). Exact-byte maintenance proofs establish every historical cut for recovery/reclaim/selected-source copies; rewrite comparison follows its explicit new-history policy.

Steps344–352 close under the measured definitions/deferrals recorded above. Steps353–358 add ADR0042/index navigation, current format rollout status, architecture/invariants, limitations and maintainer ownership for the permanent current-only oracle. Both official documentation-policy modes pass on the byte-verified1,825-file snapshot. The final compression record explicitly reports absent ADR-0004 pack measurements separately from raw logical savings.

Step359 has a bounded capture limitation: snapshot dc54dee47978dbf0eaf046cf6fe06a364a9106e5e7b789e6caee103924ece48e passes23 rules and binds every affected production path. Subsequent source changes are test-only oracle/performance code and the cfg(test) root declaration, explicitly enumerated by the binding artifact. No runtime owner implementation has changed since capture. A fresh complete post-retirement capture remains mandatory under364; the old snapshot will not be represented as capturing deleted readers. Promotion360 and retirement361–364 are not yet accepted.


### Accepted Step360 — authoritative canonical promotion

All six verified outputs are promoted, SHA256SUMS.txt and AUTHORITATIVE.txt are updated, and fixture repository commit `2b170044d3fd6b693baf0dec326c6429c66d8a9f` contains the12 exact intended paths. Original V1 files and prior metadata remain preserved under the source-preservation manifest. The preexisting dirty fixture state was hash-checked before replacement. Two Ellis originals were read-only; Windows rejected atomic replacement until that attribute was cleared. The resumed operation admitted only the exact already-verified old/new hashes, and current V2 fixtures are writable for normal owner open. All six promoted hashes and2.0 headers are rechecked.

[Promotion evidence](../benchmarks/canonical-history-promotion-2026-10-21.json) binds fixture commit, source/current hashes and owner heads. Runtime retirement361–364 now begins. The task remains incomplete until current-only post-retirement acceptance; promotion alone is not completion.

### Retirement maintenance audit correction — 2026-10-21

The first current-only unit run exposed orphan-maintenance fixtures that still depended on retired standalone carriers. New364–370 make each current owner’s bare-body pruning/reclamation behavior explicit; former364–370 move to371–377. No old physical readers are restored. Accepted1–360 retain their numbers. The runtime routing regression found in the same run remains within Step361 and must pass existing native PHY Relationship tests.


### Accepted Steps361-370 - current-format retirement

Implementation commit: `436006a989c7b6bfb07026a39bde290fa225eb6d`.

Normal Container/REL/PHY admission now requires exactly V2.0 before writable access, semantic replay or recovery. Old and unknown headers return typed UnsupportedVersion without changing source bytes. Current semantic owners require durable identity. The V1 reader/dispatch, temporary Freshness normalization, CVAFRS03/04/05 and CVAILEG1/2 compatibility, storage-upgrade modules, migration APIs and CLI migration command are removed. Catalog conversation-title backfill remains an independent current-owner operation.

Entity and Relationship description pruning removes selected bare current creation/delta bodies and re-encodes accepted redacted chains with authenticated original-carrier/manifest proofs. Reclamation classifies unpublished current Memory, Entity, Relationship, Archive and Graph bodies without admitting semantic clocks. Accepted envelopes, immutable content/body closure, owner identity, exact histories and times survive. Archive content/format backing and Ego terminal CVAETM01 are distinguished from publication metadata and Entity merge CVAETM02. Reconciliation and Graph historical readers no longer dispatch through retired physical codecs; comparison encodings retain their stable evidence schema only.

Complete root acceptance: **1,942 passed, zero failures, five existing opt-in ignores, zero filtered**, including all exhaustive recovery cases (635.61 seconds). CLI check and all **32 tests** pass. Archive roundtrip passes a prepared five-node/two-branch fixture with two fragments and exact content/path reconstruction. Explicit six-file promoted-owner admission passes exact hashes, heads, unchanged bytes and modification times. Retired API/module audit has zero hits; the current inventory classifies146 identities in61 families and rejects18 deleted identities. Removed parity-only tests and replacement coverage are recorded in the retirement-test artifact.

Step371 remains pending the serial integration build, exhaustive current-only frozen oracle, final documentation snapshot and fresh architecture capture. Parallel integration compilation encountered crate-loading failures; that attempt is not acceptance. Temporary source snapshots were preserved under `.test-tmp/out/architecture-input-exclusions` so the architecture scanner does not treat copied historical source as authored code. No older architecture snapshot is substituted for post-retirement evidence. Steps372-377 remain unaccepted until closeout.


### Accepted Steps371-377 - final hard-cut acceptance

Step371 passes current-only post-retirement verification: 1,942 root library tests, 161 integration tests across 25 suites, 32 CLI tests, six-file promoted admission, the exhaustive six-file frozen history oracle (1,485.30 seconds), and 24 complementary authoritative contracts (340.18 seconds). Root/CLI checks and formatting, examples, doctests, Archive roundtrip and both final documentation-policy modes pass. Existing opt-in ignores remain explicit; the authoritative tests were run separately against the promoted fixture root. No authoritative bytes or modification times changed.

Fresh architecture snapshot `f12c45ecf7ecf9cb30d80d743b3cf410113dff4dc1be158d1108511450d16b3a` passes all 23 rules with 98,633 source nodes, 3,489 relationships and zero compatibility warnings. The [retirement source binding](../benchmarks/canonical-history-retirement-architecture-2026-10-21.json) records 1,220 captured Rust files and 80 absent retired paths. All production paths match; one later integration-fixture update outside production ownership scope is explicitly hashed and passes the complete integration suite. The retired storage-upgrade ownership area and migration exclusions are removed from enforcement policy.

Step372 records implementation commit `436006a989c7b6bfb07026a39bde290fa225eb6d`, its complete changed-file inventory and separately hashed closeout changes in the [retirement inventory](../benchmarks/canonical-history-retirement-inventory-2026-10-21.json). The final closeout commit contains this acceptance record, evidence, documentation, policy cleanup and the current-envelope activity fixture correction. Work remains local; no push is part of acceptance.

Steps373-375 close with exact per-family/per-fixture accounting: 138,196,316 source bytes become 130,962,204 canonical bytes, saving 7,234,112 bytes (5.234663%). Every frame is charged once. ADR-0004 compression is separately deferred, with zero compressed bytes claimed. Community retains exact snapshots because reconstruction is nonunique; historical vector reuse lacks repeated same-profile corpus generations; checkpoints await a named workload and latency budget. The [final retirement report](../benchmarks/canonical-history-retirement-2026-10-21.json) links each measured decision and preserves original capture-time evidence without rewriting its historical status.

Steps376-377 accept the updated specification and shipped ownership boundaries after promotion, decoder/upgrader deletion and all post-retirement gates. Remaining limitations and measured deferrals are documented; no legacy-format runtime or converter is retained. **All 377 execution steps are accepted.**


### Accepted Steps371-377 - final hard-cut acceptance

Step371 passes current-only verification: 1,942 root library tests, 161 integration tests across 25 suites, 32 CLI tests, six-file promoted admission, the exhaustive six-file frozen history oracle (1,485.30 seconds), and 24 complementary authoritative contracts (340.18 seconds). Root/CLI checks and formatting, examples, doctests, Archive roundtrip and both final documentation-policy modes pass. Existing opt-in ignores remain explicit; authoritative tests were run separately against the promoted fixture root. No authoritative bytes or modification times changed.

Fresh architecture snapshot `f12c45ecf7ecf9cb30d80d743b3cf410113dff4dc1be158d1108511450d16b3a` passes all 23 rules with 98,633 source nodes, 3,489 relationships and zero compatibility warnings. The [source binding](../benchmarks/canonical-history-retirement-architecture-2026-10-21.json) records 1,220 captured Rust files and 80 absent retired paths. All production paths match; one later integration-fixture update outside production ownership scope is explicitly hashed and passes the complete integration suite. The retired storage-upgrade ownership area and migration exclusions are removed from enforcement policy.

Step372 records implementation commit `436006a989c7b6bfb07026a39bde290fa225eb6d`, its complete changed-file inventory and separately hashed closeout changes in the [retirement inventory](../benchmarks/canonical-history-retirement-inventory-2026-10-21.json). The final closeout commit contains this acceptance record, evidence, documentation, policy cleanup and the current-envelope activity fixture correction. Work remains local.

Steps373-375 close with exact per-family/per-fixture accounting: 138,196,316 source bytes become 130,962,204 canonical bytes, saving 7,234,112 bytes (5.234663%). Every frame is charged once. ADR-0004 compression is separately deferred, with zero compressed bytes claimed. Community retains exact snapshots because reconstruction is nonunique; historical vector reuse lacks repeated same-profile corpus generations; checkpoints await a named workload and latency budget. The [final retirement report](../benchmarks/canonical-history-retirement-2026-10-21.json) links each measured decision and preserves original capture-time evidence.

Steps376-377 accept the updated specification and shipped ownership boundaries after promotion, decoder/upgrader deletion and all post-retirement gates. Remaining limitations and measured deferrals are documented. **All 377 execution steps are accepted.**
