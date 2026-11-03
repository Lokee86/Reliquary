# CVA-to-REL source migration and proof matrix — 2026-11-03

Parent index: [Documentation index](INDEX.md)

**Status:** Source-backed impact and acceptance map for [Stage 5 format/identity design](cva-to-rel-format-and-identity-migration-2026-11-03.md). Not an implemented converter or a compiled call-site migration. The [wire inventory JSON](architecture/cva-source-wire-inventory-2026-11-03.json) contains **173 lexical marker candidates** and six direct SHA-256 identity-domain candidates, plus representative paths. Format decisions are [ADR 0048](decisions/0048-rel-current-format-and-stable-identity-domains.md).

## Purpose

Make the eventual code and corpus hard cut reviewable **by owned boundary**, not by a 12,122-match global rename. Specify the exact checks needed to protect every dangerous format, identity, owner and consumer surface.

## Overview

The 2026-11-03 tracked Rust lexical scan found **1,415 files**, **644 with CVA matches**, **12,122 case-insensitive occurrences**, and **72 filenames containing cva**; about **173 distinct eight-byte alphanumeric marker tokens** occur across production/test source. That is a discovery upper bound, including retired/invalid examples. Source-only counting neither verifies active writers nor proves all indirect/dynamic marker constructions. Every live record requires an actual codec acceptance table at implementation time.

## 1. Code naming and public interface

| Source seam | Required rename or relocation | Gate |
| --- | --- | --- |
| `src/facade/cva.rs` and all `src/facade/cva_*.rs` | Current `Cva` storage aggregate replaced by one `RelSession`; owner operations go to new `reliquary-owner-session`, multi-domain workflows to `reliquary-application`, and only narrow `Rel` public accessors stay in Root | No old `Cva` symbols/aliases or broad pub(crate) store bag; existing 519-method crosswalk and full helper/caller mapping preserved |
| `src/lib.rs` | Replace `pub mod cva`, `pub use Cva` and path modules with canonical `Rel` exports from approved packages | Rust Root no backreach and all external CLI/consumer API changes accounted for |
| `src/runtime_inference/interaction_runtime.rs`, `runtime_host_gateway*.rs` | Owner handles use `RelSession`, named workflow grants/receipts, not a renamed fat aggregate | Current owner-local worker behavior, host journals, grant revocation and stream conformance |
| `src/perception/**`, `src/retrieval/**`, `src/imports/**`, `src/rewrite/**`, `src/semantic_graph/**` | Responsibility-specific module names rather than mechanical `cva_` prefix | Existing semantic/proof behavior stays with correct owner, no deleted features |
| `cli/src/args.rs`, `cli/src/args_tests.rs`, `cli/src/commands/**` | Existing `rel` CLI subcommand drops `visible_alias = "cva"`; rename `cva: PathBuf`, display labels, aliases and old example `.cva` paths | CLI parse/help, commands, scripts and consumers accept canonical REL naming; old alias has negative test |
| `crates/**`, `tests/**`, `examples/**`, `benches/**`, generated Rust | Update test symbols, filenames, module `#[path]` links, fixtures/error text and helper paths | Case-insensitive CVA content AND basename scan zero in active tracked/generated code; no hidden exclusions |
| All active deployment/consumer API wiring | Separate consumers Warlock/Hermes and remaining callers must adopt new type/module/CLI names during the cut | Consumer conformance and no duplicate live symbols; never assume an external repo compiled from just Reliquary green checks |

## 2. Container and durable format code

| Current source family | Wire impact | Positive and negative proof |
| --- | --- | --- |
| `crates/reliquary-container/src/container.rs` | Header `CVA\0\r\n\x1a\n` → `REL\0\r\n\x1a\n`; 2.0 → **2.1**, retain 40-byte identity header | Exact kind/UUID, unsupported old signature with no repair/mutation, bad 2.1 signature/version fail |
| `container/src/publication_envelope.rs`, `version.rs` | Physical envelope and reservation version tags rebrand to REL; no semantic clock change | Same global coordinates/time; invalid duplicate/old reservation carrier rejected |
| `container/src/grouped_clock.rs` | Source-grouped carrier `CVAINSC7/8`, `CVAXMEM2` rebrand to REL; preserve grouped embedded version range | Exact commit/replay/post-sync Memory and Freshness birth; nested/partial/future carrier rejected |
| `reliquary-archive/src/**`, `reliquary-echo/src/**` | Turn/Branch/Node/Episode/File/Echo/compaction/source record tags | Same transcript/source history, FileId, fragment spans, accepted activity, Echo/compaction evidence |
| `reliquary-memory-core/src/**` | Memory body/delta/routing/history and publication markers | MemoryId/body/revision history identical; tombstone/payload-unavailable remain distinct |
| `reliquary-graph/src/**` | Node/edge/mutation/Community compact and publication records | Stable endpoint/Graph refs, Community generations and local clocks |
| `reliquary-perception-store/src/**` | Entity, mention, resolution, transitional Relationship and publication | Exact mention spans, canonical Entity IDs and historical decisions, PHY-private references |
| `reliquary-ego-store/src/**` | Identity, Personality, Anchors and compact/delta evidence | Exact Ego revisions and authorized content projections |
| `reliquary-freshness-store/src/**` | Event receipts, score, semantic import-settlement, rewrite baseline; **FRS07 source-corpus gap** | First prove six-file FRS07 old baseline; same +25/+50 receipts, F8 and score/history after conversion |
| `reliquary-vector-store/src/**` | Packed/Memory/Archive vectors, profile/generation records and relationships | Same all six hash-derived IDs, exact vector/matrix/probe bytes, rows/generations |
| `reliquary-insomnia-store/src/**`, Dream Begin/journal, Import/Retention stores | Work/attempt/lease, first pass, epoch/manifest versions | Same grouped attempts and first-cycle settlement; independent replay without model runtime |
| `reliquary-external-index-store/src/**`, `src/external_source/**` | Derived index CAS/compaction and external source Memory carriers | Rebuild only derived data; retain external host source identity and accepted publication receipts |
| `src/runtime_inference/multiplexing_journal.rs`, owner-operation journal | Existing `RQMUX*`/`RQOP*` current operational signatures are not old CVA-branded domain records | Exact host intent/status, source IDs, pending/finalize semantics; unknown status Held |

## 3. Deterministic hash-domain golden proofs

**Identity preimage bytes are preserved**, not replaced by similar `REL-...` strings. Their implementation will use versioned opaque binary constants with canonical names, and tests must prove equality to old committed IDs.

| Identity | Source function | Required comparison |
| --- | --- | --- |
| `FileId` | `reliquary-archive/src/file_store.rs::file_id` | Known filename/MIME/content/size produce identical FileId; internal project revision path remains separate |
| `ArchiveVectorId` | `reliquary-vector-store/src/archive.rs::archive_vector_id` | Same packed ID and exact ArchiveFragmentId list → same ID |
| `MemoryVectorId` | `reliquary-vector-store/src/memory.rs::memory_vector_id` | Same profile/packed/body IDs and order → same ID |
| `CompatibilityProfileId` | `reliquary-vector-store/src/profile.rs::compatibility_profile_id` | Same normalization/probes/version/floats → same ID |
| `VectorGenerationId` | `reliquary-vector-store/src/generation.rs::vector_generation_id` | Same profile/Archive ID and source version → same ID |
| `PackedVectorId` | `reliquary-vector-store/src/store.rs::packed_vector_id` | Same scalar/schema and matrix bytes → same ID |

The converter must additionally identify any ID, external source digest or persisted proof that hashes **physical record bytes** (not just the six known explicit string domains). A file checksum or serialized-record digest may legitimately change after re-encoding; a stable referenced semantic ID may not. Ambiguity is a stop condition.

## 4. Corpus and conversion proof

**Minimum already known retained cohort:** `tests/current_format_boundary.rs` lists six authoritative files, one REL + one PHY for each of three scenarios: first 14 days, first 28 days, Ellis pre-relationship. That test is currently ignored. Copy-only earlier check: three PHY reopened; three REL rejected because promoted `CVAFRS07` data did not match this branch's `CVAFRS06` implementation. The test itself cites 2026-10-23 promoted file hashes and owner heads.

Capture for **every** conversion input/output: original path and digest; output digest and version; owner kind/UUID; every physical reservation and global clock/transaction time; per-domain accepted record count and semantic version; historical fingerprint per valid cut, tombstone/retained bytes, Memory/Entity/Graph/Ego/Insomnia/Dream/Freshness state; profile/vector/generation IDs and matrix rows; source attachments/VCS refs/host source names; object-offset relocation map; retrieval scoring/ranking/selection and authorized privacy probes.

**Promotion requires:** green FRS07 baseline first, converter knows all current records, output opened by current-only 2.1 runtime, negative invalid/mixed/old format tests, no history/identity mismatch, validated cross-REL/PHY operation receipts and a manifest/rollback process that does not pretend six file renames are atomic. No production normal-open may retain a 2.0 decoder.

## 5. Remaining implementation evidence (not new design choices)

- Source-driven codec classification of all 173 lexical candidates, including constructed partial-prefix comparisons and present/retired/test cases.
- Offline old-format corpus reopen with correct FRS07 handling and full six-file proof.
- New typed 2.1 codec and converter compiled with exact reference relocation/identity vectors.
- All supported host/CLI/API callers compiled after the hard cut; pre/post behavior and performance comparisons.
- Source scans showing **zero textual old naming** in active Rust files and path names, while documents can record historical labels.
- Temporary converter and old reader deleted at the end of the verified corpus promotion.

## Related docs

- [Stage 5 migration design](cva-to-rel-format-and-identity-migration-2026-11-03.md)
- [ADR 0048](decisions/0048-rel-current-format-and-stable-identity-domains.md)
- [Machine-readable source inventory](architecture/cva-source-wire-inventory-2026-11-03.json)
- [Previous format-hard-cut policy](development-format-hard-cut-interruption-spec-2026-10-19.md)
- [Stage 4 package disposition](physical-crate-source-disposition-2026-11-03.md)

## Notes

This document is not an endorsement of blind prefix substitution. Physical file bytes/digests will change, but canonical semantic content, stable IDs and accepted history must not. The scope of the final zero-CVA code check is active compiled/source code, not historical documentation.
