# M02 step 1 — retired-profile packed-vector pruning

Status: **implemented and focused-verified**, not an M02 phase acceptance.
Repository: `Reliquary-rebuild`, branch `rebuild/clean-architecture`.
Reference behavior: both REL and PHY terminal packed-vector pruning, adapted to the native 2.1 owner and generation contracts.

## Accepted implementation boundary

- Operates only from an immutable `VerifiedOwnerCopyPlan` over the exact authenticated source owner/cut. A destination collision, changed source, unknown/active/duplicate profile, repeated destructive maintenance, or missing backing fails closed.
- Requires every selected profile to be explicitly retired. All Memory-vector and REL Archive-generation references to a selected packed object must belong exclusively to that profile. Shared backing with another profile, even a selected retired profile, is conservatively refused. REL Archive mappings that lack a generation owner are also refused; no profile ownership is inferred.
- Replaces only the proved packed payload with a typed unavailable marker. A retention manifest binds the selected profile, packed identity, matrix shape, source digest, and retirement evidence. REL/PHY reopen checks validate marker/manifest linkage, retired status, and exact referenced ownership.
- Relocates REL physical generation receipts when the shorter markers shift generation-record offsets. REL replay validates fragment/source history and matrix shape for retired pruned generations without reading intentionally unavailable rows; active generations continue validating actual rows.
- Stages, syncs, checks the source again, and independently reopens before no-replace publication. Source owner identity, clocks, active matrix data, and vector bindings remain intact.

## Verified cases

- PHY: exclusive retired profile removed; active profile rows survive; source unchanged; repeat/destination collision rejected.
- PHY: a retired and active profile sharing one packed matrix is rejected before any output.
- REL: two vector generations survive compacted physical placement; active Archive vectors remain readable, retired rows become unavailable, Archive clocks/source turns and generation identities survive. An unowned Archive mapping to the selected matrix is rejected before output.
- `cargo test -p reliquary-owner-session retired_profile_prune --locked --offline -j 1 -- --test-threads=1` — **3/3 passed**.
- `cargo test -p reliquary-owner-session --locked --offline -j 1 -- --test-threads=1` — **99/99 unit tests passed**, plus integration tests.
- `python scripts/check_architecture.py` — **passed**, 23 production packages / 76 expected edges.
- `cargo fmt --all` applied; focused tests subsequently passed.

## Remaining verification and scope

The workspace-wide `cargo check --workspace --locked --offline -j 1` attempt timed out after five minutes while building the Lore dependency stack, without a reported Rust error. Do not treat this as a passing full gate. Full workspace build/tests and multi-domain history tests remain part of M02's final acceptance gate, as do Archive, Entity/Relationship, reclamation, and fault-injection work.

This step does not imply that the rest of M02 is accepted.
