# Leiden Community recomputation acceptance

Parent index: [Acceptance evidence](./INDEX.md)

## Purpose

Record the local Reliquary 2.1 recomputation run on one inactive copy of the first 28 days of REL data.

## Overview

Implementation started at `1fee2310`. The solver uses the original scan-and-merge code from `f431401e` and `leiden-rs` 0.8.1. Subset snapshot support comes from `dca06ffa`. Capture and publication use the same active-Memory eligibility check. Only an authorized writer can publish, and owner identity or version changes invalidate a prepared result.

## Copied REL

The source was `C:/!bin/workspace-data/reliquary-native-first28d-2026-11-11/project.rel`. The inactive copy and JSON reports are in `.test-tmp/leiden-first28d-2026-11-18-e81198db/` at the repository root. The copy was created at a new path and matched the source byte for byte before preview.

| Check | Observed result |
| --- | --- |
| `rel community list <copy> --limit 128` | Reopened original generation 1 with 12 Communities. |
| `rel community recompute <copy>` | 2,188 eligible Memories, 2,188 assigned, 21 Communities, modularity `0.697415587199191`. The file hash stayed the same. |
| `rel community recompute <copy> --apply` | Same partition and metrics as preview. Published generation 2 in a `RELCMP01` frame. |
| `rel verify <copy>` and fresh read of the stored membership | Format 2.1 verification passed. Every active graph Memory appears exactly once. Zero of the 656 archived graph Memories were assigned. |
| Community history | Generation 1 retained all 12 Communities and their original 1,084 memberships. Graph and Memory versions remained at 36,405 and 2,847 respectively. Perception, Relationship, and Archive versions were also unchanged. |

Community sizes in ascending order were `1 × 5, 30, 49, 55, 60, 74, 81, 123, 132, 137, 165, 166 × 2, 169, 193, 232, 351`. Preview, apply, and fresh read of the saved groups produced the same membership SHA-256, `5717100a8d7b369dfff76852b868034c4949b88d0d1a7fb9bce29601c7220796`.

The source before and after, and the copy through preview, had SHA-256 `FE301CAB298AB305156EED85D07248312A921A9DB731DFD6FD1E261B8DA489BC`. After apply and reopen the copy had SHA-256 `8BCF792F8C63137236E657A32949D85795377E2AABCE3F77120D6FCD81D4C6F1`.

## Verification

| Command | Measured result |
| --- | --- |
| `cargo fmt --all --check` and `cargo fmt --manifest-path cli/Cargo.toml --check` | Passed. |
| `cargo test -p reliquary-graph --locked --offline -j 1` | 14 passed, including the ported Leiden fixtures and subset replay. |
| `cargo test --test community_management --locked --offline -j 1` | 3 passed. The focused recomputation test covers the archived-link case, grant enforcement, byte-preserving preview, durable membership, and idempotent apply. |
| `cargo check --workspace --locked --offline -j 1` | Passed. |
| `cargo test --workspace --locked --offline -j 1 --no-fail-fast -- --test-threads=1 --skip tests::exclusive_lease_blocks_other_processes` | 685 passed, zero failed, one filtered. The unfiltered run stopped when the existing container fixture hit Windows symlink creation error 1314. |
| `cargo build --manifest-path cli/Cargo.toml --locked --offline -j 1` | Built the CLI used for the copied REL. |
| `cargo test --manifest-path cli/Cargo.toml --locked --offline -j 1` | 17 passed. |
| `python scripts/check_architecture.py` | Passed with 23 workspace packages, 76 production edges, and 15 test-only edges. |
| `python scripts/check_documentation_entrypoints.py` and `python scripts/check_standards_snapshot.py` | Entry points passed and 28 snapshot hashes matched. |

## Related docs

- [Leiden specification](../planning/leiden-community-recomputation-spec-2026-11-18.md)
- [Leiden execution plan](../planning/leiden-community-recomputation-plan-2026-11-18.md)
- [CLI reference](../reference/cli.md)

## Notes

This is evidence for local explicit recomputation in the Reliquary core and CLI.
