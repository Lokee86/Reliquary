# REL logical activity clock — Phase 1 refactor specification

Parent index: [Documentation index](INDEX.md)

Status: **1.1–1.6 complete, with recorded integration/build verification exceptions**. This document is the historical implementation contract for the accepted-turn clock, retained as the clock prerequisite for the later [unified Memory Freshness design](memory-staleness-plan.md) and independent Ego summary scheduling. The former per-Memory generic review/Perception audit policy has since been superseded by [ADR 0039](decisions/0039-unified-deterministic-memory-freshness.md); this does not change Phase 1's implemented clock semantics.

## Purpose

### Problem statement

Ego refresh and Memory activity staleness require one owner-local, monotonic count of **accepted logical incoming turns**. Today Reliquary has an Archive-version clock, a global/container-version clock, node counts and a live interaction receipt, but none expresses that activity contract. Counting stored turn payloads or nodes in the current physical layout would couple activity to representation; counting Archive versions would also include branches, metadata, files and Episodes. Implementing another counter in Ego would give different consumers competing sources of truth.

Crucially, the repository has **two** turn acceptance paths:

- `Archive::append_node()` publishes `ArchiveRecord::Node` (`CVANODE1/2`), including historical and test/import paths.
- `Archive::ingest_turn()` publishes `ArchiveRecord::IngestedTurn` (`CVATURN1/2`), including the live `InteractionRuntime` path and attachments.

The existing `NodeIndex` already deduplicates logical identities across both. Archive's version-linked record stream already supplies a stable, ordered acceptance journal, including legacy payloads. Use both instead of creating another independently persisted event/counter log or duplicating string-keyed turn identity.

## Overview

### Target architecture

**Canonical owner:** Archive is the sole writer of a REL-local logical activity projection over its committed, version-linked `Node` and `IngestedTurn` publications. `Cva` exposes read-only queries, and the runtime reports the outcome of canonical acceptance. Ego and the future generic staleness evaluator consume the clock; they never increment it.

The projection is disposable in-memory state reconstructed from durable Archive publications. It persists *indirectly through the existing record history*, not in another file format or processing-epoch record. A fresh REL starts at zero; historical RELs acquire the correct derived clock on open without rewriting their files.

The finished implementation has one source-of-truth path:

~~~text
append_node / ingest_turn
        |
existing NodeIndex validates logical identity and payload
        |
Archive publishes one version-linked turn record if novel
        |
Archive activity projection accounts for new eligible identity
        |
Cva read API                    runtime acceptance receipt
        |                                    |
generic source-progress cut       sync before durable acknowledgment
(Phase 2; not implemented here)
~~~

`NodeIndex` remains responsible for identity/payload equality. The activity projection records a **position per unique eligible Node** and a sparse, ordered set of activity changes by Archive version, so it can answer both current and historical cuts without rescanning stored payloads per query. The implementation may use a compact ordinal-aligned vector keyed through `NodeIndex`; do **not** introduce a second `HashMap<(String,String), ...>` duplicating the existing string-keyed identity index unless a measured, justified requirement emerges.

## User / developer outcomes

- Every caller reads the same REL activity count, independent of Ego installation, Perception, clock time, or process restart.
- Existing RELs—including mixed `CVANODE`/`CVATURN` data—open without a mutable migration.
- Exactly one turn is counted per accepted logical eligible identity, despite retries, content deduplication, attachments, or branch topology.
- Callers can locate a turn's first accepted activity position and obtain the total as of an owner-local Archive-version boundary.
- Live runtime receipts state whether a message was newly accepted; the receipt is not inferred from Archive-version arithmetic.
- The next phase can implement generic source-progress staleness without modifying turn-ingest semantics again.

## Implementation decisions

### Activity definition and identity

An eligible turn is a **novel** successfully accepted Archive Node whose exact normalized `role` is `user` or `assistant`. The live `InteractionRole::Agent` already normalizes to `assistant`; do not count bare `agent`, unknown/system/tool roles, internal model reasoning, stream checkpoints, retry events or worker output that has not been accepted as a first-class user/assistant turn. Existing raw Archive role validation remains otherwise unchanged.

One logical identity is `(REL owner, conversation_id, node_id)`. The REL owner is implicit in its Archive. Different identities with identical content each count. Replaying the same identity with compatible immutable Node content/metadata counts zero. Conflicting identical IDs count zero and return the existing error. Ingested-turn attachment comparison retains its stricter current conflict semantics. A cross-format replay of a pre-existing `append_node` identity through **zero-attachment** `ingest_turn` must count zero where its payload is compatible; `append_node` of an existing `ingest_turn` Node must also count zero. Do not weaken attachment conflict rules to make cross-format replay appear compatible.

This is an **accepted-conversation activity** clock, *not* proof that project development took place. Historical source material newly imported into the REL advances its acceptance count but does not by itself qualify as recent reinforcing work for a Memory.

### Publication, durability and ordering

1. The first accepted eligible identity increments a checked `u64` activity position exactly once. Start at `0` on empty Archive and assign `1` to the first eligible turn.
2. Advance in authoritative **Archive record-version publication order across all conversations and branches**, never Node timestamp, conversation depth, canonical branch membership, content hash, global/container version, or processing completion order.
3. No counter advance for identical replay, rejected/conflicting writes, failed validation, unversioned/orphan records, unrelated Archive metadata/branch/fragment/Episode/file operations, or novel noneligible nodes.
4. Preflight the next activity position, ordinal-index capacity and authoritative Node/attachment constraints **before** version publication. Install a live activity tick only after successful publication and accepted Node/attachment insertion. Checked overflow must fail rather than wrap/saturate; never acknowledge a turn that cannot be represented by the activity index.
5. The existing low-level `Cva` API does **not** promise to sync each mutation. `InteractionRuntime::accept_turn` already calls `sync()` and may return a receipt only on success. A sync failure does not warrant a durable-success claim; reopen determines the actual recovered prefix. No new implicit fsync or cross-owner transaction model is introduced.

As a result, Archive versions and activity positions differ: a branch publication may increase the former while the latter remains unchanged. A global-version increment from another semantic owner must not change either REL activity position.

### Data structure and read surface

Suggested internal contract (exact Rust syntax and file names may be refined at implementation):

~~~rust
struct ArchiveActivityIndex {
    // Index-aligned with NodeIndex insertion ordinal; 0 means not eligible.
    first_activity_position_by_node: Vec<u64>,
    // Sparse transitions only when an eligible logical turn first publishes.
    activity_changes: Vec<(u64, u64)>, // (archive_version, activity_count)
    current_count: u64,
}

struct TurnAcceptance<T> {
    value: T,
    inserted: bool,                   // true for a novel Node, even if noneligible
    activity_position: Option<u64>,  // FIRST position if eligible, including replay
    rel_turn_count: u64,             // current clock as of this accepted operation
}
~~~

Extend `NodeIndex` with one package-local ordinal lookup, or an equivalent way to obtain the insertion slot already held by `DenseLookup`. Do not introduce a new public identity registry. The activity index stores no turn bodies. For each newly accepted Node, append either its nonzero assigned position (eligible) or `0` (noneligible). This alignment must hold on live insert *and* Archive rebuild. A duplicate insertion never changes the original slot or position.

Public `Cva` read methods:

~~~rust
fn rel_turn_count(&self) -> u64;
fn activity_position_for_turn(
    &self, conversation_id: &str, node_id: &str
) -> Option<u64>;
fn activity_cut_at_archive_version(
    &self, archive_version: u64
) -> Result<u64, ArchiveError>;
~~~

`activity_position_for_turn` returns the **original** position of an eligible turn, including a repeated delivery; it returns `None` for missing or noneligible Nodes. `activity_cut_at_archive_version(0)` is `0`; versions above the current Archive head error. At any existing non-turn Archive version it returns the preceding activity count. Binary search over `activity_changes` is sufficient; no per-version dense cut table is needed. Positions are REL-local and only meaningful in their associated Archive history.

Both internal acceptance paths return the internal typed result. Preserve existing public `Cva::append_node*() -> Node` and `Cva::ingest_turn() -> IngestedTurn` results by extracting `value`, rather than spreading a transitional receipt API across hundreds of callers. Provide a focused internal Cva ingest-with-receipt method for `InteractionRuntime`. Extend `InteractionReceipt` with `inserted`, `activity_position` and `rel_turn_count` while retaining `archive_version`. The new receipt must reflect the canonical result, not a before/after version comparison.

### Open/recovery and format ownership

`ArchiveOpenState::ingest_version()` already consumes the verified record-version stream, applies newly referenced records once, and ignores repeated pointers to the same published backing record. Extend `apply_record` to propagate the **NodeIndex insertion result** for `Node` and `IngestedTurn` records to the activity projection with the assigned `archive_version`.

- Count only records actually reached through a validated version link. An orphan `CVATURN`/`CVANODE` payload is inert even if its bytes survive physical recovery.
- Existing `CVANODE1/2` and `CVATURN1/2` are decoded by current Archive code. Do **not** introduce another on-disk turn format, persistent counter payload, or dedicated data migration.
- A repeated Archive version reference to previously applied backing data changes neither the count nor the identity's first position; querying its Archive-version cut still returns the current count.
- After reopening a mixed-format REL, the derived projection must exactly match a one-pass reference count of **first successfully applied eligible identities** in Archive publication order. The current `stats().nodes` and `archive_version()` are not a substitute.

At an invalid/conflicting record, follow the existing Archive-open error contract; do not silently skip corruption just to reconstruct a count. Physical repacking must preserve the record-version acceptance order.

### Physical repack, migration and reconciliation

**Identity of a physical repack:** Principal backfill, project-backed attachment repack and storage reclamation preserve the same semantic Archive-version sequence. Compare the owner ID, total activity count, per-turn first positions, and historical cuts before/after. Add these checks to existing post-reopen semantic-equality gates, with targeted fixture tests.

**Fresh migration:** `migration_rel.rs` reconstructs an output by replaying Archive records; its new container can have different Archive/global version numbers and may omit redundant no-op replay publications. Require same *unique eligible identity set*, total activity count and relative first-acceptance ordering. Numerical `activity_cut_at_archive_version` equality requires a proven one-to-one mapping of source and destination version histories; otherwise it is **not** a migration invariant. Existing future persisted staleness baselines referencing source version/position must not be copied into a nonidentical history without explicit remapping.

**Append-only reconciliation:** `Cva::reconcile` copies whichever side contains the other as a prefix and validates the result; the copied side must preserve all activity cuts and positions exactly.

**Divergent reconciliation:** `cva_reconcile_repack.rs` replays all left Archive history and the novel right tail, typically into new version numbers. Count each destination-accepted eligible identity once, retain the destination's actual first-acceptance order, and refuse conflicting duplicate identities as current code does. Never sum left and right activity totals or treat their original position numbers as interchangeable. A right-side duplicate compatible with left produces no new activity tick. Reconciliation can reorder/rebase history; any future outstanding source-cut dependency must be mapped explicitly or invalidated/fail closed, not relabeled as the same absolute counter.

The existing conversation-compaction store is a separately retained text-summary representation; replacing those records must not change the version-linked turn acceptance journal. Future *actual truncation of Archive turn history* requires its own durable activity checkpoint plus identity/cut preservation design and is excluded here.

### Tests and files that define the contract

Real source change neighborhood:

| Responsibility | Current source / test surface |
| --- | --- |
| Legacy Node acceptance | `src/archive/archive.rs`, `src/archive/archive_record_index.rs`, `src/archive/history_tests.rs` |
| CVATURN acceptance and duplicate checks | `src/archive/turn_ingest_store.rs`, `src/archive/turn_ingest_tests.rs` |
| Versioned publication/rebuild | `src/archive/archive_history.rs`, `src/archive/archive_rebuild.rs`, `src/archive/archive_history_codec.rs` |
| Public facade | `src/facade/cva.rs`, `src/facade/cva_turn_ingest.rs` |
| Durable live receipt | `src/runtime_inference/interaction_runtime.rs`, `src/runtime_inference/interaction_runtime_tests.rs` |
| Migration/reconciliation | `src/facade/migration_rel.rs`, `src/facade/cva_reconcile_archive.rs`, `src/facade/cva_reconcile_repack.rs`, merge tests |
| Physical preservation | `src/facade/cva_principal_repack.rs`, `src/facade/cva_repack.rs`, `src/facade/storage_reclamation.rs`, existing repack tests |

Arcana was unavailable for this repository at specification time (missing `.arcana/CURRENT`); this neighborhood was verified through direct source, existing callers, tests and documented ownership. Do not block the implementation on a new Lexicon/Arcana scan.

## Migration strategy

**Scoped, compatibility-preserving external boundary with an internal hard cut.**

- Existing `CVANODE1/2`, `CVATURN1/2`, Archive version records, and persisted user REL files are real compatibility obligations. Continue decoding them in the one existing Archive rebuild path.
- Existing public Cva return types have many live call sites; keep those external method contracts stable and route their internals directly through a **single canonical Archive activity implementation**. This is not a duplicate compatibility owner.
- The internal acceptance/refactor path can enter a short, explicit red migration window while both Archive insertion functions and the runtime receipt are migrated together. Do not add a shadow counter in Cva/Ego, temporary dual-write activity records, a forwarding service, or obsolete old-shape internal aliases.
- Close the migration window only after live and reopen paths call the same accounting logic and all externally supported persisted formats still open.

### Implementation sequence and local gates

**1.1 — Contract tests / reference ledger**

Build a table-driven reference ledger representing eligible unique logical IDs in version-linked acceptance order. Add tests for the behaviours below using the final intended Cva read interface. The tests may be **expected red (including a missing-API compile failure)** until 1.2/1.3: this is an explicitly scoped internal migration window, not a reason to add temporary ignored tests or disposable shims. Independently retain existing green behavioural characterization tests for Node/attachment duplicate rules.

1.1 implementation: `tests/rel_activity_clock_contract.rs` defines an explicit reference ledger and contracts for live acceptance, identity/replay, roles, branch/conversation order, attachments, rejected writes, empty/independent RELs, historical cuts, reopen, mixed legacy/current records, orphan payloads, repeated version pointers and unrelated global publications. These tests intentionally depend on the final missing Cva read APIs; no ignored tests or production shims were added. Runtime/checkpoint contracts and topology-specific preservation tests remain assigned to 1.4/1.5. Existing library characterization: 744 passed, one unrelated Lore historical-read test failed on Windows file locking (OS error 32).

1.1 verification (2026-10-02): `cargo fmt --check` passed. `cargo check --test rel_activity_clock_contract` reached the target and reported only E0599 errors for the three intentionally missing read methods. Both documentation checks reported the same five existing structure/index findings in `insomnia-explicit-memory-commit-plan.md`; no activity-clock documentation findings. The earlier integration code-generation attempt timed out while compiling dependencies, then verification switched to the compile-only gate. File, Fragment and Episode publications also have explicit predecessor-cut coverage. No production API, storage format or writer ownership changed.

**1.2 — One canonical live projection**

Implement ordinal `NodeIndex` lookup and the Archive activity projection. Change both live turn acceptance paths in the same step, with shared eligibility/accounting and overflow preflight. The projection is advanced after successful version publication and accepted Node/attachment insertion, never on retry or unrelated Archive mutation. Implement the current-count and per-turn-position read surfaces.

1.2 implementation: `archive_activity.rs` owns the ordinal projection, shared checked preflight and canonical typed acceptance. Both live paths use it; NodeIndex supplies ordinal identity lookup and reserves its records/dense lookup before publication. Cva preserves existing return types and adds current-count/first-position reads. Six focused tests exercise live semantics, receipt values, checked overflow before any write, rejected attachment drift, failed publication, dense growth and continued acceptance after reopen. Basic version-linked rebuild wiring was included as a necessary dependency so persisted Nodes and the new ordinal vector cannot diverge on subsequent live writes; this does not close 1.3's historical-query/reference-ledger verification gate. No new storage format or runtime receipt API was introduced.

1.2 verification (2026-10-02): library `cargo check`, root formatting and CLI formatting passed. `cargo test --lib` passed all 751 tests, including the six new activity tests (zero ignored). The reference-ledger target reports only three E0599 errors for the historical-cut query pending 1.3. Production builds currently warn that the three internal receipt fields are only read by tests; runtime consumption follows in 1.4. Both documentation checks retain the same five existing findings in the Insomnia explicit-commit plan. Separate CLI locked check/test fail in `lore-transport` on missing Quinn `max_rtt`/`is_crypto` methods: the CLI manifest lacks the root's vendored `quinn-proto` patch, an existing independent packaging gap. The required architecture refresh was attempted with a bounded 90-second limit and timed out during Lexicon Rust/Python analysis before Pitlord checking. Direct source/caller review verified the shared Archive writer; semantic policy enforcement remains unverified. No further scan was required to complete 1.2.

**1.3 — Validated reopen and historical queries**

Attach the projection to `ArchiveOpenState` and apply only version-linked turn publications. Implement historical cut queries; compare live and reopened projections on mixed legacy/current formats, orphan payloads, repeated version links and interleaved metadata/other-owner global versions. Keep only one reconstruction implementation, reusing the live index's registration logic.

1.3 implementation: Archive and Cva expose historical cuts via binary predecessor search over sparse activity changes; version zero returns zero and versions above head return `InvalidArchiveRecordVersion`. Validated rebuild reuses shared registration and propagates actual Node insertion outcomes for both record forms. The reference ledger covers legacy/current formats, orphan payloads, repeated backing links, unrelated global versions and non-turn publications. Additional recovery cases cover compatible identities in distinct cross-format backing records, conflicting identities, malformed version links, a zero-activity prefix and byte-for-byte nonmutating open/read. No persisted format changed.

1.3 verification (2026-10-02): all ten reference-ledger integration tests and all 751 library tests passed, zero ignored; `cargo check` and `cargo fmt --check` passed. An initial default `cargo test` attempt encountered missing crate artifacts and rustc internal errors in existing Lore integration. A serial retry passed the library/focused targets but full `cargo test -j 1` stopped at the existing `archive_open_profile` example's global allocator conflicting with `lore_base`; the full default gate is therefore not green. Both documentation checks reported only the same five pre-existing Insomnia-plan findings. The separate CLI Quinn patch gap and bounded architecture-refresh timeout recorded in 1.2 remain unresolved; those unchanged failing gates were not repeated. Direct source review confirms one Archive projection/rebuild owner. Internal receipt-field and native linker warnings remain; runtime receipt consumption follows in 1.4.

**1.4 — One runtime receipt**

Expose the internal acceptance receipt at Cva's canonical ingest seam; wire `InteractionRuntime::accept_turn` to report `inserted`, first `activity_position` and current `rel_turn_count` after existing `sync()`. Ensure resumed/checkpointed streams never count until their logical user/assistant turn actually enters Archive.

1.4 implementation: Cva's package-local `ingest_turn_with_receipt` forwards Archive's typed acceptance to runtime while preserving public `ingest_turn -> IngestedTurn`. `InteractionReceipt` now includes `inserted`, original `activity_position` and current `rel_turn_count`, constructed only after existing sync succeeds. Runtime never infers novelty from versions. Tests cover imported Node/live replay, later activity and metadata, reopen, rejected conflicts, Agent normalization, checkpoint/interrupted/resumed streams and durable receipt retention when later scheduling fails. No new persisted state or durability boundary was introduced.

1.4 verification (2026-10-02): `cargo test --lib -j 1` passed all 752 tests and the reference-ledger target passed all ten contracts, zero ignored. `cargo check`, root formatting and CLI formatting passed. Production check no longer warns about unused acceptance fields. Full `cargo test -j 1` remains blocked by the existing `archive_open_profile`/`lore_base` global allocator conflict. Locked CLI check failed on the existing missing Quinn `max_rtt`/`is_crypto` methods; the CLI test build emitted the same errors and was cancelled after that blocker was established. Both documentation checks retain only the five existing Insomnia-plan findings. Direct source/caller review confirms streamed completion uses the same accept/sync seam; no secondary activity writer or new persisted format exists. The prior bounded architecture refresh remains unverified after its timeout; no new scan was required for this consumer wiring. Native linker warnings remain. Preservation and fixture/measurement gates remain in 1.5/1.6.

**1.5 — Repack/reconcile/migration gates**

Add exact-cut preservation assertions for physical repacks, identity/count/order assertions for fresh migration and divergent replays, exact-cut assertions for copy-only reconciliation, and a negative case for incompatible duplicate IDs. No new on-disk formats, history checkpoint records or cross-owner inference work.

1.5 implementation: Archive supplies package-local comparisons over its canonical projection. Physical repacks and copy-only reconciliation validate exact positions and sparse historical transitions after reopening; fresh migration validates eligible identity/order preservation, and divergent reconciliation validates left-first identity union ordering and recovered output history. Seven topology tests cover physical copies, both copy directions, compatible/conflicting divergence, semantic no-op fallback, rebased migration versions and negative preservation comparisons.

1.5 verification (2026-10-02): all 759 library tests passed, zero ignored, including seven new topology contracts. Formatting passed. An initial build timed out during dependency compilation; subsequent verification completed after contention on a shared Cargo target directory cleared. `cargo check`, `cargo fmt --check` and all ten reference-ledger integration contracts passed. Full `cargo test -j 1` and an isolated example-check attempt were cancelled after triggering another dependency rebuild; they produced no new full-gate result. The previously established `archive_open_profile`/`lore_base` allocator conflict remains an unresolved integration gate, as does the separate CLI Quinn patch gap. Both documentation checks reported the same five existing Insomnia-plan findings. No public API or persisted format changed; Archive remains the comparison/accounting owner. No standards or Pitlord policy changed; the prior bounded architecture-refresh timeout remains unverified, and direct source review confirms existing facade ownership boundaries. Native linker warnings remain.

**1.6 — Integration and measurement**

Run focused Archive, runtime, migration, reconciliation and repack suites, then `cargo fmt --check`, `cargo check`, `cargo test`, and the repository's documentation checker. Inspect the frozen 14-/28-day/Ellis REL fixtures **read-only if locally accessible**. The previously recorded 28-day fixture has approximately **16,465 ingested logical turns**, but its exact eligible clock value must be established by the version-linked *mixed-format* ledger and roles, not assumed from a prior CVATURN-only count. Record reopen index construction cost, live-acceptance overhead and compact index memory footprint before adding any cache.

1.6 measurement (2026-10-02): three locally accessible authoritative RELs were inspected through hash-verified disposable copies because Cva::open requests write access and the Ellis original is read-only. Originals and copies retained identical SHA-256 bytes before/after inspection. The independent ledger follows validated Archive version links, decodes Node and IngestedTurn forms, deduplicates logical identities, applies exact user/assistant eligibility, checks every historical cut and first position, and compares rebuilt activity vectors exactly. No persisted counter, cache or format was added.

Windows x64, unoptimized test profile, isolated Cargo target, incremental compilation disabled. Reopen values are warm-cache three-run medians (hashing/copying precedes open); replay values are nine-run medians with alternating baseline/indexed order. Node cloning/destruction, payload decoding, file I/O and sync are outside replay timing. The signed median replay difference estimates in-memory live accounting overhead, not end-to-end ingestion latency. Results are observations, not timing thresholds or release-performance claims.

| Fixture | Eligible turns / nodes | Archive head | Warm reopen median (ms) | NodeIndex replay (ms) | NodeIndex + activity replay (ms) | Accounting delta (ns/node) | Used vector bytes | Capacity bytes |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 14-day | 5857 / 5857 | 7444 | 694.570 | 14.446 | 15.018 | 97 | 140568 | 196608 |
| 28-day | 16465 / 16465 | 20935 | 2123.056 | 39.139 | 45.382 | 379 | 395160 | 786432 |
| Ellis | 2972 / 2972 | 3763 | 513.782 | 5.767 | 6.204 | 147 | 71328 | 98304 |

The activity struct adds 56 bytes; vector figures exclude allocator rounding and other Archive/NodeIndex state. All three corpora contain only user/assistant Nodes. The 14-day fixture has 2,808 user and 3,049 assistant identities, with CVATURN2/CVATURN1 respectively; the 28-day fixture has 7,244 user and 9,221 assistant identities in CVATURN1; Ellis has 1,486 of each, with CVATURN2/CVATURN1 respectively. The formerly approximate 28-day count is independently verified as exactly 16,465 eligible identities. Synthetic contracts retain CVANODE1/2, noneligible-role, orphan, conflict and repeated-pointer coverage.

| Authoritative fixture under `reliquary-fixtures/local/authoritative` | SHA-256 |
| --- | --- |
| `chatgpt-first14d/project.prj.rel` | `49751a62dd8644800d19976a0696a69366dc1e4f8d7c39bb4b69fbd7a8a5d18b` |
| `chatgpt-first28d/project.prj.rel` | `8f4a2bf871d7f5368edde93954a2260a29c86131b28475b323a7b6c70f8c51e4` |
| `rhelm-david-r-ellis-pre-relationship/project.prj.rel` | `3efd9c666aa992686abd8aac9b77cfc7dc2a0e15d6fb26ec01f8666db175e476` |

1.6 verification (2026-10-02): all 760 library tests passed, zero ignored, including Archive/runtime/migration/reconciliation/repack suites and the default bounded measurement fixture. All ten reference-ledger integration contracts and all three authoritative fixture measurements passed; root formatting passed. Initial production check failed with missing `parking_lot` metadata in Lore. After interruption cleared the queued job records, a bounded production-check retry failed on a missing cached `syn` artifact in `windows-interface`, before reaching Reliquary. A three-minute full-test retry timed out while rebuilding dependencies and yielded no new full-gate result; the previously established `archive_open_profile`/`lore_base` allocator conflict remains unresolved. CLI Quinn packaging and the prior architecture-refresh timeout remain separate recorded gaps. No standards, Pitlord policy, public API or persisted format changed in 1.6. Archive remains the activity owner. Both structural and change-impact documentation checks report only the same five pre-existing findings in `docs/insomnia-explicit-memory-commit-plan.md`: missing Purpose, Overview, Related docs and Notes sections, and absent documentation-index linkage. No activity-clock documentation findings were reported.

### Required test matrix

| Fixture | Expected activity / cut behavior |
| --- | --- |
| New REL | 0; cut at Archive version 0 = 0 |
| New user, new assistant | +1 each; positions 1 then 2 |
| Two distinct IDs with identical contents or timestamps | +1 for each |
| Same compatible ID through either acceptance API | 0; original first position returned |
| Conflicting Node or attachment replay | existing error; 0; no new position |
| Novel system/tool Node and streaming checkpoint | 0; accepted system Node has no activity position |
| New conversation / sibling branch in same REL | positions share one REL-wide sequence |
| Branch, Episode, file, memory/global changes | no activity advance; non-turn Archive cuts use predecessor |
| Raw unversioned/orphan turn payload | 0 after reopen |
| Legacy Node + current IngestedTurn | both eligible once; cross-format duplicate does not recount |
| Repeated Archive version pointer to same backing record | no second tick |
| Reopen after accepted writes | same current count, per-turn position and historical cuts |
| Principal / attachment / reclamation repack | exact preservation of clock and all logical cuts |
| Full migration | same unique eligible turns and relative acceptance order; map historical version cuts only if bijective |
| Append-only reconcile | copied-side positions and cuts unchanged |
| Divergent reconcile with duplicate and novel IDs | count union once in actual destination order; conflicting IDs reject |
| Unrelated REL activity / wall-clock passage | existing REL activity stays unchanged |
| Imported old turns | advance accepted activity but do not imply fresh supporting work |

## Out of scope

Generic staleness policies and checkpoint persistence; Memory freshness/summary priority; the Perception stale-Memory audit; Chronos event-time interpretation; Ego scheduling or summary generation; Episode-barrier/global processing settlement; changing Archive or Container storage formats; future history-pruning checkpoints; cross-owner activity merging as one clock; broad import redesign.

## Completion gates

1. **Single owner:** exactly one Archive-side eligibility/accounting mechanism serves both Node and IngestedTurn acceptance, live writes and validated reopen. No Ego/Cva/runtime-owned shadow counter or duplicated string-key index.
2. **Correct semantics:** all acceptance/duplicate/failure, mixed-role, mixed-conversation/branch and cross-format test rows pass.
3. **Historical correctness:** version-linked publications alone define reconstructed activity; every exposed per-turn position and historical cut agrees before and after reopen.
4. **Compatibility:** existing persisted legacy/current Archive formats still open without a mutating migration; public existing Cva result types remain unchanged; runtime has one richer acknowledged receipt.
5. **Topology:** physical repacks preserve exact activity history; fresh migration and divergent reconciliation preserve the correct logical identity/order semantics, without assuming portable absolute cuts.
6. **No incidental behaviour:** no new processing-epoch system, per-Memory inference, staleness threshold, Episode scheduling or immediate Ego execution has been introduced.
7. **Integration:** complete targeted test matrix, whole-repository Rust verification, documentation verification and a bounded historical fixture check (subject to fixture availability). Document any pre-existing unrelated failures rather than misreporting a clean run.
8. **Ownership check:** direct source/caller inspection confirms no residual secondary writer. When a current Arcana snapshot becomes available, it may supplement this check but never replace source and test verification.

## Related docs

- [Activity staleness and Memory revalidation](memory-staleness-plan.md)
- [Ego Memory-Web summary](ego-web-synthesis-plan.md)
- [Perception subsystem plan](perception-subsystem-plan.md)
- [Storage format](storage-format.md)
- [Roadmap](roadmap.md)

## Notes

Source-progress and source event-time are separate axes. The REL-wide activity cut available at a given Archive version is not automatically the cut for an `EpisodeId`: Episodes identify per-conversation slices and can be materialized after other conversations have advanced. Later readiness work must explicitly define which Archive acceptance boundary its global prerequisite settlement covers; Phase 1 exposes the primitive without modifying Episode records.

