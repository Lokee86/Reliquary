# Minimal Lossless History Storage — Linear Implementation Plan

Date: 2026-10-19
Status: Implementation-ready; 6/243 execution steps complete
Source: [Minimal lossless history storage — refactor specification](minimal-lossless-history-storage-spec-2026-10-19.md)

## Overview

This plan sequences owner-local historical verification, the persisted-format boundary, compact codecs, lossless migration, and final integration. Progress is recorded only after each numbered gate passes.

## Purpose

Implement compact ordinary REL/PHY history without weakening historical reconstruction.

This plan does not change fresh-history rewrite, pruning policy, or pack compression. Rewrite remains the explicit history-discard operation. ADR 0004 remains responsible for packing/compression.

## Execution Rules

- Execute numbered steps in order unless a phase explicitly permits parallel work.
- Every numbered step owns one bounded responsibility.
- Integration gates are checkpoints, not disguised implementation steps.
- Mark a step complete only after its stated gate passes.
- Record under each completed step: `Commit:`, `Files:`, `Verification:`, `Blockers:`.
- Persisted V1.0 REL/PHY files are the compatibility boundary. Do not dual-write old and compact history.
- If a numbered step still proves too large, split it before implementation rather than absorbing more work into it.
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
| 17 | Luna | Build deterministic cut enumeration for authoritative fixtures. | Cut enumerator returns stable Archive/Memory/Entity/Graph/vector/Ego/Freshness/global cut sets. |
| 18 | Sol | Implement whole-owner projection at CVA global ordering point G using existing visibility rules. | Interleaved synthetic owner fixture matches hand-authored expected state. |
| 19 | Luna | Generate baseline historical fingerprint artifacts for all authoritative fixtures. | Baseline artifacts persisted; fixture hashes unchanged. |

### Step 1 execution record

Commit: `a2c9a2d` (clean baseline).
Files: [Execution evidence](minimal-lossless-history-storage-execution-2026-10-19.md#step-1--execution-baseline).
Verification: clean status, V1.0 source/header contract, accepted Step 65 test baseline; fresh format and documentation checks pass.
Blockers: none for the baseline record; fresh full-suite rerun is collecting.

### Step 2 execution record

Commit: pending execution-evidence commit.
Files: [Execution evidence](minimal-lossless-history-storage-execution-2026-10-19.md#step-2--authoritative-fixture-identity).
Verification: two full read-only hash passes; six accepted hashes unchanged; exact byte/header inventory.
Blockers: none.

### Step 3 execution record

Commit: pending execution-evidence commit.
Files: [Execution evidence](minimal-lossless-history-storage-execution-2026-10-19.md#step-3--durable-writer-inventory).
Verification: 117 record identities in 66 explicit families; direct and delegated writer audit.
Blockers: none.

### Step 4 execution record

Commit: pending execution-evidence commit.
Files: [Execution evidence](minimal-lossless-history-storage-execution-2026-10-19.md#step-4--semantic-owner-and-clock-map).
Verification: all family groups mapped to concrete owner and version/admission behavior.
Blockers: none.

### Step 5 execution record

Commit: pending execution-evidence commit.
Files: [Execution evidence](minimal-lossless-history-storage-execution-2026-10-19.md#step-5--retained-responsibility-classification).
Verification: 66 family names classified exactly once; set equality and uniqueness assertions pass.
Blockers: none.

### Step 6 execution record

Commit: pending execution-evidence commit.
Files: [Structural baseline evidence](minimal-lossless-history-storage-execution-2026-10-19.md#step-6--structural-baseline-and-refresh-limitation).
Verification: coverage/ownership guards passed; existing snapshot identity recorded; bounded refresh limitation explicitly documented.
Blockers: fresh Lexicon/Arcana/Pitlord evidence remains required at later structural gates.

### Integration Gate A — Historical oracle

Run existing historical/lifecycle/reconciliation tests plus the new oracle on V1.0 files. Gate passes only if oracle output agrees with current public/internal historical APIs.

---

## Phase B — Container Version Boundary

| Step | Owner | Work | Gate |
| ---: | --- | --- | --- |
| 20 | Sol | Choose and encode the compact container format version. | New header round-trips. |
| 21 | Sol | Make old reader reject the compact version unless routed through new compact open logic. | Older-format path fails with typed unsupported-version error. |
| 22 | Sol | Add explicit V1.0 read-only mode for REL. | V1.0 REL opens for inspection/migration without mutation. |
| 23 | Sol | Add explicit V1.0 read-only mode for PHY. | V1.0 PHY opens for inspection/migration without mutation. |
| 24 | Sol | Add typed `StorageFormatUpgradeRequired` mutation failure for V1.0 REL. | REL mutation fails before append. |
| 25 | Sol | Add typed `StorageFormatUpgradeRequired` mutation failure for V1.0 PHY. | PHY mutation fails before append. |
| 26 | Sol | Make new REL creation use the compact format version. | New REL reopens through compact path. |
| 27 | Sol | Make new PHY creation use the compact format version. | New PHY reopens through compact path. |

---

## Phase C — Lossless Upgrade Skeleton

| Step | Owner | Work | Gate |
| ---: | --- | --- | --- |
| 28 | Sol | Add transformed-copy upgrade request/result types and output-exists protection. | Minimal API tests pass. |
| 29 | Sol | Implement REL upgrade destination creation preserving owner UUID. | Minimal REL preserves owner identity. |
| 30 | Sol | Implement PHY upgrade destination creation preserving owner UUID. | Minimal PHY preserves owner identity. |
| 31 | Sol | Preserve CVA global-version numbers during transformed-copy upgrade. | Synthetic multi-version file retains exact global numbers. |
| 32 | Sol | Preserve transaction timestamps during upgrade. | Timestamp map compares exactly before/after. |
| 33 | Sol | Preserve owner-local version numbers during upgrade. | Synthetic interleaved owners retain exact local cuts. |
| 34 | Sol | Preserve accepted physical ordering required by existing visibility semantics without reissuing semantic history. | Synthetic ordering oracle matches source. |
| 35 | Sol | Add source-drift detection between upgrade planning and execution. | Changed source rejects before promotion. |
| 36 | Sol | Add partial-output cleanup/abort behavior. | Failed upgrade never leaves a promotable partial destination. |
| 37 | Luna | Add corruption, output-exists, abort, and drift regression fixtures. | All failure modes leave source untouched. |

### Integration Gate B — Format boundary and upgrader shell

Run header/open/create/read-only/mutation-rejection/upgrade-shell tests for REL and PHY. No semantic owner codec is changed yet.

---

## Phase D — Memory Delta Contract

| Step | Owner | Work | Gate |
| ---: | --- | --- | --- |
| 38 | Sol | Inventory every Memory field and classify immutable vs mutable. | Field table complete. |
| 39 | Sol | Define optional-field clear vs unchanged semantics. | Table-driven contract tests pass. |
| 40 | Sol | Define collection-field delta semantics, if any. | Canonical operation rules documented/tested. |
| 41 | Sol | Freeze Memory creation record semantics. | Creation contract matches V1.0 first revision. |
| 42 | Sol | Freeze Memory delta record semantics. | Delta contract can represent every legal V1.0 later revision. |
| 43 | Sol | Define canonical Memory delta ordering/field-mask rules. | Equivalent changes encode identically. |

## Phase E — Memory Codec and Replay

| Step | Owner | Work | Gate |
| ---: | --- | --- | --- |
| 44 | Sol | Implement compact Memory creation encoder. | Golden encode tests pass. |
| 45 | Sol | Implement compact Memory creation decoder/validation. | Round-trip + malformed/truncated tests pass. |
| 46 | Sol | Implement compact Memory delta encoder. | Golden encode tests pass. |
| 47 | Sol | Implement compact Memory delta decoder/validation. | Malformed/unknown/noncanonical tests fail closed. |
| 48 | Sol | Implement pure Memory delta application over prior accepted revision. | Valid delta materializes expected postimage. |
| 49 | Sol | Add validate-before-apply guard so invalid delta cannot mutate projection. | Negative tests prove prior state unchanged. |
| 50 | Sol | Teach compact Memory rebuild to replay creation + delta + tombstone. | Synthetic histories reopen identically. |
| 51 | Sol | Preserve exact mutation-ID retry/conflict behavior in compact replay. | Exact retry idempotent; conflicting payload rejects. |

## Phase F — Memory Writer Cutover and Migration

| Step | Owner | Work | Gate |
| ---: | --- | --- | --- |
| 52 | Sol | Hard-cut ordinary compact Memory first-publication writer to creation record. | No legacy full-record write on compact files. |
| 53 | Sol | Hard-cut ordinary compact Memory revision writer to delta record. | Later revisions emit only compact deltas. |
| 54 | Sol | Preserve compact Memory tombstone semantics. | Terminal behavior matches V1.0. |
| 55 | Sol | Update grouped Insomnia Memory creation encoding inside its existing atomic completion record. | Grouped publication atomicity/retry tests pass. |
| 56 | Sol | Update external Memory publication encoding inside its existing atomic publication record. | External publication/routing tests pass. |
| 57 | Sol | Migrate V1.0 standalone Memory first revisions to compact creation. | Creation parity passes. |
| 58 | Sol | Migrate V1.0 standalone later Memory revisions to compact deltas. | Revision parity passes. |
| 59 | Sol | Migrate V1.0 grouped Insomnia Memory publications. | Grouped histories preserve versions/IDs/bodies. |
| 60 | Sol | Migrate V1.0 external Memory publications. | External histories preserve owner references/routing. |
| 61 | Sol | Migrate V1.0 Memory tombstones. | Historical pre-tombstone revisions and terminal state match. |
| 62 | Luna | Add Memory adversarial migration fixtures for optional clears, supersession, archive/lifecycle changes, and malformed deltas. | All valid histories match; invalid ones reject. |
| 63 | Luna | Add Memory retry/conflict migration fixtures. | Retry semantics identical. |
| 64 | Sol | Measure raw Memory-history bytes before/after compact migration, excluding compression. | Benchmark artifact records zero semantic mismatch and byte delta. |

### Integration Gate C — Memory reference owner

Run Memory lifecycle/history, Insomnia, external publication, imports, rewrite/prune/reclaim interaction, reconciliation, and historical-oracle suites.

---

## Phase G — Shared Delta Mechanics

| Step | Owner | Work | Gate |
| ---: | --- | --- | --- |
| 65 | Sol | Extract only the proven field-mask/optional-clear codec helpers from Memory. | Memory tests remain green. |
| 66 | Sol | Extract only the proven validate-before-apply helper pattern. | Memory behavior unchanged. |
| 67 | Sol | Document which semantics remain owner-specific and must not enter the helper module. | No generic semantic-owner abstraction is introduced. |

---

## Phase H — Entity Compact History

| Step | Owner | Work | Gate |
| ---: | --- | --- | --- |
| 68 | Sol | Freeze Entity creation/delta/terminal field contract. | Contract covers all current Entity revisions. |
| 69 | Sol | Implement Entity creation codec. | Round-trip/corruption tests pass. |
| 70 | Sol | Implement Entity delta codec. | Field/evidence delta tests pass. |
| 71 | Sol | Implement Entity delta application/rebuild. | Historical revisions reconstruct exactly. |
| 72 | Sol | Hard-cut compact Entity writers to creation+delta while keeping merge/retirement/tombstone explicit. | Compact file emits no legacy postimage revisions. |
| 73 | Sol | Extend upgrader for Entity creation/revisions/terminal records. | V1.0→compact Entity oracle parity passes. |
| 74 | Luna | Add Entity adversarial evidence/add/remove/retry/merge/tombstone fixtures. | Invalid histories fail closed; valid histories match. |
| 75 | Sol | Measure raw Entity-history bytes before/after. | Size artifact recorded. |

---

## Phase I — Relationship / Observation Compact History

| Step | Owner | Work | Gate |
| ---: | --- | --- | --- |
| 76 | Sol | Resolve the canonical Relationship-vs-Observation target at execution time. | One target recorded; obsolete writer becomes migration-only if superseded. |
| 77 | Sol | Freeze creation/delta/terminal contract for the canonical target. | Contract covers all legal revisions. |
| 78 | Sol | Implement creation codec. | Round-trip/corruption tests pass. |
| 79 | Sol | Implement delta codec including participant/evidence operations. | Canonical collection-delta tests pass. |
| 80 | Sol | Implement delta application/rebuild. | Historical revisions reconstruct exactly. |
| 81 | Sol | Hard-cut compact writer to creation+delta. | No compact legacy postimage writer remains. |
| 82 | Sol | Extend upgrader for the canonical owner and any supported legacy input shape. | V1.0→compact parity passes. |
| 83 | Luna | Add participant/evidence/retry/terminal adversarial fixtures. | Invalid histories fail closed. |
| 84 | Sol | Measure raw history bytes before/after. | Size artifact recorded. |

---

## Phase J — Ego Compact History

| Step | Owner | Work | Gate |
| ---: | --- | --- | --- |
| 85 | Sol | Inventory mutable vs immutable fields for Identity, Anchor, Personality, and synthesis records. | Field table complete. |
| 86 | Luna | Measure repeated Ego text/payload bytes to decide whether owner-local payload identity is warranted. | Measurement-backed yes/no decision recorded. |
| 87 | Sol | Define Ego creation/delta semantics using the Step 86 decision. | Contract tests cover activation/deletion/state changes. |
| 88 | Sol | Implement Ego creation codec. | Round-trip/corruption tests pass. |
| 89 | Sol | Implement Ego delta codec. | Field-delta tests pass. |
| 90 | Sol | Implement Ego replay/materialization. | Historical Ego states reconstruct exactly. |
| 91 | Sol | Hard-cut compact Ego writers. | No compact full-postimage revision writer remains except justified immutable payload records. |
| 92 | Sol | Extend upgrader for Ego history. | REL/PHY Ego parity passes. |
| 93 | Luna | Add activation/deletion/ownership adversarial fixtures. | REL/PHY restrictions and terminal semantics remain exact. |
| 94 | Sol | Measure raw Ego-history bytes before/after. | Size artifact recorded. |

### Integration Gate D — Postimage owners

Run Entity, canonical Relationship/Observation, Ego, lifecycle, reconciliation, REL/PHY ownership, and historical-oracle suites.

---

## Phase K — Graph Encoding Audit and Optimization

| Step | Owner | Work | Gate |
| ---: | --- | --- | --- |
| 95 | Luna | Produce byte-field inventory for current Graph mutation/batch/version records. | Semantic vs framing/identity bytes classified. |
| 96 | Sol | Decide whether Graph compact-ID dictionary/ordinal encoding is worthwhile from measured fixtures. | Measurement-backed yes/no decision recorded. |
| 97 | Sol | If justified, define the Graph local-ID table format and validation rules. | Corrupt/missing mappings fail closed. |
| 98 | Sol | If justified, implement Graph compact-ID encoding/decoding behind Graph codec boundary. | Decoded canonical mutations match existing semantics. |
| 99 | Sol | Extend upgrader for compact Graph encoding if Step 98 is implemented. | Graph historical oracle parity passes. |
| 100 | Luna | Add Graph mapping corruption/order/admission regressions if compact IDs are implemented. | Invalid mappings fail closed. |

---

## Phase L — Freshness Effect History

| Step | Owner | Work | Gate |
| ---: | --- | --- | --- |
| 101 | Luna | Produce field-level inventory of Freshness event payload: causal effects, proof evidence, and derived postimages. | Exact classification recorded. |
| 102 | Sol | Define compact Freshness event format without derived postimages, preserving all causal/proof fields. | Codec contract can represent current accepted events. |
| 103 | Sol | Implement compact Freshness event encoder/decoder. | Round-trip/corruption tests pass. |
| 104 | Sol | Implement replay that recomputes postimages from prior state + effects. | Valid events reproduce current postimages. |
| 105 | Sol | Port current forged/malformed-event validation so removal of persisted postimages does not weaken checks. | Equal-or-stronger negative-test matrix passes. |
| 106 | Sol | Hard-cut compact Freshness event writer to the new effect-only representation. | Compact files emit no event postimages. |
| 107 | Sol | Extend upgrader for V1.0 Freshness events while preserving exact accepted event identity/cuts/proofs. | All historical Freshness cuts match. |
| 108 | Luna | Add import-settlement and same-cut Freshness migration regressions. | CVAFRS06 proof semantics remain unchanged. |
| 109 | Sol | Measure raw Freshness-history bytes before/after. | Size artifact recorded. |

---

## Phase M — Publication Framing

| Step | Owner | Work | Gate |
| ---: | --- | --- | --- |
| 110 | Luna | Measure current global-ticket + domain-link/version framing by owner on authoritative fixtures. | Per-owner framing-byte artifact exists. |
| 111 | Sol | Define compact publication envelope semantics without changing ADR 0003 ownership/clocks. | Format contract distinguishes global order, transaction time, local version, payload/reference. |
| 112 | Sol | Implement envelope codec/validation. | Round-trip/corruption tests pass. |
| 113 | Sol | Migrate compact Memory publication to envelope. | Memory local/global/timestamp/crash tests pass. |
| 114 | Sol | Extend upgrader for Memory publication-envelope conversion. | V1.0 Memory global/local cut parity passes. |
| 115 | Sol | Migrate Entity publication to envelope if it removes separate framing. | Entity version/oracle parity passes. |
| 116 | Sol | Migrate canonical Relationship/Observation publication to envelope if applicable. | Local/global parity passes. |
| 117 | Sol | Migrate Ego publication to envelope if applicable. | Ego parity passes. |
| 118 | Sol | Migrate Archive publication framing only if inert-backing semantics remain equivalent. | Archive cut/crash/reclamation tests pass, or explicit defer decision recorded. |
| 119 | Sol | Migrate Graph publication framing only if local Graph semantics remain unchanged. | Graph parity passes, or explicit defer decision recorded. |
| 120 | Luna | Audit upgrader coverage for each envelope-migrated owner. This step performs no implementation. | If any migrated owner lacks explicit upgrade conversion, insert one owner-specific step before Gate E; otherwise cross-format global/local/timestamp parity is complete. |

### Integration Gate E — Graph, Freshness, and publication framing

Run Graph, Freshness, version/timestamp, crash/recovery, reclamation/reconciliation, and historical-oracle suites. Confirm no new cross-owner semantic coupling.

---

## Phase N — Dream First-Pass History

| Step | Owner | Work | Gate |
| ---: | --- | --- | --- |
| 121 | Sol | Define exact Memory-population-at-cut semantics corresponding to current Dream `preexisting_ids`. | Synthetic reference tests reproduce current sets. |
| 122 | Sol | Implement Memory population reconstruction at that cut. | Fixture Dream begin sets match reconstructed sets. |
| 123 | Sol | Define canonical population digest. | Stable digest tests pass. |
| 124 | Sol | Define compact Dream Begin codec using Memory cut + digest instead of full set. | Codec round-trip/corruption tests pass. |
| 125 | Sol | Update Dream validation to reconstruct population and validate roots against it. | Existing Dream validity rules remain exact. |
| 126 | Sol | Hard-cut compact Dream Begin writer. | Compact files stop persisting full `preexisting_ids`. |
| 127 | Sol | Implement V1.0 Dream migration cut derivation. | Representable legacy passes derive exact cuts. |
| 128 | Sol | Implement V1.0 Dream migration equality proof against the legacy explicit set. | Mismatch rejects. |
| 129 | Sol | Implement explicit-set exception encoding for legacy passes not representable by one exact cut. | Exceptional history remains semantically exact. |
| 130 | Luna | Add wrong-cut, forged-digest, root-outside-population, and source-in-population regressions. | Invalid histories fail closed. |
| 131 | Sol | Measure Dream first-pass raw bytes before/after. | Size artifact recorded. |

---

## Phase O — Dream Pair History

| Step | Owner | Work | Gate |
| ---: | --- | --- | --- |
| 132 | Sol | Define validated owner-local Memory-ID table for Dream pair physical encoding. | Table format/validation tests pass. |
| 133 | Sol | Implement compact Dream pair encoder/decoder using local references. | Decoded canonical pair set matches existing semantics. |
| 134 | Sol | Hard-cut compact Dream pair writer. | No compact full-ID pair record writer remains. |
| 135 | Sol | Extend upgrader for V1.0 Dream pairs. | Pair-set parity passes. |
| 136 | Luna | Add missing/corrupt mapping and retry regressions. | Invalid mappings fail closed. |
| 137 | Sol | Measure Dream-pair raw bytes before/after. | Size artifact recorded. |

### Integration Gate F — Dream

Run Dream/Freshness/import historical suites plus oracle comparison on Dream-bearing fixtures.

---

## Phase P — Interaction Stream

| Step | Owner | Work | Gate |
| ---: | --- | --- | --- |
| 138 | Sol | Freeze interaction stream state-machine semantics from current cumulative records. | Begin/append/interrupted transition table exists. |
| 139 | Sol | Define `StreamBegin` compact record. | Codec tests pass. |
| 140 | Sol | Define `StreamAppend` exact UTF-8 suffix record. | Codec tests pass. |
| 141 | Sol | Define terminal/interrupted compact record. | Codec tests pass. |
| 142 | Sol | Implement compact stream replay/materialization. | Every checkpoint reconstructs exact visible text/status. |
| 143 | Sol | Hard-cut compact stream writer to begin/append/terminal records. | No cumulative-prefix compact writer remains. |
| 144 | Sol | Update stream reconciliation for compact records. | Latest-state reconciliation matches current behavior. |
| 145 | Sol | Extend upgrader from cumulative V1.0 checkpoints to compact stream deltas. | Historical checkpoint parity passes. |
| 146 | Luna | Add long-stream linear-growth benchmark. | Growth is linear in appended text. |
| 147 | Luna | Add crash/reopen/interruption regressions. | Displayed-text durability invariant remains intact. |

---

## Phase Q — External Derived Index

| Step | Owner | Work | Gate |
| ---: | --- | --- | --- |
| 148 | Sol | Freeze the semantic boundary: `CVAXIDX1` is derived state; `CVAXMEM1` remains authoritative publication. | Boundary documented in tests/code comments. |
| 149 | Sol | Decide current-state vs incremental derived-index persistence based on rebuild/write behavior. | One strategy recorded. |
| 150 | Sol | Implement compact ExternalIndex codec for the selected strategy. | Round-trip/current-state tests pass. |
| 151 | Sol | Implement compact ExternalIndex rebuild/current projection. | Lookup behavior matches V1.0. |
| 152 | Sol | Make superseded derived index storage reclaimable without semantic prune markers. | Reclaim tests pass. |
| 153 | Sol | Extend upgrader to migrate only the current required derived index state. | External Memory publication history remains untouched. |
| 154 | Luna | Add damaged-index rebuild/discard regressions. | Existing recovery contract remains intact. |

### Integration Gate G — Operational/derived stores

Run interaction runtime, external-source index/publication, reconciliation, reclaim, and reopen suites.

---

## Phase R — Entity Resolution History

| Step | Owner | Work | Gate |
| ---: | --- | --- | --- |
| 155 | Luna | Classify every Entity Resolution persisted field as semantic proof, retry state, or reconstructible postimage/candidate material. | Field table complete. |
| 156 | Sol | Freeze compact Entity Resolution history contract from that classification. | Contract preserves every authoritative field. |
| 157 | Sol | Define initial candidate/evidence record. | Codec tests pass. |
| 158 | Sol | Define attempt/status delta record. | Codec tests pass. |
| 159 | Sol | Implement replay/materialization. | Accepted decisions/retries reconstruct exactly. |
| 160 | Sol | Hard-cut compact Entity Resolution writer. | No compact repeated full candidate snapshot remains unless classified authoritative. |
| 161 | Sol | Extend upgrader for V1.0 Entity Resolution history. | Decision/proof parity passes. |
| 162 | Luna | Add retry/conflict/import-readiness/adversarial resolution fixtures. | Existing Entity effects and readiness decisions remain identical. |
| 163 | Sol | Measure raw Entity Resolution history before/after. | Size artifact recorded. |

---

## Phase S — Community Proof Boundary

| Step | Owner | Work | Gate |
| ---: | --- | --- | --- |
| 164 | Luna | Identify every accepted proof that references Community generation/graph version/name. | Consumer inventory complete. |
| 165 | Sol | Define the exact reconstruction claim required to eliminate repeated membership snapshots. | Claim names graph cut + algorithm/policy version + any non-deterministic retained data. |
| 166 | Luna | Test that claim against authoritative/synthetic Community histories. | Exact yes/no evidence recorded. |
| 167 | Sol | If exact reconstruction is proven, define compact Community membership history; otherwise record explicit no-change decision. | One decision closed. |
| 168 | Sol | If compacting, implement Community codec/replay/writer. | Historical proof identity remains valid. |
| 169 | Sol | If compacting, extend upgrader. | Community oracle/proof parity passes. |
| 170 | Luna | If compacting, add semantic-name/non-determinism regressions. | Non-recomputable names/evidence remain retained. |
| 171 | Sol | If compacting, measure raw Community history before/after; otherwise mark measurement not required. | Evidence recorded. |

### Integration Gate H — Proof-boundary stores

Run Entity Resolution, Community, Dream/Freshness proof consumers, import readiness, and historical-oracle suites.

---

## Phase T — Archive Vector Generation Decision

| Step | Owner | Work | Gate |
| ---: | --- | --- | --- |
| 172 | Luna | Build repeated same-profile Archive-vector-generation workload. | Reproducible workload exists. |
| 173 | Luna | Measure unchanged exact vector-byte duplication across generations separately from compatibility-profile multiplicity. | Byte-duplication artifact exists. |
| 174 | Sol | Decide whether row/block reuse is justified by Step 173. | Explicit implement/defer decision recorded. |
| 175 | Sol | If justified, define immutable exact vector row/block identity preserving byte-distinct compatible embeddings. | Identity contract tests pass. |
| 176 | Sol | If justified, implement reusable row/block storage. | Exact vector bytes round-trip. |
| 177 | Sol | If justified, implement generation manifest/delta referencing row/block identities. | Historical generation materialization exact. |
| 178 | Sol | If justified, hard-cut compact Archive-vector generation writer. | No redundant full-matrix compact writer remains. |
| 179 | Sol | If justified, extend upgrader for historical generations. | Exact vector-byte/generation parity passes. |
| 180 | Luna | If justified, add retrieval and corruption regressions. | Retrieval parity and fail-closed behavior pass. |
| 181 | Sol | If justified, measure raw vector-generation bytes before/after; otherwise mark phase deferred. | Evidence recorded. |

### Integration Gate I — Vectors

Run profile, packed-vector, Memory-vector, Archive-vector, generation, retrieval, prune/reclaim, and rewrite-interaction suites.

---

## Phase U — Upgrade Coverage Completion

| Step | Owner | Work | Gate |
| ---: | --- | --- | --- |
| 182 | Luna | Re-run durable-family inventory against current source after all codec migrations. | No new/unhandled writer family appears. |
| 183 | Luna | Audit upgrade coverage for Archive-owned durable families after all earlier migrations. This step performs no implementation. | Either Archive coverage is complete or this plan is expanded with one explicit step per missing family before execution continues. |
| 184 | Luna | Audit upgrade coverage for import epoch and Historical Freshness settlement families. This step performs no implementation. | Either import-evidence coverage is complete or explicit per-family steps are inserted before execution continues. |
| 185 | Sol | Add/verify upgrader support for Echo records only. | Echo historical evidence parity passes. |
| 186 | Sol | Add/verify upgrader support for project-history correlation records only. | Project-history evidence parity passes. |
| 187 | Sol | Reject unknown/unsupported semantic authority explicitly during upgrade. | Unknown authority fails closed. |
| 188 | Sol | Reopen upgraded REL and validate all owner invariants. | REL validation passes. |
| 189 | Sol | Reopen upgraded PHY and validate all owner invariants. | PHY validation passes. |

---

## Phase V — Authoritative Fixture Migration

| Step | Owner | Work | Gate |
| ---: | --- | --- | --- |
| 190 | Luna | Upgrade disposable copy of 14-day REL. | Destination reopens; source hash unchanged. |
| 191 | Luna | Upgrade disposable copy of 14-day PHY. | Destination reopens; source hash unchanged. |
| 192 | Luna | Upgrade disposable copy of 28-day REL. | Destination reopens; source hash unchanged. |
| 193 | Luna | Upgrade disposable copy of 28-day PHY. | Destination reopens; source hash unchanged. |
| 194 | Luna | Upgrade disposable copy of Ellis REL. | Destination reopens; source hash unchanged. |
| 195 | Luna | Upgrade disposable copy of Ellis PHY. | Destination reopens; source hash unchanged. |
| 196 | Luna | Produce per-family raw-byte accounting for all six migrations. | Logical-history size report exists; compression excluded. |

---

## Phase W — Historical Equivalence Verification

| Step | Owner | Work | Gate |
| ---: | --- | --- | --- |
| 197 | Sol | Exhaustively compare Memory historical cuts source vs compact fixtures. | Zero mismatches. |
| 198 | Sol | Exhaustively compare Archive historical cuts where tractable. | Zero mismatches. |
| 199 | Sol | Exhaustively compare Entity and Relationship/Observation cuts. | Zero mismatches. |
| 200 | Sol | Exhaustively compare Graph cuts. | Zero mismatches. |
| 201 | Sol | Exhaustively compare Ego cuts. | Zero mismatches. |
| 202 | Sol | Exhaustively compare Freshness activity/event cuts. | Zero mismatches. |
| 203 | Sol | Compare vector-generation cuts and exact vector bytes. | Zero mismatches. |
| 204 | Sol | Compare Dream proof state. | Zero mismatches. |
| 205 | Sol | Compare global ordering points exhaustively where tractable and deterministically sample only where documented runtime makes exhaustive comparison unreasonable. | Zero mismatches; sampling rationale recorded if used. |

---

## Phase X — Maintenance and Reconciliation Verification

| Step | Owner | Work | Gate |
| ---: | --- | --- | --- |
| 206 | Sol | Run legacy-decoded vs legacy-decoded reconciliation reference cases. | Baseline recorded. |
| 207 | Sol | Run compact vs compact reconciliation cases. | Decisions match baseline semantics. |
| 208 | Sol | Run cross-format canonical-history reconciliation where supported. | Encoding does not affect conflict decisions. |
| 209 | Sol | Run compact REL prune tests. | Maintenance behavior unchanged. |
| 210 | Sol | Run compact PHY prune tests. | Maintenance behavior unchanged. |
| 211 | Sol | Run compact reclaim tests. | Reclamation preserves semantic history. |
| 212 | Sol | Run compact repair tests. | Repair behavior unchanged. |
| 213 | Sol | Run fresh-history rewrite from compact sources. | Rewrite semantics unchanged. |
| 214 | Sol | Reopen all maintenance outputs and rerun relevant oracle checks. | Zero semantic mismatch beyond explicit maintenance policy. |

---

## Phase Y — Performance and Checkpoint Decision

| Step | Owner | Work | Gate |
| ---: | --- | --- | --- |
| 215 | Luna | Measure cold reopen old vs compact fixtures. | Results recorded. |
| 216 | Luna | Measure warm reopen old vs compact fixtures. | Results recorded. |
| 217 | Luna | Measure recent historical lookup old vs compact. | Results recorded. |
| 218 | Luna | Measure middle historical lookup old vs compact. | Results recorded. |
| 219 | Luna | Measure oldest historical lookup old vs compact. | Results recorded. |
| 220 | Luna | Measure upgrade throughput. | Results recorded. |
| 221 | Luna | Measure peak migration memory. | Results recorded. |
| 222 | Sol | Decide whether replay cost justifies derived checkpoints. | Implement/defer decision recorded from measurements. |
| 223 | Sol | If justified, specify checkpoint semantics under `version-history-plan.md`. | Checkpoint is explicitly disposable/non-authoritative. |
| 224 | Sol | If justified, implement checkpoint generation/load path. | Deleting checkpoints and rebuilding yields identical state. |
| 225 | Sol | If justified, add checkpoint corruption/rebuild tests. | Corruption cannot alter authority. |

---

## Phase Z — Architecture Closeout

| Step | Owner | Work | Gate |
| ---: | --- | --- | --- |
| 226 | Sol | Write compact-history ADR capturing only hard-to-reverse decisions. | ADR reviewed against implementation. |
| 227 | Sol | Update ADR index. | Navigation correct. |
| 228 | Luna | Update `storage-format.md` with compact record families and migration boundary. | Format doc matches source. |
| 229 | Luna | Update `architecture.md` and `invariants.md`. | Ownership/history invariants current. |
| 230 | Luna | Update `current-limitations.md` and `maintainer-map.md`. | Operational limitations/navigation current. |
| 231 | Luna | Update affected API/development docs and docs index. | Documentation compliance passes. |
| 232 | Luna | Refresh Lexicon/Arcana from final source. | Fresh snapshot available or explicit bounded limitation recorded. |
| 233 | Sol | Audit reverse dependencies of all legacy writers. | Legacy writers have no ordinary compact-file callers. |
| 234 | Sol | Audit mixed-format append paths. | None remain. |
| 235 | Sol | Audit compatibility leakage outside open/migration boundaries. | None remain. |
| 236 | Sol | Audit unexpected cross-owner coupling introduced by compact framing/deltas. | None found or documented/fixed. |
| 237 | Sol | Confirm with direct source search that no migration-only alias/fallback/shadow writer remains. | Hard-cut cleanup complete. |

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
| 238 | Sol | Record final implementation commits and changed-file inventory in this plan. | Execution record complete. |
| 239 | Sol | Record actual raw logical-history savings by family and by fixture. | Logical savings report complete. |
| 240 | Sol | Record ADR-0004 compression results separately from logical-history savings. | No conflation of compression and logical compaction. |
| 241 | Sol | Record every measurement-gated defer decision (Community/vector/checkpoints/etc.). | Deferred work has explicit evidence/reason. |
| 242 | Sol | Update spec status to match shipped architecture and remaining limits. | Spec and implementation agree. |
| 243 | Sol | Mark plan complete only after confirming no temporary migration scaffolding remains. | Final hard-cut acceptance complete. |

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

The numbered tables are execution responsibilities; the unnumbered integration gates are acceptance checkpoints.

## Tracking

This decomposition contains **243 execution steps** because the previous 74-step version bundled independent schema, codec, replay, writer, migration, proof, fixture, and measurement responsibilities.

The number is not a target. It is the consequence of making those failure domains explicit.

If any one of these steps still requires multiple independent implementation passes, split it before starting. Do not preserve the numbering at the expense of bounded execution.
