# Leiden Community recomputation execution plan

Parent index: [Planning](./INDEX.md)

## Purpose

Implement the [Leiden recomputation specification](leiden-community-recomputation-spec-2026-11-18.md) through the normal REL entry point and verify it on one real copy. The current source anchor is Reliquary `1fee2310`.

## Overview

Use a single implementation lane. Land the port, owner workflow, and CLI in dependency order. Use focused checks during development and run the applicable repository gates once at integration.

## Work packages

| Work package | Depends on | Deliverable and exit check |
| --- | --- | --- |
| 1. Restore solver and storage support | Current core baseline | Port the pure solver from `f431401e` and the existing subset codec from `dca06ffa`. Confirm the ported deterministic and modularity fixtures pass and existing subset membership reopens. |
| 2. Connect the REL workflow | 1 | Build the owner-session input and save check, Application calculation, and Gateway and Root calls. Exercise both modes in one focused Root regression using an archived Memory that still has a graph link. Check exact coverage, durable reopen, and reuse of an identical result. |
| 3. Connect CLI and verify the fixture | 2 | Wire the preview and apply modes through the existing local host, update the CLI reference and affected owner documentation, then run the copied-REL acceptance below. |

Start from the existing [Community management test](../../tests/community_management.rs) and [CLI dispatch](../../cli/src/semantic_inspection.rs). Use the existing tests around Community replay and verified copy for those touched paths. Run the repository's formatting, applicable locked offline Rust and CLI checks, and architecture check at the final integration checkpoint. Record the commands and outcomes with the local commit.

## Copied REL acceptance

1. Hash the source at `C:/!bin/workspace-data/reliquary-native-first28d-2026-11-11/project.rel`. Copy it to a separate inactive path and confirm the starting hashes match.
2. Run `reliquary rel community recompute <copy>` and then `reliquary rel community recompute <copy> --apply`. Record eligible and assigned counts, Community sizes, modularity, and the saved generation. Compare preview and apply on the unchanged input.
3. In a fresh process, reopen the copy through the upgraded core and CLI. Compare exact memberships with the accepted result, verify one assignment per eligible Memory and zero archived assignments, and verify the previous 12-community generation and its memberships are still readable. Confirm the source hash matches its starting value.

Report the commit and the copied REL path with the measured results. The [acceptance record](../acceptance/leiden-community-recomputation-2026-11-18.md) captures the run.

## Related docs

- [Leiden recomputation specification](leiden-community-recomputation-spec-2026-11-18.md)
- [Development workflow](../development/workflow.md)

## Notes

The plan uses the fixture's existing graph and precedes any separate product rollout.
