# Owner Lifecycle crate extraction evidence — 2026-10-27

## Purpose

Record the verified scope and remaining work for the Owner Lifecycle extraction.

## Overview

Status: production cutover implemented; full acceptance is not verified because the preserved REL fixtures require the separately excluded CVAFRS07 recovery. Baseline `390095ec` on `refactor/crate-first-restart`; Chronos specification commit `8468348b` arrived during execution and was preserved; the pre-existing Graph evidence modification is unrelated and preserved.

REL and PHY use the compiled crate to initialize shared Memory, Graph, Entity and Relationship formats, traverse complete physical replay, reconstruct Graph against accepted Memory and Entity facts, and check cross-owner references and global-coordinate uniqueness. REL also dispatches ordinary Archive/Memory/Graph frames and ordered interaction, compaction and Echo evidence there and checks Archive file links against historical Memory. The visitor retains product-specific proof handling; a scan is accepted only after every callback succeeds. The bounded reader is used by Dream replay. No wire format changed.

The integration gate results and inherited limits are recorded below.

## Replay inventory and boundary

REL creation establishes Archive, Memory, Graph, Entity and Relationship formats in that order, followed by Insomnia and vector/profile formats, then sync. PHY omits Archive and Insomnia and initializes the same shared core before its vector/profile formats and sync. Both construct the product owner above the coordinator.

REL open checks the durable owner header, opens a leased Container, and visits its complete physical frames in order. Rewrite lineage is checked first. Ordinary Archive, Memory, Graph and Container-version frames take the compiled core dispatch. Publication envelopes retain their ordered Memory, Entity, Relationship, Archive and Graph authorization chain. Fallback frames retain maintenance, import epoch, external index and publication, Archive and grouped Memory birth, Graph, Perception, vector, Echo/Ego, Dream and Insomnia handling in their established order. After scan admission, the higher owner validates and finalizes stores; Dream compact Begin reads the preceding accepted population before that frame advances the prefix, and Insomnia completion authorization runs against finalized Archive and Memory. Import epoch settlement and semantic freshness remain higher-level policy.

PHY performs its header and scope rejection, lineage and REL-only exclusion before Memory, Entity, Graph and Relationship replay, then finishes shared structural checks and the remaining product stores. Both use the compiled Graph reconstruction and historical local Relationship reference checks. REL also checks Archive file links against historical Memory there. Bounded prefix traversal is passive and rejects an interior frame boundary. History selects and interprets its own cuts; it does not become a coordinator dependency.

The concrete input/output boundary is `initialize_core(&mut Container, &MemoryStore, &mut GraphStore, &EntityStore, &RelationshipStore)`, `replay_open(&mut Container, visitor)`, `ingest_rel_core_frame(ObjectRef, bytes, global, &mut ArchiveOpenState, &mut MemoryOpenState, &mut GraphOpenState)`, `finish_graph_replay(GraphOpenState, &MemoryStore, &EntityStore) -> GraphStore`, and `replay_prefix(&Container, physical_end, visitor)`. Native errors remain distinguished. The prefix caller supplies an already chosen cut; physical frame boundaries alone do not authorize semantic evidence. Cross-store validators read the finished stores and return the existing product error distinctions. The root consumes these owned stores and continues to construct `Cva` and `Phylactery`.

The remaining fallback replay and special Dream/Insomnia/Import logic depend on root-only proof, semantic policy and product state. They deliberately remain in the higher coordinator rather than replacing authorization with a header check or introducing a reverse dependency. Vector recovery, pruning, repair, publication, and service operations also stay above this boundary.

## Verification

- `cargo check -p reliquary-owner-lifecycle --locked --offline` and `cargo check --workspace --locked --offline`: passed.
- `cargo test -p reliquary-owner-lifecycle --locked --offline`: four passed, including physical order, failed replay write admission, bounded-frame rejection and passive prefix reading.
- `cargo fmt --all -- --check`, `git diff --check`, and `ddocs check --indexes --links`: passed.
- Root `current_format_boundary`: six passed and the explicitly gated six-file corpus test was ignored. Focused Container owner, Dream population, Import epoch scope, history cuts, Graph, Archive and Relationship replay, retained Insomnia completion, Import, Memory and Relationship lifecycle modules: 104 tests passed, including Echo replay. Additional named replay/fault/reopen tests passed, including the new missing External Index backing regression.
- `cargo test --manifest-path cli/Cargo.toml --locked --offline`: 32 passed with TEMP and TMP inside the repository.
- The authoritative historical cut test with `history-oracle-fixtures` failed on the first REL with `unsupported Freshness prototype format; explicit migration is required`. This is the existing CVAFRS07 limitation documented in [Current limitations](current-limitations.md), not a new format regression; the specification explicitly excludes that recovery. Thus all six authoritative fixtures were not verified by this run.
- `python scripts/check_architecture.py` passed against the existing Arcana snapshot: 25 rules and no violations. A fresh Lexicon scan did not finish during this run and was interrupted, so the passing architecture check uses the earlier snapshot. The documentation guard reports pre-existing unmapped paths; the modified subsystem documents and new crate Code Map are updated. Documentation policy reports existing missing-section findings in other extraction documents.

## Notes

The crate owns only the concrete bounded portions listed above. Population selection, semantic authorization, import settlement and the remaining product-only replay adapters remain above it because their proof and policy APIs are not lower-store authority. No compatibility reader or new wire format was introduced. No CVAFRS07 recovery is claimed.

## Related docs

- [Owner Lifecycle extraction specification](owner-lifecycle-crate-extraction-spec-2026-10-27.md)
