# Offline REL and PHY history migration specification

Parent index: [Planning](./INDEX.md)

## Purpose

Bring the retained history of the original 28-day REL and PHY into native Reliquary 2.1. The working 28-day snapshot carried over the current REL state and started each Memory at revision one. The original pair still holds the older transcripts, revisions, and processing history needed for historical reads and provenance.

## Overview

The first acceptance inputs are `project.prj.rel` and `user.phy` under `reliquary-fixtures/local/authoritative/chatgpt-first28d/`. Bind the run to both SHA-256 pins in that directory's `SHA256SUMS.txt` and read the owners through verified disposable copies. The deliverables are an offline converter, an independent verifier, two new native 2.1 files, and a paired verification receipt. This acceptance applies to the two pinned inputs.

Core baseline is `184857a5`. Recover the converter and verifier at `5f2b1a7c`, the source witness work at `3b7cab0b` based on the FRS07 reader at `c6010fd3`, and the existing migration handoff at `bc9ec7f0` on `continuation/d01-d03`.

## Conversion and verification architecture

- **Old-source process.** The isolated F03 reader exports typed source values and their original clocks, scope, and references. Reuse its custody and independent comparison scripts to freeze the expected history for these two file hashes before generating candidates.
- **Native writer.** Reuse the D01 composer in `tools/d01-converter/compose_authentic.py` and the separate `target-writer` package. The composer orders a private, hash-bound transfer and accounts for each observed source frame and its contained records. The writer encodes each record through its native store and validates the file through normal owner-session replay. Reuse `append_bound_publication_envelope` for reservations and `commit_embedded_version_range_at` for grouped clocks.
- **Independent verifier.** Reuse the D02 reader and comparison scripts in `tools/d02-parity/`. Read the candidates through current-only 2.1 admission in another process. Compare that output with the frozen source observations and recompute the referenced values independently. Conversion receipts supply custody and relocation claims for the verifier to check.

Both outputs use current 2.1 headers, records, and normal owner admission. Start with current encoders and replay in the owning store. Port the semantics of archived Archive bindings, retained Freshness authority, Entity Resolution, Community lineage, and grouped publications into those owners. Add a canonical native record only for retained information that current records cannot express, and include it in owner-kind validation, historical inspection, and the affected maintenance paths. The FRS07 reader and transfer schema belong to the offline tools.

## History and identity contract

| Applies to | Required preservation |
| --- | --- |
| Both owners | Durable owner ID and kind, immutable body and record IDs, exact retained revisions and terminal states, original event times, and published values at the corresponding historical cuts. Include the observed Entity, Relationship, Graph, Community, and vector histories, including membership, profile, retirement, and receipt bindings. |
| REL | Archive bodies, branches, Episodes, and first-applying publication bindings. Resolve Memory provenance back to the recorded turn and evidence range. Carry the retained Insomnia and Dream work, import and Freshness journals and receipts, and any stored project or file bindings. |
| PHY | Private Memory bodies and revision history, routing and retained owner or Ego state where present, and provenance back to the exact authorized paired REL when the source binds one. Preserve PHY ownership and native grant boundaries. |

Keep the original publication order, local and global versions, signed or absent timestamps, reservations, and atomic publication groups. Preserve source lineage in the output and receipt. Use native 2.1 generation and cut tokens, with a verified correspondence for values derived from the encoding. Store a recognized source scope as inert 2.1 metadata and record an absent source scope as such in the receipt, with the neutral header value in the output. Identify each specimen by source file hash and owner ID because the 14-day and 28-day variants share IDs.

Account for the history actually present in the source. Preserve recorded pruned, tombstoned, and unavailable states at their original positions. Carry accepted semantic values and opaque proof evidence from the source. Use deterministic derivation only where the existing records prove the result. An unknown carrier, missing mapping, or unexplained difference stops acceptance.

Relocate file-backed references through the existing D01 mapping pattern. Resolve each referenced source object by its file identity, bounds, and digest. Check the native reference against its owning file, semantic ID, and payload after rewrite. Preserve exact vector values and their generation receipts. Apply the same rule to retained Archive backing and attachment references where present.

## Publication and completion

Reuse the writer's same-directory staging, sync, fresh reopen, and create-only publication. Bind each candidate to its source hash, owner ID and kind, versions, transfer digest, target digest, and relocation evidence. Sync and publish a create-only pair receipt after both candidates pass their independent comparisons. A failed run produces a scoped diagnostic and leaves any partial output unaccepted. A retry uses verified copies and a fresh destination. Keep payloads and transfers private in the local scratch area.

Completion requires both native owners to reopen independently, all observed families and references to compare, and every supported historical cut to agree in value or in its source-recorded availability. Compare the ordered events and cut membership directly so intermediate changes are covered even when the final state agrees. Confirm that the source and candidate hashes are stable across the relevant read-only runs. The [execution plan](owner-history-migration-plan-2026-11-18.md) defines the focused checks and the evidence to record.

## Related docs

- [Execution plan](owner-history-migration-plan-2026-11-18.md)
- [Storage architecture](../architecture/storage.md)
- [Owner sessions](../architecture/owner-session.md)
- [Current compatibility limits](../limits/current-limitations.md)

## Notes

Delivery ends with the verified pair and its receipts in a disposable workspace. The existing 28-day 2.1 snapshot at `8c373364` records the earlier current-state transfer. Pin the original pair for this migration.
