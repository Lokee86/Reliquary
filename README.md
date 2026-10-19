# Reliquary Memory v2

Reliquary is the Rust semantic storage and runtime subsystem used by Warlock for durable project/context state. Phylactery is the companion user-global memory owner.

Reliquary is a library/engine, not a standalone product UI. Warlock owns project creation, application orchestration, repository integration, and presentation; Reliquary owns typed REL/PHY semantic state, persistence, retrieval primitives, and bounded runtime behavior.

## Current model

A current Project Reliquary is stored as a typed `.prj.rel` file. Organization and Connection Reliquaries use `.org.rel` and `.con.rel`; Phylactery uses `.phy`. Legacy `.cva` files remain readable as legacy Project Reliquaries and can be explicitly migrated into the current typed format.

Project files and semantic state have separate authorities:

```text
Warlock project
├── Lore or Git repository
│   └── project files and project-file history
└── .warlock/<project>.prj.rel
    └── transcript, provenance, Memory, Graph, Echo,
        semantic history, retrieval state, and runtime state
```

Lore is Warlock's default managed project repository. Git is the explicit user-owned repository case. Reliquary records repository-neutral revision correlations and exact historical project-file references without duplicating repository-backed file payloads inside the REL.

## Implemented now

The current implementation includes:

- typed Reliquary and Phylactery files with durable owner UUIDs and explicit Project/Organization/Connection scope identity;
- explicit legacy REL/PHY migration and current-format reopen validation;
- Archive source history, conversation metadata, branch topology, exact transcript reconstruction, active-branch search, fragments, deterministic Episodes, attachments, and file-to-Memory provenance;
- repository-backed Project revision correlation and historical `ProjectFileRef` attachment bindings;
- durable assistant-stream checkpoints, interruption recovery, conversation compaction, and turn-attached Echo execution evidence;
- authoritative immutable Memory bodies/revisions with provenance and source chronology;
- owner-local Graph relationships, Dream classification/verification/publication, lifecycle projection, duplicate/supersession handling, and deterministic temporal analysis;
- owner-local Leiden Community snapshots and validated Community-routed Memory retrieval with exact fallback;
- Insomnia extraction, bounded historical evidence, durable completion receipts, User/Project routing, REL→PHY publication, retry/backpressure handling, and automatic Memory-vector completion;
- packed vectors, compatibility profiles, Archive/Memory vector bindings, vector generations, exact semantic retrieval, lexical retrieval, and hybrid Archive search;
- machine-local `reliquary.cfg`, encrypted provider credentials, model capability routing, OpenAI-compatible transport, and ChatGPT/Codex device authorization;
- `InteractionRuntime`, `ReliquaryRuntimeHost`, and configured runtime routes for long-lived Warlock integration;
- current-format REL comparison, divergent semantic reconciliation, typed conflicts, safe promotion, and derived-vector rebuild;
- a detachable repository-local CLI for development, migration, import, configuration, Insomnia, and vector workflows.

Exact current contracts are intentionally owned by the focused documentation rather than duplicated here.

## Current boundaries

Reliquary does **not** own the Warlock UI, project-file VCS, hosted coordination, or cross-owner final context composition. Lore/Git owns project-file history. Warlock owns project/repository bootstrap and the application lifecycle around Reliquary. Ego will own higher-level cross-owner context synthesis.

The REL semantic timeline remains independent of project-repository history. Repository revisions can be correlated with REL semantic cuts, but neither history is authoritative over the other.

Known incomplete areas include production OS credential-store integration, broader provider/import adapters, richer file-management surfaces, cross-owner context composition, whole-REL historical restore/branching, generalized concurrent writers, and production storage hardening. See [Current limitations](docs/current-limitations.md) for the authoritative list.

## Architecture rule

> Defer mechanics, not ownership.

Durable responsibilities have explicit owners. Archive, Memories, Graph, project-revision correlation, Echo, vector state, Insomnia operational state, and other persisted domains keep their own contracts instead of being collapsed into a generalized semantic database. Project repository history remains a separate authority.

## Development

The core crate and repo-local CLI are verified separately. Start with [Development](docs/development.md) for current commands, fixtures, and test gates.

## Documentation

Start with:

- [Operator and integration manual](docs/manual/INDEX.md) — how to use Reliquary.
- [Documentation index](docs/INDEX.md)
- [Architecture](docs/architecture.md)
- [Storage format](docs/storage-format.md)
- [Rust API](docs/api.md)
- [Maintainer map](docs/maintainer-map.md)
- [Behavioral contracts](docs/behavioral-contracts.md)
- [Current limitations](docs/current-limitations.md)
- [Roadmap](docs/roadmap.md)

Architectural rationale is retained separately in [ADRs](docs/decisions/INDEX.md). Implemented behavior belongs in current architecture/reference documents; future and unresolved work belongs in the roadmap.

## License

Reliquary is licensed under the [PolyForm Shield License 1.0.0](LICENSE.md). It permits use, modification, and distribution for permitted purposes, but does not permit using the software to provide a competing product.
