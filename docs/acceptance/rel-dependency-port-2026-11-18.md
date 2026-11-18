# Reliquary 2.1 REL dependency port acceptance

Parent index: [Acceptance evidence](./INDEX.md)

## Purpose

Record the local checks for durable REL dependency declarations, authorized resolution and inherited request context.

## Overview

The implementation starts from `aa724265` and adapts the declaration and validation code from `f431401e`. It uses the existing REL writer, Gateway mount and grant boundaries, and context workflow. The durable format adds an owner-local `RELDEP01` declaration generation to the existing container framing.

## Verified behavior

- The owner-session fixture opened an existing Connection-scoped REL at generation zero, verified byte-preserving reads, and replayed canonical replacement and clearing. A malformed dependency frame prevented reopen. The PHY admission fixture refused a wrapped REL dependency record.
- The Root fixture resolved a diamond with a deeper ancestor, excluded a branch behind a denied owner and kept an unrelated REL out of context. It exercised grant checks, a missing mount, a stale expected generation and competing reciprocal edits. The actual request carried eligible Ego syntheses in resolution order. Compaction omitted inherited text. Mount replacement, Memory-cut changes, dependency changes during compaction and credential revocation invalidated the corresponding prepared request.
- The maintenance fixture retained both declarations byte-for-byte in prefix reconciliation, reclamation, same-owner rewrite and a divergent copy whose dependencies were in the shared prefix. It refused an identity-changing rewrite and a divergent dependency tail before publication.
- Gateway traversal now uses the existing uncertain-write fence before reading a mounted declaration. A dependency update that may have reached the file therefore cannot leave another update validating against the previous declaration before reopen.

## Fresh-process CLI run

The CLI from the pre-port build created four small REL 2.1 files with Organization, Connection, Project and Unspecified scope. The updated CLI verified each file without changing its hash. It read the Organization REL at generation zero, appended two direct dependencies at generation one, and then reopened it in separate `list`, `resolve` and `verify` processes.

| Check | Observed result |
| --- | --- |
| Direct declaration | Project and Connection owners in canonical owner ID order. |
| Authorized order | Unspecified shared dependency, Project, Connection, then the Organization primary. |
| Order with only the primary mounted | The primary alone. |
| Primary compatibility | The original 206 bytes were present at the start of the 282-byte file. The owner ID and scope were identical. |
| Read-only mounts | The Project, Connection and Unspecified files matched their hashes from before the primary write and reopen. |

The primary's preserved prefix SHA-256 was `C8EE20F6D15E7FCC2C740DC840E309EDAA399411848737347D375C822B2A4BF6`. The saved file SHA-256 was `23DB82E8D5B09020E437A36E12B828659B8043ABF5DD3B17BBE38B31141C28CF`. The script, fixture files and machine-readable results are in `.test-tmp/rel-dependency-port-2026-11-18/`.

## Integration checks

| Command | Result |
| --- | --- |
| `cargo fmt --all --check` and `cargo fmt --manifest-path cli/Cargo.toml --check` | Passed. |
| `cargo test --workspace --locked --offline -j 1 --no-fail-fast -- --test-threads=1 --skip tests::exclusive_lease_blocks_other_processes` | 688 passed with one pre-existing Windows privilege-dependent test filtered. This run preceded the final uncertain-write guard. |
| `cargo test --manifest-path cli/Cargo.toml --locked --offline -j 1` | 17 passed with the final guard. Its CLI binary ran the fresh-process check. |
| `cargo check --workspace --locked --offline -j 1` | Passed with the final guard. |
| `cargo test -p reliquary-owner-session --lib --locked --offline -j 1 rel_dependency_declarations_preserve_legacy_identity_and_replay` | Passed with the final guard, including refusal of an uncertain writer's dependency projection. |
| `cargo test --test rel_dependencies --test g4_maintenance --locked --offline -j 1 rel_dependency_ -- --test-threads=1` | Both the Root graph/context test and maintenance history test passed with the final guard. |
| `python scripts/check_architecture.py` | 23 packages, 76 production edges, 15 test-only edges. |

## Related docs

- [REL dependency specification](../planning/rel-dependency-port-spec-2026-11-18.md)
- [Execution plan](../planning/rel-dependency-port-plan-2026-11-18.md)
- [Public product API](../public-product-api.md)
- [CLI reference](../reference/cli.md)

## Notes

The full workspace invocation filters the existing container test that requires Windows symlink creation privilege. The fixtures are synthetic.
