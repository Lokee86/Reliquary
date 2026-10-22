# Dependency Convergence — Phase 1: Physical Storage and Owner-Store Foundation

Date: 2026-10-21
Status: Implemented and accepted locally on Windows; see the linked verification record for evidence and platform limits
Parent: [Dependency Convergence Architecture Refactor](dependency-convergence-architecture-refactor-2026-10-21.md)
Predecessor: Phase 0 graph/enforcement baseline; successor: Phase 2 owner-local semantic boundaries

## 1. Problem and verified baseline

Phase 1 extracts an independently understandable physical persistence boundary without moving semantic authority into Container or prematurely implementing the Phase 2 owner separations.

The following is the pre-implementation source baseline captured on 2026-10-21,
not the current working-tree behavior. The implementation evidence and final
gate status are tracked in the [verification record](dependency-convergence-phase1-verification-2026-10-22.md).

- `src/container/container.rs` owns physical framing, identity headers, raw object coordinates, append/read, chunk split/merge, physical truncate, and global/transaction-time bookkeeping.
- `src/container/container_scan.rs` has `open_read_only_for_scan` and `open_scanned`. The latter scans semantic-opaque bytes through a visitor **but reopens writable and truncates an incomplete tail** via `recover_truncated_tail`. Consequently ordinary `Container::open` and `Cva::open` may change bytes before owner replay/semantic validation completes.
- `Container` contains `write_admission_closed` / `owner_write_scope`; `cva_lifecycle.rs` sets the former and `cva_historical_import.rs` toggles the latter. These flags are *in-process admission policy*, not an exclusion guarantee against a second writer or process.
- `cva_lifecycle.rs` and `phylactery_lifecycle.rs` coordinate owner ingest inside `Container::open_scanned` visitor closures. These are **higher-level composition callbacks invoked by the physical scanner**: even without a Rust import from Container to semantic types, they invert execution control and must become caller-driven frame consumption. Owner replay orchestration may remain in the existing `Cva`/`Phylactery` lifecycle location during Phase 1; moving all stores is Phase 2/6 work.
- Ordinary `Cva::open` currently calls `conversation_compactions.recover(&mut container)` (stale compaction reclamation, physical chunk writes/merge/tail truncate) and `entity_resolutions.repair_retired_entity_references(...)` (appends a semantic revision). `Phylactery::open` also calls the Entity-resolution repair. These **semantic/physical mutations are independent of the incomplete-tail truncation** and must be audited separately. Opening and inspecting cannot implicitly authorize them.
- `src/rewrite/file.rs` implements transformed-copy workflows that reread/revalidate source bytes, create and verify a distinct destination, then publish it with `std::fs::rename`. Such source-reading and destination-publishing operations must not be mistaken for in-place source repairs; the lease/snapshot design must cover both without introducing needless writer contention.
- Container already calls `crate::container_version::observe_version_payload_state`. Audit whether this module is physically pure before moving it; relocation is justified by semantics, not file names.
- Existing read-only bounded visitor APIs and publication-envelope tests provide real protecting seams; do not replace them with an elaborate generic storage provider.
- `docs/storage-format.md` currently describes incomplete-tail recovery on ordinary open. That statement must be corrected after cutover, along with callers/tests that depend on it.

**Updated prerequisite (2026-10-21):** The original source-inspection evidence gap is now closed by the [Phase 0 source graph and reviewed ledger](dependency-convergence-phase0-2026-10-21.md). Phase 1 must take its exact source-edge inventory from that retained capture, and use the [accepted logical target DAG](dependency-convergence-target-dag-2026-10-21.md) as the architectural destination. The coarse 19-area snapshot and 34-component logical graph are intentionally different resolutions; do not assume a current path already has one final owner or reset the capture to clear debt.

## 2. Scope and deliverable

Produce one canonical lower boundary for header/framing/version-clock inspection, **caller-driven bounded frame iteration**, physical snapshot reads, exclusive write admission and explicit repair. Owner-store replay consumes the physical reader from above; Container never calls owner/semantic visitor closures. The existing facade lifecycle may still assemble owners at the top pending Phases 2/6. **Ordinary `Container::open`, `Cva::open` and `Phylactery::open` must not repair, reclaim, rewrite, append or otherwise mutate persistent bytes as an implicit side effect.** Phase 1 makes the on-open paths explicit, while later phases retain ownership of complex semantic normalization and maintenance rules.

This phase **does not** move the historical/import/reconciliation coordinators, rewrite the settlement, redesign compact-history data, move all owner logic, or complete public-facade convergence. Preserve V2 physical encoding, ObjectRef semantics, envelope reservations, owner-local interpretation, and current semantic behavior.

## 3. Target architecture and dependency law

```text
Cva / Phylactery owner lifecycle (higher-level orchestration)
          │  owns the replay loop, validates semantic evidence
          ├──────────────────→ owner-local replay states
          │
          └──── pulls opaque frames, clocks ───→ Container reader
                                                │
                                                v
                                            filesystem / OS

Explicit authorized repair / writable owner session ──→ Container writer
```

Dependencies are consumer → dependency, with **no reverse import or execution callback**. This is the Phase 1 portion of the accepted [34-component DAG](dependency-convergence-target-dag-2026-10-21.md), not a mandate to extract the entire future `owner-lifecycle` component now.

- **Physical authority:** file identity/header, physical frame boundaries, ObjectRefs, framed envelopes, physical global reservations, timestamp observation, byte allocation, physical sync, physical corruption reporting, exclusive writer access and explicitly authorized tail repair.
- **Semantic owner authority:** whether a frame is an accepted Memory/Archive/Entity/other publication, semantic object/revision reachability, proof verification, orphan retention, mutation eligibility, import epochs, owner-local reconstruction, and semantic corruption classification.
- **Composition authority:** order in which owner replay states are fed, admission closure for an active import, semantic reconciliation, lifecycle and publication orchestration. A coordinating caller may obtain a physical writer token; it cannot waive OS-level writer exclusivity.
- **Read/open/repair separation:** inspect, validate, scan, probe, history reads and ordinary open/replay cannot change bytes. A normal writable owner open may acquire writer authority and validate/reconstruct state, **but obtaining that authority is not consent to perform semantic cleanup or physical repair**. Mutating maintenance requires an explicitly selected and authorized operation, with validated evidence.
- **No callback inversion:** the higher-level lifecycle **pulls** successive opaque frames/physical coordinates/global-clock evidence from Container. Container must not invoke a closure that performs owner ingestion, calls a facade, mutates owner stores, or schedules runtime work. Preserve bounded streaming and consistent scan-order/clock semantics without an all-record buffer.
- **Independent access capabilities:** distinguish read-only stable inspection/snapshot, writable owner session, and explicit repair/maintenance publication; none may be silently upgraded into another. No Container import of owner stores, `Cva`, RuntimeHost or model/runtime components.
- Keep one concrete Container implementation. Introduce traits only if an actual external/substitutable boundary is demonstrated; don't manufacture `StoragePort`/`Repository` wrapper stacks.

## 4. Decisions

### D1 — One canonical physical scanning engine

Unify framing, boundary checking, version-clock observation, and incomplete-tail classification behind one **read-only** scan implementation. Return a physical scan outcome including the accepted complete-prefix endpoint, observed clock state, and whether a truncated/incomplete tail was found. Do not silently classify complete-but-invalid records as incomplete recoverable tails. Preserve all existing timestamp, deferred-reservation, grouped-publication, and overflow rules.

Expose a **pull-based bounded reader** (iterator/cursor/`next_frame`-style, precise API chosen in P1.1) returning opaque frame bytes/borrowed views, ObjectRef or physical positions, and the same accepted global-clock/frontier evidence previously supplied by `open_scanned`. The caller owns the loop and routes each frame into owner `ingest`/`finish` methods; Container neither invokes semantic closures nor holds borrowed payloads across advancing frames. Define streaming error/lifetime semantics, stable prefix/EOF handling and clock observation order explicitly. Reuse the same physical parser for bounded-prefix historical reads and normal open; do not build an O(all records) index or an extra replay framework.

### D2 — Explicit writable open and recovery

Separate opening for inspection from acquiring a writable session. The writable path must first establish exclusion, then inspect the file and report any incomplete tail; it must not silently repair. A deliberately requested recovery step may truncate **only** a proven incomplete physical tail, after validating the accepted physical prefix and relevant owner admission policy. Following repair, sync, reopen/replay and verify accepted state. A complete malformed publication remains a hard failure, not an invitation to truncate.

For callers that previously relied on implicit recovery, migrate the *actual* recovery workflow to an explicit `inspect → classify → authorize → repair → reopen` sequence at the owning coordinator. Normal open must report a typed **repair-required** result when a file cannot be safely accepted as-is; it must not silently retry with write access, repair the physical tail, or append a semantic normalization record. Do not make a generic `open(auto_repair=true)` convenience route.

**Known non-tail open-time mutations:** `conversation_compactions.recover` frees/coalesces/reclaims obsolete chunks and `entity_resolutions.repair_retired_entity_references` appends canonicalized statuses in REL/PHY. Inventory all other post-replay mutators. Phase 1 must make these paths non-implicit: validate and expose needed corrective action or route through an **explicitly authorized** existing owner/maintenance entrypoint. Preserve existing semantic safety; if acceptance requires correction before a writable session can be used, fail closed with actionable typed evidence rather than accepting an inconsistent owner. Detailed cross-owner repair policy remains in Phases 2/3. Do not simply delete accepted recovery behavior or hide it in writable open.

### D3 — Writer exclusivity distinct from semantic admission

Acquire and hold an exclusive **per-file/identity** OS-backed writer lease for the lifetime of any writable Container session; acquire it **before** writable inspection/admission and every mutation path. A second writer must fail without changing data. Do not treat an in-memory import admission flag or path string as a lock. Reads are allowed concurrently only with a proven **stable snapshot / generation-pinned read**; otherwise acquire a coherent read lease or fail explicitly. A mutable file descriptor is never handed to a read-only capability.

**Three different cases require separate authority:** (1) mounted/ordinary **writable owner session**: hold the exclusive writer lease while open, with no implicit cleanup; (2) **read-only source inspection and transformed-copy planning**: acquire stable evidence for source bytes/head/identity/generation without demanding an unnecessary exclusive writer lease; revalidate after emission; (3) **destination creation/publication or in-place repair**: guard exact destination identity/path against aliasing, replacement and concurrent writers, verify publication preconditions, and hold appropriate exclusion across the final rename/atomic replacement. A source used by maintenance may itself be mounted; select a coherent snapshot or quiesce/deny, rather than racing live writes. Do not lock an unrelated source for writing merely because a distinct destination is being created. The implementation must state the OS-specific handle/lock lifecycle and what races each mode refuses.

The existing `write_admission_closed` / `owner_write_scope` logic is an **owner policy gate**, not the physical lock. Preserve its import-boundary behavior in the correct upper layer; don't let a caller-controlled boolean bypass physical exclusivity. The physical write API must not be callable using a read-only session.

**Before implementation**, record the concrete Windows and supported Unix lock primitive, file-ID/inode/handle identity, hard-link/symlink/rename alias strategy, lease release on drop/crash, replacement and parent-directory publication semantics, lock ordering, unsupported-platform failure, and interaction with held Runtime mounts. No unchecked inspect→write upgrade, lock lost across replacement, or non-atomic destination check-then-rename may be treated as acceptable.

### D4 — Narrow physical contract

Retain concrete `Container` and `ObjectRef` unless a measured reverse dependency justifies relocation. Choose **distinct read-only snapshot, writer-owning session and explicit repair capabilities** with the smallest coherent public/internal interface; preserve bounded record views without exposing the raw writable file to callers. Physical errors distinguish unsupported version/identity, malformed complete framing, incomplete tail, writer conflict, unstable snapshot, stale/replaced source, unauthorized repair and publication conflicts. Higher layers map these into owner/facade/CLI errors without Container depending upward on their types.

`container_version`/publication-envelope helpers may live with Container only to the extent they inspect physical time/reservation/envelope binding. Semantic publication validation stays in owner code. No new persisted tag or format revision is authorized by this phase.

### D5 — Replay independence without premature owner migration

A store's `ingest`/`finish` or open-state reconstruction accepts physical frame evidence from the **caller-driven** scan and the owner's required inputs without constructing `Cva`/Gateway/RuntimeHost. `Cva::open` and `Phylactery::open` may remain top-level composition callers until Phases 2/6, but **they may not invoke semantic or physical mutation as an implicit side effect of open**. Snapshot-read, writable-session and explicit repair responsibilities must be distinguished at these entrypoints. Preserve existing replay order, identical clocks, exact accepted evidence and global uniqueness checks. Cross-owner validator relocation and the detailed semantics of Entity/compaction maintenance remain Phase 2/3; do not convert them into a new generic owner manager under Phase 1.

### D6 — Current-format hard cut

Current V2 files remain V2. Avoid changing on-disk bytes entirely if possible. If a verified necessary format change occurs, follow development policy: offline migrate current retained development corpus, verify exact accepted history/clocks/proofs, commit promoted corpus separately, and delete conversion/old readers immediately. No dual readers or compatibility adapters remain. Do not revive V1.

## 5. Alternatives considered

| Approach | Decision | Why |
| --- | --- | --- |
| Add a Storage trait per owner | Reject | Shallow abstractions and extra forwarding instead of real ownership |
| Preserve auto-repairing `open` and add inspection | Reject | Leaves surprising mutating path active |
| Keep Container→semantic visitor callbacks because they do not statically import semantic types | Reject | Hides execution/control inversion inside physical replay |
| Move all owner-level reconciliation into Container or the Phase 1 owner-lifecycle | Reject | Violates the no-upward invariant or expands into Phase 2/3 |
| Treat import admission flags as write lock | Reject | No cross-process exclusivity |
| Make all owner replay independent before physical extraction | Reject | Repeats Phase 2 and widens migration |
| Single concrete physical engine; explicit inspect/write/repair; owner-local replay | **Select** | Deep boundary, minimal interfaces, direct hard cut |

## 6. Linear execution plan — 60 bounded tasks

This is the canonical implementation sequence. It replaces the earlier 23 broad steps without changing decisions D1–D6, Phase 1 scope, or the seven sections P1.0–P1.6. Each task must record changed source paths/ownership, source edge or callback effect, focused verification command and result, baseline bytes/clocks where relevant, and deleted obsolete path or later deletion ID. A green logical target check alone never proves the Rust source meets the target.

**Critical transition order:** P1.2 builds a bounded read-only cursor **without migrating normal writable owner opening**. P1.3 establishes the OS writer lease, source snapshot, and explicitly authorized compaction/Entity corrections **before** migrating Cva/Phylactery and retiring mutating Container open. This avoids a read-only handle being forced to write or a temporarily unsupported writable owner session. Any legacy compatibility entrypoint is temporary only and deleted in P1.5. No gate accepts an unbuildable mid-hard-cut state.

Each row is bounded by its named source responsibility and independently verifiable. The prerequisite column uses a conservative linear order; specific independent tasks may be grouped only when tests prove they have no cross-owner conflicts. A task can share a commit with the immediate next task if its intermediate API cannot compile, but both task proofs must be preserved. No new feature, second store, generic storage port, runtime redesign or storage format change is authorized.

### P1.0 — Baseline and ownership inventory

| Task | Prerequisite | Concrete bounded change | Per-task verification or deletion |
| --- | --- | --- | --- |
| **P1.0.01** | Phase 0 | Pin the reviewed 19-area Phase 0 evidence against the logical 34-component target (convergence-graph and target DAG). | Record before-edge IDs and original capture ID; do not reset debt. |
| **P1.0.02** | P1.0.01 | Inventory Container open/open_scanned/read-scan callers with read, write, replay and source locations. | Every call-site classified by owner and planned access mode. |
| **P1.0.03** | P1.0.02 | Map Cva and Phylactery lifecycle replay callbacks, exact ingest order, clock results and errors. | Write an owner-by-owner replay-order/parity oracle. |
| **P1.0.04** | P1.0.03 | Inventory raw physical mutators, append/envelope, allocator split/merge, import admission flags. | Each operation assigned physical writer and semantic approval owner. |
| **P1.0.05** | P1.0.04 | Inventory ordinary-open compaction recovery and retired-Entity repair in both REL and PHY. | Name every automatic mutation and its explicit replacement action. |
| **P1.0.06** | P1.0.05 | Inventory maintenance/rewrite sources, destinations, temp files and final rename (rewrite/file.rs). | Source-snapshot vs target-writer/rename and alias matrix complete. |
| **P1.0.07** | P1.0.06 | Freeze V2 REL/PHY fixture hashes, replay clock/proof baseline, existing tests and supported OS locks. | B0 measured baseline and file/lease requirements; pre-existing failures recorded. |

**Gate B0:** Call-site, side-effect and OS lock/source/destination inventory complete, linked to the frozen reviewed Phase 0 baseline; no graph recapture.

### P1.1 — Physical contract freeze

| Task | Prerequisite | Concrete bounded change | Per-task verification or deletion |
| --- | --- | --- | --- |
| **P1.1.01** | P1.0.07 | Specify opaque physical frame, ObjectRef, EOF/tail and borrowed-byte lifetime. | Owner schemas excluded; frame contract and exact old replay order fixed. |
| **P1.1.02** | P1.1.01 | Specify global version, envelope, reservation and transaction-time updates (container_version.rs). | Document clock advance relative to payload validation and owner ingest. |
| **P1.1.03** | P1.1.02 | Design one bounded pull reader for normal and prefix scans; caller drives replay. | No semantic visitor/callback; one-frame buffer lifetime defined. |
| **P1.1.04** | P1.1.03 | Specify physical error classification for incomplete tails, invalid complete frames, header and version. | Typed outcomes and non-mutating inspection failure contract fixed. |
| **P1.1.05** | P1.1.04 | Choose actual OS-backed writer-lock primitives and process-crash behavior for supported platforms. | Windows/Unix decision recorded, including unsupported-platform refusal. |
| **P1.1.06** | P1.1.05 | Choose file identity, alias, symlink/hardlink, lock-order and mount conflict rules. | No pathname-only claim of exclusivity or unchecked lease upgrade. |
| **P1.1.07** | P1.1.06 | Specify stable source read/snapshot, writer handles, explicit repair and atomic output publication. | Source and destination authority and stale-head refusal frozen. |
| **P1.1.08** | P1.1.07 | Freeze staged migration and deletion map: cursor, writer lease, explicit repairs, REL/PHY cutover, removal. | B1 concrete signatures; no speculative traits, format/API expansion or read-to-write gap. |

**Gate B1:** Deep physical API and actual OS lock/identity contracts frozen; deletion, error, snapshot and cutover ordering accepted.

### P1.2 — Read-only physical cursor before cutover

| Task | Prerequisite | Concrete bounded change | Per-task verification or deletion |
| --- | --- | --- | --- |
| **P1.2.01** | P1.1.08 | Consolidate header/frame boundary parsing shared by container_scan and bounded Container traversal. | Golden ObjectRefs, offsets and lengths retained. |
| **P1.2.02** | P1.2.01 | Implement opaque bounded next-frame cursor in container_scan.rs. | A direct test proves caller-driven traversal and O(one frame) buffering. |
| **P1.2.03** | P1.2.02 | Integrate physical version/envelope/reservation observation in cursor iteration. | Exact global clock/transaction-time parity tests. |
| **P1.2.04** | P1.2.03 | Expose bounded-prefix and historical cursor views using the same parser. | Prefix cut evidence matches old versioned-visitor semantics. |
| **P1.2.05** | P1.2.04 | Differentiate tail truncation vs corrupt complete record, invalid header/version and overflow. | Error fixtures fail without source mutation. |
| **P1.2.06** | P1.2.05 | Migrate read-only header/inspect/probe users; do not change normal writable owner open yet. | Inspection hash parity and no write-capability acquisition. |
| **P1.2.07** | P1.2.06 | Exercise new reader through existing container_tests and publication_envelope_clock_tests. | Focused cursor/framing/error tests pass; collect commands. |
| **P1.2.08** | P1.2.07 | Capture before/after REL and PHY owner-state oracles with retained corpus and source hashes. | Replay order, accepted evidence and clocks pinned for later cutover. |
| **P1.2.09** | P1.2.08 | Freeze read-only cursor checkpoint; list temporary old open_scanned users for deletion. | M1 passes reader only; existing writable open stays until P1.3 writer authorization. |

**Gate M1:** New read-only cursor tested and bounded, without forcing any old writable owner-open caller into an intermediate read-only write path.

### P1.3 — Writer authority and coordinated open hard cut

| Task | Prerequisite | Concrete bounded change | Per-task verification or deletion |
| --- | --- | --- | --- |
| **P1.3.01** | P1.2.09 | Implement file-ID-bound OS writer lease in Container, without new lock service. | Two-process conflict and crash release tested. |
| **P1.3.02** | P1.3.01 | Gate Container create and identity creation behind held writer authority. | No conflicting or partial unauthorized file on failed create. |
| **P1.3.03** | P1.3.02 | Acquire and hold lease before any writable Container open/replay. | Read-only and writer handle capabilities structurally distinct. |
| **P1.3.04** | P1.3.03 | Gate append and publication-envelope/global-version writes with writer authority. | Read-only attempted append fails; original global clocks remain. |
| **P1.3.05** | P1.3.04 | Gate write_chunk_prefix, split_chunk, merge_adjacent_chunks and allocator writes. | No raw in-place write bypass through archive allocator. |
| **P1.3.06** | P1.3.05 | Gate truncation, writer sync and other direct physical mutation APIs. | Tail and chunk tests reject read-only handle mutation. |
| **P1.3.07** | P1.3.06 | Keep import scope/write admission as higher semantic permission, not a physical lock. | Authorized epoch writes work and cannot override file lease. |
| **P1.3.08** | P1.3.07 | Test lock file identity across independent processes, aliases, symlinks and hardlinks. | Typed contention failures, crash unlock, source bytes unchanged. |
| **P1.3.09** | P1.3.08 | Implement stable source-read evidence or typed refusal during concurrent writes. | Pinned file identity/head or coherent lease; no gratuitous source writer lock. |
| **P1.3.10** | P1.3.09 | Extract Archive conversation_compactions.recover into explicit authorized maintenance call. | Stale chunk reclaimed only by chosen operation, with equivalent result. |
| **P1.3.11** | P1.3.10 | Extract Entity resolution retired-reference canonicalization into explicit owner repair call. | REL and PHY can refuse/open read-only without append, repair on request. |
| **P1.3.12** | P1.3.11 | Migrate facade/cva_lifecycle.rs to caller-driven reader after writer and repair routes exist. | REL replay parity and typed repair-required outcomes; no callback inversion. |
| **P1.3.13** | P1.3.12 | Migrate facade/phylactery_lifecycle.rs to caller-driven reader. | PHY replay parity and no on-open Entity repair. |
| **P1.3.14** | P1.3.13 | Remove writable reopen and implicit tail truncation from Container::open path. | Normal open for valid/invalid files never changes bytes. |
| **P1.3.15** | P1.3.14 | Implement explicit bounded physical-tail repair under writer and accepted-owner prefix validation. | Only verified incomplete tail truncated; sync and reopened state checked. |
| **P1.3.16** | P1.3.15 | Adapt rewrite/file.rs source planning and revalidation to pinned read/snapshot evidence. | Changed source identity/head refuses without altering source. |
| **P1.3.17** | P1.3.16 | Guard rewritten destination create, verify and final rename against aliases, existing output and races. | No inadvertent overwrite, source mutation or unlocked publisher. |
| **P1.3.18** | P1.3.17 | Sweep prune/repack/reclaim/import/copy and runtime-mounted operations for alternate writer paths. | M2 cross-process and semantic repair gates pass; normal REL/PHY open pure. |

**Gate M2:** Writer/snapshot/publication authority passes cross-process proof; REL/PHY normal open is caller-driven and nonmutating; physical and semantic repairs are explicit.

### P1.4 — Owner replay and minimum physical relocation

| Task | Prerequisite | Concrete bounded change | Per-task verification or deletion |
| --- | --- | --- | --- |
| **P1.4.01** | P1.3.18 | Verify Archive and Memory ingest/finish consume caller-pulled frames in REL lifecycle. | Exact Memory/Archive/provenance and clock replay parity. |
| **P1.4.02** | P1.4.01 | Verify Entity, Relationship, Graph and Community replay for REL/PHY. | Same tombstones, links and graph acceptance; no lower callback. |
| **P1.4.03** | P1.4.02 | Verify Freshness, Dream, Insomnia and Import epoch replay from physical evidence. | Same pending/completion/settlement and global frontier proofs. |
| **P1.4.04** | P1.4.03 | Verify vectors/profiles, Ego, compaction and maintenance proof replay for both owners. | Canonical V2 state unchanged; compaction repair still explicit. |
| **P1.4.05** | P1.4.04 | Audit container_version, publication_envelope, ObjectRef and errors for actual pure/physical ownership. | Relocate only necessary physical primitives, no generic shared dumping ground. |
| **P1.4.06** | P1.4.05 | Preserve Cva/Phylactery as higher replay composition until Phase 2/6; record deferred owner imports. | No Container -> facade/runtime dependency or semantic callback. |
| **P1.4.07** | P1.4.06 | Run full owner replay/invalid-evidence parity against frozen before-images. | M3 all owner histories/bytes/clocks match and bounded reader remains sole scanner. |

**Gate M3:** All owner replay behavior, historical evidence and V2 clocks preserved; no physical callback inversion or premature semantic-owner extraction.

### P1.5 — Deletion and anti-regression ratchet

| Task | Prerequisite | Concrete bounded change | Per-task verification or deletion |
| --- | --- | --- | --- |
| **P1.5.01** | P1.4.07 | Delete mutating open_scanned, semantic visitor entrypoint and redundant frame scanners. | Source search finds no caller; tests reference only supported contracts. |
| **P1.5.02** | P1.5.01 | Enforce Container logical allowlist and no upward/ancestral imports in Pitlord. | Non-cyclic lower->higher negative fixture must fail source enforcement. |
| **P1.5.03** | P1.5.02 | Guard against callback inversion, writable handle escape, implicit recovery and OS lock bypass. | Source/control-flow regressions demonstrably fail tests or checks. |
| **P1.5.04** | P1.5.03 | Refresh Arcana/Pitlord source graph and reconcile reviewed debt without resetting original before-image. | No new/unreviewed/stale findings; evidence freshness verified. |
| **P1.5.05** | P1.5.04 | Remove migration-only shims and document remaining genuine Phase 2/3 owner dependencies. | D no Container inversion, mutating ordinary open or duplicate scanner. |

**Gate D:** Source-level ratchet rejects upward imports even when noncyclic; no obsolete callback/mutating-open/write bypass or stale baseline debt.

### P1.6 — Documentation and integration acceptance

| Task | Prerequisite | Concrete bounded change | Per-task verification or deletion |
| --- | --- | --- | --- |
| **P1.6.01** | P1.5.05 | Update Architecture, Invariants, Storage Format, maintenance and behavioral-contract docs for actual access behavior. | Current docs no longer claim automatic tail/compaction/Entity repair. |
| **P1.6.02** | P1.6.01 | Update parent spec, logical DAG source crosswalk, roadmap, INDEX and Phase 2 handoff. | Only true non-physical edges deferred; no roadmap features imported. |
| **P1.6.03** | P1.6.02 | Run focused Container, clock, REL/PHY open, alias lock, repair, rewrite, import, maintenance tests. | Record exact commands, expected assertions, outcomes and source hashes. |
| **P1.6.04** | P1.6.03 | Run locked Rust/CLI checks and test builds, architecture ratchet and docs checks. | Record full runs and failures distinctly; don't claim unexecuted passes. |
| **P1.6.05** | P1.6.04 | Run named opt-in canonical V2/history corpus proof and inspect source file hashes. | Same accepted history/global ordering; no retained corpus mutation. |
| **P1.6.06** | P1.6.05 | Review all B0/B1/M1/M2/M3/D evidence, deletion list and git diff; record checkpoint. | A requires no missing physical source ratchet, old open route or unchecked exception. |

**Gate A:** Retained corpus, functional tests, source enforcement, docs, and handoff checked with actual evidence; no deferred Container inversion.

**Final command ledger (execute when applicable, do not assume results):** `cargo check --locked`; `cargo test --locked --no-run -j 2`; `cargo test --locked --lib container`; `cargo test --locked --test current_format_boundary`; `cargo test --locked --test history_rewrite_contract`; `cargo check --manifest-path cli/Cargo.toml --locked`; `cargo test --manifest-path cli/Cargo.toml --locked`; `python scripts/check_architecture.py --refresh`; `python scripts/check_dependency_convergence.py`; `python scripts/check_component_dag.py --self-test`; `ddocs check --indexes --links`; `git diff --check`. Named opt-in ignored historical-corpus tests require their documented flags, fixture setup and separate execution report. Test additions described above are requirements, not assertions that those tests exist or pass now.
## 7. Required tests and evidence

| Contract | Required proof |
| --- | --- |
| Read/open purity | bytes, size, digest and clocks unchanged after inspect, historical reads, **normal REL/PHY open** and failed open: valid, truncated, malformed, unsupported, unaccepted-owner, stale-compaction and retired-Entity-reference inputs |
| Pull replay and version parity | caller owns loop, Container never invokes owner callbacks; exact accepted frame refs, clocks/frontier, reservations, transaction times and owner replay outcomes before/after; bounded prefix parity |
| Explicit physical and semantic repair | incomplete physical tail only under classified + exclusive + owner-approved repair; stale compaction free/coalesce and Entity-resolution append only through explicit owner/maintenance authority; invalid complete payload/identity/version never mutates |
| Writer exclusivity and identity | two processes and held Runtime owner; hardlink/symlink/rename aliases, stale/replaced handles, create, crash release, lock ordering; no partial writes or lock bypass |
| Snapshot/read coherence | stable read-only source evidence under concurrent writer or typed refusal; stale snapshot/source-head changes reject, including transformed-copy planning and emission |
| Destination publication | file identity and no-overwrite guard, revalidated source, same/cross-volume handling as supported, atomic rename/replacement and contention with another publisher; source stays untouched |
| Semantic admission | import epoch closure and scoped authorized publication preserved, never bypassing physical writer exclusion |
| Streaming bounds | large payload/record populations consume bounded frame buffers; no O(all records) indexing, retained visitor callbacks or redundant full materialization |
| Owner replay | Archive, Memory, Entity, Graph, Freshness and all current REL/PHY owners ingest ordered physical frames without Container invoking semantic code or importing facade/runtime |
| Maintenance | explicit prune/rewrite/reclamation/repack/import/copy preserve exact accepted history, timestamps, proofs and safe source lifetime; no new feature behavior |
| Architecture | reviewed Phase 0 baseline retained/shrunk, real touched physical edges checked against logical allowlist; no Container upward edge **or semantic execution inversion** even if acyclic; no silent writer bypass |
| Release hygiene | full workspace build/tests, formatting, static/document checks and current-format retained corpus acceptance versus declared baseline |

Reuse existing `src/container/container_tests.rs`, `container_format_boundary_tests.rs`, `publication_envelope_clock_tests.rs`, envelope suites, `tests/current_format_boundary.rs`, `src/archive/conversation_compaction_store.rs`/allocator scenarios, `src/perception/entity_resolution_store_write.rs`/REL/PHY reopen scenarios, owner replay and maintenance/rewrite integration suites. Keep source-corruption and semantic safety refusal tests; replace only tests coupled exclusively to removed callback/open mechanics with interface-level contracts. Test direct Container reader operation **and** actual Cva/Phylactery public opening behavior. Use repository-supported commands and preserve honest baseline failures; do not fabricate a pass.

## 8. Completion criteria

1. Inspect/read/scan/normal open—including `Cva::open` and `Phylactery::open`—do not modify bytes or silently repair/normalize/reclaim. A caller explicitly requesting a writable session obtains guarded authority, **not** implicit permission to change historical state.
2. Incomplete physical tails and the identified stale-compaction/retired-Entity on-open correction cases have **distinct typed outcomes**; only explicit authorized repair/maintenance operations may change bytes, with semantic acceptance verified before and after.
3. Physical writer exclusivity holds across processes and aliases for all mutations; read-only sources use validated stable snapshots or fail closed, and rewritten outputs are safely guarded through publication/rename, including mounted Runtime contention.
4. Semantic admission policy (including import epochs) is distinct from and subordinate to physical exclusivity.
5. Container and physically owned clock/envelope helpers depend **only on their explicitly listed lower pure contracts**; no owner/facade/runtime imports, callback-based semantic invocation or disallowed upward/ancestral dependency remains, whether or not cyclic.
6. **Caller-driven bounded frame iteration** supplies owner replay evidence in exact order; Container never calls an owner visitor. Cva/Phylactery may compose owners from above; lower owner stores need no facade/runtime construction.
7. All V2 accepted-state, clocks, timestamps, owner semantics and current corpus remain correct; no gratuitous on-disk change.
8. Old implicit-recovery and semantic callback APIs, on-open mutations, duplicate scan variants, and bypassable writer shims are removed—not kept as compatibility alternatives. Explicit authorized maintenance remains supported.
9. The Phase 1 physical source paths are mapped to the accepted **34-component DAG** and permanently ratcheted against cycles, **noncyclic upward backreach**, unlisted edges, and callback control inversion. Documentation accurately describes actual current behavior, not future semantics.
10. Full integration gates pass against the declared baseline; discrepancies and deferred Phase 2/3 concerns are recorded.

## 9. Exclusions and handoff

Not in Phase 1: owner-level semantic contract redesign, cross-owner history coordination, settlement minimization/format changes, project VCS/path convergence, runtime/facade retirement, Context Engine compaction features, Observations, Ego synthesis, explicit Memory commit, provider integration, new synchronization/versioning/compression/retrieval optimizations.

**Phase 2 handoff:** a coherent physical read/snapshot/writer/explicit-repair boundary, caller-driven owner replay, documented REL/PHY open purity, accepted source/destination lock identity rules, **no physical-layer upward import or semantic callback**, and a ratcheted source-edge inventory of remaining store-to-store/domain dependencies. Phase 1 must not implement Phase 2 owner read/proof or Phase 3 cross-owner maintenance redesign merely for convenience, and must not defer any remaining Container-level inversion as later-phase debt.
## Purpose

Define the Phase 1 physical storage boundary and its required acceptance gates.

## Overview

The preceding contract defines scope, ownership and verification requirements.

## Related docs

[Architecture](architecture.md), [file lease contract](dependency-convergence-phase1-lock-contract-2026-10-21.md), [target DAG](dependency-convergence-target-dag-2026-10-21.md).

## Notes

The V2 encoding remains unchanged. Current behavior and unfinished acceptance are recorded separately.