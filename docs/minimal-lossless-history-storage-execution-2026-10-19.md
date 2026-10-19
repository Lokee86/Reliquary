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

These sampled determinism checks do not claim exhaustive cross-format parity; exhaustive cut enumeration and baseline artifacts remain Steps 17–19. All six authoritative hashes were re-read afterward and still match the accepted baseline. Opt-in fixture checks use the `history-oracle-fixtures` Cargo feature and add no ignored tests to the ordinary suite.

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

## Fresh baseline observations

The completed root baseline reports **1,515 passed / 5 failed / 5 ignored**, in 419.85 seconds of test execution. Both exhaustive import byte-prefix recovery tests and the long finite-receipt/reopen test pass. Failures were three external-publication tests, one Lore file-lock test and one decision-endpoint test-server socket race. All seven `runtime_host::gateway::external_tests` pass in isolation with `TEMP=TMP=<repository>/.test-tmp`. The initial longer `.test-tmp/lossless-baseline` scratch path exceeds the Win32 rename path limit in frozen external-plan installation; the shorter path retains all scratch inside the repository. The Lore test passes in isolation, matching the previously recorded transient Windows file-lock behavior. The exact decision-endpoint failure passes through Workspace command execution (1/1); a sandboxed direct loopback retry cannot reach its local server, so that restricted retry is not authoritative network-test evidence. No production fix or test weakening was needed. This is a qualified baseline, not an all-green full-suite claim; integration binaries were not reached after the library failure.

Fresh root and CLI `cargo check --locked` pass. CLI tests pass **32/32**. Root/CLI formatting and both documentation-policy modes pass. Subsequent documentation runs must avoid racing scratch-directory removal by active byte-prefix tests; one concurrent structural scan encountered a disappearing scratch directory and was rerun after those tests finished.

## Related docs

- [Specification](minimal-lossless-history-storage-spec-2026-10-19.md)
- [Linear plan](minimal-lossless-history-storage-linear-plan-2026-10-19.md)
- [Storage format](storage-format.md)

## Notes

Authoritative fixtures are read through read-only file handles. Any owner API that can repair on open must operate on a disposable repository-local copy.
