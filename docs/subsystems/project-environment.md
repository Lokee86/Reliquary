# Project Environment

Parent index: [Subsystem documentation](./INDEX.md)

## Purpose

Describe current project repository/file ownership and the Git/Lore integration boundary used by Reliquary and Warlock.

## Overview

Project Environment resolves project repository roots, revision identity, file references, upload materialization and repository-specific checkpoint behavior through one private `ValidatedProjectScope`. Upper lifecycle composition creates/opens RELs and publishes initial correlations; Project Environment returns repository preparation facts and rollback capability. Project VCS history remains owned by the selected repository backend rather than being recreated inside REL semantic history.

## Repository modes

Current repository modes distinguish:

- managed Lore;
- externally/manual managed Lore; and
- Git.

The mode derives from durable project revision correlation and repository-management policy.

## Project files

`ProjectFileRef` identifies project content through repository/revision semantics rather than unstable ambient paths. Reads and uploaded attachments resolve through the project repository boundary.

The scope binds the selected canonical local mount to the independently accepted backend kind, repository ID and current repository-relative project subtree. Historical reads additionally require the referenced subtree to occur in that REL's accepted correlation history. File paths use strict repository-relative components, exclude reserved state and cannot escape their accepted subtree. Historical files can be absent from the working tree.

Upload preparation validates the current scope before backend mutation, excludes sibling dedupe candidates, and materializes only inside the selected project. Gateway captures the owning binding/history under the owner lock, performs repository I/O outside it, and reauthorizes delivery/publication. Repository proof and mounted `ViewRel`/`ManageProject` grants are independent checks.

## Creation and adoption

Project creation establishes the selected repository mode and the REL/project correlation. Adoption validates an existing project source before attaching it to Reliquary.

The unassociated creation/adoption exception uses the explicitly selected folder and existing lifecycle admission; it is not available for an already associated REL. Git bootstrap preserves HEAD and index. Lore initialization and explicit manual checkpoints retain existing policy.

Whole-checkout moves work when durable identity and the relative selected subtree stay unchanged. The existing caller contract cannot distinguish a changed subtree from a sibling selection, so validation refuses that change. Previously enrolled paths remain historical read authority only. A future caller must supply independent owner/mount relocation evidence before relative-subtree reassociation can be supported.

## VCS authority

Git/Lore own project file ancestry and revisions. REL/PHY own semantic Memories, Graph, Entities, Observations, provenance, and their own crash/history semantics.

Reliquary may retain exact project revision/file references, but it must not grow a duplicate project VCS merely to support semantic provenance.

## Failure behavior

- invalid/missing repository roots fail explicitly;
- unsupported or conflicting repository state is not auto-rewritten;
- project reads bind to exact known revision semantics;
- upload/materialization errors do not create semantic publication; and
- retry cannot silently retarget a different repository revision.

## Code Map

- `src/project_environment.rs`
- `src/project_environment/**`
- `src/facade/cva_project_environment.rs`
- `src/runtime_inference/runtime_host_gateway_ambient_project.rs`
- `src/project_environment_*_tests.rs`

## Related docs

- [Architecture](../architecture.md)
- [Roadmap](../roadmap.md)
- [ADR 0027](../decisions/0027-warlock-project-repositories-and-reliquary-storage-boundary.md)
- [ADR 0028](../decisions/0028-project-folder-and-repository-bootstrap-contract.md)
- [ADR 0037](../decisions/0037-reliquary-context-memory-project-environment.md)

## Notes

Archive project correlations, exact file bindings and ordinary embedded files remain semantic history. No duplicate repository tree/ancestry implementation was identified for deletion. Stored PRJCOR02/PRJFILE1 bytes and durable Warlock-named backend markers are unchanged.
