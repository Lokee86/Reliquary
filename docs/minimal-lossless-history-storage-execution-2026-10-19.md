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



Implementation commit: `3cc6631`; representation/proof-digest clarification and acceptance ledger: `ace0a33`. Accepted range is 1–92 in the consolidated 271-step plan.



The temporary converter previously returned a staged file whose owner reopen rejected a copied `CVAHPR01` proof after Memory carrier conversion. It now rejects that unsupported authority with a typed error before returning a result. Genuine prepruned V1 input remains readable and unchanged; no final destination or staging debris remains. Upgrade tests **13/13**, legacy proof regressions **6/6** pass. A targeted authoritative run stages all six supported inputs, proves absence of this proof family, reproduces every accepted payload-family/file-byte count and preserves source hashes/bytes/size/mtime (**1/1**, 91.69 seconds; `.test-tmp/lossless-unsupported-proof-authoritative-staging.log`). This does not claim support for translating existing maintenance proofs.



Entity contracts cover all twelve record fields, the six mutable field bits, optional UUID set/replace/clear, exact predecessor revisions, unchanged-metadata fresh mutations, legal late global tickets and backward update times within the creation bound. Explicit pruning evidence preserves original-carrier and manifest digests; pure construction grants no file-level authorization. Terminals remain separate owner operations. Actual V1 REL/PHY characterization **2/2**, contract tests **11/11**, broad Entity filter **187/187** (`.test-tmp/lossless-entity-contract-regressions-final.log`) and root check pass. Earlier Windows loopback restrictions and a nonblocking fixture read were classified; the unrestricted whole filter passed unchanged.



Documentation impact: inspected architecture, storage format, current limitations, maintainer map and policy amendment; updated specification, plan/crosswalks, limitations and execution evidence. Both documentation-policy modes pass; no public API or CLI behavior changes. Architecture impact: new code remains in existing Entity and storage-upgrade ownership; no new semantic authority or generic framework. The fresh shared-helper snapshot covers `bafdaae`; a later cohesive Entity integration refresh remains required before its architecture gate. The updated 271-step plan incorporates former R1–R22 requirements in its final promotion and retirement gates; those gates remain pending.



## Entity codec and concrete consumers — Steps 93–100 in verification



Implementation commit: `ace0a33`. Entity creation/delta codecs, candidate application/rebuild, semantic reconciliation, normalized import evidence, strict interval classification, shared REL/PHY carrier reclamation and canonical retained-structure encoding are implemented. Ordinary draft writers and temporary Entity conversion remain later gates. The standalone codec uses `CVAENC01`/`CVAEND01`; its six-byte family prefixes avoid the existing `CVAECHO1` namespace. A broad regression exposed and corrected the initial unreleased prefix collision before acceptance.



Focused broad Entity filter passes **220/220** (`.test-tmp/lossless-entity-codec-replay-import-regressions.log`), including all 64 changed-field masks, independent golden frames, checksummed corruption, optional UUID set/replace/clear, interleaved IDs, late global tickets, direct/merge terminals, exact terminal retry distinctions, unavailable-versus-authored placeholder state, both owner reclamation paths, canonical descriptor identity, and the existing legacy migration/Echo regression. Root format and compilation checks pass (`.test-tmp/lossless-entity-format.log`, `.test-tmp/lossless-entity-check.log`); both documentation modes pass (`.test-tmp/lossless-entity-batch-docs-structural.log`, `.test-tmp/lossless-entity-batch-docs-impact.log`). The full root test build remains running; no completion is claimed. An additional compatibility regression for retrying an older retained mutation while the Entity is active is included in the next focused run; exact optional UUID still participates in conflict checks.



Steps 101–104 proof/pruning integration is under verification separately: explicit proof extraction, original-carrier digest substitution, exact manifest coverage and complete redacted-chain rewriting have native REL/PHY and rechecksummed tamper fixtures prepared. No acceptance or measurement result is claimed for that integration yet. Existing pruning policy remains unchanged, including REL's required net file-size reduction. Native fixtures retain enough selected text to cover proof overhead; no whole-file savings claim is inferred merely from plaintext destruction.



Verification scheduling correction: the premature full-root sweep was stopped during unrelated example linking to release the serialized build queue for the cohesive Entity tests. `.test-tmp/lossless-entity-root-tests.log` is an interrupted build, **not a passing test gate**. Root format/check and the completed 220-test focused run above remain valid. The full root sweep is deferred to the Entity integration boundary, consistent with the plan's expensive-check batching rule.



The first six-file Entity authoritative run passed **1/1**, zero mismatches, **184.11 seconds**, with exact per-revision records, actual historical Entity fingerprints at every local cut, terminal descriptors, transaction times, identities and unchanged source bytes/hash/size/mtime. Its provisional ordinary-frame checksums produced **15,663 bytes of net Entity-carrier growth**; [pre-refinement diagnostic](../benchmarks/lossless-entity-pre-refinement-2026-10-19.json) preserves all six measurements. This is not Step 107 acceptance or corpus promotion. Whole-file results include Memory conversion and retained import-proof transport.



Measurement-backed wire refinement removes the unnecessary 32-byte trailer from ordinary Entity frames, matching the existing Memory/legacy integrity boundary. Explicit description-proof frames retain their checksum and all original-carrier, manifest and source-history validation. The canonical unreleased tags hard-cut to exact ordinary EOF without a fallback for the provisional trailer. Steps 93–95 framing/application checks and the six-file authoritative matrix are rerun before accepting the refined wire. The follow-up focused build initially hit Windows LNK1104 because the preceding authoritative executable was still running; executions are now serialized as well as compilations. No semantic failure is inferred from that file lock.



Documentation impact: storage-format documents implemented codecs, semantic consumer seams and pending proof integration; plan/spec retain 92 accepted steps until remaining checks pass. Architecture impact: all new modules stay within existing Entity, import, facade and storage-upgrade owners; the direct audit at 999 Rust files found zero missing/overlapping owners. Fresh Entity integration architecture enforcement remains a gate, not inherited from the earlier Memory helper snapshot.



## Entity writer, proof, migration and measurement — Steps 96–107



The final Entity wire has exact EOF for ordinary creation/delta frames and a SHA-256 trailer only for explicit description-proof frames. The final focused regression passes **231/231**, zero failures (`.test-tmp/lossless-entity-final-focused.log`), revalidating Steps 93–95 and covering semantic reconciliation/import evidence, strict interval admission, reclamation, exact older mutation retry, both terminal forms, ordinary writer cutover and native REL/PHY pruning. Forged, missing, duplicate and detached proof carriers reject; failed open preserves the original file before recovery/repair writes. Independent Luna review found no concrete defect in these boundaries.



The final authoritative conversion test passes **1/1** over all six files, **189.50 seconds**, zero mismatches (`.test-tmp/lossless-entity-authoritative-final.log`). It compares exact retained records and actual owner historical fingerprints at every Entity-local cut, terminal maps/descriptors, owner/file identities, heads and every global transaction time. Source SHA-256, bytes, size and mtime remain unchanged. Staging is private and removed on drop; this does not promote the canonical corpus. Existing source maintenance-history proof remains a typed unsupported conversion authority.



[Step 107 measurement](../benchmarks/lossless-entity-migration-step107-2026-10-19.json) records **322,375 → 301,910 raw Entity carrier bytes**, a **20,465-byte reduction**, without compression. The six combined staged files grow by **6,248,686 bytes** relative to the source because whole-file measurements include Memory conversion and retained import-proof transport. This is not an Entity-only file-size claim; the pre-refinement diagnostic remains separately recorded.



Root format/check pass (`.test-tmp/lossless-entity-final-format.log`, `.test-tmp/lossless-entity-final-check.log`). Both documentation modes pass (`.test-tmp/lossless-entity-final-docs-structural.log`, `.test-tmp/lossless-entity-final-docs-impact.log`). Direct source ownership audit finds **1,016 Rust files in 20 areas**, zero missing or overlapping owners. Full root/whole-owner and refreshed graph architecture checks remain required at Integration Gate D; the interrupted earlier root build and old shared-helper graph snapshot are not current Entity acceptance evidence.



Documentation impact:

- Inspected: architecture, invariants, maintainer map, roadmap, history plan, source specification and format policy.

- Updated: storage format, architecture ordering, current limitations, execution evidence and byte artifacts; acceptance ledger follows the implementation commit.

- Not affected: public API signatures and user-facing CLI commands.

- Compliance check: both documentation modes pass; final ledger changes receive another structural check.

- Known documentation gaps: full canonical promotion and runtime retirement remain pending in the integrated final gates.



Architecture impact:

- Standards added or changed: none.

- Ownership or boundary impact: concrete Entity codecs, publication/replay, proof admission and private upgrade seams remain under existing owners; no generalized semantic authority.

- Pitlord enforcement impact: direct path ownership passes; fresh graph enforcement is scheduled with the cohesive postimage-owner Integration Gate D.

- Other verification impact: CLI validation follows the changed ordinary writer behavior; full repository checks remain required at Gate D.

- Known architectural gaps: Relationship and later owner migrations are unfinished; no corpus promotion or legacy retirement is claimed.



Entity implementation is committed as `73e88ae`. CLI format/check and **32/32 tests** pass (`.test-tmp/lossless-entity-final-cli-format.log`, `.test-tmp/lossless-entity-final-cli-check.log`, `.test-tmp/lossless-entity-final-cli-tests.log`). Steps 96–107 are accepted independently against this implementation. Step 108 selects Relationship as the still-canonical persisted target; eight explicit consumer/pruning gates expand pending work to 279 rows while preserving accepted numbering. The next execution item is Step 109.



## Relationship contract, codecs and reconstruction — Steps 109–112



Implementation `0650305` passes **52/52** Relationship regressions, zero failures (`.test-tmp/lossless-relationship-codec-rebuild-tests-final.log`). This includes actual V1 REL/PHY acceptance characterization, independent compact golden frames, all 32 field masks, complete participant tuple add/remove semantics, bounded exact UTF-8/reference values, candidate canonicality, current/historical owner reopen, late global tickets and preserved transaction times, explicit terminal history, inert orphans and V1 published-compact refusal. The first run's two failures were fixture preconditions (noncanonical ordering and an earlier clock failure hiding revision overflow); they were corrected without production changes. Root format/check and both documentation modes pass. Accepted range is now 1–112 of 279.



Published compact Relationship prune proofs currently fail closed at owner finish until Step 118 supplies authenticated manifest/source-history admission. Ordinary Relationship writers remain unchanged pending the concrete consumer gates. No migration or corpus promotion is claimed here.



Documentation impact:

- Inspected: architecture, invariants, maintainer map, current limitations, roadmap and history/format plans.

- Updated: source specification, storage format, current limitations, execution and acceptance ledger.

- Not affected: public API signatures and CLI commands.

- Compliance check: both structural and change-impact documentation checks pass for this source batch.

- Known documentation gaps: consumer/proof/writer/migration and final retirement acceptance remain pending.



Architecture impact:

- Standards added or changed: none.

- Ownership or boundary impact: Relationship owns its pure contract, bounded wire values, candidate reconstruction and accepted-publication replay; lifecycle only invokes the existing owner seam.

- Pitlord enforcement impact: new modules remain in existing perception ownership; fresh graph enforcement remains scheduled for Integration Gate D.

- Other verification impact: focused owner and historical tests pass; broad cross-owner checks remain required at Gate D.

- Known architectural gaps: concrete consumers and native prune proof readers precede writer cutover.



## Relationship consumers, publication, pruning and migration — Steps 113–124



The cohesive implementation resolves accepted Relationship publications by exact global/local coordinates rather than decoding a second semantic history in reconciliation. It preserves exact accepted strings, optional roles, participants/evidence, mutation identities, creation/update times, revision order and publication transaction times. Import evidence has a separate unavailable-summary variant and omits only rebased record coordinates. Strict compact interval classification includes validated terminals; orphan carriers remain inert.



Ordinary writes and exact replay share the Relationship-owned compact publication seam. Native REL/PHY pruning re-encodes complete accepted redacted chains, discards selected unmarked carriers, and authenticates every proof against terminal retained structure, manifest membership and original source-history binding. Canonical retained-structure bytes remain identical to the previous summary-redacted hash schema. Reclamation preserves published carriers and terminals through relocated references. The private offline converter composes Memory, Entity and Relationship conversion without allocating new semantic tickets or promoting output.



Focused Relationship verification passes **73/73** tests (`.test-tmp/relationship-focused-final.log`), including seven rechecksummed proof-tamper cases against both REL and PHY, exact history/retry/terminal handling, participant/evidence remove-and-add replay, typed unavailability, source immutability and orphan cleanup. Initial test-only setup errors were corrected: stored-record versus public-view types, valid nonzero coordinates for an unmarked carrier, reopening a raw test owner before tail lookup, and hashing accepted carriers rather than matching orphan IDs. No production guard was weakened.



The six frozen V1 REL/PHY conversions pass with zero semantic mismatches and unchanged source bytes, hashes, lengths and modification times (`.test-tmp/relationship-authoritative.log`). All six are zero-Relationship cases; this is recorded explicitly in [the byte artifact](../benchmarks/relationship-history-storage-2026-10-20.json). The nonempty deterministic two-revision-plus-terminal REL and PHY fixtures each change from **689 to 661 raw carrier bytes** (28 bytes saved); these are synthetic codec examples, not a retained-corpus savings estimate. Complete corpus promotion and legacy-reader retirement remain later gates.



Root format/check and locked CLI format/check pass. All **1,917 executed root tests** pass; five existing opt-in corpus/measurement/crash-subprocess entry points remain reported as ignored. All **32 CLI tests** pass. Logs are `.test-tmp/relationship-full-tests.log`, `relationship-final-check.log`, `relationship-cli-check.log` and `relationship-cli-tests.log`. Tests run with repository-local scratch paths and a Git discovery ceiling. Local HTTP fixtures required the permitted unsandboxed verification run; the sandbox failure was reproduced and the same isolated localhost test passed outside it. An existing Entity prune fixture was given enough body bytes to exercise its intended unavailable-payload check without failing the native net-shrink admission rule.



Architecture evidence: direct source ownership covers **1,042 Rust files in 20 areas**, with zero uncovered or overlapping paths. The initial graph pass exposed a stale-snapshot risk: initializing an existing Lexicon repository alone did not include the newest converter. `scripts/check_architecture.py --refresh` now explicitly scans before synchronizing Arcana. Final enforcement passes all **23 rules**, **95,098 source nodes** and **3,124 relationships**, snapshot `17e404f7eb4de346ba75f1a1861c60030df4ae9f4932479280680e4ac315bc91` (`.test-tmp/relationship-final-architecture.log`). Changed Relationship implementation hashes match milestone `41eaa0a` after LF/CRLF normalization; the two root/export registration modules are inspected separately. The unrelated Entity test fix is included in the refreshed snapshot. Ego additions after this scan require a later integration refresh.



Documentation impact:

- Inspected: architecture, invariants, maintainer map, current limitations, roadmap, history/format plans, shared ownership/seam and documentation procedures.

- Updated: storage format, current limitations, maintainer/coverage/behavioral maps, development guidance, temporary converter removal map, execution evidence and byte artifacts; acceptance ledger follows verification.

- Not affected: public API signatures, CLI command surface, Graph/Archive semantics and retained authoritative sources.

- Compliance check: structural and change-impact checks pass before the final ledger refresh; both are rerun after acceptance records.

- Known documentation gaps: the corpus has no Relationship or Ego records; nonempty behavior has explicit synthetic coverage. Full canonical promotion and runtime retirement remain pending.



Architecture impact:

- Standards added or changed: none.

- Ownership or boundary impact: Relationship retains canonical mutation/replay/retention ownership; shared maintenance checks file-level evidence. No generalized semantic owner or root was introduced.

- Pitlord enforcement impact: existing 20 areas/23 rules remain; the refresh helper now requires an explicit current-file scan.

- Other verification impact: complete root and CLI correctness runs pass; five opt-in entry points are disclosed separately.

- Known architectural gaps: later Ego/current-source integration refresh, owner conversion and runtime retirement remain pending.



## Ego field inventory and repeated-text measurement — Steps 125–126



The Step 125 inventory follows `src/ego/ego_model.rs`, `ego_codec.rs`, the Ego store and its Identity/Anchor/document writers, and the REL/PHY facades. `ego_version` is the owner-wide sequencing coordinate; each document's `revision` is its history coordinate. Neither is user-authored content.



| Record | Stable identity and transition | Mutable semantic fields | Terminal or separate state |

| --- | --- | --- | --- |

| Identity | Stable `EgoIdentityId`. | `name` and `text` change together in each postimage revision. | Deletion is terminal and writes empty name/text. Active Identity is a separate `IdentityActive(id)` event; creation's `activate` flag records initial activation. PHY-only. |

| Anchor | Stable `EgoAnchorId`. | `priority` and `text` are revisioned together. | Deletion is terminal; the current tombstone retains priority and writes empty text. Available in REL and PHY. |

| Personality | Singleton document; revision identifies its history. | Synthesized `text` and `source_memory_version` watermark. | No delete/terminal operation exists. PHY-only. |

| Web synthesis | Singleton per owner; revision identifies its history. | Synthesized `text` and `source_memory_version` source cut. | No delete/terminal operation exists. Current APIs admit it in REL and PHY. |



`IdentityActive` carries only the active Identity ID plus owner sequence, so activation does not duplicate Identity text. Current encoded payload sizes before the outer Container chunk prefix are: Legacy Identity `28 + text bytes`; Identity v2 `50 + name bytes + text bytes`; IdentityActive `32`; Personality and Web synthesis `36 + text bytes`; Anchor `46 + text bytes`. Each physical chunk adds an 8-byte little-endian payload length. These numbers describe the current codec, not a compact-format choice.



Step 126's read-only measurement is implemented in [measure_ego_history.py](../scripts/measure_ego_history.py) and recorded in [ego-payload-repetition-2026-10-20.json](../benchmarks/ego-payload-repetition-2026-10-20.json). It follows the Container header and chunk framing, recognizes the six exact Ego magic families, checks fixed layouts, UTF-8 strings, flags, priority, live text constraints and trailing bytes, and applies the current REL restriction on Identity/Personality carriers. Other payload families are skipped. This validates carrier shape and owner family but does not replay all EgoStore state transitions. The script verifies every input byte count and SHA-256 against the pinned six-file manifest before parsing, rehashes each source after measurement, and opens fixture files read-only.



The six retained containers contain **zero recognized Ego records**, hence zero measured Ego text bytes and zero observed repeated Ego values. All six source hashes matched before and after. Their current headers are already 40-byte typed identity headers, so the projected header delta is zero. This zero-observation corpus does not establish whether an owner-local payload table pays off for Ego history. The artifact reports carrier payload and framed bytes separately, exact string frequencies, and repeat-pool projections at 4-, 8-, and 16-byte reference widths. Each hypothetical pooled entry includes an 8-byte Container prefix, 8-byte Ego magic, 8-byte owner sequence, ID, 4-byte text length, and text; each reference use includes a one-byte tag and ID; inline values also pay the discriminator tag. Unique text remains inline. Allocator/index design, deletion/retention rules, migration-proof transport, and actual V1-to-compact conversion costs remain unmeasured. Decision for Step 126: **no owner-local payload table is warranted for this rollout**. The measured corpus has zero eligible values and zero net projected saving at every reference width. Use ordinary owner-specific creation/field-delta encoding, which already avoids copying unchanged text between adjacent revisions. This is a decision against adding unmeasured table/index/lifecycle complexity, not a claim that Ego text can never repeat. Reconsider only with an Ego-bearing measured workload and a net benefit beyond field deltas.



Documentation impact:

- Inspected: Steps 125–126, Ego model/codec/writers/facades, Container framing and the pinned fixture manifest.

- Updated: this execution evidence and the measurement artifact.

- Not affected: retained fixture files and hashes.

- Compliance check: all six source hashes matched before and after the read-only measurement; no cargo build or test ran.

- Known documentation gaps: the retained fixtures contain no Ego records; cross-object pooling benefit remains unmeasured for nonempty Ego workloads. The current no-pool decision makes no extrapolated savings claim.



The measurement parser is checked against an independent nonempty literal Container covering all six Ego families: six tests pass, including UTF-8 bytes, exact duplicate values, 4/8/16-byte reference projections, unique-only negative savings from discriminator overhead, malformed flags/priority, truncation and owner restrictions.


## Ego compact contract preparation — Step 127 in progress

Step 126 selects inline owner-specific creation/field-delta storage without a payload table. The contract must distinguish ordinary setter no-ops from admitted historical publications: a repeated postimage with the next document revision and owner sequence remains a publication; an Identity update may carry `activate=true`; an Anchor terminal may carry a priority different from its last live revision. Identity terminal name/text are empty and activation is false. Anchor terminal text is empty, but priority remains exact. Decoder/replay semantics, rather than only setter behavior, govern lossless conversion.

Identity and Personality remain PHY-only. Anchor and synthesis remain valid in both REL and PHY. Legacy Identity normalizes to zero ID, name `Default`, and activation true. Ego sequence remains dense and owner-local; document revisions advance independently and interleave across objects. Source Memory watermarks may decrease and remain bounded by the owner's current Memory head; they do not borrow the Ego clock. Activation-only publications retain their existing minimal carrier. No global ticket, transaction-time field, or payload identity is invented.

Four discovered consumers are explicit pending plan gates before writer cutover: historical fingerprint materialization, fresh-history rewrite emission, maintenance chain preservation, and safe rejection of unsupported Ego reconciliation/imports. Contract implementation and tests remain pending; Step 127 is not accepted by this preparation record.

## Ego compact publication and consumer batch — Steps 127–138 under validation

The Ego owner now has explicit compact creation, two-field delta, terminal and independent activation semantics. Creation revision and delta successor revision are implicit; exact semantic content, activation flags, terminal Anchor priority, independent owner sequence and source Memory watermarks survive materialization. Zero-field raw publications remain represented. Existing setter no-ops still append nothing. Accepted current-document bases retain their actual last Ego publication coordinates, without a new global clock or text pool.

The store dispatches strict compact families, rejects missing/wrong/terminal predecessors, unknown versions, invalid masks/flags/UTF-8/trailing bytes and redundant changed fields, and enforces existing owner/lifecycle rules. V1 inputs containing compact Ego frames fail the storage-version gate. Historical fingerprints resolve materialized publications through the same Ego owner. Fresh-history rewrite emits compact first revisions and independent activation while preserving its selected-state and derived-cache policies. Native REL/PHY pruning and reclamation, REL repacking, unsupported divergent reconciliation and import interval guards have explicit compact-history regressions. Ordinary writers use compact frames; the private V1 upgrader composes Ego with Memory/Entity/Relationship translation and allocates no semantic sequence.

Read-side verification passes **56 tests**, including exact legacy/compact canonical fingerprints at every REL/PHY Ego cut. The six-file authoritative Ego upgrade passes with source bytes, hashes, lengths and modification times unchanged; all six contain zero Ego records, explicitly disclosed in [the byte artifact](../benchmarks/ego-history-storage-2026-10-20.json). Nonempty raw V1 fixtures independently cover five REL and thirteen PHY publications, including Legacy Default Identity, repeated postimages, activation on update, independent activation, terminal priority changes and all four document kinds. They preserve every cut, owner UUID, global tickets and transaction times. Raw payload bytes are **256→200 REL** and **656→555 PHY**; including unchanged per-frame 8-byte lengths, **296→240** and **760→659** respectively. These are synthetic examples, not a retained-corpus savings estimate.

Development fixture errors corrected during validation include an incorrect REL open for a PHY test, overbroad CVAE prefix scans that matched Entity records, an over-limit Anchor body, a wrongly fixed activation count, and a relabeled compact carrier used as a V1 fixture. Tests now construct genuine raw V1 Ego carriers; no production guard was relaxed. Numbered Steps 127–138 are accepted against `b10815d`. Full root verification passes **1,956 executed tests across 26 suites**, with zero failures and five existing opt-in ignores (`.test-tmp/ego-full-tests.log`). Root/CLI format and compilation checks and **32 CLI tests** pass (`ego-format.log`, `ego-check.log`, `ego-cli-format.log`, `ego-cli-check.log`, `ego-cli-tests.log`). Integration Gate D remains pending the current-source architecture refresh.

Documentation impact:
- Inspected: architecture, invariants, history/storage plans, shared ownership/seams/documentation standards, Ego codecs/stores/facades, historical readers and rewrite/import/maintenance boundaries.
- Updated: storage format, current limitations, maintainer/coverage maps, temporary converter removal scope, development scan guidance, byte evidence and this execution record.
- Not affected: public API signatures, CLI command surface, source fixture bytes or canonical promotion policy.
- Compliance check: structural and change-impact modes pass before this evidence refresh and will be rerun at batch acceptance.
- Known documentation gaps: final post-Ego architecture evidence and Gate D acceptance are still collecting; retained sources have zero Ego values.

Architecture impact:
- Standards added or changed: none.
- Ownership or boundary impact: EgoStore remains the only mutable Ego semantic owner; codecs and candidate materialization remain concrete Ego modules. Historical scans consume the owner seam. No generic root/store registry was added.
- Pitlord enforcement impact: 20 existing areas/23 rules unchanged; a final Ego source refresh is collecting. The helper avoids a redundant initialization scan when configuration already matches, but always explicitly scans.
- Other verification impact: bounded codecs/replay/migration/maintenance tests, read-only authoritative conversion, all 1,956 executed root tests and 32 CLI tests pass. Direct ownership audit covers all 1,060 authored Rust files in 20 areas with zero missing/overlapping ownership.
- Known architectural gaps: final Ego snapshot enforcement and later canonical promotion/runtime retirement remain pending.

## Graph field inventory preparation — Step 139 pending Gate D

The read-only [Graph inventory](../benchmarks/graph-history-byte-inventory-2026-10-20.json) and [measurement script](../scripts/measure_graph_history.py) distinguish format markers, node mappings, mutation/batch carriers and publication links. The six immutable source files contain **7,562,328 Graph payload bytes**, or **8,349,616 bytes including Container frames**. Mappings retain 32-byte Memory or 33-byte typed semantic references plus a 4-byte internal ordinal; links retain two u64 coordinates and a 16-byte physical ObjectRef. Mutations carry two full semantic endpoints, relation kind, active flag and explicit or legacy-implicit origin; batches share only their magic/count framing.

A hypothetical new owner-local dictionary includes one complete framed header and 33 bytes per typed entry, with no assumed reuse of existing mappings. Four-byte references project **3,918,649 net bytes saved** across all six files; one-byte references are infeasible for five files and are excluded for those files. This is not implemented format evidence or a final Step 140 decision. Six independent parser/projection tests pass, including literal typed Memory widths, family-specific batch strides, malformed data, empty dictionaries and ordinal capacity. Parent review corrected initially optimistic framing/kind accounting before retaining the artifact. All source hashes match before and after. Step 139 remains unaccepted until preceding Ego/Gate D work closes.

## Freshness effect-history discovery — Step 156 preparation only

Current-source inspection found that the planned removal of persisted event postimages has already occurred: `freshness_storage.rs::put_event` serializes effects and immutable evidence, deliberately ignores the derived `records` map, and `get_event` returns an empty postimage map for owner replay. `FreshnessStore` prepares canonical candidate postimages from prior state plus effects, and rejects nonempty supplied postimages that differ before applying the event. This source fact must be verified against existing format, forgery, replay and migration tests before Steps 157–164 are accepted; adding a redundant second effect-only carrier is not assumed necessary. No Freshness runtime change or acceptance is claimed here.

| Field | Classification | Lossless requirement |
| --- | --- | --- |
| Event ID and turn | Immutable identity/order | Preserve exact accepted identity, retry/conflict behavior and canonical `(turn, event_id)` cuts. |
| Recipient Memory ID to signed effect | Accepted causal facts | Retain every recipient and effect; current scores cannot recover them. |
| Graph/Community proof | Accepted evidence | Preserve exact Graph cut and optional Community generation/Graph version; validate against owner evidence. |
| Producer/source/origin/accepted turn/policy provenance | Accepted evidence | Preserve producer-specific admission and source identity, independently of derived scores. |
| Semantic/fragment evidence | Accepted evidence | Preserve owner, fragment/source keys, conversation/node, accepted turn, policy/profile and source/target digests used by admission and import proof. |
| Per-event postimage records | Derived candidate projection | Recompute with the owner reducer; never replace causal/proof facts with final scores. |
| Physical frame order | Transport/replay ordering | Preserve the applicable event, accepted-use batch/receipt and settlement ordering contracts. |

Concrete consumers include owner replay, the `FreshnessReplayEntry` adapter, historical event-cut enumeration/fingerprints, REL reconciliation and proof rebasing, native import interval/evidence extraction, historical settlement digests and supersession, and fresh-history rewrite score recomputation. Any actual codec change must name these consumers explicitly rather than hiding them in a generic writer row. Current `CVAFRS06` settlement proof semantics remain required.

### Graph compact design constraints discovered during review (not accepted implementation)

Graph already persists dense validated `u32` local node mappings (`GraphStore::insert_node_rebuilt`), and ordinary writes establish mappings before emitting mutation carriers. Reusing those owner-local mappings is preferable to adding a second generic dictionary, provided the compact format preserves the original stable semantic references at every accepted publication. Ordinals remain encoding coordinates; fingerprints, public references, relation kinds/origins and ownership remain semantic-ID based. The four-byte projection is conservative because it charges an additional dictionary that reuse might avoid; actual savings still require encoded output measurement.

A critical admitted-order edge must survive: existing V1 replay can read a full-ID carrier, then a node mapping, then its publication marker. Mappings need only be available when the marker publishes the transaction. Compact replay must therefore retain structurally decoded unresolved ordinal carriers until their marker, and resolve against mappings available at that marker. Resolving immediately at carrier arrival would incorrectly reject accepted historical input. Conversely, a mapping after the marker cannot legitimize it. Unmarked structurally valid carriers stay inert; malformed wire frames still fail as in current replay.

Raw batches can contain different origins per mutation even though ordinary API batches use one origin. Conversion must preserve each admitted origin and mutation order rather than republishing through an API that assumes one origin. Existing divergent reconciliation may retain its explicit mixed-origin rejection. Accepted duplicate relation keys, version/transaction time, no-op, orphan and lifecycle rules remain Graph-owned. Before writer cutover, explicit adapters/tests are needed for runtime marker materialization, historical fingerprints, reconciliation/Freshness graph reads, import evidence/interval attribution, physical reclamation and maintenance copies, PHY migration/rewrite, then exact offline conversion. Source inspection, rather than an umbrella codec gate, determines those consumer responsibilities.

Graph dictionary binding creates an additional proof-reader responsibility: `history_prune_manifest::owner_history_digest` currently hashes each accepted Graph carrier directly, which presently embeds stable semantic IDs. Replacing those IDs with ordinals would leave that digest blind to a valid-but-different node mapping unless Graph's semantic mapping interpretation is included. Native prune admission must therefore use an owner-provided canonical semantic Graph carrier digest (or an equally explicit authenticated mapping binding) for compact Graph publications. A mapping alteration to another existing Memory/Entity must not preserve an authenticated source-history digest. This requires a separate retained-proof/source-history gate before writer cutover, including rechecksummed mapping-tamper tests for REL and PHY. Opaque copies preserve mappings but do not by themselves establish this proof property. Preexisting unsupported V1 prune authority remains fail-closed; no runtime compatibility bridge should be invented to absorb it.

The Graph discovery responsibilities above are now explicit numbered Steps 143–153 before ordinary writer cutover and conversion; the plan totals 294 consecutive rows. Accepted Steps 1–138 retain their identities. The formerly numbered Freshness field inventory 145 is now 156; the unregistered preparation test lives under the existing Freshness ownership directory at `src/freshness/freshness_effect_storage_contract_tests.rs`. No later phase acceptance is implied.

## Gate D and Graph inventory/decision acceptance

Integration Gate D is accepted against `b10815d`: **1,956 executed root tests**, zero failures and five existing opt-in ignores across 26 suites; **32 CLI tests**; root/CLI format/check; nonempty and six-file authoritative Ego conversion. Final architecture enforcement passes all **23 rules**, **96,374 source nodes**, **3,142 relationships**, snapshot `14b14fc52a55a56b1d69809d18cf708415f16706ab2562f6e36bcdd1f21bb739` (`.test-tmp/ego-integration-architecture.log`), zero compatibility warnings. All 28 changed production/registration Rust files match milestone bytes after line-ending normalization. Two test files have later assertion/comment corrections, fully exercised by the passing tests. The future unregistered Freshness test is not part of this snapshot; direct ownership still covers all 1,060 authored Rust files without missing/overlapping areas. Structural and change-impact documentation checks pass. This closes the previously pending Ego integration/architecture gaps; final canonical promotion/runtime retirement remain pending.

Steps **139–140** are accepted against `e9b36e9`. Parent review approves compact Graph endpoint encoding because the conservative fixed-u32 projection saves **3,918,649 bytes** across the retained fixtures. Reuse Graph's existing validated dense mappings; do not introduce another semantic-ID table. Fixed u32 values preserve the existing ordinal domain and checked exhaustion, while keeping stable semantic references at public and historical boundaries. Projections are not actual output savings: the actual codec and converter still require measurement. Source mapping interpretation, deferred marker resolution, repeated raw no-op publications, per-mutation origins and retained proof digests remain explicit contract/consumer gates. The next numbered item at this checkpoint was **141**; the plan contains **294** rows after making eleven Graph consumers/writer responsibilities explicit.

## Graph ordinal history — Steps 141–153 accepted

Implementation `864efe9` reuses the existing Graph-owned dense u32 mappings. Compact single carriers use `CVAGCM01` with a 12-byte body; batches use `CVAGCB01`, a u32 count and ordered 12-byte bodies. Stable semantic IDs remain the public/historical authority. Open replay resolves endpoints at publication markers, retaining admitted carrier→mapping→marker ordering, inert structurally valid orphans, per-mutation origins and raw repeated transactions.

Historical Graph/MemoryGraph, reconciliation, Dream/Freshness Graph evidence, completed import coordinate extraction and strict replay classification now consume the concrete Graph decoder. Reclamation retains referenced compact backing, drops orphans and relocates existing marker references. PHY semantic migration preserves source stable IDs and clocks. Ordinary REL/PHY writers and fresh-history rewrite now emit compact single/batch carriers.

Prune proofs required an explicit semantic binding: an ordinal-only raw hash cannot authenticate its ID interpretation. Graph now hashes the compact carrier's domain tag, shape/count and ordered stable typed endpoints/kind/active/origin. Legacy full-ID digest bytes remain unchanged. REL proof admission moves after validated Graph materialization. Real REL/PHY pruned outputs reopen, while changing mappings to other valid existing IDs rejects the maintenance proof.

Verification: **236/236** pre-cutover compact regressions and **240/240** after writer cutover pass, with zero failures/ignores (`.test-tmp/graph-consumers-compact-regressions-repo-temp.log`, `.test-tmp/graph-writer-compact-regressions.log`). Cases cover literal wire corruption, mapping/marker order and V1/V2 admission, REL/PHY old/compact replay, all small-fixture cuts, typed identity/retirement, independent reconciliation mapping scopes, Dream accepted-cut filters, imports, native prune/reclaim, REL principal/attachment repacks, PHY semantic migration, normal writer/no-op behavior and fresh rewrite baseline membership. Library compilation passes (`.test-tmp/graph-consumers-check.log`); source/CLI broad verification and architecture refresh belong to the following integration gate. Fixture corrections preserved production admission requirements: duplicate format frames, illegal Memory semantic revisions, insufficient prunable bytes and UUID-less legacy oracle use were corrected in test setup.

Documentation impact:
- Inspected: Graph codecs/store/replay, historical cuts, reconciliation/import/Dream readers, maintenance proof and physical-copy consumers, storage and ownership standards.
- Updated: storage format, maintainer/coverage maps, execution evidence and individual acceptance records.
- Not affected: public API signatures, CLI surface, semantic ID/clock ownership, authoritative source bytes or canonical promotion policy.
- Compliance check: both documentation modes pass before this acceptance append and will be rerun.
- Known documentation gaps: actual six-source compact Graph byte output and converter acceptance are collecting in Step 154.

Architecture impact:
- Standards added or changed: none.
- Ownership or boundary impact: GraphStore remains the sole Graph semantic owner; shared maintenance consumes its concrete semantic carrier digest after materialization. The shallow Archive digest forwarding method was removed so Archive does not gain a Graph dependency.
- Pitlord enforcement impact: existing Graph/facade/import ownership areas and 23 rules remain unchanged; source refresh is pending Integration Gate E.
- Other verification impact: 240 adjacent compact regressions and library compilation pass; next verify the private V1 converter and measured output.
- Known architectural gaps: broader integration/source refresh, canonical corpus promotion and runtime/converter retirement remain pending.

The ledger accepted **153/294** numbered steps at that checkpoint; the following review and discovery records supersede its count.

## Graph conversion, byte evidence and renewed import comparison

The private `stage_graph_storage_upgrade` composes the prior Memory/Entity/Relationship/Ego adapters and translates accepted Graph marker-linked carriers through the validated source Graph owner. It preserves single/batch shape, original per-mutation order and origins, repeated raw no-op publications, mapping bytes, original markers/global tickets and transaction times. It allocates no semantic publication and leaves unmarked backing inert. Genuine malformed V1 source histories reject without source changes, final output or staging debris. The adapter and migration-only tests are explicitly in the temporary T7 removal scope.

Three upgrade tests pass, including all six pinned authoritative V1 files (`.test-tmp/graph-upgrade-focused.log`, 160.89 seconds). Every accepted marker's coordinates and ordered stable-ID mutations, mapping bytes, Graph/MemoryGraph heads, current projection and all global transaction times compare exactly. Deterministic replay proves every prefix cut equal; the nonempty synthetic REL/PHY fixtures also compare every Graph/MemoryGraph cut fingerprint explicitly. All six source bytes, hashes, lengths and mtimes remain unchanged. Disposable staged files are not corpus promotion.

Actual [Graph storage measurement](../benchmarks/graph-history-storage-2026-10-20.json):
- 46,075 accepted publications; 98,411 Graph records before and after.
- Published carrier payloads: 5,443,274 → 1,346,932 bytes.
- All Graph payloads, including mappings/format/links: 7,562,328 → 3,465,986 bytes.
- Container-framed Graph bytes: 8,349,616 → 4,253,274 bytes.
- Actual saving: **4,096,342 bytes**, without compression or a second dictionary.
- Existing mapping payloads remain exactly **275,982 bytes**. Domain totals exclude owner file headers and do not attribute other owners' migration savings to Graph.
- Independent inventory totals agree with the before-conversion measurement.

Final reader review reopened Step 147: completed-epoch coordinate extraction was converted, but `inventory` still compared raw Graph carrier bytes as owner evidence. The supplement now resolves stable IDs through Graph and emits canonical comparison evidence, preserving shape/order. A regression proves legacy/compact and opposite mapping assignments compare equally for the same edge, while identical raw ordinal bytes representing different stable edges compare differently. The full library suite passes **1,831 tests, zero failures, five existing opt-in ignores**, including that regression (`.test-tmp/graph-full-tests.log`). Root integration completes with **1,988 passing tests across 26 suites**, zero failures and five existing opt-in ignores. Root/CLI format/check pass, and all **32 CLI tests** pass (`.test-tmp/graph-{format,check,cli-format,cli-check,cli-tests}.log`). Architecture/source refresh remains the explicit Integration Gate E responsibility. An overlapping targeted build briefly hit Windows executable locking; its retry uses the separate fixture-enabled test binary, and no production guard was changed.

## Freshness authority discovery — separate pending gate

Read-only Freshness review found an additional comparison responsibility: a completed settlement's accepted-event map can omit or name an event outside the physical import interval, while the old reconciliation comparison only checked normalized interval events. New Step **163** will compare the settlement's canonical accepted event identities and immutable causal evidence through the existing Freshness owner, using the established turn/Graph-proof coordinate normalization. Raw event payload digests cannot be compared directly because rebasing legitimately changes those coordinates. Former pending Steps 163–294 become **164–295**; Steps 1–162 retain their identities. This is an explicit owner-specific implementation gate before the previously planned regression-only row. No Freshness implementation acceptance or second effect codec is claimed here.

## Freshness effects-only verification and import authority — Steps 156–165

Source inspection confirms the effects-only event codec was already implemented before this storage plan (`20e1e9a`). Steps 157–161 therefore reuse and verify the one existing `CVAFRS02` format rather than add a redundant carrier or migration. Event IDs/order, effects, Graph/Community proof, producer/source/origin/policy and semantic/fragment evidence persist exactly. Per-event postimages are omitted and reconstructed; supplied false postimages reject before omission. Initialization, admission and settlement records retain their accepted metadata/proof role.

Four independent contract tests pin literal semantic event bytes, decoded accepted fields, invariant bytes under different derived record maps, exact reducer projection, rejected forged candidate postimages, every truncation, trailing bytes, producer/option/owner corruption and unsupported versions. The initial fixture-enabled Freshness run passes **151 tests**, including all four new contract tests and existing forgery/retry/order/scoring/import tests; its sole failure was the new migration test asking for an intentionally unsupported intra-settlement trajectory. The corrected migration test now requires matching specific fail-closed outcomes there. Development runs exposed fixture setup/assertion errors: canned extraction responses did not name imported Nodes, imported Memory needed its real initial Dream settlement, and event cuts retain both enrolled records. These were corrected without changing production admission. Final adjacent verification passes **154/154** (`.test-tmp/freshness-adjacent-final.log`); the unchanged long-owner retry test passed in the preceding run and was excluded from this short rerun. All **four** focused authority tests, including the settled real completed-import inventory path, pass (`.test-tmp/freshness-import-authority-final.log`).

New Step 163 closes a real comparison omission. Completed-epoch inventory derives the canonical semantic scope, including source-bound legacy reconstruction, then requests exactly its accepted IDs through `FreshnessStore`. The owner expands accepted-use batches and returns immutable event entries; absent or conflicting identities fail. Reconciliation compares this map after the existing coordinate normalization, including events outside the physical epoch interval. Raw payload digests remain coordinate-bound and are not compared as semantic identity. Bounded tests cover missing/changed membership, changed causal effects, legitimate turn/Graph/Community rebasing, batch expansion, missing requested IDs and a real completed-import inventory path.

Two migration tests pass (`.test-tmp/freshness-upgrade-focused.log`, 317.25 seconds): a genuine legacy-Memory synthetic source with two same-turn accepted-use effects/proofs, and all six hash-pinned authoritative V1 sources. The synthetic case compares event-inclusive cuts, derived scores, final activity projection and exact retained entries. The retained RELs contain **111 accepted events and 148,708 effects**. All retained event entries/effects/proofs, policy/current records and supported activity/settlement cuts agree after staging. Each of the 111 intra-settlement event cuts is unavailable on both sides with the exact documented error; zero successful retained event-cut fingerprints are claimed. Source bytes, hashes, lengths and mtimes remain unchanged. PHY Freshness frames are copied exactly. Original REL Freshness frames remain an exact prefix; added semantic settlement successors equal the source-derived canonical normalization bytes. No canonical fixture is promoted.

The [actual Freshness measurement](../benchmarks/freshness-history-storage-2026-10-20.json) reports all `CVAFRS02` journal rows, including initialization/admission/use receipts, separately from accepted-event/effect counts. Journal payload bytes remain **5,098,875 → 5,098,875**, and Container-framed journal bytes remain **5,100,635 → 5,100,635**: **zero saving**, because the source format already omits event postimages. Policy receipts are outside these journal totals. Three source-derived `CVAFRS06` normalizations add **7,415,813 payload bytes** (7,415,837 framed); these are actual proof additions, not event compression savings or another owner's savings. Full-file before/after totals remain in the artifact for attribution.

Documentation impact:
- Inspected: Freshness codec/reducer/admission, historical cut availability, import semantic scope/reconciliation, V1 staging and format/ownership references.
- Updated: storage format, maintainer map, documentation coverage, field inventory and execution evidence; the ledger records acceptance only after focused verification.
- Not affected: public API and CLI commands, accepted event identities/proof domains, authoritative fixture bytes and promotion policy.
- Compliance check: structural and change-impact checks pass before this evidence append; rerun at acceptance.
- Known documentation gaps: final integrated promotion/runtime retirement remain pending.

Architecture impact:
- Standards added or changed: none.
- Ownership or boundary impact: FreshnessStore remains the semantic owner; reconciliation consumes its bounded requested-event seam and canonical legacy scope adapter. No generic history registry or second reducer was added.
- Pitlord enforcement impact: existing Freshness/import ownership areas remain unchanged; refreshed source enforcement belongs to Integration Gate E.
- Other verification impact: root/CLI format and compilation pass; all 32 CLI tests pass. Focused owner, wire, migration and fixture tests are recorded above; full root rerun and architecture refresh remain at Gate E.
- Known architectural gaps: broad Gate E, final corpus promotion and temporary converter/legacy-reader retirement remain pending.

## Publication framing inventory and contract — Steps 166–167 accepted

The [read-only framing inventory](../benchmarks/publication-framing-byte-inventory-2026-10-20.json) pins all six source hashes before and after its scan. Nine independent parser/accounting tests pass. It joins tickets by explicit global IDs, expands encoded grouped ranges, and never attributes by neighboring records. There are **87,296** standalone global tickets: **87,292** have one owner link, **four** are unclaimed and none are shared. Links comprise Memory 7,719, Entity 1,141, Archive 32,357 and Graph 46,075; Relationship/Ego links are absent from the corpus. Insomnia has **1,943 embedded ranges covering 4,695 global IDs**; external-source ranges are absent. Actual ticket payload/framed bytes are partitioned once by owner plus explicit orphan/shared buckets. No envelope savings are claimed by this inventory.

Step 167 review found that ticket allocation and publication binding can occur at different physical positions and even different global ordering. Existing Memory/Relationship fixtures explicitly admit local successors using earlier reserved global tickets. A converter must preserve marker-slot/import-boundary semantics rather than sort by global version. The [draft envelope contract](publication-envelope-contract-2026-10-20.md) records this decision before any runtime codec or writer cutover. Parent review resolves this through explicit allocate-with/without-time and bind-prior-standalone-reservation clock tags. Ordinary writes allocate; conversion retains the original ticket and places a reserved binding at the original marker slot, with the source-derived import normalization prepared before relocation. The reserved mode inherits the ticket's optional time instead of duplicating it. New Step 169 owns Container clock/recovery integration, and Memory's former umbrella row is expanded into explicit Steps 170–177; former Steps 170–295 become 178–303. The plan now contains 303 consecutive execution rows, with Gate E after 184. No runtime envelope codec or writer is accepted by this contract decision.

## Physical publication envelope mechanics — Steps 168–169

`container/publication_envelope.rs` implements the accepted `CVAPUB01` header and opaque-body boundary. Five independent codec tests pin all clock tags, borrowed payload bytes, optional signed timestamp extremes, maximum structural global coordinate, zero/short/nested/malformed/unknown frames and owner-tail opacity. The first run caught an out-of-bounds truncation in the new test fixture; the corrected test and codec pass.

Container replay records standalone reservation provenance in a scan-local set. Allocation envelopes consume the exact next global and optional timestamp; reserved bindings remove one earlier standalone reservation without allocating or changing its timestamp. Grouped ranges and prior direct envelopes never authorize a reserved binding. V1 rejects envelope prefixes. The physical append seam validates before writing and only commits clock/time/override state after successful append; semantic body validation and sync remain with callers. It does not claim same-handle retry safety after a partial I/O failure; reopen retains existing tail recovery.

Fourteen clock/recovery tests cover actual append/reopen, signed/nonmonotonic/missing time, reserved bindings after higher tickets, unknown/future/grouped/direct/repeated reservations, allocation gaps/duplicates, overflow, closed write admission, malformed complete frames without repair and every incomplete frame prefix for both allocation and binding. The historical physical-frontier adapter uses the same reservation-aware observer. Every synthetic physical global cut matches the existing high-water convention, including exclusion of a late low-global binding from earlier physical cuts.

Verification: **29/29** envelope-filter tests pass, including the nineteen new codec/clock cases (`.test-tmp/publication-envelope-focused.log`). Five adjacent filters pass **318 unique tests** with zero failures/ignores: Container 40, global 22, compact 241, reconciliation 45 and transaction-time 7 (`.test-tmp/envelope-*-regressions.log`). This includes the full existing historical native checkpoint byte-prefix recovery case. Root/CLI format and compilation pass, and all **32 CLI tests** pass. Whole-root/source architecture refresh remains Integration Gate E. Ordinary semantic writers still emit their existing carriers/tickets/links; Memory-owned body and consumer migration begins at Step 170.

Documentation impact:
- Inspected: Container clock, raw append/read, scan/recovery, physical historical frontier, owner format and native import binding constraints.
- Updated: storage format, maintainer and coverage maps, envelope contract and execution evidence.
- Not affected: public CLI/API surface, owner semantic clocks/IDs, ordinary publication writer shape or canonical fixture hashes.
- Compliance check: root/CLI format/check, focused/adjacent/CLI tests and both structural/change-impact documentation checks pass for this batch.
- Known documentation gaps: remaining owner cutover, final promotion and compatibility retirement are pending.

Architecture impact:
- Standards added or changed: none.
- Ownership or boundary impact: Container owns framing/allocation provenance only; opaque bodies remain concrete-owner authority. Historical frontier shares that same mechanical observer.
- Pitlord enforcement impact: existing Container/facade areas remain unchanged; refreshed source enforcement remains Gate E.
- Other verification impact: nineteen new physical tests plus 318 unique adjacent cases and 32 CLI tests pass.
- Known architectural gaps: per-owner body admission/proof/copy/writer/converter gates and final source refresh remain pending.

## Memory envelope bodies, replay and history — Steps 170–172 verified

Memory owns the `CVAMC002`, `CVAMD002` and `CVAMT002` envelope bodies. They retain the existing local/revision/identity/causal fields while the outer physical envelope supplies global order. The codec shares concrete Memory value helpers, not an encoded-old-record stripping adapter. Seven independent literal/corruption tests pass, covering external global context, optional clears, empty deltas, malformed coordinates/masks/UTF-8 and unchanged input contracts on rejection.

MemoryOpenState admits these direct bodies through the same creation validation, pure delta application, dense local sequence and terminal machinery. Accepted body backing, temporal evidence, provenance and routing validation remain in owner finish/lifecycle. Unknown physical envelope bodies fail closed while Memory is the only migrated semantic envelope consumer; future owner gates must extend explicit dispatch before their writers cut over. Raw 002 bodies outside a physical envelope reject, and V1 continues to reject physical envelopes.

The initial publication filter passes **80/80**, including actual REL/PHY facade creation reopen and unknown-body rejection. Five focused replay tests pass after adding invalid successor state preservation and explicit raw-body/V1 boundary cases. Reverse-global standalone reservations preserve records and terminal state, global head and exact optional timestamps. Ordinary Memory writes still emit their current marker-bound carriers until Step 177; no premature writer cutover is claimed.

Review exposed an inherited mutation-before-overflow corner in `MemoryStore::insert_rebuilt` and `insert_tombstone_rebuilt`. Both now compute the next local version before changing accepted maps; a dedicated regression snapshots revisions/current/mutation/terminal state at exhaustion. The overflow regression passes. Final adjacent Memory verification passes **278 tests**, zero failures and one existing opt-in ignore (`.test-tmp/memory-envelope-adjacent-repo-network.log`). A localhost HTTP mock failed under restricted networking; its permitted rerun passes, without production changes. All **seven** focused replay/history tests pass (`.test-tmp/memory-envelope-historical-final.log`), including every local cut for marker/envelope REL/PHY fixtures with reversed globals and terminal state, plus every local/global cut and exact timestamp in a real grouped-Insomnia-plus-envelope REL. Six external grouped-publication and fourteen Insomnia completion regressions pass. Root/CLI format/check, **32 CLI tests**, and both documentation checks pass. Gate E remains the whole-root/source refresh checkpoint.

Documentation impact:
- Inspected: Memory carrier/body codecs, rebuild, immutable body/provenance/routing checks, terminal lifecycle, historical fingerprints and grouped publication authority.
- Updated: storage format, maintainer map, documentation coverage and execution evidence.
- Not affected: ordinary Memory writer shape, public API/CLI commands, semantic identity/clocks and canonical source fixtures.
- Compliance check: root/CLI format/check, seven codec and seven replay/history tests, overflow regression, 278 adjacent Memory tests, grouped suites, 32 CLI tests and both documentation modes pass.
- Known documentation gaps: downstream import/proof/copy/writer/converter migration and final promotion/retirement remain pending.

Architecture impact:
- Standards added or changed: none.
- Ownership or boundary impact: Memory owns its body format and reduction; Container supplies physical global context only. Shared value helpers stay under the concrete Memory codec.
- Pitlord enforcement impact: existing Memory/Container areas remain unchanged; source refresh remains Integration Gate E.
- Other verification impact: actual REL/PHY reopen, reverse-global and grouped coexistence, historical cuts and failure atomicity are exercised.
- Known architectural gaps: Steps 173 onward and final integrated architecture enforcement remain pending.


## Memory envelope consumers and ordinary writer — Steps 173–177 verified

Reconciliation reads both marker-bound and envelope publication coordinates, resolves accepted records through MemoryStore and retains source transaction times. Completed-import evidence canonicalizes accepted records through the established comparison encoding, including terminal evidence without broadening the existing unsupported terminal-replay API. Five focused regressions pass, including actual divergent reconciliation and a real completed import containing an envelope delta. Fifty adjacent reconciliation tests pass.

Native prune source-history evidence reconstructs comparison-only marker/carrier facts from each envelope; it does not persist legacy frames or give the physical layer semantic ownership. Exact global watermark parity passes against marker-bound creation/delta/terminal history with reverse globals. Real REL/PHY prune outputs reopen, retain unavailable-body descriptors and reject an otherwise valid mutation-ID change in an accepted creation. The fixture checks the Memory-body count; the preexisting REL destroyed-body-byte counter covers Archive contents rather than Memory payloads.

Reclamation keeps accepted envelope creations and their immutable bodies live, drops inert orphan carriers/bodies, and preserves inline publication bytes after strict supported-body validation. Unknown bodies fail closed pending explicit owner migration. Actual REL/PHY outputs retain revisions, hydrated bodies and optional times while leaving source bytes unchanged. Native REL attachment repack compares every Memory-local history cut and all global transaction times. Semantic rewrite tests honor the current-selection policy, and ownerless PHY semantic migration tests exercise accepted live envelope revisions with source times. Existing legacy owner-ID migration terminal limitations are not broadened by these adapters.

Ordinary Memory creation, revision and terminal publication now each append one allocation envelope. Retry paths return before append/allocation. Grouped Insomnia/external publication remains atomic under its existing owner path. Literal old-carrier and V1 migration fixtures remain explicit; current writer assertions inspect envelope bodies. The legacy V6 grouped-completion fixture adapter restores actual legacy tickets/carriers/markers for direct Memory envelopes before writing a V1 header, rather than reheadering V2 authority.

Verification: the complete root run passes **2,045 tests**, zero failures and five existing ignores across 26 suites, including **1,888 unit tests** and the long native byte-prefix/retry sweeps (`.test-tmp/memory-envelope-runtime-root-tests.log`). All root test/example binaries compile. Root/CLI format and compilation checks, **32 CLI tests** and both documentation modes pass. The preceding consumer publication filter passed 91 tests; the first writer filter exposed an obsolete marker-only comparison helper, and the wider Memory run exposed legacy terminal fixtures still using the ordinary writer. These fixture/assertion issues are corrected and all pass in the full run. New writer tests check every incomplete final-envelope frame prefix, exact earlier state/clock recovery and byte-stable retries. Source architecture refresh remains Integration Gate E.

Documentation impact:
- Inspected: direct and grouped Memory publication, owner replay, reconciliation/import evidence, prune proof, reclamation, native copies, semantic rewrite and owner-ID migration.
- Updated: storage format, maintainer map, documentation coverage and this execution record.
- Not affected: public CLI/API shape, canonical source fixtures, grouped transaction contracts and semantic identity.
- Compliance check: root/CLI format/check, 2,045 root tests, 32 CLI tests and both documentation modes pass.
- Known documentation gaps: offline envelope conversion, subsequent owner framing and final promotion/retirement remain pending.

Architecture impact:
- Standards added or changed: none.
- Ownership or boundary impact: concrete Memory adapters own semantic coordinates and canonical evidence; Container allocates physical envelopes only. No generalized semantic registry was introduced.
- Pitlord enforcement impact: existing owner boundaries remain; source refresh remains Integration Gate E.
- Other verification impact: full root/CLI checks pass for this Memory runtime milestone; Gate E will repeat broad checks after the remaining framing owners.
- Known architectural gaps: offline converter and later owner-specific framing gates remain pending.


## Envelope owner batch — Steps178–199 accepted against `ca60b5c`

The item-specific gates178–199 are accepted; Archive contract Step 200 is accepted against `2597c73`; next is its body codec Step 201. The broader root command is running against the implementation snapshot, with Integration Gate E pending after remaining owner framing. Memory offline conversion replaces accepted direct links with reserved-binding envelopes at their original slots, retains inert backing and grouped publication authority, and rejects legacy `CVAHPR01` rather than inventing replacement proof. The completed-import boundary regression uses a genuine V1 `CVAILEG1` proof whose prefix contains the reserved ticket; migration must rebind that proof into a V2 transport wrapper. V2-only wrappers cannot be used as fabricated V1 inputs.

The optimized authoritative Memory check passed all six pinned files: all owner-local cuts, global frontiers, revision body/routing facts, terminal state, transaction times, source bytes/SHA-256/mtime, and unpublished-stage cleanup. Runtime was 1,178.86 seconds. [Measured results](../benchmarks/memory-envelope-migration-2026-10-20.json) report actual standalone framing separately from grouped/external atomic frames. These results do not claim whole-container savings. Item-specific acceptance is recorded against `ca60b5c`; integrated promotion and retirement remain pending.

Entity envelope codecs, replay, history, reconciliation/import, pruning, reclamation/copies, ordinary writers and offline conversion have passed their item-specific gates; broader integration remains under verification. The authoritative comparison helper now reads direct and merge terminals from envelopes as well as legacy links. The corrected authoritative run passed all six files in 125.07 seconds; [measured results](../benchmarks/entity-envelope-migration-2026-10-20.json) retain accepted source hashes and per-owner byte accounting. Broader integration remains under verification. Rebuild overflow checks now precede semantic index mutation.

Relationship body codecs and explicit consumer adaptations have passed their item-specific gates. Luna contributed adversarial codecs, replay, pruning, reclamation, copy, import-evidence, migration and overflow fixtures. The ordinary Relationship writer cutover is implemented and its four REL/PHY writer/recovery tests pass. The concrete contract and plan distinguish synthetic runtime coverage from the pinned corpus's zero Relationship population.

Documentation impact:
- Inspected: owner publication contracts, retained history, import authority, proof/copy/converter boundaries.
- Updated: envelope contract, storage format, maintainer map, coverage map, limitations, expanded owner-specific plan, measurement and this execution record.
- Not affected: public API/CLI shape and authoritative source files.
- Compliance check: both documentation modes and 32 CLI tests passed earlier in this batch; repeat after final edits.
- Known documentation gaps: Archive/Graph framing and integrated promotion/retirement remain pending.

Architecture impact:
- Standards added or changed: none.
- Ownership or boundary impact: concrete domain codecs interpret bodies; physical Container clock/envelope ownership is unchanged.
- Pitlord enforcement impact: refreshed scan completed with no violations across 23 rules; later source edits require final refresh.
- Other verification impact: latest-source focused/broad verification is still running.
- Known architectural gaps: remaining owner framing, integrated promotion and runtime retirement remain pending.


The [linear plan](minimal-lossless-history-storage-linear-plan-2026-10-19.md) records separate commit/files/verification/blockers for each accepted item. Relationship applicability is measured in [six-source staging evidence](../benchmarks/relationship-envelope-applicability-2026-10-20.json); it establishes zero population, while synthetic tests establish populated runtime/conversion correctness. The full unit sweep passed1,963 tests with5 ignored, exposing one obsolete destination-wire assertion; its corrected focused test passes against identical production code. Full repository verification is running separately.


Archive Step 201 accepted against `24572e4`: 15 body codec tests and 106 Archive-filtered regressions pass. Step 202 replay is in progress and unaccepted. The Windows mock-server accepted-socket correction `a65650d` passes its previously failing controller regression; a clean full-root sweep remains required.


Archive Step 202 accepted against `ed7e21a`: 108 Archive-filtered tests pass, including the four new owner-admission/repeat/terminal cases. Step 203 reader parity is in progress. Ordinary Archive writers and offline conversion remain unchanged until their explicit gates.


Archive consumer gates203–207 accepted against `9e153aa`: 111 Archive, 86 import, 50 reconciliation, 3 principal backfill, 7 pruning and 17 reclamation tests pass. All-family historical/current/time parity and eleven-cut proof parity include a repeated binding. Full root/CLI and architecture integration refresh remain Gate E work. Next is ordinary Archive writer cutover208.


Archive writer gate208 accepted against `2801100`: 113 Archive, 86 import, 50 reconciliation and 30 upgrade tests pass. This also revalidates the additional import Episode-scope consumer exposed by ordinary native writes. Step209 offline conversion and pinned corpus measurement is next; full-root/CLI/architecture integration remains pending Gate E.


Archive gate209 accepted against `ea26adc`: six-source parity passes in 169.26 seconds, all source bytes/hashes/mtimes unchanged. The measured 32,357 Archive publications save 659,536 framed owner bytes; see the tracked Archive envelope migration JSON. This excludes reservations and other owners. The supplemental global-counter preflight and explicit legacy fixture updates pass; 1,988 broad units pass, with five ignored and three long integration sweeps deferred. Next210 freezes the Graph envelope contract.


Graph contract210 accepted against `6946ce4` after six-source inventory and concrete codec/replay/proof/migration review. Per-row origins are preserved; no uniform-origin rule is invented. Next211 implements the current single/batch body codec, while Graph ordinary writers remain unchanged until218.


Graph body codec211 accepted against `b939ff2`: all 11 compact Graph codec tests pass, including five new native tests covering literal bytes, enum matrix, strict framing and reference adaptation. Next212 adds owner admission and concrete REL/PHY routing; current Graph writers remain unchanged.


Graph admission212 accepted against `aea161e`: 105 publication and 117 Graph regressions pass, one existing Graph case ignored. New tests cover both owners, per-row origin batches, mapping/clock/shape failure atomicity, version format guard, repeated-link rejection and local exhaustion. Next213 updates concrete historical/current readers; Graph ordinary writing remains on the prior carrier/link shape until218.


Graph reader gate213 accepted against `e5ec250`: four historical reader tests and 118 Graph regressions pass, with one existing ignored. Populated every-cut parity includes sparse MemoryGraph positions, ordered and one-member batches, per-row origins, mapping/baseline identity, optional time and lifecycle exclusion. Next214 migrates reconciliation/import intervals and normalized evidence.


Graph import gate214 accepted against `bfb20d1`: two native interval tests, 56 reconciliation and 99 Graph regressions pass. Exact order, one-member batch shape, optional times, normalized evidence and duplicate no-op are covered; existing mixed-origin replay rejection stays atomic. Next215 binds native Graph source-history and downstream proof consumers.


Graph consumer gates215–217 accepted against `c0a6b0f`: 107 Graph tests and 2,009 broad units pass, five ignored and three long sweeps deferred to Gate E. Exact native/compact proof parity, transaction authority, accepted Dream/Community evidence after unrelated prune/reopen, both-owner reclamation, repacks, fresh rewrite/follow-up writes and owner-ID migration are covered. Test compilation and formatting pass. Next218 cuts ordinary Graph single/batch writers over; converter219 and integrated coverage220 remain separate gates.

Graph checkpoint verification: CLI formatting/check and all 32 CLI tests pass. Both structural and change-impact documentation policy checks pass on the isolated tracked-source validation snapshot. Root test compilation passes; the remaining three long integration sweeps and refreshed architecture enforcement stay explicit Gate E work.


Graph writer gate218 accepted against `308e725`: 108 Graph checks pass including every incomplete final-frame prefix/reopen/retry, followed by 110 Graph checks and 2,012 broad units with the converter candidate. Five existing ignored and three long Gate E sweeps remain. Ordinal/body planning precedes writes and GraphStore installs mapping/transaction state only after the native allocation envelope succeeds. Next219 measures the staged native Graph converter against six frozen sources.


Graph converter219 accepted against `e1e4d9e`, measured in `0113e78`: six pinned conversions pass in 288.52 seconds, 46,075 publications, unchanged source bytes/hashes/mtimes. Graph native frames save 1,059,725 bytes versus compact carrier/link framing (3,927,132 → 2,867,407); shared reservations, mappings and other owners excluded. Luna audit220 finds no missing selected owner/family/responsibility after this measurement. Integration Gate E remains active: exhaustive recovery, historical-oracle/integration suites and architecture refresh have not all completed. Step221 is not active until that gate passes.


Gate E verification progress: 157 integration tests across 24 passing suites; 32 CLI tests; root doctests; and all three long recovery sweeps pass. The exhaustive checkpoint sweep passes all 1,046 non-vector and 1,484 vector byte prefixes on isolated rerun (242.18 seconds); the initial combined run hit Windows access denied during one retry. Archive legacy fixture assumptions were corrected in `cb95e38`; all ten activity contracts and workspace management pass. Historical fixture oracles and architecture refresh are still pending, so Gate E is not accepted yet.

Historical-oracle Gate E run passes: 192 tests, one existing ignored, 388.44 seconds. The pinned baseline comparison and Memory/Archive/Entity/Relationship/Graph/Ego/Dream/Freshness/vector historical fixture checks pass. All source code is committed; refreshed architecture enforcement is the only remaining Gate E requirement.


Read-only Phase N preparation expanded the unexecuted Dream block from11 to27 rows: distinct Memory-local/global order, direct/Insomnia/external prefix birth/cohort authority, concrete REL coordination, exception-capable codec/replay, source/destination consumers and maintenance transforms are now explicit. The current plan is220/355, with accepted1–220 unchanged and Gate E still pending architecture enforcement. No Phase N implementation crossed that gate.


Gate E architecture refresh passes: snapshot `4cc4320fdda5709a82b7f7a751a7ee3853248b44fd445e468eb6782d7754f694`, zero Arcana compatibility warnings, 20 areas/23 rules, no violations across 76,739 source nodes and519 relationships. Final audit then identified completed-import Graph duplicate-member authority as supplemental Step221 before gate acceptance. Its partial/full-overlap regressions and correction are now explicit; the plan is220/356 and Dream starts222 after Gate E.


Graph supplemental221 accepted against 9ce3476: both completed-epoch overlap regressions fail before and pass after, retaining ordered transaction facts and unchanged inputs through repeat reconciliation. Broad revalidation passes2,014 units with five existing ignored; three exhaustive sweeps already passed and new Dream references are verified separately. The initial sandbox run could not connect to local mock servers; an unrestricted rerun passes. Root test compilation and all23 architecture rules pass. Integration Gate E is accepted with the existing full recovery/integration/CLI/historical-oracle and refreshed architecture evidence. Progress221/356; next222 freezes legacy Dream population semantics.


Dream reference222 accepted against `37210c8`: four runtime Begin population tests pass, including archived/terminal births, reversed local/global orders in both directions and grouped siblings. Contract223 accepted against `24d1c53`; the Memory identity prefix is explicitly distinct from final Dream population authority. Progress223/356; reader224 is in progress.


Memory prefix224 accepted against `21b4ec9` (implementation first captured in `1aaf6cd`):22 Memory compact replay/copy/prune tests and the accepted external publication fixture pass. Body pruning preserves first identity/global metadata in REL and PHY. Physical Begin authority225 is frozen in the Dream population contract, including recoverable pending intents and strict physical-prefix evidence. Progress225/356; direct-birth projection226 is active. Documentation validation now uses a verified independent Git snapshot; an earlier missing nested Git directory resolved to the parent and captured the in-progress224 implementation with acceptance docs, without loss or premature step acceptance.


Direct-birth prefix226 accepted against `ccc0c94`: five focused projection/correlation cases, six Dream journal regressions and four legacy population references pass. REL validates owner and first Memory identity/mutation at the requested local cut; journal evidence retains all consumed mutations so later accepted retries cannot erase an earlier birth. Current source admission and writers remain unchanged; physical Begin scan integration is229. Progress226/356; Insomnia proof/cohort projection227 is active.


Post226 broad library revalidation passes2,024 tests, five existing ignored and only three separately passed exhaustive recovery/long-owner sweeps filtered (19.93 seconds). Both structural and impact documentation checks pass on the isolated validation snapshot.


Insomnia prefix227 accepted against `99e7f04`: three focused current/legacy proof cases and2,027 broad units pass, five existing ignored and three previously verified exhaustive sweeps filtered. Reopen shares the exact extracted owner-validation checks with the prefix reader; no-proof legacy completions remain non-births. Progress227/356; external-source prefix228 is active.


External prefix228 accepted against `ad45319`: populated legacy/current format parity and2,028 broad units pass, five existing ignored and three separately verified exhaustive sweeps filtered. Before REL coordination, discovery requires a separate Container bounded version-aware reader: new229 owns physical-frontier/no-repair correctness, former229 onward shift byone. Progress228/357; coordinator is230. Accepted1–228 retain their identities.


Bounded physical reader229 accepted against `aeb7a43`: three new cases and all17 envelope-clock tests pass, including reverse reserved bindings, grouped ranges, malformed/V1 framing, clipped boundary and a valid prefix before a later incomplete tail. Source bytes and live clocks/times remain unchanged. Progress229/357; concrete REL Begin coordinator230 is active.


REL population coordinator230 accepted against `944231e`: eight focused module cases,2,036 broad units and root test compilation pass. Five existing ignored and three separately verified exhaustive sweeps remain excluded. The held-container bounded query checks exact Memory head and physical birth/cohort presence without body/current-liveness lookup, with reusable prefix state for later replay integration. Primary architecture/maintainer/population docs describe the focused REL module; owner boundaries and policy are unchanged. Progress230/357; digest231 is active.


Population digest231 accepted against `99a9e73`: two canonical hash cases pass with independently regenerated literal vectors using one NUL byte. Wire contract232 accepted against `e79cda6`: CVDREAM2 Begin retains common pass/source/turn/graph authority, with cut+digest or exact explicit set and implicit first revision1. Existing journal kinds remain unchanged; V2 owner integration and writing are later gates. Progress232/357; encoder233 is next.


Current Begin encoder233 accepted against `17844a3`: four tests pass, with independent literal CompactCut/nonempty/empty ExplicitSet bytes and pre-emission invalid-field checks. No file replay or ordinary writer changed. Progress233/357; strict decoder234 is active.


Strict decoder234 accepted against `f810554`: nine codec tests pass, covering independent fixtures, all truncated prefixes, invalid framing and bounded count/length rejection. Progress234/357; physical replay/materialization235 is active.
