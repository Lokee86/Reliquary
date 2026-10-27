# Maintenance Store crate extraction — specification

**Status:** Specified, implementation not begun (2026-10-27). **Baseline:** verify current HEAD. **Target:** `crates/reliquary-maintenance-store`. Correction-only. **Sequence:** after External Index Store integration; do not mix uncommitted cuts.

## Ownership and discovery gate
Maintenance Store may own **only existing durable, maintenance-specific records and their intrinsic codec/replay invariants**. It does **not** own prune/rewrite/compact algorithms, history selection, cross-owner reconstruction, Container repair, graph/vector/Memory authority, or mutation orchestration. Container remains the only permitted direct Reliquary dependency.

**Critical qualification:** The current repository has a placeholder crate but does not expose a clearly identified standalone `MaintenanceStore` or `maintenance_journal` under `src`. Therefore the first checkpoint is an evidence-driven *ownership inventory*, not a presumed code move. Inspect `src/facade/cva_history_prune.rs`, `cva_repack.rs`, `cva_archive_history_prune.rs`, `cva_description_prune.rs`, related maintenance/rewrite/compact helpers, Container and history codecs, plus owner-specific prune paths. Identify actual maintenance-owned persistent records, writer, replay reader, format magic and tests. **If none exist, stop with an explicit no-op finding and propose removing or reserving this crate rather than manufacturing a maintenance ledger or proxy layer.** A placeholder library with no concrete ownership is not a successful extraction.

## Required boundaries
1. **Physical state:** Container alone owns frames, atomic write/sync, inspect/repair, rewrite and file replacement. Maintenance Store cannot duplicate Container's journal, physical manifest or transaction log.
2. **Domain state:** Archive, Memory, Graph, Dream, Ego, Freshness, Vectors and derived index stores each retain their own durable records. Maintenance's cross-owner decisions remain higher; do not move owner-local codecs into Maintenance Store merely because maintenance uses them.
3. **Existing maintenance-specific evidence (if found):** move only canonical values/codec/passive projection and exact append contract into the lower crate. Preserve current wire bytes and format; no legacy reader or new migration. Caller provides already-authorized proof/cuts. No direct dependency on owner stores other than Container.
4. **Read vs mutation:** History remains read-only. Maintenance orchestrates mutating prune/rewrite; no implicit mutations during reopen. Preserve tombstone/history reconstruction, equivalent accepted historical state and lossless retention contracts.
5. **API:** Inventory borrowed return types before relocating; use canonical type ownership/owned internal conversions where needed without duplicate cache or unapproved public break.

## Bounded execution
1. Freeze HEAD/status and baseline tests; preserve unrelated `docs/graph-crate-extraction-evidence-2026-10-26.md`. Build exact inventory of purported maintenance durable records with producer/consumer/format. Decide **extract / no-op** with evidence.
2. Only if real store exists: extract codec and model, then replay and append with Container-only dependency. Golden-wire tests and malformed/current-format rejection precede root cutover.
3. Rewire all maintenance and history consumers to one lower store, delete displaced root owner state, keep coordinator and other owners above. No forwarding facade that maintains duplicate state.
4. Verify real prune, normal rewrite, fresh rewrite, tombstone historical reconstruction, REL/PHY parity, no-op preservation, fault/partial-prefix recovery and all relevant cross-owner tests.
5. Run `cargo check --workspace --locked --offline`, `cargo fmt --all -- --check`, focused maintenance/history tests and `cargo test --manifest-path cli/Cargo.toml --locked --offline`; record exact results in `docs/maintenance-store-crate-extraction-evidence-2026-10-27.md`. Commit only if actual owner work extracted. Otherwise record the no-op and obtain an explicit crate-map decision before any stub commit.

**Out of scope:** new maintenance schema, generic journal, additional lifecycle manager, automatic migration, semantic improvements, runtime architecture ratchets, CVAFRS07 corpus claims without execution.
