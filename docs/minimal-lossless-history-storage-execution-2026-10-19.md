# Minimal lossless history storage — execution evidence

Date: 2026-10-19

## Purpose

Record verified execution evidence for the [linear plan](minimal-lossless-history-storage-linear-plan-2026-10-19.md). The plan remains the authority for step ordering and completion.

## Overview

The accepted import/pruning/rewrite and decision changes were integrated before this work began. No compact storage mutation is permitted before the historical-oracle gate.

## Step 1 — execution baseline

- Branch: `main`.
- Baseline commit: `a2c9a2def76a1e635f8b70300107a013f983af78`.
- Accepted implementation commits: `06b11ab` (semantic import, pruning, rewrite) and `dd6ad76` (decision endpoint inheritance).
- Dirty status: clean, verified after committing the two planning documents and their documentation-index links.
- Container format: V1.0; `CURRENT_VERSION` in `src/container/container.rs` is `{ major: 1, minor: 0 }`. The typed REL/PHY header is 40 bytes; legacy 16/24-byte headers remain compatibility inputs.
- Accepted test baseline: the integrated implementation's [Step 65 acceptance](semantic-import-pruning-history-rewrite-evidence-2026-10-18.md#step-65--final-integration-and-compatibility-acceptance) records root formatting/check passing, 1,497 library passes / 1 transient Windows Lore file-lock failure / 5 ignored, an isolated pass of that failure, the passing compatibility/maintenance matrix, 16 rewrite-contract passes, and 32 CLI passes. This is the accepted baseline, not a claim that every historical suite was green without qualification.
- Fresh baseline checks: root/CLI formatting and both documentation-policy modes pass. A fresh root test run and root/CLI checks are being collected; results are recorded below when available.
- Test execution settings: `CARGO_INCREMENTAL=0`, `CARGO_BUILD_JOBS=2`; `TEMP` and `TMP` point inside `.test-tmp/lossless-baseline/`.
- The initial compile-only test attempt was cancelled before test execution to set repository-local scratch paths; it is not counted as a test result.

## Step 2 — authoritative fixture identity

`scripts/history_fixture_inventory.py` reads all six files twice using read-only handles, checks the typed V1.0 header/kind, and compares hashes with the accepted Step 62 corpus. It writes [the fixture baseline](../benchmarks/history-fixture-baseline-2026-10-19.json).

Verification: six fixtures, **138,196,316 bytes**, identical across both passes and identical to all six accepted Step 62 hashes. No source fixture was changed.

## Step 3 — durable writer inventory

`scripts/history_writer_inventory.py` produces [the exact family/source inventory](../benchmarks/history-durable-family-baseline-2026-10-19.json): **117 identities / 66 families**, plus 119 direct append call sites. It fails for an unclassified eight-byte REL/PHY magic. Compatibility reader identities are included explicitly; their presence does not imply an ordinary writer exists.

The physical append search is supporting lexical evidence. This reviewed table includes delegating calls and mutable physical writes that a direct-call search does not establish:

| Families | Production writer / boundary |
| --- | --- |
| Container header, global tickets | `container.rs` create methods; `container_version.rs` allocation and grouped range observation |
| Archive format/publication | `archive_history.rs` initialize/publish |
| Archive content, Node, Branch | `archive_store.rs` put-content; `archive.rs` append-node/branch |
| Conversation metadata, terminal closure | `archive_conversation_metadata.rs`; `archive_conversation_tombstone.rs` |
| Fragment, Episode, File, turn attachment, File–Memory link | `fragment_store.rs`, `episode_store.rs`, `file_store.rs`, `turn_ingest_store.rs`, `file_memory_link_store.rs`; accepted through Archive publication |
| Import receipt | `archive_import.rs`; accepted through Archive publication |
| Archive prune, Memory prune, Entity/Relationship description prune, packed-vector prune, maintenance manifest | `cva_history_prune.rs`, `phylactery_history_prune.rs`, `cva_description_prune.rs`; separate validated-copy transformations |
| Conversation compaction | `conversation_compaction_store.rs` and allocator; `Container::write_chunk_prefix`, `split_chunk`, `merge_adjacent_chunks` mutate allocated/free extents |
| Interaction stream | `interaction_stream_store.rs` append checkpoint |
| Echo | `echo_store.rs` ordered evidence append |
| Project correlation, Project File binding | `project_history_store.rs`; `project_file_binding_store.rs` |
| Memory format, body, revision, publication, terminal, routing | `memory_store.rs`; grouped and external publication carriers below share Memory application, not duplicate standalone publication |
| Processing epoch | `processing_epoch_store.rs` |
| Entity revisions, merge/direct terminal, publication | `entity_store.rs` |
| Entity Resolution initial/attempt/status/terminal | `entity_resolution_store.rs`, delegated by `_write.rs` and `_lifecycle.rs` |
| Transitional Relationship revisions/terminal/publication | `relationship_store.rs`; Observation persistence has no separate current writer |
| Graph format/catalogue/mutation/batch/publication | `graph_store.rs`, `graph_store_write.rs` |
| Graph rewrite baseline | `graph_rewrite_baseline.rs`, invoked only by explicit fresh-history rewrite |
| Community membership and semantic name | `community_store.rs` |
| Dream pairs | `dream_pair_history.rs` |
| Dream Freshness journal | owner encoding in `freshness_dream_journal.rs`; appends in `cva_freshness.rs`, `freshness_access.rs`, Insomnia and historical-import adapters |
| Freshness policy, events, import settlements, legacy activation, rewrite baseline | `freshness_storage.rs`, `freshness_semantic_activation.rs`, `cva_freshness.rs`, `cva_historical_import.rs`, `cva_semantic_source.rs`, `freshness_access.rs`, rewrite adapters. New scoped import settlement is CVAFRS06; unscoped CVAFRS04 remains supported |
| Import start/completion | `imports/epoch_store.rs`; production completion requires coordinator authority |
| Insomnia format/work | `insomnia/store.rs`, `insomnia/store/transitions.rs`; in-memory leases/retries do not imply additional durable records |
| Insomnia grouped completion | `insomnia/processor/application.rs`, `insomnia/cva.rs`; CVAINSC6 contains accepted Memory records/bodies, clocks, routing and birth evidence atomically |
| External derived index | `external_source/store.rs` |
| External Memory publication | `external_source/publication.rs`; CVAXMEM1 is the atomic Memory/body/routing/birth authority |
| Ego Identity, activation, Personality, Anchor, synthesis | `ego_store.rs` append/apply; concrete `_identities.rs`, `_anchors.rs`, `_documents.rs` callers |
| Packed vectors, Memory vectors, Archive vectors, profiles/retirements, vector generations | concrete stores in `src/retrieval/`; generation publication owns activation |
| REL metadata, PHY principal profile | `rel_metadata_store.rs`; `phylactery_profile_store.rs` |
| Rewrite lineage | `rewrite/file.rs` CVARGN01 append on a new history generation |
| Legacy workspace, Dream cooldown, import legacy epoch, older magic variants | explicit migration/read compatibility; no ordinary canonical writer. Maintenance may preserve or transform accepted legacy input |
| Byte-preserving or relocating copy paths | `cva_repack.rs`, `cva_reconcile_repack.rs`, `storage_reclamation.rs`, `imports/reconciliation_interval.rs`, `cva_reconcile_memory.rs`, `cva_reconcile_freshness.rs`, rewrite file/state adapters. They retain concrete owner validation rather than introducing new families |

Outside this inventory's authority: config/credentials/master-key files, repository-external gateway journals, and CVAXPLN1 frozen external processing plans. The latter is explicitly classified as outside REL/PHY by the script. Zero-filled compaction allocation/free extents are physical mechanics, not an unknown semantic family.

Verification: inventory generation succeeds with no unknown or missing identity. Direct source searches for `.append(` and compaction's in-place write/split/merge calls were reviewed against this table.

## Step 4 — semantic owner and clock map

The following maps every Step 3 family group to its authority. A shared physical carrier does not transfer ownership to Container or the facade.

| Family group | Semantic owner and accepted coordinates |
| --- | --- |
| Header/global tickets | Container owns physical identity and dense global ordering / optional transaction time. No semantic ancestry |
| All Archive publications (including receipt and terminal closure) | Archive owns dense Archive version plus allocated global version; Node parent/Branch revision semantics remain conversation-local |
| Archive content | Archive owns immutable content ID; inert until referenced by accepted publication |
| Standalone Memory revisions/terminal | Memory owns dense Memory version, per-ID revision, mutation ID and global version |
| Grouped Insomnia/external Memory publication | Memory owns the embedded contiguous Memory/global ranges; Insomnia/external source owns atomic publication/retry evidence; one physical carrier |
| Memory body/routing | Memory owns body identity and source/routing validation; no independent semantic clock |
| Entity | Entity owns dense Entity version, per-ID revision and global version; merge and direct terminal remain explicit |
| Transitional Relationship | Relationship owns dense Relationship version, per-ID revision and global version. Canonical-target decision remains Step 76 |
| Graph | Graph owns dense full-Graph version and accepted global version; Memory-Graph cut advances only for qualifying Memory topology. Catalogue ordinals are physical owner-local references |
| Graph rewrite baseline | Graph owns explicit rewritten topology/counters; ordinary compact migration must not reissue it as a new semantic history |
| Community membership/name | Community owns generation and source graph cut; names bind Community/baseline identity, naming contract and source, without an independent revision clock or global ticket |
| Processing epoch | Processing owner records per-Memory lane epoch; clock-neutral to Memory/Graph/global semantic versions |
| Dream pairs/cooldown/Freshness pass | Dream owns accepted pair facts and pass lifecycle/proofs; no new global clock. Legacy cooldown decoding remains migration-local |
| Freshness policy/events/settlements/activation/baseline | Freshness owns policy, activity/event cuts, exact receipt identity and ordered settlement linkage. Archive supplies REL activity; equal activity cuts do not erase physical acceptance order |
| Import epoch/legacy epoch | Import coordinator owns start/completion identity, owner watermarks and settlement bindings; records allocate no semantic ticket |
| Insomnia format/work | Insomnia owns Episode work state, attempts and leases; operational clock-neutral state |
| Entity Resolution | Perception resolution owns mention key/revision/status/proof and attempt time; accepted Entity effects retain Entity clocks |
| Ego | Ego owns independent contiguous mutation version and optimistic per-record revisions; activation is separate from content revision and consumes no global ticket |
| Packed/Memory/Archive vectors | Concrete vector stores own immutable exact numeric bytes and profile/body/fragment bindings; backing objects do not independently activate generations |
| Compatibility profile | Profile owner owns immutable compatibility identity and explicit retirement; no fabricated global/local semantic cut |
| Vector generation | Generation owner owns dense vector version, generation ID, global version and profile/population activation |
| Conversation compaction | Conversation compaction owns latest accepted owner-local state/free allocation, without Archive/global publication |
| Interaction stream | Interaction stream owns message/session identity and monotone checkpoint/interrupted state; no Archive/global ticket |
| External index | ExternalIndexStore owns rebuildable latest derived source state; no semantic history clock |
| Echo / Project correlation / File binding | Their concrete Archive-adjacent stores own evidence order/references; no automatic Archive/global version allocation |
| REL metadata / PHY profile | Respective metadata owner owns latest-wins state; neither label nor display name becomes owner identity or a semantic clock |
| Prune markers / maintenance manifest | Domain owners prove eligibility; OwnerMaintenanceStore authenticates destructive-copy evidence; no new semantic mutation clock |
| Rewrite lineage | RewriteLineageStore owns history-generation/predecessor identity; distinct from owner UUID and global ordering |
| Legacy workspace | Compatibility decoder only; no current mutable semantic owner |

Verification: allocation sites in `container_version.rs`, `archive_history.rs`, Memory/Entity/Relationship/Graph/generation writers, and clock-neutral append/apply stores inspected. No family is left with an implicit clock owner.

## Step 5 — retained responsibility classification

Each inventory family has exactly one primary disposition below. Proof-bearing semantic or operational carriers retain their proof obligations; a primary disposition is not permission to delete embedded fields.

| Disposition | Exact inventory family names |
| --- | --- |
| Physical metadata | `container_header`, `global_ticket`, `archive_publication`, `memory_format`, `memory_publication`, `insomnia_format`, `legacy_workspace` |
| Immutable payload | `archive_content`, `memory_body`, `packed_vectors`, `memory_vectors`, `archive_vectors`, `compatibility_profile` |
| Semantic history | `archive_node`, `archive_branch`, `archive_conversation`, `archive_terminal`, `archive_fragment`, `archive_episode`, `archive_file`, `archive_turn_attachment`, `archive_file_memory_link`, `memory_revision`, `memory_terminal`, `memory_routing`, `entity`, `relationship`, `graph`, `graph_rewrite_baseline`, `dream_pairs`, `freshness_policy`, `freshness_events`, `freshness_rewrite_baseline`, `insomnia_completion`, `external_publication`, `ego_identity`, `ego_personality`, `ego_anchor`, `vector_generation` |
| Proof/evidence | `archive_import_receipt`, `archive_prune`, `echo`, `project_correlation`, `project_file_binding`, `memory_prune`, `entity_prune`, `entity_resolution`, `relationship_prune`, `community_name`, `dream_freshness`, `freshness_import_settlement`, `freshness_legacy_activation`, `import_epoch`, `import_legacy_epoch`, `owner_maintenance`, `rewrite_lineage` |
| Derived/recovery state | `conversation_compaction`, `interaction_stream`, `processing_epoch`, `community_membership`, `legacy_dream_cooldown`, `insomnia_work`, `external_index`, `ego_synthesis`, `rel_metadata`, `phy_profile` |

Semantic-history exceptions are intentional: graph-rewrite and Freshness-rewrite baselines authenticate the start of an explicitly new history; a lossless upgrade must retain their meaning. Vector/profile immutable families include retirement/prune facts, which remain required evidence even when their payload is classified primarily immutable. Community membership is derived but accepted proofs may require exact historical identity; Steps 164–171 must prove reconstructibility before removing snapshots. Ego synthesis and stream checkpoints may be derived/recovery state without being safe to recompute through inference or drop during an incomplete operation. REL/PHY presentation metadata is current-state metadata, not historical owner semantics; exact migration still preserves its current required values.

Verification: the 66 exact family names above cover the inventory once, with no duplicate classification. Classification adds no writer, codec, deletion or semantic clock change.

## Step 6 — structural baseline and refresh limitation

Before any Rust source change, `python scripts/check_architecture.py --refresh` passed the 92-fixture P4 guard, the 166-public-API classification guard, the 11-fixture external ownership guard and its production-boundary check. Lexicon started Rust analysis but produced no new CURRENT snapshot during the bounded run; it was interrupted after approximately three minutes. No fresh Arcana/Pitlord pass is claimed.

The pre-existing `.lexicon/CURRENT` and `.arcana/CURRENT` both identify `sha256:8b081bb6cadcab766ac48e9c3b645d55d150a4c47ea1b9e37977f517fc98477a`. That is the prior integrated Step 65 snapshot, not a refreshed snapshot of later code. Generated graph state remains disposable and ignored. The exact lexical writer inventory supplements this bounded analysis limitation; it does not replace final structural enforcement.

## Step 7 — canonical fingerprint contract

The internal `#[cfg(test)]` module `src/facade/historical_fingerprint.rs` defines schema 1 for one owner projection. It adds no public API, persisted record, runtime owner, or writer. The record exposes schema, durable owner UUID, semantic domain, local cut, optional exact event cut, row count, and SHA-256 digest.

Digest preimage, in order:

1. `reliquary:historical-owner-state:v1\0` domain separator;
2. little-endian u32 schema 1 and 16 owner UUID bytes;
3. domain string framed by a little-endian u64 byte count;
4. little-endian u64 local cut;
5. event-cut presence byte, followed by little-endian u64 only when present;
6. little-endian u64 row count;
7. rows sorted by `(family UTF-8 bytes, stable key bytes)`, each with independently u64-length-framed family, key and canonical JSON semantic value.

JSON objects sort keys recursively; array order and exact Unicode/byte representation remain meaningful. Null, absent fields and empty strings differ. Signed/unsigned exact 64-bit integers remain exact; floating-point values must be supplied as integer bit patterns. Duplicate row keys or invalid nested values reject before changing the projection. Owner adapters must sort set-valued semantic collections themselves and may not supply storage offsets, encoded record bytes or Debug strings as semantic values.

An independently calculated empty Memory-cut-3 digest for owner UUID `[7; 16]` is `8d713b0e9efa2ef12fb9eef2878e7bdfba1c217a3b738f284c7edcf4e0ee000a`. Six focused tests cover canonical ordering, collision-resistant framing, identity/cut binding, exact semantic differences, invalid-input atomicity and this golden vector. Verification: all six tests pass both in the integrated crate test executable and in a standalone compilation of the exact source module with the same dependency libraries. The integrated executable uses a separate repository-local output name because the baseline suite held the ordinary Windows test executable open during linking.

## Step 8 — Archive historical fingerprint

`historical_archive_fingerprint.rs` selects only accepted `ArchiveRecordVersion` publications through the requested Archive cut. It canonicalizes decoded semantic fields for every Archive variant, preserves publication/global/time coordinates, and includes the exact referenced content bytes once per ContentId. Pruned content remains explicit evidence; Project-backed files preserve exact repository references. Physical ObjectRef locations, unpublished records and unreferenced content are excluded. Retained accepted revisions are part of the oracle, so both current and historical reconstruction errors remain observable.

The branch projection agrees with existing `branch_at` at every cut of a synthetic history. Dedicated tests cover terminal closure, reopen, inert backing, and reclaim with a proven change in published ObjectRef location. The fixture feature runs only on newly created writable byte copies; using `fs::copy` incorrectly preserved authoritative Windows read-only attributes and was corrected in the harness without changing sources.

The authoritative fixture run passes **3/3** (62.22 seconds), including repeated empty/first/middle/head cuts for all three RELs:

| REL | Archive head | Tested cuts |
| --- | ---: | --- |
| first 14 days | 7,526 | 0, 1, 3,763, 7,526 |
| first 28 days | 21,068 | 0, 1, 10,534, 21,068 |
| Ellis | 3,763 | 0, 1, 1,881, 3,763 |

Final strengthened synthetic checks pass **3/3**: public historical API agreement/inert backing, terminal closure across reopen, and authenticated reclamation that actually relocates published objects.

These sampled determinism checks do not claim exhaustive cross-format parity; exhaustive cut enumeration and baseline artifacts remain Steps 19–21. All six authoritative hashes were re-read afterward and still match the accepted baseline. Opt-in fixture checks use the `history-oracle-fixtures` Cargo feature and add no ignored tests to the ordinary suite.

## Step 9 — Memory historical fingerprint

`historical_memory_fingerprint.rs` reads accepted MemoryStore revisions independent of the standalone/grouped/external carrier. It materializes canonical revisions, immutable bodies, body-bound routing and external evidence, explicit terminal coordinates and current membership at a Memory version. Every MemoryRecord, MemorySourceRef and temporal-inference field is destructured exhaustively: adding a field requires updating the oracle rather than silently omitting it. Pruned bodies remain explicit unavailable evidence instead of invented payload. Existing historical getters expose current body-bound routing attachments across revisions; this adapter follows that rule and does not invent an independent routing clock.

Two synthetic checks pass: historical revisions/current membership across archive metadata changes, terminal publication and reopen; and explicitly pruned PHY payload matched against the public historical unavailable result. The first authoritative run passes **3/3**, fingerprinting every retained Memory revision twice and stable empty/middle/head owner cuts across all six fixtures (36.73 seconds):

| Fixture | REL revisions | PHY revisions |
| --- | ---: | ---: |
| first 14 days | 3,348 | 421 |
| first 28 days | 5,727 | 490 |
| Ellis | 1,452 | 976 |

Total: **12,414 revisions**. All six source hashes remain unchanged. The strengthened final run passes **3/3** (40.20 seconds), including latest oracle membership equality with `MemoryStore::current_ids` for all six files.

## Step 10 — Entity historical fingerprint

`historical_entity_fingerprint.rs` projects all accepted Entity revisions through a local cut, including stable global identity, exact name/aliases/summary, mutation/timing coordinates, current membership, direct terminal facts and merge replacement references. Exhaustive EntityRecord destructuring prevents silent field omission. Pruned descriptions use explicit unavailable payload rather than interpreting decoder placeholders as authored empty text.

Verification passes **3/3** (25.48 seconds): every cut of a synthetic revision/merge/direct-tombstone history survives reopen; public historical pruned-description behavior agrees; and all six authoritative fixtures have deterministic sampled cuts and exact latest membership agreement.

| Fixture | REL head / revisions | PHY head / revisions |
| --- | --- | --- |
| first 14 days | 150 / 150 | 0 / 0 |
| first 28 days | 671 / 668 | 35 / 35 |
| Ellis | 199 / 192 | 86 / 84 |

The corpus contains **1,129 payload revisions** and **12 terminal version positions**. The first-14-day PHY Entity owner is empty; nonempty lifecycle, merge and prune behavior is covered explicitly by the synthetic cases rather than attributed to that empty fixture.

## Step 11 — Relationship historical fingerprint

`historical_relationship_fingerprint.rs` projects the currently canonical persisted Relationship owner. Exhaustive record/participant destructuring retains qualified EntityRef and MemoryRef endpoints, participant roles, evidence, summary, mutation identity, publication coordinates, lifecycle and exact unavailable-summary evidence. No Observation target or migration decision is introduced.

Verification passes **3/3** (23.18 seconds): synthetic qualified participant/evidence revisions, every local cut through tombstone and reopen, and pruning with structural evidence retained and public unavailable-summary agreement. All six authoritative Relationship owners have head zero and zero revisions; their empty projections are deterministic. Nonempty behavior is established by the synthetic cases, not by those fixtures. All six authoritative source hashes remain unchanged.

## Step 12 — Graph historical fingerprint

`historical_graph_fingerprint.rs` reads accepted Graph version publications and their decoded semantic changes. It preserves typed endpoint identities, relation kind/active/origin, transaction/global/local coordinates, exact publication time, inactive revisions and the isolated Memory population admitted by a fresh-history baseline. Physical node ordinals, chunk references and unpublished backing are excluded. MemoryGraph uses the last qualifying full-Graph version, not a separately dense counter; nonqualifying local cuts are rejected for its projection.

Graph history is independent of current Memory/Entity lifecycle exclusions. Verification applies the existing owner visibility rule when comparing current topology, and checks that a Memory terminal does not fabricate a Graph publication or alter an earlier Graph fingerprint. Whole-owner historical visibility remains Step 20. The baseline owner/generation/policy tuple is validated against RewriteGenesis on open and belongs to that cross-owner lineage proof comparison; Step 20 explicitly owns it, while this Graph fingerprint records the admitted node population.

The initial fixture run passed both tests (57.85 seconds). Its latest projections agree with replayed active full-Graph relations, MemoryGraph relations and clock:

| Fixture | REL Graph / MemoryGraph | PHY Graph / MemoryGraph |
| --- | --- | --- |
| first 14 days | 7,913 / 7,913 | 667 / 667 |
| first 28 days | 25,337 / 25,337 | 1,690 / 1,637 |
| Ellis | 6,309 / 6,309 | 4,159 / 3,889 |

Luna review strengthened cached determinism checks into comparisons across independent file scans and added a hand-authored first-publication fingerprint. Additional synthetic cases cover typed-only clock gaps, principal associations, lifecycle exclusions, and isolated nodes after authentic fresh-history rewrite. The first strengthened run exposed invalid fixture assumptions about principal kind and source Graph population; these were corrected without changing production behavior. Final synthetic verification passes **3/3** (0.06 seconds). The strengthened authoritative fixture test passes with two independent scans and exact current full/Memory relation, Memory node population and clock agreement (the preceding combined run took 68.23 seconds and still contained the subsequently corrected principal fixture). The independent first-publication fingerprint agrees. All six source hashes remain unchanged.

## Step 13 — Vector Generation historical fingerprint

`historical_vector_generation_fingerprint.rs` selects accepted generations through VectorVersion, preserving every generation identity, profile identity, Archive-vector identity, source Archive cut, local/global publication coordinates and transaction time. It materializes the referenced immutable closure: exact compatibility policy/probe fields and f32 bit patterns, ordered fragment mapping, packed schema/count/length and exact matrix bytes. Pruned matrices retain explicit unavailable payload and manifest commitment rather than an empty matrix. No physical locations or unreferenced immutable objects enter the projection.

Activation is per profile at the requested VectorVersion and agrees with `vector_generation_at`. Retired profiles retain historical activation; present live eligibility additionally depends on active profile, import boundary and current Archive coverage. Those live filters do not become an invented VectorVersion retirement clock. PHY has no Vector Generation owner in the current format.

Luna implemented the bounded synthetic/fixture tests, and a separate Luna review found no concrete code-level correctness defect. The first run passes **2/2** (32.22 seconds): synthetic activation at cuts 0/1/2, earlier-cut stability after publication, retirement and reopen, plus independent reads/public activation comparisons on all three REL fixtures. Fixture heads are **1**, **1**, and **2** for first-14-day, first-28-day, and Ellis respectively. Final synthetic checks pass **2/2** (0.08 seconds), including authentic retired-profile pruning, retained historical activation, public unavailable-matrix behavior, explicit nonzero prune commitment and deterministic reopen. That test uses the existing 512-dimension prune fixture contract: a flushed fragment population and a matrix large enough to satisfy the existing shrink guard. All six source hashes remain unchanged. Root formatting and the 92-fixture/166-API coverage and 11-fixture ownership/production guards pass; the earlier Lexicon refresh limitation remains explicitly recorded.

## Step 14 — Ego historical fingerprint

`historical_ego_fingerprint.rs` selects accepted records through EgoVersion. Exhaustive matching retains identity names/text/revisions, active identity changes, identity/anchor terminal records, exact anchor priorities, Personality/Synthesis text and source Memory watermarks. Legacy Identity normalizes to the same default named identity semantics rather than treating its wire carrier as semantic state. Ego has no stored global transaction coordinate or timestamp; none is fabricated in this projection.

Verification passes **4/4** (29.50 seconds): PHY identity revision/activation/legal deletion, anchor revisions/priorities/deletion, Personality/Synthesis, no-op version stability and historical getters; REL anchor/synthesis cuts; legacy carrier equivalence; and repeated independent reads/current getter comparisons on all six fixture copies. All authoritative Ego heads are **zero**; meaningful nonempty history coverage is supplied explicitly by the synthetic cases. All six source hashes remain unchanged. Luna wrote bounded tests and independently reviewed the adapter without finding a concrete correctness defect. An initial test-only moved-value compile error was corrected before this passing run.

## Step 15 — Freshness activity and exact-event fingerprints

`historical_freshness_fingerprint.rs` binds either an activity-turn query or the exact inclusive stable event ID plus MemoryGraph bound. Schema 1's numeric event slot stays absent: the exact string/graph cut is an explicit canonical row, not an invented ordinal. Score settlement and event selection reuse `FreshnessStore::records_at_cut` and `ordering::ordered_events` through a test-only owner seam. Numerical policy and current activation/explicit-marker state are clock-neutral metadata across cuts; no unsupported historical activation clock is invented.

Exhaustive serializers retain enrollment/admission, rewrite baselines and carry, accepted event identity/effects/provenance/Graph/Community/semantic evidence, accepted-use receipts and semantic import settlements. Accepted-use receipt identity is activity-bounded because it has no independent event subclock. Derived event postimages, physical prefixes, current object locations and physical compatibility bindings are excluded. Import settlements are reconstructed into the same semantic scope using a read-only helper extracted from existing normalization; normalization still persists and validates the predecessor proof as before. The oracle canonicalizes the representation without appending and excludes the carrier predecessor from semantic hashing.

Verification gates pass separately:

- Authoritative fixture gate **1/1** (137.29 seconds): repeated activity cuts 0/first/middle/head, full-map agreement with public historical reads, repeated exact head cuts and bounded public single-record API comparisons. A single-record read replays the complete owner, so large fixtures sample five deterministic IDs after comparing the full map; synthetic exact cuts compare every ID. An initial unbounded single-record fixture pass was cancelled and replaced with this bounded API-boundary check.
- Final exact-cut synthetic **1/1** (0.92 seconds): an authentically admitted target decays to -50 at turn 1,500, then two valid first-link principals produce 0 and 50 at same-turn cuts; stable IDs and Graph proof bounds select the right state. Distinct query points bind distinct fingerprints even when their records agree. Later-turn append, invalid cuts and reopen are covered. Fixture setup was corrected to meet existing publication-provenance/principal admission; production validation was unchanged.
- Strengthened existing normalization regression **1/1** (0.33 seconds): fingerprints/records agree before normalization, after normalization, after reopen and after reclamation; the oracle leaves source bytes unchanged, and the forged predecessor is still rejected. Luna review identified the initial carrier-dependent parity defect and confirmed the shared reconstruction preserves existing runtime normalization behavior.

| REL | Activity head | MemoryGraph head | Retained event/proof rows |
| --- | ---: | ---: | ---: |
| first 14 days | 5,857 | 7,913 | 36 |
| first 28 days | 16,465 | 25,337 | 30 |
| Ellis | 2,972 | 6,309 | 45 |

Normal `cargo check --locked` passes after the helper extraction. All six authoritative hashes remain unchanged. PHY has no Freshness activity owner. Exhaustive cut enumeration and persisted baseline artifacts remain Steps 19–21.

## Plan expansion — separate Dream state families

After Step 15, source/spec review confirmed that accepted pending/completed first-pass proof (Step 16) does not cover REL/PHY monotone Dream pair membership or effective Dream processing-lane state. Pair membership is explicitly semantic in the specification; lane state changes future eligibility. Neither has a local history clock. They must not become hidden work in the whole-owner gate.

Under the plan's explicit-owner-step rule, two bounded oracle steps were inserted as Steps 17 and 18. Former Steps 17–243 move to 19–245; verified Steps 1–15 and their commit identities remain unchanged. The plan is now **15/245**. Structural verification confirms every number 1–245 exactly once, with integration gates unnumbered. Historical global visibility for these clock-neutral states remains an explicit concern for Step 20 and may not be inferred from a timestamp or another owner's version.

## Dream and cut-enumeration batch — Steps 16–19

The revised execution rules group adjacent acceptance items into a coherent batch. The test-only Dream proof projection includes pending passes, settled markers, publication initialization intents/birth facts, accepted Graph proof cuts, and retained completed passes. A settled marker alone is not verified no-link evidence. Pair membership and processing-lane records remain separate owner domains. Both use an explicit clock-neutral current snapshot (`local_cut = 0`), with no borrowed Memory, Graph, activity, or global clock. Processing rows retain lane, satisfied epoch, cadence version and checkpoint time; legacy cooldown conversion is tested through the existing explicit migration seam.

Cut enumeration includes the empty prefix and every retained dense owner-local/global position. MemoryGraph positions are selected by one scan of qualifying full-Graph publications, including gaps; synthetic expected positions are full Graph `[0,1,2,3]` and MemoryGraph `[0,1,3]`. Freshness enumerates activity positions separately from exact stable event prefixes at their accepted proof Graph cuts, collapsing identical accepted event receipts and rejecting conflicting identities. This is not a Cartesian event-ID/Graph-position product or a claim of exhaustive fingerprint comparison.

Shared focused verification passes **17/17**, including existing matching historical Dream regressions and the new synthetic gates. Shared authoritative gates pass **4/4** (151.25 seconds): three REL proof histories, all six REL/PHY pair sets, all six processing states, and all six cut enumerations agree across repeated reads and reopen. All authoritative processing-lane sets are empty, so nonempty epoch/eligibility/legacy-conversion coverage is synthetic. Normal `cargo check --locked` and formatting pass. Source fixtures are read-only inputs copied with `io::copy`; no repeated source-hash sweep is claimed for this batch. No persisted format or runtime authority changes are part of this batch.

Development runs caught and corrected synthetic lifecycle eligibility and nanosecond epoch-boundary mistakes before acceptance; the final focused run is green. Logs: `.test-tmp/lossless-dream-and-cuts-synthetic-verified.log`, `.test-tmp/lossless-dream-and-cuts-fixtures.log`, and `.test-tmp/lossless-dream-check.log`. Broad cross-domain verification remains at Gate A.

## Whole-owner and baseline batch — Steps 20–21

The test seam composes REL and PHY owner-local projections at a global ordering point, recording independent local cuts and applying Memory/Entity/Observation lifecycle visibility to active Graph edges. Clocked records retain their individual global coordinates even inside a grouped completion. Clock-neutral Ego, Dream, processing and REL Freshness use the accepted fixed-source replay prefix selected by the existing container scanner's post-observation high-water rule. Its V1 byte boundary and disposable prefix file are mechanical scratch state and never enter a fingerprint. The seam is test-only and introduces no runtime global snapshot API or shared publication manifest.

A real publication writes its Dream recovery intent before allocating the next global ticket. Therefore a live-head fingerprint can change during later source growth, even when its typed cuts do not. Historical baselines are recorded only after fixing the source snapshot. Tests explicitly preserve this behavior rather than inventing a missing clock. A genuine two-record grouped completion verifies individual typed Memory visibility at an interior G; its clock-neutral physical frame remains atomic.

Validated rewrite lineage includes generation, full RewriteGenesis identity and Graph baseline owner/generation/policy. Baseline node admission is selected from the replay prefix. The authentic rewritten-file test recomputes the checksum of a forged baseline policy and confirms semantic mismatch rejection on open. Final focused integration gates pass **7/7** (0.43 seconds), including REL/PHY interleaving, terminal visibility, grouped-publication cuts, lineage validation and reopen parity.

The first authoritative baseline attempt stopped on the existing source error `import settlement cut does not retain an intra-settlement trajectory`. Accepted settlements retain a settled state, not every intermediate trajectory. Baseline serialization records this exact established Freshness unavailability with cut metadata; it does not create a score or swallow unexpected errors. Cut positions/counts and query availability are separate facts.

A second baseline attempt exposed a whole-owner prefix with missing Archive payload/reference closure. Read-only inspection of the first-14-day source at G=9,469 found all 5,857 turn records in the selected prefix, but 99 attachment occurrences without local bytes or their later repository-file bindings (95 unique bindings exist later in the file). The complete source opens successfully. The whole-owner oracle must reject this earlier unresolved view; it may not pull later proof backward. The adapter now distinguishes this typed `MissingArchiveReference` outcome from unexpected errors, and an explicit delayed-binding synthetic regression covers it. The baseline records the rejected cut with its availability reason.

Baseline generation passes **1/1** (278.72 seconds). The 103,216-byte [baseline artifact](../benchmarks/historical-owner-baseline-2026-10-19.json) covers all six authoritative owners and all **12,414** retained Memory revisions in ordered digests. Per-domain/global cut fingerprints are explicitly sampled, with complete cut counts; 12 sampled Freshness event cuts and two sampled REL global cuts are recorded unavailable with precise source reasons. All six source hashes still match the accepted baseline. A read-only Luna review found no material coverage, determinism, hash-check or broad-error-suppression defect. The broad Gate A library/integration run (`cargo test --lib --tests --locked -- --test-threads=2`) passes **1,714 tests across 25 binaries**, with five existing ignores and no failures. The final seven-test focused run additionally covers the subsequently added typed reference-unavailability case. Documentation tests pass (zero doctests). The initial unfiltered run was stopped while linking unrelated examples, then replaced by the explicit library/integration target run; no result is claimed for the stopped invocation. Normal library checking passes. The architecture refresh passes 92 coverage fixtures, 166 API classifications, 11 external ownership fixtures and production boundaries, but its Lexicon phase remained at `analyzing rust` without a new snapshot during a bounded attempt and was stopped. Lexicon/Arcana CURRENT remain `sha256:8b081bb6cadcab766ac48e9c3b645d55d150a4c47ea1b9e37977f517fc98477a`; the retained-snapshot Pitlord policy check passes all 23 rules with no violations, a second quiet Rust analysis remained active but produced no snapshot during a bounded 7.5-minute attempt and was stopped before advancing the source. No fresh structural snapshot is claimed. This is a structural-evidence freshness gap, not a functional Gate A failure.


## Container header boundary batch — Steps 22–23

The compact physical format is version **2.0**, preserving the existing 40-byte identity-header shape and stable owner UUID. Header mechanics share an explicit expected-version parser; legacy V1 open remains strict and the internal compact route selects exactly V2.0. Version mismatch returns `ContainerError::UnsupportedVersion` before identity or semantic payload dispatch. This stage leaves ordinary creation and owner lifecycle routing at V1 until the read-only and admission cutover is implemented; no compact semantic record writer is enabled.

Luna supplied focused tests and independently reviewed the implementation. The four new tests pass, covering both owner identities/UUIDs, cross-route rejection, unknown minor versions, and rejection before malformed identity/payload visitor execution. Final shared container verification passes **17/17** (the four new boundary tests and all thirteen existing container tests), including exact V2 header bytes, legacy round-trips, typed identities, transaction versions, and existing tail-repair/read-only-stream behavior. `cargo check --locked` and formatting pass. An initial broader substring filter also selected the exhaustive historical byte-prefix recovery test; it was stopped and replaced by exact module filters, with no complete result claimed for that stopped invocation. Logs: `.test-tmp/lossless-format-boundary-container-tests-final.log` and `.test-tmp/lossless-format-boundary-check.log`. Current ownership guards pass (92 coverage fixtures, 166 API classifications, 11 external ownership fixtures and production boundaries); retained-snapshot Pitlord passes all 23 rules. A fresh Rust analysis was stopped after a bounded attempt of approximately five minutes without a new snapshot. Lexicon/Arcana CURRENT remain `sha256:8b081bb6cadcab766ac48e9c3b645d55d150a4c47ea1b9e37977f517fc98477a`; fresh structural evidence remains an explicit gap. Logs: `.test-tmp/lossless-format-boundary-architecture.log` and `.test-tmp/lossless-format-boundary-policy.log`. Both documentation-policy modes pass.

The next owner-lifecycle work has two concrete open-time mutation hazards: conversation-compaction recovery frees/coalesces/truncates stale chunks, and Entity-resolution repair appends a new resolution revision when canonicalizing retired Entity references. Read-only inspection must retain accepted stored history without either durable repair. Maintenance's `create_empty_like` must also preserve V2 before compact reclamation is enabled; it currently selects the creator by header shape. These are explicit cutover constraints, not completed compatibility work.


## V1 read-only and V2 creation cutover — Steps 24–31

Open-path discovery split the compaction and Entity-resolution read-only projection responsibilities into explicit owner-specific acceptance items. The plan now has 247 contiguous steps. Both domains previously wrote during open, but require different evidence: compaction recovery frees/coalesces/truncates physical chunks, whereas retired Entity references append a new resolution revision. V1 inspection now shares the existing compaction stale-selection rule without reclaim and preserves stored Entity-resolution revisions/references while validating historical targets. V2 writable open retains both current recovery behaviors.

Container probes headers through a read-only handle, accepts exactly V1.0 or V2.0, and opens a writable handle only for V2 after revalidating its actual header. Strict V1 owner APIs pass their version requirement through the actual replay open. Incomplete V1 tails return `TruncatedChunk` unchanged, while V2 tail recovery remains enabled. V1 sync is a no-op. New ordinary creation uses V2; physical maintenance copies remain V2 and preserve identity/UUID.

The format-only mutation guard precedes the independent import admission fence and owner-write-scope exception. Concrete owner stores and public facade mutators preflight it before retry/no-op branches, validation, resident changes or output creation. Coverage includes Archive, Memory, Entity/Relationship, Graph, Ego, Freshness/Dream, processing/pairs, vector/profile/generation stores, Insomnia, import coordinators, pruning/reclamation/repacking and ordinary reconciliation. No semantic codec or clock layout changes in this batch.

The focused boundary/container run passes **26/26**, including exact Memory retry, an empty Graph batch, exact Ego update, metadata/profile no-ops, raw owner-scope bypass, output-before-admission refusal, duplicate compaction recovery and stored retired-reference preservation. Root test compilation and CLI tests pass (**32/32**). Completed root library/integration verification passes **1,728 tests across 25 binaries** (1,571 library tests, 157 integration tests; five existing ignores, no failures). The separate authoritative V1 gate passes **1/1** against all six sources, with accepted SHA-256 and modification times unchanged. Root/CLI checks, documentation tests and both formatting checks pass. Current ownership guards pass (92 coverage fixtures, 166 API classifications, 11 external ownership fixtures and production boundaries); retained-snapshot policy passes all 23 rules. The architecture refresh reached `analyzing rust` without producing a new snapshot during the prior bounded run; the structural-evidence freshness gap remains explicit. Logs: `.test-tmp/lossless-v1-cutover-all-tests.log`, `.test-tmp/lossless-v1-cutover-authoritative.log`, `.test-tmp/lossless-v1-cutover-architecture.log` and `.test-tmp/lossless-v1-cutover-policy.log`. Source fixture verification uses read-only handles directly and compares each accepted SHA-256 before/after plus modification time.

## Lossless upgrade shell

Steps 32-41 introduce private request, frozen source plan, staging evidence, typed failures and abort cleanup. Planning refuses existing destination entries and requires an exactly V1.0 complete physical stream with retained owner identity/UUID. The source is always read-only. Identityless legacy headers reject rather than inventing an identity. This validates physical framing; full semantic migration is still pending.

The shell exclusively owns a temporary sibling directory, creates a V2 REL/PHY carrier with the retained UUID, and delegates bounded frame transport to Container. It retains all frame bytes/order and embedded global/local versions and timestamps without semantic re-publication. The destination digest, with only its version header normalized for comparison, must equal the frozen source digest and length. Compact physical reopen verifies identity and global head. Same-size source drift is checked against both the retained source handle and current path after staging; existing and late-created outputs are refused. The guard removes owned partial output after handles close, including error/abort paths. There is no public execution or promotion API until owner migration and equivalence acceptance are integrated.

Final shell tests pass **8/8** (0.24 seconds): minimal REL/PHY identity/reopen, exact global tickets and missing/repeated/nonmonotone timestamps, every-cut interleaved Memory/Entity historical fingerprints, clock-neutral frontier/chunk ordering, existing outputs and source aliases, truncated-input preservation, before/during-copy drift, injected abort and late output collision. Initial development runs caught misuse of the deliberately V1-blocked maintenance-copy helper; staging now explicitly creates the V2 destination. A hard-link fixture was replaced with a lexical source alias because Windows sandbox hard-link creation is unavailable; no hard-link test result is claimed.

Gate B passes **34/34**: shell 8, facade boundary 9, header boundary 4, existing container 13. A first header filter selected zero tests and was corrected to the actual module path; only the four-test rerun is counted. Normal `cargo check --locked` passes. Logs: `.test-tmp/lossless-upgrade-shell-tests-verified.log`, `.test-tmp/lossless-upgrade-gate-b-boundary.log`, `.test-tmp/lossless-upgrade-gate-b-container-final.log`, `.test-tmp/lossless-upgrade-gate-b-existing-container.log`, `.test-tmp/lossless-upgrade-shell-check-final.log`. Authoritative fixtures were not opened or transformed by this shell batch; six-source preservation belongs to the accepted cutover batch.

Architecture refresh passes current guards (92 coverage fixtures, 166 API classifications, 11 external ownership fixtures and production boundaries), then remains at `analyzing rust` during a bounded attempt and is stopped. No fresh snapshot is claimed. Retained-snapshot checking passes all 23 policy rules with no violations. Logs: `.test-tmp/lossless-upgrade-shell-architecture.log` and `.test-tmp/lossless-upgrade-shell-policy.log`. The structural-evidence freshness limitation remains explicit.

## Memory delta contract

Steps 42-47 are accepted in `4652466`. The specification field table covers all 21 draft, 28 hydrated and 26 durable-record fields, checked against source declarations. Only semantic ID/body and hydrated title/content are immutable across revisions; in particular V1 does not enforce a fixed `created_at_ns`, so later deltas retain changes to it. Routing is independently body-bound immutable metadata, not a revision delta.

The private owner-local contract distinguishes unchanged from replacement, including explicit optional clear. Creation retains the exact first accepted record and required nonzero coordinates. Delta binds stable identity/body, predecessor/successor revision, exact new mutation and advancing publication coordinates; a legitimate metadata-empty revision remains representable. Twenty typed changes have a fixed mask/order, and an exhaustive record pattern forces future field additions into an explicit contract decision. Structured inference uses whole-value replacement retaining resolution order; no new set semantics or generic cross-owner framework is introduced. Body/provenance/codec validation remains owned by the corresponding subsequent implementation steps.

Final contract verification passes **7/7** (0.03 seconds), including actual Memory temporal-proof order replacement/clear, the complete optional-transition table, identity/body/revision/clock failures, empty metadata revisions, canonical masks/reserved-bit rejection, and creation/later-record compatibility through V1 read-only reopen with exact source-byte preservation. Initial six-test development verification also passed before adding the real V1 compatibility case and strengthening the ordered-proof fixture. Normal `cargo check --locked`, formatting and retained-snapshot policy (23 rules) pass; current ownership guards remain green. Logs: `.test-tmp/lossless-memory-contract-tests-final.log`, `.test-tmp/lossless-memory-contract-check.log`, `.test-tmp/lossless-memory-contract-policy.log`. No compact wire bytes, replay cutover or ordinary writer changes are claimed; Step 48 is next.

Documentation impact:
- Inspected: architecture, invariants, storage format, maintainer map, limitations, roadmap, version-history plan, ADR 0002, refactor specification and shared engineering standards.
- Updated: architecture, storage format, maintainer map, limitations, specification, linear plan and execution evidence.
- Not affected: public API/CLI reference and existing semantic writer behavior; all new contracts and shell execution are private.
- Compliance check: structural and change-impact documentation-policy checks; UTF-8 and contiguous plan-number validation.
- Known documentation gaps: none identified in this batch; fresh structural snapshot remains unavailable as documented.

Architecture impact:
- Standards added or changed: none.
- Ownership or boundary impact: private upgrade coordinator owns source binding/staging cleanup; Container owns bounded frame transport; Memory owns its compact contract.
- Pitlord enforcement impact: existing 23 rules pass against retained structural evidence; current ownership guards pass; bounded refresh did not produce a fresh snapshot.
- Other verification impact: Gate B 34/34 and Memory contract 7/7; prior cutover root suite 1,728/1,728 and CLI 32/32 remain separately recorded, not claimed as rerun after contract additions.
- Known architectural gaps: complete semantic migration, public upgrade promotion and compact codecs remain planned work; fresh structural evidence is still unavailable.

## Memory codec and replay

Steps 48-55 are accepted in `d5b8a79`. The compact owner codec implements independent `CVAMC001` creation and `CVAMD001` delta carriers with exact ID, mutation and version coordinates. Creation implies revision one and carries the immutable body reference plus full initial metadata; delta carries the predecessor/successor envelope, ordered field mask and changed values only. Legacy Memory value helpers were extracted unchanged into `memory_codec_values.rs`; there is no legacy record nested in a new compact envelope and no dual write. Body/routing/grouped/external carriers retain their existing contracts until their explicit cutover steps.

The pure applier constructs a separate candidate and derives its canonical difference against the prior record, rejecting invalid envelopes and redundant equal-value operations. MemoryOpenState queues both carrier forms by object reference and admits them only through the existing CVAMEMV1 publication marker. Embedded global/local coordinates must equal the marker. Unversioned deltas/creations remain inert; known compact family prefixes with unknown versions reject. Prior state and local clocks survive failed delta validation; temporal proof body/time bindings are checked before compact insertion. Existing terminal tombstone replay remains authoritative. V1 owners containing compact Memory frames reject without mutation.

The observed-global-head rule remains the existing nonzero/at-or-before rule, not a newly imposed physical-frontier equality. Review raised the late-ticket case; source inspection confirmed that existing V1 clocked visibility uses retained semantic coordinates. A strengthened regression creates an actual V1 stream whose later physical Memory binds to an earlier allocated ticket and compares its exact records/coordinates with compact V2 replay, while verifying V1 source bytes unchanged. Rejecting that accepted V1 behavior would have narrowed lossless compatibility, so no stricter admission rule was introduced.

Final focused verification passes **31/31** (0.07 seconds): contract 7, codec 9, pure application 5, replay 7 and retry/conflict 3. It covers independent hand-authored goldens, every truncated prefix, trailing bytes, invalid tags/UTF-8/lengths/masks/envelopes, all typed changes and optional clears, exact history/tombstones, inert interrupted carriers, marker mismatch, invalid proof binding, V1 preservation/late visibility and exact retry/conflict coordinates. Earlier development compilation caught two test-only Rust borrow errors and a golden-offset mistake; they were corrected before acceptance. Logs: `.test-tmp/lossless-memory-codec-replay-tests-accepted.log` (final), `.test-tmp/lossless-memory-codec-apply-tests.log` (initial focused gate).

Existing Memory codec, ordinary publication, lifecycle, temporal inference, source-time and routing regressions pass **29/29** across six module filters. CLI verification passes **32/32**, and normal root/CLI checks and formatting pass. The initial CLI run passed 30 and failed two configuration tests because their default host temporary location denied writes; rerunning all 32 with TEMP/TMP set to repository-local `.test-tmp` passes without production changes. Logs: `.test-tmp/lossless-memory-existing-regressions.log`, `.test-tmp/lossless-memory-codec-cli-tests-verified.log`, `.test-tmp/lossless-memory-codec-cli-check.log`, `.test-tmp/lossless-memory-replay-check.log`. The full root suite is not claimed as rerun after these codec additions; broad integration remains Gate C after writer/migration cutover.

Built-in Luna subagents supplied independent golden/adversarial fixtures, pure application and retry fixtures, and bounded source review. The primary agent implemented codecs/replay, reviewed and corrected the contributions, and ran acceptance tests. No Codex/Hermes connector execution was used. Retained-snapshot Pitlord passes all 23 rules and current ownership guards pass; the fresh structural-snapshot limitation remains the same documented gap. Log: `.test-tmp/lossless-memory-codec-policy.log`.

Documentation impact:
- Inspected: existing storage, invariants, architecture, ownership map, limitations, roadmap, history and refactor contracts plus shared standards.
- Updated: architecture, storage format, maintainer map, limitations, specification, plan and execution evidence.
- Not affected: public delta API/CLI commands (none added), ordinary standalone/grouped/external writer encodings and authoritative fixture bytes.
- Compliance check: root/CLI formatting, structural/change-impact documentation policy, plan numbering and source/contract coverage checks.
- Known documentation gaps: none identified for accepted codec/replay behavior.

Architecture impact:
- Standards added or changed: none.
- Ownership or boundary impact: Memory owns codec/application/replay; Container continues to own physical mechanics; no new store or clock.
- Pitlord enforcement impact: current guards and 23 retained-snapshot rules pass; no fresh structural graph is claimed.
- Other verification impact: 31 focused, 29 existing Memory and 32 CLI tests pass; broad Gate C is still pending.
- Known architectural gaps: ordinary compact writers, grouped/external cutover, lossless owner migration, full upgrader promotion and measurement remain future steps; fresh structural evidence remains unavailable.

## Fresh baseline observations

The completed root baseline reports **1,515 passed / 5 failed / 5 ignored**, in 419.85 seconds of test execution. Both exhaustive import byte-prefix recovery tests and the long finite-receipt/reopen test pass. Failures were three external-publication tests, one Lore file-lock test and one decision-endpoint test-server socket race. All seven `runtime_host::gateway::external_tests` pass in isolation with `TEMP=TMP=<repository>/.test-tmp`. The initial longer `.test-tmp/lossless-baseline` scratch path exceeds the Win32 rename path limit in frozen external-plan installation; the shorter path retains all scratch inside the repository. The Lore test passes in isolation, matching the previously recorded transient Windows file-lock behavior. The exact decision-endpoint failure passes through Workspace command execution (1/1); a sandboxed direct loopback retry cannot reach its local server, so that restricted retry is not authoritative network-test evidence. No production fix or test weakening was needed. This is a qualified baseline, not an all-green full-suite claim; integration binaries were not reached after the library failure.

Fresh root and CLI `cargo check --locked` pass. CLI tests pass **32/32**. Root/CLI formatting and both documentation-policy modes pass. Subsequent documentation runs must avoid racing scratch-directory removal by active byte-prefix tests; one concurrent structural scan encountered a disappearing scratch directory and was rerun after those tests finished.

## Shared delta mechanics Steps 88–90

Accepted against `bafdaae`, reconciled after the `14bcba3` handoff. Direct diff confirms no runtime/CLI/ownership-policy change between those checkpoints. `src/compact_delta.rs` extracts only typed unchanged/set diff, explicit optional clear, known-mask checking and pure candidate-field application. Memory retains every semantic/wire rule and its validate-envelope/build-candidate/canonical-validate/admit sequence. Independent Luna review found no behavior or wire change.

Verification: compact Memory **38/38** (`.test-tmp/lossless-shared-delta-memory-tests.log`); root format/check (`.test-tmp/lossless-shared-delta-root-check.log`); CLI format/check and **32/32** (`.test-tmp/lossless-shared-delta-cli-check.log`, `.test-tmp/lossless-shared-delta-cli-tests.log`); both documentation modes (`.test-tmp/lossless-shared-delta-docs.log`). Current-source ownership assigns **987 Rust files to 20 areas with zero missing/overlapping owners**. Fresh enforcement for `bafdaae` completed successfully (`.test-tmp/lossless-shared-delta-architecture-refresh.log`): all 23 rules, 20 areas, 91,596 source nodes and 3,079 relationships; snapshot `dc86275347d852f5e310ecc559673cfa34b33e5c381b94e07cfd6fac10526864`, no compatibility warnings.

Documentation impact: architecture/storage-format/maintainer-map define the value-only shared seam; plan/amendment reflect accepted gates. No public API or persisted-format change. Architecture impact: one pure `compact-codec` area owns the helper; semantic owner admission and storage authority remain unchanged. The helper architecture freshness gap is closed. A subsequently reproduced maintenance-proof source case adds fail-closed Step 91; Entity contract preparation is now Step 92.

## Related docs

- [Specification](minimal-lossless-history-storage-spec-2026-10-19.md)
- [Linear plan](minimal-lossless-history-storage-linear-plan-2026-10-19.md)
- [Storage format](storage-format.md)

## Notes

Authoritative fixtures are read through read-only file handles. Any owner API that can repair on open must operate on a disposable repository-local copy.

Batch compliance: formatting, normal library check, documentation structural/change-impact checks, 23 retained-snapshot architecture policy rules and current ownership guards pass. No production semantic owner, public API, ordinary writer or persisted format changed in Steps 16–21.


## Standalone Memory writer and reader cutover

Steps 56-62 accepted against implementation `a7a485d`. Discovery inserted four explicit reader responsibilities (59-62), increasing the plan from 247 to 251 steps.

- Compact standalone tests: **36/36**, `.test-tmp/lossless-memory-standalone-tests-final.log`.
- Import evidence/admission tests: **3/3**, `.test-tmp/lossless-memory-import-readers-tests-final.log`.
- Existing boundary, upgrade, Memory, reconciliation, reclamation and import regressions: **61/61**, `.test-tmp/lossless-memory-standalone-regressions.log`.
- Root and CLI checks passed; CLI **32/32**; formatting passed.
- Current ownership guards and all 23 Pitlord rules passed using the retained architecture snapshot (`.test-tmp/lossless-memory-standalone-policy.log`). The previously recorded refresh stall remains an evidence-freshness gap; this is not a fresh graph claim.

Documentation impact: inspected storage-format, current-limitations and maintainer-map; updated standalone writer/readers and remaining grouped/external/migration limits. No end-to-end migration or measured-savings claim is made. Broad Memory integration remains Gate C.
Architecture impact: Memory remains semantic publication authority; reconciliation reads validated publications, import evidence retains its established normalized representation, and physical reclamation recognizes compact backing carriers. No standards or policy changes.


## Memory grouped publication and migration — integration in progress

Implemented after the accepted Step 62 checkpoint: canonical V7 Insomnia compact creations, V2 external compact creations, isolated legacy reads, private standalone/grouped/external Memory conversion and exact terminal transport. Focused atomic publication run passed **10/10**; migration-filter run passed **14/14**. CLI checks and **32/32** CLI tests passed. These are focused development results, not acceptance of Gate C.

The first authoritative run exposed legacy import receipts binding the original physical prefix. Explicit Steps 72-77 were inserted for contract/encoding/decoding/replay/migration/adversarial coverage; review then inserted Steps 78-80 for physical-maintenance rebinding and proof-equivalence checks. The plan now contains 260 contiguous execution rows. No unfinished proof work is hidden in the Memory gate.

The CVAILEG2 transport retains exact original CVAILEG1 bytes and separately validates the transformed prefix, original owner and unique settlement identity/digest. Reclaim/repack/prune compose the import-owned rebind helper; semantic evidence comparison unwraps the original proof. This remains private disposable staging with no public promotion API.

A sandboxed library sweep reached broad owner tests but local loopback transport tests failed to connect, and one newly added proof fixture lacked native birth evidence. A representative loopback test passed unchanged outside the sandbox. The sandboxed sweep was interrupted; it is not a passing gate. Luna is correcting/verifying the proof fixture and maintenance regressions. Exhaustive authoritative migration verification is being moved to an optimized build because materializing every cut is expensive in debug mode. No provisional byte measurement is accepted as a savings result.

Documentation impact: architecture, storage-format, maintainer-map, current-limitations and this plan/evidence are updated. Structural and change-impact checks are rerun for the batch. Architecture impact: explicit storage-upgrade orchestration ownership added to Pitlord; a direct current-source audit found every authored Rust file assigned exactly once. Current guards and retained-snapshot 23-rule policy passed earlier. A bounded architecture refresh exceeded 55 seconds while analyzing Rust; its child subsequently completed Lexicon init, but no completed Arcana refresh/enforcement is claimed. Fresh graph evidence remains outstanding.

Verification environment follow-up: concurrent release/dependency compilation and integration linking exhausted Windows paging allocation (E0786/mmap OS error 1455; Lore LLVM allocation failure). Both jobs ended without a passing gate. Builds are now serialized with one compiler job; this is a resource failure, not an accepted semantic result. Latest CLI remains 32/32. The optimized authoritative run has not yet completed.

A runtime V1 REL/PHY regression confirmed that successor Memory revisions can bind earlier unique global tickets while owner-local versions advance (`v1_rel_and_phy_reopen_accept_successor_with_earlier_global_ticket`, 1/1 passed). The compact contract's stronger global-order requirement was therefore corrected to nonzero retained global identity; marker/global-claim admission remains owner-controlled. Step 70 now explicitly includes these late-ticket coordinates. Migration/replay parity regression is being extended before reacceptance.

Continuation verification: all **38/38** focused compact Memory tests pass, including V1 late-ticket migration for both REL and PHY (`.test-tmp/lossless-memory-compact-serial.log`). All **6/6** legacy-epoch tests pass, covering CVAFRS03/04/05 source normalization, exact transported proof, tampering, source preservation and reclaim/repack/prune (`.test-tmp/lossless-proof-and-versions-serial.log`). The V4 fixture now derives its physical cut after constructing the genuine legacy source. Production admission was not weakened to accommodate an empty invalid V4 settlement.

Six explicit source-normalization responsibilities were inserted before maintenance rebinding; the plan contains **266** unique contiguous rows. At that checkpoint, exactly **62** records were accepted. Both documentation-policy modes and retained-snapshot architecture checks passed. Authoritative parity, final raw-byte measurement and fresh architecture evidence were still outstanding at that point.

The serialized full library/integration run completed successfully: **1,800 passed, 0 failed, 5 existing ignores across 25 test binaries** (`.test-tmp/lossless-memory-gate-c-serial.log`). This includes both exhaustive import recovery tests. The six-file authoritative parity/measurement run uses a serialized optimized build with Lore optimization disabled and Reliquary at optimization level 1 to avoid the earlier compiler allocation failure. The complete six-file measurement remains pending.

## Steps 63–86 — grouped publication, migration, and import-proof transport

Accepted against commit `9996c28`. The ledger now records every execution step from **1 through 86**; Step 87 and Integration Gate C remain pending.

- Grouped Insomnia and external Memory publication use compact creation carriers within their existing atomic publication envelopes. Focused publication tests pass **10/10** (`.test-tmp/lossless-atomic-memory-focused-accepted.log`); existing Insomnia regressions pass **83/83** (`.test-tmp/lossless-grouped-memory-regressions.log`).
- Standalone, grouped, external and terminal Memory migration preserve accepted IDs, bodies, revisions, transaction coordinates and owner references. The migration filter passes **14/14** (`.test-tmp/lossless-memory-migration-all-final.log`). Adversarial and replay coverage passes **38/38** (`.test-tmp/lossless-memory-compact-serial.log`), including legal late-ticket visibility for V1 REL and PHY.
- CVAILEG2 transport retains exact original CVAILEG1 evidence and separately binds the transformed prefix. CVAFRS03/04/05 normalization, tamper rejection, source preservation, and migrated-owner reclaim/repack/prune pass **6/6** (`.test-tmp/lossless-proof-and-versions-serial.log`). The V4 fixture derives its physical cut from a genuine legacy source. Production validation remains unchanged.
- The authoritative `chatgpt-first14d/project.prj.rel` and `chatgpt-first14d/user.phy` migrations report zero semantic mismatches in `.test-tmp/lossless-memory-authoritative-optimized-serial.log`; the REL case also reopens with the exact imported proof retained and the transformed-prefix binding valid. Step 76 is accepted on this authoritative imported REL case. For REL, total file bytes rise from **31,853,182** to **33,291,095** (**+1,437,913**) because the canonical semantic proof is retained alongside the original receipt. This is not whole-file savings evidence. The PHY total file falls from **814,287** to **777,735**; no compression is included.
- The full root suite passes **1,800 tests, 0 failures, and 5 existing ignores across 25 binaries** (`.test-tmp/lossless-memory-gate-c-serial.log`). CLI verification remains **32/32** from the accepted prior run. This supports the batch but does not close Integration Gate C.
- Retained-snapshot architecture policy passes all **23 rules**. Fresh architecture evidence remains a gap; no current structural snapshot refresh is claimed. Both documentation-policy modes passed for the batch.

Step 87 remains unaccepted: the completed six-file parity and raw-byte measurement, with the required six-file report, is still pending. The first authoritative REL/PHY outputs above establish only the cases named; they are not generalized into a whole-file savings claim.

## Step 87 and Integration Gate C

The complete authoritative run subsequently passed **1/1** in **2288.21 seconds**, with all six sources reporting **zero semantic mismatches** (`.test-tmp/lossless-memory-authoritative-optimized-serial.log`). Every Memory local cut and retained revision/body/routing fingerprint, terminal descriptor and global transaction time matches; REL import evidence and canonical Freshness cuts match. Source bytes, SHA-256, size and modification times remain unchanged.

[Raw measurement artifact](../benchmarks/lossless-memory-migration-step87-2026-10-19.json), commit `c69f279`, records each raw payload family, signed whole-file delta and accepted source hash. Memory carriers shrink by **1,146,938 bytes** across six files. The three REL files grow because the temporary bridge retains old import proof and adds canonical semantic successors; the three PHY files shrink. Combined whole-file change is **+6,269,151 bytes**. No compression is involved and no whole-file savings claim is made.

The intervening policy amendment was completed first: I1–I12 at `fd61a1e`, with [inventory/caller/acceptance evidence](development-format-interruption-execution-2026-10-19.md). That work changed no runtime code or corpus bytes. This Memory-only bridge is not final canonical V2 corpus promotion; R1–R22 remain mandatory.

Gate C is accepted against `9996c28`: full root **1,800 passed, zero failures, five existing ignores across 25 binaries**; compact Memory **38/38**; legacy proof/versions/maintenance **6/6**; final CLI format/check and **32/32 tests** (`.test-tmp/lossless-memory-cli-final-check.log`, `.test-tmp/lossless-memory-cli-final-tests.log`); complete authoritative run above. Fresh architecture enforcement passes all **23 rules**, **91,587 source nodes**, **3,079 relationships**, snapshot `83e12942f18deb03747af6995bab271892f420c07e31629418eb1da560c95706`. The previous architecture freshness gap is closed.

Documentation impact: inspected and updated plan, execution, architecture, development, storage format, policy amendment and ADR 0041; both documentation modes pass for the interruption. No API/manual runtime behavior changed. Architecture impact: no new source ownership or runtime coupling in the interruption/measurement records; temporary migration seams remain tagged for mandatory post-promotion removal. Next original step is 88.

Fresh architecture evidence subsequently completed successfully with `python scripts/check_architecture.py --refresh` (`.test-tmp/lossless-memory-architecture-refresh-final.log`). Lexicon snapshot `sha256:83e12942f18deb03747af6995bab271892f420c07e31629418eb1da560c95706` was synchronized into Arcana without compatibility warnings. All **23** Pitlord rules pass against **91,587 source nodes and 3,079 relationships**, along with the ownership guards. This closes the earlier architecture freshness gap for implementation `9996c28`; the authoritative measurement gate subsequently passed as recorded above.

## Maintenance-proof guard and Entity contract — Steps 91–92

The temporary converter previously returned a staged file whose owner reopen rejected a copied `CVAHPR01` proof after Memory carrier conversion. It now rejects that unsupported authority with a typed error before returning a result. Genuine prepruned V1 input remains readable and unchanged; no final destination or staging debris remains. Upgrade tests **13/13**, legacy proof regressions **6/6** pass. A targeted authoritative run stages all six supported inputs, proves absence of this proof family, reproduces every accepted payload-family/file-byte count and preserves source hashes/bytes/size/mtime (**1/1**, 91.69 seconds; `.test-tmp/lossless-unsupported-proof-authoritative-staging.log`). This does not claim support for translating existing maintenance proofs.

Entity contracts cover all twelve record fields, the six mutable field bits, optional UUID set/replace/clear, exact predecessor revisions, unchanged-metadata fresh mutations, legal late global tickets and backward update times within the creation bound. Explicit pruning evidence preserves original-carrier and manifest digests; pure construction grants no file-level authorization. Terminals remain separate owner operations. Actual V1 REL/PHY characterization **2/2**, contract tests **11/11**, broad Entity filter **187/187** (`.test-tmp/lossless-entity-contract-regressions-final.log`) and root check pass. Earlier Windows loopback restrictions and a nonblocking fixture read were classified; the unrestricted whole filter passed unchanged.

Documentation impact: inspected architecture, storage format, current limitations, maintainer map and policy amendment; updated specification, plan/crosswalks, limitations and execution evidence. Both documentation-policy modes pass; no public API or CLI behavior changes. Architecture impact: new code remains in existing Entity and storage-upgrade ownership; no new semantic authority or generic framework. The fresh shared-helper snapshot covers `bafdaae`; a later cohesive Entity integration refresh remains required before its architecture gate. The updated 271-step plan incorporates former R1–R22 requirements in its final promotion and retirement gates; those gates remain pending.
