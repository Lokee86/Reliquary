# Minimal Lossless History Storage — Linear Implementation Plan

Date: 2026-10-19
Status: 112/279 execution steps accepted; compatibility-policy interruption I1–I12 complete (`fd61a1e`). Integration Gate C passed. Shared mechanics Steps 88–90 verified against `bafdaae`; guard and Entity contract Steps 91–92 against `3cc6631`; Entity codecs/rebuild and Steps 96–107 revalidated against `73e88ae`; Relationship contract/codecs/rebuild Steps 109–112 against `0650305`; next Step 113. Runtime retirement remains pending. Rebased checkpoint: [handoff](minimal-lossless-history-storage-rebased-freeze-2026-10-19.md).
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
| 131 | Sol | Hard-cut compact Ego writers. | No compact full-postimage revision writer remains except justified immutable payload records. |
| 132 | Sol | Extend upgrader for Ego history. | REL/PHY Ego parity passes. |
| 133 | Luna | Add activation/deletion/ownership adversarial fixtures. | REL/PHY restrictions and terminal semantics remain exact. |
| 134 | Sol | Measure raw Ego-history bytes before/after. | Size artifact recorded. |

### Integration Gate D — Postimage owners

Run Entity, canonical Relationship/Observation, Ego, lifecycle, reconciliation, REL/PHY ownership, and historical-oracle suites.

---

## Phase K — Graph Encoding Audit and Optimization

| Step | Owner | Work | Gate |
| ---: | --- | --- | --- |
| 135 | Luna | Produce byte-field inventory for current Graph mutation/batch/version records. | Semantic vs framing/identity bytes classified. |
| 136 | Sol | Decide whether Graph compact-ID dictionary/ordinal encoding is worthwhile from measured fixtures. | Measurement-backed yes/no decision recorded. |
| 137 | Sol | If justified, define the Graph local-ID table format and validation rules. | Corrupt/missing mappings fail closed. |
| 138 | Sol | If justified, implement Graph compact-ID encoding/decoding behind Graph codec boundary. | Decoded canonical mutations match existing semantics. |
| 139 | Sol | Extend upgrader for compact Graph encoding if Step 130 is implemented. | Graph historical oracle parity passes. |
| 140 | Luna | Add Graph mapping corruption/order/admission regressions if compact IDs are implemented. | Invalid mappings fail closed. |

---

## Phase L — Freshness Effect History

| Step | Owner | Work | Gate |
| ---: | --- | --- | --- |
| 141 | Luna | Produce field-level inventory of Freshness event payload: causal effects, proof evidence, and derived postimages. | Exact classification recorded. |
| 142 | Sol | Define compact Freshness event format without derived postimages, preserving all causal/proof fields. | Codec contract can represent current accepted events. |
| 143 | Sol | Implement compact Freshness event encoder/decoder. | Round-trip/corruption tests pass. |
| 144 | Sol | Implement replay that recomputes postimages from prior state + effects. | Valid events reproduce current postimages. |
| 145 | Sol | Port current forged/malformed-event validation so removal of persisted postimages does not weaken checks. | Equal-or-stronger negative-test matrix passes. |
| 146 | Sol | Hard-cut compact Freshness event writer to the new effect-only representation. | Compact files emit no event postimages. |
| 147 | Sol | Extend upgrader for V1.0 Freshness events while preserving exact accepted event identity/cuts/proofs. | All historical Freshness cuts match. |
| 148 | Luna | Add import-settlement and same-cut Freshness migration regressions. | CVAFRS06 proof semantics remain unchanged. |
| 149 | Sol | Measure raw Freshness-history bytes before/after. | Size artifact recorded. |

---

## Phase M — Publication Framing

| Step | Owner | Work | Gate |
| ---: | --- | --- | --- |
| 150 | Luna | Measure current global-ticket + domain-link/version framing by owner on authoritative fixtures. | Per-owner framing-byte artifact exists. |
| 151 | Sol | Define compact publication envelope semantics without changing ADR 0003 ownership/clocks. | Format contract distinguishes global order, transaction time, local version, payload/reference. |
| 152 | Sol | Implement envelope codec/validation. | Round-trip/corruption tests pass. |
| 153 | Sol | Migrate compact Memory publication to envelope. | Memory local/global/timestamp/crash tests pass. |
| 154 | Sol | Extend upgrader for Memory publication-envelope conversion. | V1.0 Memory global/local cut parity passes. |
| 155 | Sol | Migrate Entity publication to envelope if it removes separate framing. | Entity version/oracle parity passes. |
| 156 | Sol | Migrate canonical Relationship/Observation publication to envelope if applicable. | Local/global parity passes. |
| 157 | Sol | Migrate Ego publication to envelope if applicable. | Ego parity passes. |
| 158 | Sol | Migrate Archive publication framing only if inert-backing semantics remain equivalent. | Archive cut/crash/reclamation tests pass, or explicit defer decision recorded. |
| 159 | Sol | Migrate Graph publication framing only if local Graph semantics remain unchanged. | Graph parity passes, or explicit defer decision recorded. |
| 160 | Luna | Audit upgrader coverage for each envelope-migrated owner. This step performs no implementation. | If any migrated owner lacks explicit upgrade conversion, insert one owner-specific step before Gate E; otherwise cross-format global/local/timestamp parity is complete. |

### Integration Gate E — Graph, Freshness, and publication framing

Run Graph, Freshness, version/timestamp, crash/recovery, reclamation/reconciliation, and historical-oracle suites. Confirm no new cross-owner semantic coupling.

---

## Phase N — Dream First-Pass History

| Step | Owner | Work | Gate |
| ---: | --- | --- | --- |
| 161 | Sol | Define exact Memory-population-at-cut semantics corresponding to current Dream `preexisting_ids`. | Synthetic reference tests reproduce current sets. |
| 162 | Sol | Implement Memory population reconstruction at that cut. | Fixture Dream begin sets match reconstructed sets. |
| 163 | Sol | Define canonical population digest. | Stable digest tests pass. |
| 164 | Sol | Define compact Dream Begin codec using Memory cut + digest instead of full set. | Codec round-trip/corruption tests pass. |
| 165 | Sol | Update Dream validation to reconstruct population and validate roots against it. | Existing Dream validity rules remain exact. |
| 166 | Sol | Hard-cut compact Dream Begin writer. | Compact files stop persisting full `preexisting_ids`. |
| 167 | Sol | Implement V1.0 Dream migration cut derivation. | Representable legacy passes derive exact cuts. |
| 168 | Sol | Implement V1.0 Dream migration equality proof against the legacy explicit set. | Mismatch rejects. |
| 169 | Sol | Implement explicit-set exception encoding for legacy passes not representable by one exact cut. | Exceptional history remains semantically exact. |
| 170 | Luna | Add wrong-cut, forged-digest, root-outside-population, and source-in-population regressions. | Invalid histories fail closed. |
| 171 | Sol | Measure Dream first-pass raw bytes before/after. | Size artifact recorded. |

---

## Phase O — Dream Pair History

| Step | Owner | Work | Gate |
| ---: | --- | --- | --- |
| 172 | Sol | Define validated owner-local Memory-ID table for Dream pair physical encoding. | Table format/validation tests pass. |
| 173 | Sol | Implement compact Dream pair encoder/decoder using local references. | Decoded canonical pair set matches existing semantics. |
| 174 | Sol | Hard-cut compact Dream pair writer. | No compact full-ID pair record writer remains. |
| 175 | Sol | Extend upgrader for V1.0 Dream pairs. | Pair-set parity passes. |
| 176 | Luna | Add missing/corrupt mapping and retry regressions. | Invalid mappings fail closed. |
| 177 | Sol | Measure Dream-pair raw bytes before/after. | Size artifact recorded. |

### Integration Gate F — Dream

Run Dream/Freshness/import historical suites plus oracle comparison on Dream-bearing fixtures.

---

## Phase P — Interaction Stream

| Step | Owner | Work | Gate |
| ---: | --- | --- | --- |
| 178 | Sol | Freeze interaction stream state-machine semantics from current cumulative records. | Begin/append/interrupted transition table exists. |
| 179 | Sol | Define `StreamBegin` compact record. | Codec tests pass. |
| 180 | Sol | Define `StreamAppend` exact UTF-8 suffix record. | Codec tests pass. |
| 181 | Sol | Define terminal/interrupted compact record. | Codec tests pass. |
| 182 | Sol | Implement compact stream replay/materialization. | Every checkpoint reconstructs exact visible text/status. |
| 183 | Sol | Hard-cut compact stream writer to begin/append/terminal records. | No cumulative-prefix compact writer remains. |
| 184 | Sol | Update stream reconciliation for compact records. | Latest-state reconciliation matches current behavior. |
| 185 | Sol | Extend upgrader from cumulative V1.0 checkpoints to compact stream deltas. | Historical checkpoint parity passes. |
| 186 | Luna | Add long-stream linear-growth benchmark. | Growth is linear in appended text. |
| 187 | Luna | Add crash/reopen/interruption regressions. | Displayed-text durability invariant remains intact. |

---

## Phase Q — External Derived Index

| Step | Owner | Work | Gate |
| ---: | --- | --- | --- |
| 188 | Sol | Freeze the semantic boundary: `CVAXIDX1` is derived state; `CVAXMEM1` remains authoritative publication. | Boundary documented in tests/code comments. |
| 189 | Sol | Decide current-state vs incremental derived-index persistence based on rebuild/write behavior. | One strategy recorded. |
| 190 | Sol | Implement compact ExternalIndex codec for the selected strategy. | Round-trip/current-state tests pass. |
| 191 | Sol | Implement compact ExternalIndex rebuild/current projection. | Lookup behavior matches V1.0. |
| 192 | Sol | Make superseded derived index storage reclaimable without semantic prune markers. | Reclaim tests pass. |
| 193 | Sol | Extend upgrader to migrate only the current required derived index state. | External Memory publication history remains untouched. |
| 194 | Luna | Add damaged-index rebuild/discard regressions. | Existing recovery contract remains intact. |

### Integration Gate G — Operational/derived stores

Run interaction runtime, external-source index/publication, reconciliation, reclaim, and reopen suites.

---

## Phase R — Entity Resolution History

| Step | Owner | Work | Gate |
| ---: | --- | --- | --- |
| 195 | Luna | Classify every Entity Resolution persisted field as semantic proof, retry state, or reconstructible postimage/candidate material. | Field table complete. |
| 196 | Sol | Freeze compact Entity Resolution history contract from that classification. | Contract preserves every authoritative field. |
| 197 | Sol | Define initial candidate/evidence record. | Codec tests pass. |
| 198 | Sol | Define attempt/status delta record. | Codec tests pass. |
| 199 | Sol | Implement replay/materialization. | Accepted decisions/retries reconstruct exactly. |
| 200 | Sol | Hard-cut compact Entity Resolution writer. | No compact repeated full candidate snapshot remains unless classified authoritative. |
| 201 | Sol | Extend upgrader for V1.0 Entity Resolution history. | Decision/proof parity passes. |
| 202 | Luna | Add retry/conflict/import-readiness/adversarial resolution fixtures. | Existing Entity effects and readiness decisions remain identical. |
| 203 | Sol | Measure raw Entity Resolution history before/after. | Size artifact recorded. |

---

## Phase S — Community Proof Boundary

| Step | Owner | Work | Gate |
| ---: | --- | --- | --- |
| 204 | Luna | Identify every accepted proof that references Community generation/graph version/name. | Consumer inventory complete. |
| 205 | Sol | Define the exact reconstruction claim required to eliminate repeated membership snapshots. | Claim names graph cut + algorithm/policy version + any non-deterministic retained data. |
| 206 | Luna | Test that claim against authoritative/synthetic Community histories. | Exact yes/no evidence recorded. |
| 207 | Sol | If exact reconstruction is proven, define compact Community membership history; otherwise record explicit no-change decision. | One decision closed. |
| 208 | Sol | If compacting, implement Community codec/replay/writer. | Historical proof identity remains valid. |
| 209 | Sol | If compacting, extend upgrader. | Community oracle/proof parity passes. |
| 210 | Luna | If compacting, add semantic-name/non-determinism regressions. | Non-recomputable names/evidence remain retained. |
| 211 | Sol | If compacting, measure raw Community history before/after; otherwise mark measurement not required. | Evidence recorded. |

### Integration Gate H — Proof-boundary stores

Run Entity Resolution, Community, Dream/Freshness proof consumers, import readiness, and historical-oracle suites.

---

## Phase T — Archive Vector Generation Decision

| Step | Owner | Work | Gate |
| ---: | --- | --- | --- |
| 212 | Luna | Build repeated same-profile Archive-vector-generation workload. | Reproducible workload exists. |
| 213 | Luna | Measure unchanged exact vector-byte duplication across generations separately from compatibility-profile multiplicity. | Byte-duplication artifact exists. |
| 214 | Sol | Decide whether row/block reuse is justified by Step 213. | Explicit implement/defer decision recorded. |
| 215 | Sol | If justified, define immutable exact vector row/block identity preserving byte-distinct compatible embeddings. | Identity contract tests pass. |
| 216 | Sol | If justified, implement reusable row/block storage. | Exact vector bytes round-trip. |
| 217 | Sol | If justified, implement generation manifest/delta referencing row/block identities. | Historical generation materialization exact. |
| 218 | Sol | If justified, hard-cut compact Archive-vector generation writer. | No redundant full-matrix compact writer remains. |
| 219 | Sol | If justified, extend upgrader for historical generations. | Exact vector-byte/generation parity passes. |
| 220 | Luna | If justified, add retrieval and corruption regressions. | Retrieval parity and fail-closed behavior pass. |
| 221 | Sol | If justified, measure raw vector-generation bytes before/after; otherwise mark phase deferred. | Evidence recorded. |

### Integration Gate I — Vectors

Run profile, packed-vector, Memory-vector, Archive-vector, generation, retrieval, prune/reclaim, and rewrite-interaction suites.

---

## Phase U — Upgrade Coverage Completion

| Step | Owner | Work | Gate |
| ---: | --- | --- | --- |
| 222 | Luna | Re-run durable-family inventory against current source after all codec migrations. | No new/unhandled writer family appears. |
| 223 | Luna | Audit upgrade coverage for Archive-owned durable families after all earlier migrations. This step performs no implementation. | Either Archive coverage is complete or this plan is expanded with one explicit step per missing family before execution continues. |
| 224 | Luna | Audit upgrade coverage for import epoch and Historical Freshness settlement families. This step performs no implementation. | Either import-evidence coverage is complete or explicit per-family steps are inserted before execution continues. |
| 225 | Sol | Add/verify upgrader support for Echo records only. | Echo historical evidence parity passes. |
| 226 | Sol | Add/verify upgrader support for project-history correlation records only. | Project-history evidence parity passes. |
| 227 | Sol | Reject unknown/unsupported semantic authority explicitly during upgrade. | Unknown authority fails closed. |
| 228 | Sol | Reopen upgraded REL and validate all owner invariants. | REL validation passes. |
| 229 | Sol | Reopen upgraded PHY and validate all owner invariants. | PHY validation passes. |

---

## Phase V — Authoritative Fixture Migration (offline-only)

| Step | Owner | Work | Gate |
| ---: | --- | --- | --- |
| 230 | Luna | Upgrade disposable copy of 14-day REL. | Destination reopens; source hash unchanged. |
| 231 | Luna | Upgrade disposable copy of 14-day PHY. | Destination reopens; source hash unchanged. |
| 232 | Luna | Upgrade disposable copy of 28-day REL. | Destination reopens; source hash unchanged. |
| 233 | Luna | Upgrade disposable copy of 28-day PHY. | Destination reopens; source hash unchanged. |
| 234 | Luna | Upgrade disposable copy of Ellis REL. | Destination reopens; source hash unchanged. |
| 235 | Luna | Upgrade disposable copy of Ellis PHY. | Destination reopens; source hash unchanged. |
| 236 | Luna | Produce per-family raw-byte accounting for all six migrations. | Logical-history size report exists; compression excluded. |

---

## Phase W — Historical Equivalence Verification

| Step | Owner | Work | Gate |
| ---: | --- | --- | --- |
| 237 | Sol | Exhaustively compare Memory historical cuts source vs compact fixtures. | Zero mismatches. |
| 238 | Sol | Exhaustively compare Archive historical cuts where tractable. | Zero mismatches. |
| 239 | Sol | Exhaustively compare Entity and Relationship/Observation cuts. | Zero mismatches. |
| 240 | Sol | Exhaustively compare Graph cuts. | Zero mismatches. |
| 241 | Sol | Exhaustively compare Ego cuts. | Zero mismatches. |
| 242 | Sol | Exhaustively compare Freshness activity/event cuts. | Zero mismatches. |
| 243 | Sol | Compare vector-generation cuts and exact vector bytes. | Zero mismatches. |
| 244 | Sol | Compare Dream proof state. | Zero mismatches. |
| 245 | Sol | Compare global ordering points exhaustively where tractable and deterministically sample only where documented runtime makes exhaustive comparison unreasonable. | Zero mismatches; sampling rationale recorded if used. |

---

## Phase X — Maintenance and Reconciliation Verification

| Step | Owner | Work | Gate |
| ---: | --- | --- | --- |

| 246 | Sol | Verify compact-to-compact reconciliation against the frozen V1 semantic decision oracle. | Current-only reconciliation agrees with recorded baseline; no dual-format runtime path. |

| 247 | Sol | Run compact REL prune tests. | Maintenance behavior unchanged. |
| 248 | Sol | Run compact PHY prune tests. | Maintenance behavior unchanged. |
| 249 | Sol | Run compact reclaim tests. | Reclamation preserves semantic history. |
| 250 | Sol | Run compact repair tests. | Repair behavior unchanged. |
| 251 | Sol | Run fresh-history rewrite from compact sources. | Rewrite semantics unchanged. |
| 252 | Sol | Reopen all maintenance outputs and rerun relevant oracle checks. | Zero semantic mismatch beyond explicit maintenance policy. |

---

## Phase Y — Performance and Checkpoint Decision

| Step | Owner | Work | Gate |
| ---: | --- | --- | --- |
| 253 | Luna | Measure cold reopen old vs compact fixtures. | Results recorded. |
| 254 | Luna | Measure warm reopen old vs compact fixtures. | Results recorded. |
| 255 | Luna | Measure recent, middle and oldest historical lookup on frozen V1 and canonical V2 fixtures. | Three distinct cut-age metrics recorded; no runtime V1 reader retained. |


| 256 | Luna | Measure upgrade throughput. | Results recorded. |
| 257 | Luna | Measure peak migration memory. | Results recorded. |
| 258 | Sol | Decide whether replay cost justifies derived checkpoints. | Implement/defer decision recorded from measurements. |
| 259 | Sol | If justified, specify checkpoint semantics under `version-history-plan.md`. | Checkpoint is explicitly disposable/non-authoritative. |
| 260 | Sol | If justified, implement checkpoint generation/load path. | Deleting checkpoints and rebuilding yields identical state. |
| 261 | Sol | If justified, add checkpoint corruption/rebuild tests. | Corruption cannot alter authority. |

---

**Mandatory dependency:** finish canonical owner converters, verify historical equivalence, promote the retained development corpus, then delete legacy readers and one-time converters; rerun complete acceptance after deletion. These gates replace the appended R1–R22 plan rather than adding another workstream.

## Phase Z — Architecture Closeout

| Step | Owner | Work | Gate |
| ---: | --- | --- | --- |
| 262 | Sol | Write compact-history ADR capturing only hard-to-reverse decisions. | ADR reviewed against implementation. |
| 263 | Sol | Update ADR index. | Navigation correct. |
| 264 | Luna | Update `storage-format.md` with compact record families and migration boundary. | Format doc matches source. |
| 265 | Luna | Update `architecture.md` and `invariants.md`. | Ownership/history invariants current. |
| 266 | Luna | Update `current-limitations.md` and `maintainer-map.md`. | Operational limitations/navigation current. |
| 267 | Luna | Update affected API/development docs and docs index. | Documentation compliance passes. |
| 268 | Luna | Refresh Lexicon/Arcana from final source. | Fresh snapshot available or explicit bounded limitation recorded. |
| 269 | Sol | Promote all verified canonical V2 development fixtures; update SHA-256 manifests and commit fixture repository. | All retained inputs migrated and validated before promotion; hash and commit recorded. |
| 270 | Sol | Remove V1 runtime container reader and dispatch. | Only V2 opens normally; old headers yield typed unsupported error. |
| 271 | Sol | Remove CVAFRS03/04/05 normalization and CVAILEG1/2 epoch/prefix runtime compatibility. | No reachable legacy codec; CVAFRS06 semantic proof remains valid. |
| 272 | Sol | Remove temporary upgrader APIs, CLI commands, translation codecs and obsolete tests after promotion. | No migration-only code reachable; negative tests retained. |
| 273 | Sol | Audit legacy references/mixed writes, verify owner boundaries and rerun root, CLI, history, prune/rewrite, reconcile, Ellis, structural and docs acceptance. | Current-format-only post-retirement verification passes. |

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
| 274 | Sol | Record final implementation commits and changed-file inventory in this plan. | Execution record complete. |
| 275 | Sol | Record actual raw logical-history savings by family and by fixture. | Logical savings report complete. |
| 276 | Sol | Record ADR-0004 compression results separately from logical-history savings. | No conflation of compression and logical compaction. |
| 277 | Sol | Record every measurement-gated defer decision (Community/vector/checkpoints/etc.). | Deferred work has explicit evidence/reason. |
| 278 | Sol | Update spec status to match shipped architecture and remaining limits. | Spec and implementation agree. |
| 279 | Sol | Mark complete only after canonical fixture promotion, legacy decoder and upgrader deletion, and post-retirement acceptance. | Final hard-cut acceptance complete. |

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

This revised decomposition contains **279 execution steps**. The initial 243-step expansion split the previous 74-step version into independent schema, codec, replay, writer, migration, proof, fixture, and measurement responsibilities. Execution discovery added explicit Dream pair-membership and processing-eligibility oracle steps before whole-owner integration; they are separate from accepted first-pass proof and must not be absorbed into the whole-owner gate. V1 open-path discovery added separate conversation-compaction and Entity-resolution read-only projection checks before owner lifecycle cutover; their physical reclamation and durable revision-repair failure domains require independent evidence.

Standalone Memory writer cutover exposed four legacy-carrier readers with separate owners/failure domains: REL reconciliation, completed-import evidence, import interval admission and shared physical reclamation. These are explicit Steps 59-62; former Steps 59-247 move to 63-251. They may not be absorbed into the Memory integration gate.

Steps 63–86 are accepted against `9996c28`; Step 87 and Integration Gate C passed against the six-file artifact `c69f279`. Steps 88–90 passed against `bafdaae`, including refreshed architecture enforcement. Steps 91–92 passed against `3cc6631`; Steps 93–95 passed against `ace0a33`. Steps 96–107 passed against `73e88ae`. The accepted range is 1–107; remaining owner converters and integrated final promotion/retirement gates still gate final acceptance.

The number is not a target or an estimate of required work sessions. It is the consequence of making acceptance/failure domains explicit.

A healthy execution run is expected to close several adjacent items when they share one implementation seam. The plan should therefore be read as **279 acceptance checks grouped into phases**, not isolated mini-projects.

If any one item still requires multiple independent implementation passes, split it before starting. If several adjacent items are naturally one coherent implementation pass, execute them together and mark each gate independently. Do not preserve numbering conventions at the expense of efficient, bounded execution.
Authoritative Memory migration exposed the legacy `CVAILEG1` import receipt's binding to original physical prefix bytes. Steps 72-77 now explicitly own its contract, encoder, decoder, replay validation, migration binding and adversarial verification before Memory measurement/Gate C. Former Steps 72-251 move to 78-257. The later post-migration import-family audit remains required and does not substitute for these execution steps.

Steps 78-80 explicitly cover transported import-proof maintenance and evidence equivalence discovered during review. Former Steps 78-257 move to 81-260; ordinary maintenance cannot copy a stale physical-prefix binding.

Legacy settlement normalization still reads original physical cuts. Steps 78-83 explicitly extract read-only source normalization and cover CVAFRS03/04/05 before relocation and maintenance. Former Steps 78-260 move to 84-266. The source proof remains retained; canonical semantic normalization supplies the existing validated successor rather than rewriting physical evidence.
Entity discovery added eight explicit adapter/evidence responsibilities and a reproduced maintenance-proof failure added one fail-closed converter gate, increasing the pre-consolidation plan from 266 to 275 (subsequently reduced to 271). Accepted Steps 1–90 are unchanged; the Phase H crosswalk preserves pending original work.


## Replan crosswalk

Accepted steps 1–90 and their evidence are unchanged. Former future 238 (redundant legacy reconciliation), 240 (runtime cross-format reconciliation), and 250–251 (duplicated age-specific benchmark setup) are removed or merged. Former 265–269 are rewritten as actual fixture promotion, decoder/upgrader deletion and post-deletion verification, not mere audits. All pending rows are renumbered in order; final step 271. The former R1–R22 are no longer a separate execution stream. No source implementation or corpus migration is implied.

## Relationship execution-discovery crosswalk

After Entity Step 107, the current source still persists Relationship; Observation has no separate durable owner and Graph Observation nodes are rejected. Eight concrete consumers have independent failure domains and cannot be hidden in the writer or converter row. New Steps 113–118 cover reconciliation, import evidence, interval admission, reclamation, retained structure and shared proof readers. Former writer Step 113 becomes 119; new Steps 120–121 cover native REL/PHY pruning. Former converter/adversarial/measurement 114–116 become 122–124; former 117–271 move to 125–279. Accepted Steps 1–107 are unchanged. Historical crosswalk counts above describe their dated replans; the current total is 279. Integrated corpus promotion and runtime retirement retain all consolidated former R1–R22 requirements.
