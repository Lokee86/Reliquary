# Project Environment

Parent index: [Subsystem documentation](./INDEX.md)

## Purpose

Describe current project repository/file ownership and the Git/Lore integration boundary used by Reliquary and Warlock.

## Overview

Project Environment resolves project repository roots, revision identity, file references, upload materialization, project creation/adoption, and repository-specific checkpoint behavior. Project VCS history remains owned by the selected repository backend rather than being recreated inside REL semantic history.

## Repository modes

Current repository modes distinguish:

- managed Lore;
- externally/manual managed Lore; and
- Git.

The mode derives from durable project revision correlation and repository-management policy.

## Project files

`ProjectFileRef` identifies project content through repository/revision semantics rather than unstable ambient paths. Reads and uploaded attachments resolve through the project repository boundary.

Upload preparation validates/materializes the host input before the resulting file reference participates in semantic operations.

## Creation and adoption

Project creation establishes the selected repository mode and the REL/project correlation. Adoption validates an existing project source before attaching it to Reliquary.

Repository discovery must not accidentally treat the enclosing Reliquary development repository as the project being adopted.

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

- `crates/reliquary-project-environment/src/**` - Git/Lore and file operations
- `src/project_environment.rs` and `src/project_environment/lifecycle.rs` - Root lifecycle and correlation
- `src/project_environment_*_tests.rs`

## Related docs

- [Project Environment crate extraction specification](../project-environment-crate-extraction-spec-2026-10-28.md) — the extraction contract and acceptance criteria.
- [Architecture](../architecture.md)
- [Roadmap](../roadmap.md)
- [ADR 0027](../decisions/0027-warlock-project-repositories-and-reliquary-storage-boundary.md)
- [ADR 0028](../decisions/0028-project-folder-and-repository-bootstrap-contract.md)
- [ADR 0037](../decisions/0037-reliquary-context-memory-project-environment.md)

## Notes

Future simplification may move more project-history responsibilities to Lore/Git, but current project/revision/file resolution behavior remains owned here.
