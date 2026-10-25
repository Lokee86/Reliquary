# Container crate extraction — implementation specification

**Status:** Approved architecture target; extraction not started. 2026-10-25.  
**Working branch:** `refactor/crate-first-restart` (from `c43d33c`).  
**Destination:** `crates/reliquary-container` (currently an intentionally empty Cargo library).

## 1. Problem and scope

The pre-refactor `Container` is physically implemented under the root `reliquary-memory` package, not a separately compiled owner. Its scanning implementation calls a higher-level visitor during open; `container_version.rs` directly invokes Insomnia and External Source completion decoders. Normal `Container::open_scanned` may open writable and truncate a trailing incomplete frame. A separate writer/process may enter the same file without a durable writer lease. These are concrete ownership, replay-order, corruption and isolation problems, **not** reasons to defer the crate split.

**Deliverable:** one real, independently compiled physical storage library that owns current REL/PHY framing, file identity, opaque physical object references, global/transaction clock transport, bounded read cursors, OS file leases, physical write exclusion and explicit incomplete-tail repair. Existing Archive/Memory/etc. owners and `Cva`/`Phylactery` consume it downward. No durable semantic payload interpretation or application-level authorization moves to Container.

This is the **first implementation extraction**, not a storage redesign or a new functionality phase. Preserve current format bytes and public behavior except the already accepted correction that ordinary open/inspection must not mutate files.

## 2. Governing contracts

- [Crate layout specification](crate-first-layout-spec-2026-10-25.md), [crate map](architecture/crate-map.json), and [component DAG](architecture/component-dag.json): `container` (logical rank 1) depends only on local `pure-codec` and `review-clocks` (rank 0). Other crates may import Container; **Container cannot import them or the root product**.
- [ADR 0001](decisions/0001-purpose-built-database-ownership.md): physical container does not own semantic databases, lifecycle, proof or mutation eligibility.
- [Storage format](storage-format.md), [ADR 0042](decisions/0042-minimal-lossless-current-history.md): V2 physical header and current publication envelope, owner-specific history and exact physical clocks are preserved.
- [Invariants](invariants.md), [Behavioral contracts](behavioral-contracts.md): wrong owner, invalid complete publication, overflow and unauthorized mutation fail closed; accepted owner history remains unchanged.
- [Native import](native-import.md), [Semantic maintenance](subsystems/semantic-maintenance.md), [Scope ownership](subsystems/scope-ownership.md): import write admission and repair authorization belong above Container; read-only History cannot repair.
- Abandoned-refactor design/input **only**, from Git bundle/old `main`: `docs/dependency-convergence-phase1-storage-foundation-spec-2026-10-21.md`, `docs/dependency-convergence-phase1-lock-contract-2026-10-21.md`. Source changes: `8614547` (physical access/leases), `408d775` (read-only History consumption), `ccd949f` (Maintenance writer consumer), `f431401` (final scan consumer). These are **salvage references**, not commits to cherry-pick wholesale.
- The Step 1 recovery inventory is stored outside this worktree at `C:\\!bin\\backups\\Reliquary-crate-restart-2026-10-25\\recovery-inventory.md`; the backup also preserves the authoritative corpus. The accepted `CVAFRS07` settlement recovery is tracked separately; this extraction must not reinstate an old format reader.

**Baseline evidence:** `c43d33c` passed 1,942 library tests. Its Container source *does not* yet implement the later exclusive lease and no-mutation-open correction. Do not use green baseline tests alone as acceptance of the target contract.

## 3. Canonical directory and ownership map

```text
crates/reliquary-container/
├── Cargo.toml
└── src/
    ├── lib.rs                  # curated public exports; no facade dependency
    ├── container.rs            # physical session, header, read/write/identity
    ├── scan.rs                 # pull-based bounded frames; inspection outcome
    ├── version.rs              # global clock/reservations/transaction time
    ├── publication_envelope.rs # CVAPUB01 physical envelope
    ├── grouped_clock.rs        # physical grouped clock prefixes only
    ├── lease.rs                # OS lock, file identity and lifetime
    ├── access.rs               # stable physical read source and mutation gate
    ├── repair.rs               # explicit incomplete-tail plan/execution
    ├── codec.rs                # pure scalar delta/field-change mechanics
    └── tests/                  # existing focused unit tests (named modules)
```

Move the real definitions from:
- `src/container/container.rs`, `container_scan.rs`, `container_error.rs`, `container_version.rs`, `publication_envelope.rs`; and `src/compact_delta.rs` (only pure mechanics needed by multiple owners).
- Selectively reuse `src/container/container_access.rs`, `container_lease.rs`, `container_repair.rs`, `grouped_clock.rs` from `8614547` and later physical-source corrections. No duplicate implementation in the old root.
- Move existing Container unit tests (`container_tests.rs`, `container_format_boundary_tests.rs`, `publication_envelope_tests.rs`, `publication_envelope_clock_tests.rs`, `transaction_time_tests.rs`) to their defining crate **only when they test physical behavior**. Split out their `Cva`/Insomnia/External Source scenarios as root integration tests; do not pull semantic services into Container's dev-dependencies. Reuse the post-refactor `file_lease_process_tests.rs` for genuinely separate-process contention.

The exact physical module names may be consolidated during extraction when a module does not earn its own interface. Do **not** manufacture a file/module for every planned type.

### What explicitly stays elsewhere

`src/facade/cva_lifecycle.rs`, `phylactery_lifecycle.rs` and `src/owner_lifecycle.rs`: owner assembly/replay and authorization, eventually `reliquary-owner-lifecycle`. `src/facade/semantic_maintenance.rs` and `src/rewrite/**`: maintenance validation, repair selection and publication. `src/runtime_inference/insomnia/completion.rs`, `src/external_source/publication.rs`: their complete semantic records and validation. `src/freshness_storage.rs`, Archive/Memory/Graph/Perception stores: owners of their domain's persisted meaning.

## 4. Public interface and behavior

The library exposes an intentionally **small physical contract** (the list below identifies responsibilities, not a demand to retain all old `pub(crate)` names).

| Capability | Required result/invariant |
| --- | --- |
| `Container` session + typed `ContainerIdentity`, `FileKind`, `FormatVersion` | Typed REL/PHY identity, owner UUID, current V2 admission and exact header validation |
| Opaque `ObjectRef` and bounded `read`/physical append | Reference validation, overflow checks, no raw `File` exposure or semantic ID interpretation |
| Stable `PhysicalReadSource` and pull `PhysicalCursor` | Sequential opaque frames, physical locations and clock observations, finite-prefix reads without hidden writes |
| Exclusive writable open and held shared-read snapshots | OS-backed file identity, nonblocking writer contention and held-handle lifecycle; concurrent local mutation fences |
| Typed physical clock operations | Allocation/reservations, `CVAPUB01` bind/allocate, transaction-time ordering and grouped physical version ranges |
| Explicit incomplete-tail classification/repair plan | Physical accepted-prefix boundary, no spontaneous truncation; repair only under a held writer session **after** higher owner approval |
| `ContainerError` and physical sync | Stable typed faults for unsupported version, corruption, incomplete tail, writer conflict, stale identity, unsupported locking and unauthorized physical access |

**Direction:** a higher owner obtains a bounded cursor and **pulls** one physical frame at a time, validates/ingests into its own state, and then advances. No `Container::open_scanned(visitor_that_mutates_owner)` as a cross-crate owner replay API. Do not buffer the complete physical log in a vector just to avoid a callback.

**Admission vs authorization:** a writer lease proves physical exclusion, **not** that a caller may amend a Memory, import, or Archive. Owner Lifecycle and the owning stores decide semantic admissions. Preserve the existing import-fenced write behavior without exporting an unchecked boolean bypass as general public convenience. Use a narrowly held writer/scope capability where justified; do not introduce a generic policy registry, global lock manager, or semantic callbacks. A read session cannot be upgraded to repair implicitly.

**Physical clock parser:** existing `src/container/container_version.rs` calls
`crate::insomnia::completion::decode_embedded_version_range` and
`crate::external_source::publication::decode_embedded_version_range`.
These are the exact upward imports to eliminate. Move only the **validated physical grouped-clock prefix** recognition into Container (the later `grouped_clock.rs` is a reference), retaining current byte layouts and clock/frontier behavior. Preserve existing physical frame length/checksum checks that are necessary to observe a valid physical clock; leave semantic completion/evidence validity and admission policy in the respective owners. No generic owner decoder registry, version-bearing callbacks, new magic values or re-encoding. A malformed complete clock-bearing frame remains an error; an incomplete tail is classified separately.

**Inspection/open/repair:** ordinary open must validate without changing source bytes. Incomplete final physical tails are reported by a typed outcome; complete corrupt records are rejected. Explicit repair requires a validated prefix, stable file identity, held writer exclusion, observed length, owner authorization and post-repair reopen/verification. Existing transformations and rewrite source/destination publication remain higher Maintenance responsibilities. Reuse the previously verified Windows `LockFileEx` and Unix file-lock behavior where applicable, including alias/path-identity fencing and fail-closed unsupported filesystems. Do not describe advisory OS locks as protection against hostile direct writes.

**Dependency budget:** `reliquary-container` uses `std` and the minimum actual physical support dependencies (e.g. `uuid` and digest for physical checksums if needed). It imports **no other Reliquary crate**, no provider, `Cva`, runtime, Archive or semantic decoder. Pure codecs/clocks from the accepted DAG remain in its internal modules; avoid a separate `common`/contract crate without a demonstrated need.

## 5. Staged hard-cut migration

One bounded extraction, with short coherent integration checkpoints; *not* a new phase-gate bureaucracy.

1. **Physical cut:** move core physical types, codec and framing functions from the root to the Container crate. Promote only consumed methods to cross-crate public visibility; replace `crate::Container`-style imports in callers with the actual Cargo dependency. Preserve root re-exports only where they are part of the existing supported public product API, not as an internal forwarding layer. Compile the crate independently. Do not retain an old root physical implementation.
2. **Close the real inversions:** replace the Insomnia/External embedded clock imports using physical-only grouped-clock parsing. Convert owner-scanned callbacks to caller-driven cursors. Port normal open to no-mutation inspection and separately gated writer/repair, using selectively recovered Phase 1 lease/access code.
3. **Consumer integration:** adapt the current root `Cva`, `Phylactery`, Archive/Memory and import/rewrite/facade callers to use the new physical API. Preserve exact error translations and release/lock lifetimes. Keep semantic replay, incomplete-tail authorization and open-time cleanup decisions in their owners. Remove displaced root `#[path]` modules and obsolete forwarding methods.
4. **Behavioral gate and clean-up:** transfer actual physical tests, run direct cross-process/file tests and selected root owner reopening/maintenance contracts. Delete temporary compatibility helpers and update source maps/documents. No new graph inventory or compiler evidence generator.

An intermediate red build is allowed while consumers are being switched. Finish this cut with the root compiling against the real crate and with **one** canonical implementation. Do not leave two versions alive for incremental safety. Do not pre-extract Archive, Memory or Owner Lifecycle merely to make Container appear independent; those are subsequent real extraction units.

## 6. Verification — existing evidence over new machinery

**During the cut:** `cargo check -p reliquary-container --locked` and `cargo test -p reliquary-container --locked`; existing physical frame/header/version/clock tests, supported OS lease/read-snapshot tests and root targeted `Cva`/`Phylactery` open/repair behavior. Reuse or transplant actual regression assertions from `8614547`, not its 126-file change set or generated reports.

**Complete Container acceptance:**
- `cargo check --workspace --locked` and `cargo fmt --all -- --check` pass on the resulting 31-package workspace; the independent CLI remains separately lock-tested where public surfaces changed.
- `cargo test -p reliquary-container --locked`, relevant root `--lib` and integration tests pass. No duplicate tests solely mirroring moved source.
- Current REL and PHY normal opening, no-mutation inspection, exclusive second writer rejection, snapshot lifetime, stable alias identity, reserved/global/transaction clock replay, incomplete-tail identification, explicit authorized recovery, invalid-complete-record rejection, bytes under prefix/cut scan and exact error classes are covered.
- Container crate has **zero internal references to root/semantic crates**; no remaining production physical implementation at the old root path. Use `rg` and Cargo manifests to inspect remaining consumers; Arcana is optional if its index is current, not a blocking dependency.
- The actual integrated current-format reader must reopen the verified promoted `CVAFRS07` corpus **on disposable copies before any mainline replacement or final product acceptance**. This may require later, separately salvaged Import/Freshness code; do not invent Container-specific compatibility readers to make a prematurely broadened test green.
- [Architecture](architecture.md), [Storage format](storage-format.md), [Maintainer map](maintainer-map.md), [Behavioral contracts](behavioral-contracts.md), [Development](development.md) and [Crate layout spec](crate-first-layout-spec-2026-10-25.md) accurately describe the resulting current source and any scoped acceptance limitation.

**The acceptance result is the independently compiled physical crate and working product**, not a zero-cycle synthetic graph report. Keep existing meaningful tests; discard Phase 1 proof ledgers and Phase 7 verification machinery.

## 7. Explicit exclusions

No new file schema, format upgrade or storage compression; no rewriting Insomnia/External Source business decoders; no semantic import-settlement or historical reconciliation redesign; no additional external provider or host adapter; no preemptive extraction of other 29 crates; no migration of the authoritative fixture originals in place; no old-format runtime fallback; no generic framework of pluggable storage interfaces or permanent alias facades.

## 8. Decisions to check in implementation (non-blocking)

- Select the smallest physical cursor value/lifetime shape that preserves bounded replay and error ordering; the Phase 1 `PhysicalCursor` implementation is candidate code, not automatic final truth.
- Inspect the three later Container touches (`408d775`, `ccd949f`, `f431401`) for genuine reader/publisher corrections; salvage only those whose behavior is still required. File-level reuse must be proven by compilation and existing tests.
- The baseline's public `Container` API exists; coordinate semantic owners' public error conversion carefully. Rust visibility changes alone are not an authorization mechanism.
