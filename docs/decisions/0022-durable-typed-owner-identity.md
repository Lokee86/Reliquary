# ADR 0022: Durable typed owner identity

## Status

Accepted for durable header UUID identity — 2026-08-26. The typed REL class/prefix scheme in this ADR was superseded by ADR 0029 on 2026-09-04: new homogeneous RELs use `rel-<uuid>`, while legacy typed RELs retain `proj-`/`org-`/`con-` only to preserve historical identity. The `phy-<uuid>` Phylactery identity and header-UUID durability remain current.

Supersedes the WorkspaceMetadata identity/type portions of ADR 0017 and the WorkspaceMetadata reconciliation boundary in ADR 0019. Their remaining product-host and reconciliation decisions stay in force.

## Purpose

Make durable REL/PHY identity a creation invariant and remove optional workspace metadata from the core storage architecture.

## Context

Reliquary reconciliation originally depended on `WorkspaceMetadata.id`. Ordinary REL creation did not require that record, so valid current files could exist without the identity required by reconciliation. Phylactery had no equivalent stable file identity at all.

The same metadata record also carried a display name and one workspace type. Display naming can be derived from the filename/host, while a single workspace/domain type is too restrictive for overlapping and unconventional uses such as fiction plus research. Optional domain adapters belong to the host/capability layer and may overlap without restriction.

## Decision

Every newly created typed REL or PHY receives a UUID in the container header before creation returns.

At the time of this decision, the canonical durable owner ID was derived from authoritative type/scope plus that UUID:

```text
Project REL       proj-<uuid>
Organization REL  org-<uuid>
Connection REL    con-<uuid>
Phylactery        phy-<uuid>
```

The prefix is not stored independently. REL/PHY kind and REL scope already live in the typed header, so the canonical ID is derived rather than duplicated.

Copy, move, rename, cloud-conflicted replication, and reconciliation repack preserve the UUID. Intentionally creating a new durable owner generates a new UUID.

`WorkspaceMetadata`, its store/codec/rebuild machinery, and the `create_workspace` lifecycle are removed from the current runtime. Filenames may provide display names to Warlock. Optional domain adapters/capabilities are separate, unrestricted, and not part of core identity.

Reconciliation compares canonical owner IDs. A file without a durable owner UUID must be migrated before owner-ID-based reconciliation.

Cross-file object references may use the minimal form:

```text
owner_id + local_object_id
```

For Memories this becomes `owner_id + MemoryId`.

## Physical format

The earlier typed header was 24 bytes. The current typed header is 40 bytes and preserves the original first 24 bytes:

```text
0..24   existing magic/version/length/kind/scope/reserved fields
24..40  owner UUID bytes
```

Legacy 16-byte CVA headers and earlier 24-byte typed REL/PHY headers remain readable and are migrated explicitly through a semantic repack into a new 40-byte file. Migration never rewrites the source in place.

## Consequences

- Durable identity exists independently of optional application metadata.
- REL and PHY use the same simple identity rule.
- Reconciliation no longer needs `WorkspaceMetadata.id` or missing-workspace-metadata errors.
- Repacking preserves one UUID rather than cloning an unrelated metadata record.
- Display names and domain adapters cannot accidentally become semantic identity.
- Existing reconciliation tests that manually manufacture `WorkspaceMetadata` must be rewritten around copied typed REL fixtures.
- Old files without header UUIDs require the explicit migration operation before reconciliation.

## Implementation boundary

Current implementation lives in:

- `src/container/container.rs`
- `src/container/container_scan.rs`
- `src/facade/cva_lifecycle.rs`
- `src/facade/phylactery_lifecycle.rs`
- `src/facade/cva_reconcile.rs`
- `src/facade/cva_reconcile_repack.rs`
- `src/facade/migration.rs` + private `src/facade/migration_rel.rs` / `src/facade/migration_phy.rs`

The former `workspace_metadata*.rs` and `cva_workspace.rs` subsystem is removed.
