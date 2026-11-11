# Deferred: homogeneous RELs and inter-REL dependency graph

Parent index: [Documentation](./INDEX.md)

## Purpose

Restore the previously accepted REL architecture without holding up the Hermes integration test. RELs are homogeneous memory owners. Organization, project, subproject, connection, and similar terms are **not fixed REL classes**. A directed acyclic dependency graph between REL owners supplies ambient context inheritance.

## Overview

**Decision authority:** ADR 0029, *Homogeneous Reliquaries and dependency-based context inheritance* (accepted 2026-09-04, preserved in Git as blob `8129a84b0f77c51945e3b9892cc0ced2b91b5a2`), and ADR 0037, *Reliquary owns the Context/Memory/Project Environment* (accepted 2026-09-24, Git blob `8706a2b0ccfa88c02a09e4f6400125d00c6ec07`). Those design records are not in the current working documentation tree; read the archived decisions before implementation.

**Observed rebuild debt:** current REL 2.1 retains `OwnerScope::{Unspecified, Organization, Project, Connection}` in owner-session/container metadata, the CLI defaults REL creation to `Project`, and the Hermes host rejects certain valid RELs according to the header scope. This contradicts ADR 0029's prohibition on giving REL type labels behavioral authority. The original ADR also requires zero-or-more directed dependency edges by durable REL owner ID; current rebuild support and complete acceptance for that graph have not been established.

## Deferred correction

1. **Audit actual dependencies and capabilities.** Inventory every scope-dependent check, persisted header field, CLI flag/default, API argument and test in the rebuilt Reliquary and adapters. Separately audit the state of durable inter-REL dependency edges and mounted dependency closure. Do not conflate the internal semantic Memory/Entity graph, existing project-to-REL routing, or host `Topology.projects[*].parent_id` with ADR 0029's arbitrary multi-parent REL dependency DAG.
2. **Make REL semantics homogeneous.** Remove organizational scope as an access/behavior predicate. A REL's host role comes from an authorized mount and route, not its persistent scope byte. Retain optional, unconstrained `type` as UI metadata only where the original ADR specifies it. Do not create organization/project/connection subclasses.
3. **Maintain old owner compatibility.** Current files may contain legacy scope bytes; decide and test a precise read/write transition without overwriting or rekeying any existing owner. No "change scope" operation is needed merely to mount a file. Preserve owner IDs, semantic history, provenance, and writer authorization. If eliminating the field requires a new format, specify and independently accept that format before writing it.
4. **Restore/verify inter-REL dependency DAG behavior** inside Reliquary: multiple dependencies, arbitrarily deep closure, cycles and unresolved owner checks, deterministic dependencies-before-dependent composition, and sibling exclusion from ambient context. Ownership and writing remain local; neither dependency edges nor type metadata confer write rights or repository access.
5. **Update all host adapters and CLI** to use REL owner identity plus explicit role/mount/dependency policy. Remove the temporary Hermes scope exception only after the underlying product and full consumer contract consistently ignore legacy scope. Preserve profile security, allowed owners, and accepted session routes.
6. **Acceptance:** test old project/organization/connection-scoped 2.1 files as ordinary RELs in General and dedicated host roles, exact read/verification and byte preservation, cross-owner graph cycles/multiple parents/sibling isolation, stable retrieval/provenance, and format/CLI interop. Include real corpus copies and external-consumer tests.

## Immediate bridge (already scoped separately)

The Hermes plugin temporarily permits a valid legacy project-scoped REL as **General memory** by relaxing only the main-owner scope restriction in its host. Existing project-specific REL restrictions and the legacy format stay unchanged until the broader correction. This is not an architectural fix, nor permission to rewrite existing RELs.

## Related docs

- [Operator workflow](operator-workflow.md)
- [Public API](public-product-api.md)
- [Project correlation acceptance](project-correlation-acceptance-2026-11-05.md)

## Notes

**Roadmap only.** The current task does not change Reliquary core, on-disk format, owner IDs, graph runtime, or the historical two-day corpus. The deferred correction must be implemented against the original ADRs and current acceptance gates, not reconstructed from obsolete `OwnerScope` values.
