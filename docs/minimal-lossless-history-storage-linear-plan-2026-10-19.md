# Minimal Lossless History Storage — Linear Implementation Plan

Date: 2026-10-19
Status: Implementation-ready; 21/245 execution steps complete
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
- Persisted V1.0 REL/PHY files are the compatibility boundary. Do not dual-write old and compact history.
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

Commit: pending shared whole-owner/baseline batch commit.
Files: `src/facade/historical_global_{cuts,frontier}*.rs`, `historical_whole_{owner,phy}_fingerprint*.rs`, test-only Graph baseline seam and crate wiring; [evidence](minimal-lossless-history-storage-execution-2026-10-19.md#whole-owner-and-baseline-batch--steps-2021).
Verification: final focused checks **7/7**; hand-authored REL/PHY interleaving, lifecycle visibility, actual grouped-publication interior cuts, fixed-source repeat/reopen, authentic rewrite identity and checksum-valid forgery rejection, typed missing-reference rejection.
Blockers: none. A global number alone does not freeze clock-neutral recovery appends; historical projections bind a fixed source. Incomplete reference closure is unavailable, not an invented historical state.

### Step 21 execution record

Commit: pending shared whole-owner/baseline batch commit.
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
| 24 | Sol | Add explicit V1.0 read-only mode for REL. | V1.0 REL opens for inspection/migration without mutation. |
| 25 | Sol | Add explicit V1.0 read-only mode for PHY. | V1.0 PHY opens for inspection/migration without mutation. |
| 26 | Sol | Add typed `StorageFormatUpgradeRequired` mutation failure for V1.0 REL. | REL mutation fails before append. |
| 27 | Sol | Add typed `StorageFormatUpgradeRequired` mutation failure for V1.0 PHY. | PHY mutation fails before append. |
| 28 | Sol | Make new REL creation use the compact format version. | New REL reopens through compact path. |
| 29 | Sol | Make new PHY creation use the compact format version. | New PHY reopens through compact path. |

---

## Phase C — Lossless Upgrade Skeleton

| Step | Owner | Work | Gate |
| ---: | --- | --- | --- |
| 30 | Sol | Add transformed-copy upgrade request/result types and output-exists protection. | Minimal API tests pass. |
| 31 | Sol | Implement REL upgrade destination creation preserving owner UUID. | Minimal REL preserves owner identity. |
| 32 | Sol | Implement PHY upgrade destination creation preserving owner UUID. | Minimal PHY preserves owner identity. |
| 33 | Sol | Preserve CVA global-version numbers during transformed-copy upgrade. | Synthetic multi-version file retains exact global numbers. |
| 34 | Sol | Preserve transaction timestamps during upgrade. | Timestamp map compares exactly before/after. |
| 35 | Sol | Preserve owner-local version numbers during upgrade. | Synthetic interleaved owners retain exact local cuts. |
| 36 | Sol | Preserve accepted physical ordering required by existing visibility semantics without reissuing semantic history. | Synthetic ordering oracle matches source. |
| 37 | Sol | Add source-drift detection between upgrade planning and execution. | Changed source rejects before promotion. |
| 38 | Sol | Add partial-output cleanup/abort behavior. | Failed upgrade never leaves a promotable partial destination. |
| 39 | Luna | Add corruption, output-exists, abort, and drift regression fixtures. | All failure modes leave source untouched. |

### Integration Gate B — Format boundary and upgrader shell

Run header/open/create/read-only/mutation-rejection/upgrade-shell tests for REL and PHY. No semantic owner codec is changed yet.

---

## Phase D — Memory Delta Contract

| Step | Owner | Work | Gate |
| ---: | --- | --- | --- |
| 40 | Sol | Inventory every Memory field and classify immutable vs mutable. | Field table complete. |
| 41 | Sol | Define optional-field clear vs unchanged semantics. | Table-driven contract tests pass. |
| 42 | Sol | Define collection-field delta semantics, if any. | Canonical operation rules documented/tested. |
| 43 | Sol | Freeze Memory creation record semantics. | Creation contract matches V1.0 first revision. |
| 44 | Sol | Freeze Memory delta record semantics. | Delta contract can represent every legal V1.0 later revision. |
| 45 | Sol | Define canonical Memory delta ordering/field-mask rules. | Equivalent changes encode identically. |

## Phase E — Memory Codec and Replay

| Step | Owner | Work | Gate |
| ---: | --- | --- | --- |
| 46 | Sol | Implement compact Memory creation encoder. | Golden encode tests pass. |
| 47 | Sol | Implement compact Memory creation decoder/validation. | Round-trip + malformed/truncated tests pass. |
| 48 | Sol | Implement compact Memory delta encoder. | Golden encode tests pass. |
| 49 | Sol | Implement compact Memory delta decoder/validation. | Malformed/unknown/noncanonical tests fail closed. |
| 50 | Sol | Implement pure Memory delta application over prior accepted revision. | Valid delta materializes expected postimage. |
| 51 | Sol | Add validate-before-apply guard so invalid delta cannot mutate projection. | Negative tests prove prior state unchanged. |
| 52 | Sol | Teach compact Memory rebuild to replay creation + delta + tombstone. | Synthetic histories reopen identically. |
| 53 | Sol | Preserve exact mutation-ID retry/conflict behavior in compact replay. | Exact retry idempotent; conflicting payload rejects. |

## Phase F — Memory Writer Cutover and Migration

| Step | Owner | Work | Gate |
| ---: | --- | --- | --- |
| 54 | Sol | Hard-cut ordinary compact Memory first-publication writer to creation record. | No legacy full-record write on compact files. |
| 55 | Sol | Hard-cut ordinary compact Memory revision writer to delta record. | Later revisions emit only compact deltas. |
| 56 | Sol | Preserve compact Memory tombstone semantics. | Terminal behavior matches V1.0. |
| 57 | Sol | Update grouped Insomnia Memory creation encoding inside its existing atomic completion record. | Grouped publication atomicity/retry tests pass. |
| 58 | Sol | Update external Memory publication encoding inside its existing atomic publication record. | External publication/routing tests pass. |
| 59 | Sol | Migrate V1.0 standalone Memory first revisions to compact creation. | Creation parity passes. |
| 60 | Sol | Migrate V1.0 standalone later Memory revisions to compact deltas. | Revision parity passes. |
| 61 | Sol | Migrate V1.0 grouped Insomnia Memory publications. | Grouped histories preserve versions/IDs/bodies. |
| 62 | Sol | Migrate V1.0 external Memory publications. | External histories preserve owner references/routing. |
| 63 | Sol | Migrate V1.0 Memory tombstones. | Historical pre-tombstone revisions and terminal state match. |
| 64 | Luna | Add Memory adversarial migration fixtures for optional clears, supersession, archive/lifecycle changes, and malformed deltas. | All valid histories match; invalid ones reject. |
| 65 | Luna | Add Memory retry/conflict migration fixtures. | Retry semantics identical. |
| 66 | Sol | Measure raw Memory-history bytes before/after compact migration, excluding compression. | Benchmark artifact records zero semantic mismatch and byte delta. |

### Integration Gate C — Memory reference owner

Run Memory lifecycle/history, Insomnia, external publication, imports, rewrite/prune/reclaim interaction, reconciliation, and historical-oracle suites.

---

## Phase G — Shared Delta Mechanics

| Step | Owner | Work | Gate |
| ---: | --- | --- | --- |
| 67 | Sol | Extract only the proven field-mask/optional-clear codec helpers from Memory. | Memory tests remain green. |
| 68 | Sol | Extract only the proven validate-before-apply helper pattern. | Memory behavior unchanged. |
| 69 | Sol | Document which semantics remain owner-specific and must not enter the helper module. | No generic semantic-owner abstraction is introduced. |

---

## Phase H — Entity Compact History

| Step | Owner | Work | Gate |
| ---: | --- | --- | --- |
| 70 | Sol | Freeze Entity creation/delta/terminal field contract. | Contract covers all current Entity revisions. |
| 71 | Sol | Implement Entity creation codec. | Round-trip/corruption tests pass. |
| 72 | Sol | Implement Entity delta codec. | Field/evidence delta tests pass. |
| 73 | Sol | Implement Entity delta application/rebuild. | Historical revisions reconstruct exactly. |
| 74 | Sol | Hard-cut compact Entity writers to creation+delta while keeping merge/retirement/tombstone explicit. | Compact file emits no legacy postimage revisions. |
| 75 | Sol | Extend upgrader for Entity creation/revisions/terminal records. | V1.0→compact Entity oracle parity passes. |
| 76 | Luna | Add Entity adversarial evidence/add/remove/retry/merge/tombstone fixtures. | Invalid histories fail closed; valid histories match. |
| 77 | Sol | Measure raw Entity-history bytes before/after. | Size artifact recorded. |

---

## Phase I — Relationship / Observation Compact History

| Step | Owner | Work | Gate |
| ---: | --- | --- | --- |
| 78 | Sol | Resolve the canonical Relationship-vs-Observation target at execution time. | One target recorded; obsolete writer becomes migration-only if superseded. |
| 79 | Sol | Freeze creation/delta/terminal contract for the canonical target. | Contract covers all legal revisions. |
| 80 | Sol | Implement creation codec. | Round-trip/corruption tests pass. |
| 81 | Sol | Implement delta codec including participant/evidence operations. | Canonical collection-delta tests pass. |
| 82 | Sol | Implement delta application/rebuild. | Historical revisions reconstruct exactly. |
| 83 | Sol | Hard-cut compact writer to creation+delta. | No compact legacy postimage writer remains. |
| 84 | Sol | Extend upgrader for the canonical owner and any supported legacy input shape. | V1.0→compact parity passes. |
| 85 | Luna | Add participant/evidence/retry/terminal adversarial fixtures. | Invalid histories fail closed. |
| 86 | Sol | Measure raw history bytes before/after. | Size artifact recorded. |

---

## Phase J — Ego Compact History

| Step | Owner | Work | Gate |
| ---: | --- | --- | --- |
| 87 | Sol | Inventory mutable vs immutable fields for Identity, Anchor, Personality, and synthesis records. | Field table complete. |
| 88 | Luna | Measure repeated Ego text/payload bytes to decide whether owner-local payload identity is warranted. | Measurement-backed yes/no decision recorded. |
| 89 | Sol | Define Ego creation/delta semantics using the Step 88 decision. | Contract tests cover activation/deletion/state changes. |
| 90 | Sol | Implement Ego creation codec. | Round-trip/corruption tests pass. |
| 91 | Sol | Implement Ego delta codec. | Field-delta tests pass. |
| 92 | Sol | Implement Ego replay/materialization. | Historical Ego states reconstruct exactly. |
| 93 | Sol | Hard-cut compact Ego writers. | No compact full-postimage revision writer remains except justified immutable payload records. |
| 94 | Sol | Extend upgrader for Ego history. | REL/PHY Ego parity passes. |
| 95 | Luna | Add activation/deletion/ownership adversarial fixtures. | REL/PHY restrictions and terminal semantics remain exact. |
| 96 | Sol | Measure raw Ego-history bytes before/after. | Size artifact recorded. |

### Integration Gate D — Postimage owners

Run Entity, canonical Relationship/Observation, Ego, lifecycle, reconciliation, REL/PHY ownership, and historical-oracle suites.

---

## Phase K — Graph Encoding Audit and Optimization

| Step | Owner | Work | Gate |
| ---: | --- | --- | --- |
| 97 | Luna | Produce byte-field inventory for current Graph mutation/batch/version records. | Semantic vs framing/identity bytes classified. |
| 98 | Sol | Decide whether Graph compact-ID dictionary/ordinal encoding is worthwhile from measured fixtures. | Measurement-backed yes/no decision recorded. |
| 99 | Sol | If justified, define the Graph local-ID table format and validation rules. | Corrupt/missing mappings fail closed. |
| 100 | Sol | If justified, implement Graph compact-ID encoding/decoding behind Graph codec boundary. | Decoded canonical mutations match existing semantics. |
| 101 | Sol | Extend upgrader for compact Graph encoding if Step 100 is implemented. | Graph historical oracle parity passes. |
| 102 | Luna | Add Graph mapping corruption/order/admission regressions if compact IDs are implemented. | Invalid mappings fail closed. |

---

## Phase L — Freshness Effect History

| Step | Owner | Work | Gate |
| ---: | --- | --- | --- |
| 103 | Luna | Produce field-level inventory of Freshness event payload: causal effects, proof evidence, and derived postimages. | Exact classification recorded. |
| 104 | Sol | Define compact Freshness event format without derived postimages, preserving all causal/proof fields. | Codec contract can represent current accepted events. |
| 105 | Sol | Implement compact Freshness event encoder/decoder. | Round-trip/corruption tests pass. |
| 106 | Sol | Implement replay that recomputes postimages from prior state + effects. | Valid events reproduce current postimages. |
| 107 | Sol | Port current forged/malformed-event validation so removal of persisted postimages does not weaken checks. | Equal-or-stronger negative-test matrix passes. |
| 108 | Sol | Hard-cut compact Freshness event writer to the new effect-only representation. | Compact files emit no event postimages. |
| 109 | Sol | Extend upgrader for V1.0 Freshness events while preserving exact accepted event identity/cuts/proofs. | All historical Freshness cuts match. |
| 110 | Luna | Add import-settlement and same-cut Freshness migration regressions. | CVAFRS06 proof semantics remain unchanged. |
| 111 | Sol | Measure raw Freshness-history bytes before/after. | Size artifact recorded. |

---

## Phase M — Publication Framing

| Step | Owner | Work | Gate |
| ---: | --- | --- | --- |
| 112 | Luna | Measure current global-ticket + domain-link/version framing by owner on authoritative fixtures. | Per-owner framing-byte artifact exists. |
| 113 | Sol | Define compact publication envelope semantics without changing ADR 0003 ownership/clocks. | Format contract distinguishes global order, transaction time, local version, payload/reference. |
| 114 | Sol | Implement envelope codec/validation. | Round-trip/corruption tests pass. |
| 115 | Sol | Migrate compact Memory publication to envelope. | Memory local/global/timestamp/crash tests pass. |
| 116 | Sol | Extend upgrader for Memory publication-envelope conversion. | V1.0 Memory global/local cut parity passes. |
| 117 | Sol | Migrate Entity publication to envelope if it removes separate framing. | Entity version/oracle parity passes. |
| 118 | Sol | Migrate canonical Relationship/Observation publication to envelope if applicable. | Local/global parity passes. |
| 119 | Sol | Migrate Ego publication to envelope if applicable. | Ego parity passes. |
| 120 | Sol | Migrate Archive publication framing only if inert-backing semantics remain equivalent. | Archive cut/crash/reclamation tests pass, or explicit defer decision recorded. |
| 121 | Sol | Migrate Graph publication framing only if local Graph semantics remain unchanged. | Graph parity passes, or explicit defer decision recorded. |
| 122 | Luna | Audit upgrader coverage for each envelope-migrated owner. This step performs no implementation. | If any migrated owner lacks explicit upgrade conversion, insert one owner-specific step before Gate E; otherwise cross-format global/local/timestamp parity is complete. |

### Integration Gate E — Graph, Freshness, and publication framing

Run Graph, Freshness, version/timestamp, crash/recovery, reclamation/reconciliation, and historical-oracle suites. Confirm no new cross-owner semantic coupling.

---

## Phase N — Dream First-Pass History

| Step | Owner | Work | Gate |
| ---: | --- | --- | --- |
| 123 | Sol | Define exact Memory-population-at-cut semantics corresponding to current Dream `preexisting_ids`. | Synthetic reference tests reproduce current sets. |
| 124 | Sol | Implement Memory population reconstruction at that cut. | Fixture Dream begin sets match reconstructed sets. |
| 125 | Sol | Define canonical population digest. | Stable digest tests pass. |
| 126 | Sol | Define compact Dream Begin codec using Memory cut + digest instead of full set. | Codec round-trip/corruption tests pass. |
| 127 | Sol | Update Dream validation to reconstruct population and validate roots against it. | Existing Dream validity rules remain exact. |
| 128 | Sol | Hard-cut compact Dream Begin writer. | Compact files stop persisting full `preexisting_ids`. |
| 129 | Sol | Implement V1.0 Dream migration cut derivation. | Representable legacy passes derive exact cuts. |
| 130 | Sol | Implement V1.0 Dream migration equality proof against the legacy explicit set. | Mismatch rejects. |
| 131 | Sol | Implement explicit-set exception encoding for legacy passes not representable by one exact cut. | Exceptional history remains semantically exact. |
| 132 | Luna | Add wrong-cut, forged-digest, root-outside-population, and source-in-population regressions. | Invalid histories fail closed. |
| 133 | Sol | Measure Dream first-pass raw bytes before/after. | Size artifact recorded. |

---

## Phase O — Dream Pair History

| Step | Owner | Work | Gate |
| ---: | --- | --- | --- |
| 134 | Sol | Define validated owner-local Memory-ID table for Dream pair physical encoding. | Table format/validation tests pass. |
| 135 | Sol | Implement compact Dream pair encoder/decoder using local references. | Decoded canonical pair set matches existing semantics. |
| 136 | Sol | Hard-cut compact Dream pair writer. | No compact full-ID pair record writer remains. |
| 137 | Sol | Extend upgrader for V1.0 Dream pairs. | Pair-set parity passes. |
| 138 | Luna | Add missing/corrupt mapping and retry regressions. | Invalid mappings fail closed. |
| 139 | Sol | Measure Dream-pair raw bytes before/after. | Size artifact recorded. |

### Integration Gate F — Dream

Run Dream/Freshness/import historical suites plus oracle comparison on Dream-bearing fixtures.

---

## Phase P — Interaction Stream

| Step | Owner | Work | Gate |
| ---: | --- | --- | --- |
| 140 | Sol | Freeze interaction stream state-machine semantics from current cumulative records. | Begin/append/interrupted transition table exists. |
| 141 | Sol | Define `StreamBegin` compact record. | Codec tests pass. |
| 142 | Sol | Define `StreamAppend` exact UTF-8 suffix record. | Codec tests pass. |
| 143 | Sol | Define terminal/interrupted compact record. | Codec tests pass. |
| 144 | Sol | Implement compact stream replay/materialization. | Every checkpoint reconstructs exact visible text/status. |
| 145 | Sol | Hard-cut compact stream writer to begin/append/terminal records. | No cumulative-prefix compact writer remains. |
| 146 | Sol | Update stream reconciliation for compact records. | Latest-state reconciliation matches current behavior. |
| 147 | Sol | Extend upgrader from cumulative V1.0 checkpoints to compact stream deltas. | Historical checkpoint parity passes. |
| 148 | Luna | Add long-stream linear-growth benchmark. | Growth is linear in appended text. |
| 149 | Luna | Add crash/reopen/interruption regressions. | Displayed-text durability invariant remains intact. |

---

## Phase Q — External Derived Index

| Step | Owner | Work | Gate |
| ---: | --- | --- | --- |
| 150 | Sol | Freeze the semantic boundary: `CVAXIDX1` is derived state; `CVAXMEM1` remains authoritative publication. | Boundary documented in tests/code comments. |
| 151 | Sol | Decide current-state vs incremental derived-index persistence based on rebuild/write behavior. | One strategy recorded. |
| 152 | Sol | Implement compact ExternalIndex codec for the selected strategy. | Round-trip/current-state tests pass. |
| 153 | Sol | Implement compact ExternalIndex rebuild/current projection. | Lookup behavior matches V1.0. |
| 154 | Sol | Make superseded derived index storage reclaimable without semantic prune markers. | Reclaim tests pass. |
| 155 | Sol | Extend upgrader to migrate only the current required derived index state. | External Memory publication history remains untouched. |
| 156 | Luna | Add damaged-index rebuild/discard regressions. | Existing recovery contract remains intact. |

### Integration Gate G — Operational/derived stores

Run interaction runtime, external-source index/publication, reconciliation, reclaim, and reopen suites.

---

## Phase R — Entity Resolution History

| Step | Owner | Work | Gate |
| ---: | --- | --- | --- |
| 157 | Luna | Classify every Entity Resolution persisted field as semantic proof, retry state, or reconstructible postimage/candidate material. | Field table complete. |
| 158 | Sol | Freeze compact Entity Resolution history contract from that classification. | Contract preserves every authoritative field. |
| 159 | Sol | Define initial candidate/evidence record. | Codec tests pass. |
| 160 | Sol | Define attempt/status delta record. | Codec tests pass. |
| 161 | Sol | Implement replay/materialization. | Accepted decisions/retries reconstruct exactly. |
| 162 | Sol | Hard-cut compact Entity Resolution writer. | No compact repeated full candidate snapshot remains unless classified authoritative. |
| 163 | Sol | Extend upgrader for V1.0 Entity Resolution history. | Decision/proof parity passes. |
| 164 | Luna | Add retry/conflict/import-readiness/adversarial resolution fixtures. | Existing Entity effects and readiness decisions remain identical. |
| 165 | Sol | Measure raw Entity Resolution history before/after. | Size artifact recorded. |

---

## Phase S — Community Proof Boundary

| Step | Owner | Work | Gate |
| ---: | --- | --- | --- |
| 166 | Luna | Identify every accepted proof that references Community generation/graph version/name. | Consumer inventory complete. |
| 167 | Sol | Define the exact reconstruction claim required to eliminate repeated membership snapshots. | Claim names graph cut + algorithm/policy version + any non-deterministic retained data. |
| 168 | Luna | Test that claim against authoritative/synthetic Community histories. | Exact yes/no evidence recorded. |
| 169 | Sol | If exact reconstruction is proven, define compact Community membership history; otherwise record explicit no-change decision. | One decision closed. |
| 170 | Sol | If compacting, implement Community codec/replay/writer. | Historical proof identity remains valid. |
| 171 | Sol | If compacting, extend upgrader. | Community oracle/proof parity passes. |
| 172 | Luna | If compacting, add semantic-name/non-determinism regressions. | Non-recomputable names/evidence remain retained. |
| 173 | Sol | If compacting, measure raw Community history before/after; otherwise mark measurement not required. | Evidence recorded. |

### Integration Gate H — Proof-boundary stores

Run Entity Resolution, Community, Dream/Freshness proof consumers, import readiness, and historical-oracle suites.

---

## Phase T — Archive Vector Generation Decision

| Step | Owner | Work | Gate |
| ---: | --- | --- | --- |
| 174 | Luna | Build repeated same-profile Archive-vector-generation workload. | Reproducible workload exists. |
| 175 | Luna | Measure unchanged exact vector-byte duplication across generations separately from compatibility-profile multiplicity. | Byte-duplication artifact exists. |
| 176 | Sol | Decide whether row/block reuse is justified by Step 175. | Explicit implement/defer decision recorded. |
| 177 | Sol | If justified, define immutable exact vector row/block identity preserving byte-distinct compatible embeddings. | Identity contract tests pass. |
| 178 | Sol | If justified, implement reusable row/block storage. | Exact vector bytes round-trip. |
| 179 | Sol | If justified, implement generation manifest/delta referencing row/block identities. | Historical generation materialization exact. |
| 180 | Sol | If justified, hard-cut compact Archive-vector generation writer. | No redundant full-matrix compact writer remains. |
| 181 | Sol | If justified, extend upgrader for historical generations. | Exact vector-byte/generation parity passes. |
| 182 | Luna | If justified, add retrieval and corruption regressions. | Retrieval parity and fail-closed behavior pass. |
| 183 | Sol | If justified, measure raw vector-generation bytes before/after; otherwise mark phase deferred. | Evidence recorded. |

### Integration Gate I — Vectors

Run profile, packed-vector, Memory-vector, Archive-vector, generation, retrieval, prune/reclaim, and rewrite-interaction suites.

---

## Phase U — Upgrade Coverage Completion

| Step | Owner | Work | Gate |
| ---: | --- | --- | --- |
| 184 | Luna | Re-run durable-family inventory against current source after all codec migrations. | No new/unhandled writer family appears. |
| 185 | Luna | Audit upgrade coverage for Archive-owned durable families after all earlier migrations. This step performs no implementation. | Either Archive coverage is complete or this plan is expanded with one explicit step per missing family before execution continues. |
| 186 | Luna | Audit upgrade coverage for import epoch and Historical Freshness settlement families. This step performs no implementation. | Either import-evidence coverage is complete or explicit per-family steps are inserted before execution continues. |
| 187 | Sol | Add/verify upgrader support for Echo records only. | Echo historical evidence parity passes. |
| 188 | Sol | Add/verify upgrader support for project-history correlation records only. | Project-history evidence parity passes. |
| 189 | Sol | Reject unknown/unsupported semantic authority explicitly during upgrade. | Unknown authority fails closed. |
| 190 | Sol | Reopen upgraded REL and validate all owner invariants. | REL validation passes. |
| 191 | Sol | Reopen upgraded PHY and validate all owner invariants. | PHY validation passes. |

---

## Phase V — Authoritative Fixture Migration

| Step | Owner | Work | Gate |
| ---: | --- | --- | --- |
| 192 | Luna | Upgrade disposable copy of 14-day REL. | Destination reopens; source hash unchanged. |
| 193 | Luna | Upgrade disposable copy of 14-day PHY. | Destination reopens; source hash unchanged. |
| 194 | Luna | Upgrade disposable copy of 28-day REL. | Destination reopens; source hash unchanged. |
| 195 | Luna | Upgrade disposable copy of 28-day PHY. | Destination reopens; source hash unchanged. |
| 196 | Luna | Upgrade disposable copy of Ellis REL. | Destination reopens; source hash unchanged. |
| 197 | Luna | Upgrade disposable copy of Ellis PHY. | Destination reopens; source hash unchanged. |
| 198 | Luna | Produce per-family raw-byte accounting for all six migrations. | Logical-history size report exists; compression excluded. |

---

## Phase W — Historical Equivalence Verification

| Step | Owner | Work | Gate |
| ---: | --- | --- | --- |
| 199 | Sol | Exhaustively compare Memory historical cuts source vs compact fixtures. | Zero mismatches. |
| 200 | Sol | Exhaustively compare Archive historical cuts where tractable. | Zero mismatches. |
| 201 | Sol | Exhaustively compare Entity and Relationship/Observation cuts. | Zero mismatches. |
| 202 | Sol | Exhaustively compare Graph cuts. | Zero mismatches. |
| 203 | Sol | Exhaustively compare Ego cuts. | Zero mismatches. |
| 204 | Sol | Exhaustively compare Freshness activity/event cuts. | Zero mismatches. |
| 205 | Sol | Compare vector-generation cuts and exact vector bytes. | Zero mismatches. |
| 206 | Sol | Compare Dream proof state. | Zero mismatches. |
| 207 | Sol | Compare global ordering points exhaustively where tractable and deterministically sample only where documented runtime makes exhaustive comparison unreasonable. | Zero mismatches; sampling rationale recorded if used. |

---

## Phase X — Maintenance and Reconciliation Verification

| Step | Owner | Work | Gate |
| ---: | --- | --- | --- |
| 208 | Sol | Run legacy-decoded vs legacy-decoded reconciliation reference cases. | Baseline recorded. |
| 209 | Sol | Run compact vs compact reconciliation cases. | Decisions match baseline semantics. |
| 210 | Sol | Run cross-format canonical-history reconciliation where supported. | Encoding does not affect conflict decisions. |
| 211 | Sol | Run compact REL prune tests. | Maintenance behavior unchanged. |
| 212 | Sol | Run compact PHY prune tests. | Maintenance behavior unchanged. |
| 213 | Sol | Run compact reclaim tests. | Reclamation preserves semantic history. |
| 214 | Sol | Run compact repair tests. | Repair behavior unchanged. |
| 215 | Sol | Run fresh-history rewrite from compact sources. | Rewrite semantics unchanged. |
| 216 | Sol | Reopen all maintenance outputs and rerun relevant oracle checks. | Zero semantic mismatch beyond explicit maintenance policy. |

---

## Phase Y — Performance and Checkpoint Decision

| Step | Owner | Work | Gate |
| ---: | --- | --- | --- |
| 217 | Luna | Measure cold reopen old vs compact fixtures. | Results recorded. |
| 218 | Luna | Measure warm reopen old vs compact fixtures. | Results recorded. |
| 219 | Luna | Measure recent historical lookup old vs compact. | Results recorded. |
| 220 | Luna | Measure middle historical lookup old vs compact. | Results recorded. |
| 221 | Luna | Measure oldest historical lookup old vs compact. | Results recorded. |
| 222 | Luna | Measure upgrade throughput. | Results recorded. |
| 223 | Luna | Measure peak migration memory. | Results recorded. |
| 224 | Sol | Decide whether replay cost justifies derived checkpoints. | Implement/defer decision recorded from measurements. |
| 225 | Sol | If justified, specify checkpoint semantics under `version-history-plan.md`. | Checkpoint is explicitly disposable/non-authoritative. |
| 226 | Sol | If justified, implement checkpoint generation/load path. | Deleting checkpoints and rebuilding yields identical state. |
| 227 | Sol | If justified, add checkpoint corruption/rebuild tests. | Corruption cannot alter authority. |

---

## Phase Z — Architecture Closeout

| Step | Owner | Work | Gate |
| ---: | --- | --- | --- |
| 228 | Sol | Write compact-history ADR capturing only hard-to-reverse decisions. | ADR reviewed against implementation. |
| 229 | Sol | Update ADR index. | Navigation correct. |
| 230 | Luna | Update `storage-format.md` with compact record families and migration boundary. | Format doc matches source. |
| 231 | Luna | Update `architecture.md` and `invariants.md`. | Ownership/history invariants current. |
| 232 | Luna | Update `current-limitations.md` and `maintainer-map.md`. | Operational limitations/navigation current. |
| 233 | Luna | Update affected API/development docs and docs index. | Documentation compliance passes. |
| 234 | Luna | Refresh Lexicon/Arcana from final source. | Fresh snapshot available or explicit bounded limitation recorded. |
| 235 | Sol | Audit reverse dependencies of all legacy writers. | Legacy writers have no ordinary compact-file callers. |
| 236 | Sol | Audit mixed-format append paths. | None remain. |
| 237 | Sol | Audit compatibility leakage outside open/migration boundaries. | None remain. |
| 238 | Sol | Audit unexpected cross-owner coupling introduced by compact framing/deltas. | None found or documented/fixed. |
| 239 | Sol | Confirm with direct source search that no migration-only alias/fallback/shadow writer remains. | Hard-cut cleanup complete. |

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
| 240 | Sol | Record final implementation commits and changed-file inventory in this plan. | Execution record complete. |
| 241 | Sol | Record actual raw logical-history savings by family and by fixture. | Logical savings report complete. |
| 242 | Sol | Record ADR-0004 compression results separately from logical-history savings. | No conflation of compression and logical compaction. |
| 243 | Sol | Record every measurement-gated defer decision (Community/vector/checkpoints/etc.). | Deferred work has explicit evidence/reason. |
| 244 | Sol | Update spec status to match shipped architecture and remaining limits. | Spec and implementation agree. |
| 245 | Sol | Mark plan complete only after confirming no temporary migration scaffolding remains. | Final hard-cut acceptance complete. |

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

This decomposition contains **245 execution steps**. The initial 243-step expansion split the previous 74-step version into independent schema, codec, replay, writer, migration, proof, fixture, and measurement responsibilities. Execution discovery added explicit Dream pair-membership and processing-eligibility oracle steps before whole-owner integration; they are separate from accepted first-pass proof and must not be absorbed into the whole-owner gate.

The number is not a target or an estimate of required work sessions. It is the consequence of making acceptance/failure domains explicit.

A healthy execution run is expected to close several adjacent items when they share one implementation seam. The plan should therefore be read as **245 acceptance checks grouped into phases**, not 245 isolated mini-projects.

If any one item still requires multiple independent implementation passes, split it before starting. If several adjacent items are naturally one coherent implementation pass, execute them together and mark each gate independently. Do not preserve numbering conventions at the expense of efficient, bounded execution.