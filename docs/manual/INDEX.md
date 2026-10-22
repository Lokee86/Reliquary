# Reliquary Operator and Integration Manual

This manual is the task-oriented entry point for using Reliquary. It explains **what to do** and **in what order**. Architecture, storage encoding, and decision rationale remain in the canonical reference docs.

## Start here

- [Getting started](getting-started.md) — create and inspect REL/PHY files and understand the available interfaces.
- [Core concepts](core-concepts.md) — REL, PHY, Archive, Memory, Entity, Graph, Episodes, Dream, Insomnia, and Perception.
- [Processing pipeline](processing-pipeline.md) — how source turns become Memories and derived semantic structure.
- [Working with Memories and Entities](memories-and-entities.md) — publish, revise, inspect, and associate durable semantic objects.
- [Graph and Communities](graph-and-communities.md) — full semantic Graph vs the Memory-only Dream/Leiden projection.
- [Retrieval](retrieval.md) — Archive search, Memory-Web retrieval, vectors, Communities, and stale-index handling.
- [Migration and reconciliation](migration-and-reconciliation.md) — move old files forward and merge divergent REL copies.
- [Integration guide](integration-guide.md) — embed Reliquary behind a product/runtime surface without duplicating its ownership logic.
- [Troubleshooting](troubleshooting.md) — common failures and the correct recovery path.

## Status convention

The manual distinguishes current capability from incomplete work:

- **Current** means implemented and covered by the repository's current contracts/tests.
- **Not yet wired** means supporting primitives exist, but the normal production workflow does not yet connect them end to end.
- **Planned** means future behavior owned by the roadmap or subsystem plan.

Do not infer current capability from an ADR or design plan alone.

## Manual rule

> Workflows belong here. Architecture belongs in the reference docs.

The manual may tell you to publish an Entity association with the current Graph version. The architecture/API/storage docs define why that version exists, its persistence format, and its invariants.

## Reference documentation

- [Documentation index](../INDEX.md)
- [Architecture](../architecture.md)
- [Rust API](../api.md)
- [Repo-local CLI](../cli.md)
- [Current limitations](../current-limitations.md)
- [Roadmap](../roadmap.md)
- [Architectural decisions](../decisions/INDEX.md)


## Direct Files

<!-- doc-ledger:files:start -->

- [core-concepts.md](core-concepts.md) - Core Concepts documentation.
- [getting-started.md](getting-started.md) - Getting Started documentation.
- [graph-and-communities.md](graph-and-communities.md) - Graph And Communities documentation.
- [integration-guide.md](integration-guide.md) - Integration Guide documentation.
- [memories-and-entities.md](memories-and-entities.md) - Memories And Entities documentation.
- [migration-and-reconciliation.md](migration-and-reconciliation.md) - Migration And Reconciliation documentation.
- [processing-pipeline.md](processing-pipeline.md) - Processing Pipeline documentation.
- [retrieval.md](retrieval.md) - Retrieval documentation.
- [troubleshooting.md](troubleshooting.md) - Troubleshooting documentation.
<!-- doc-ledger:files:end -->

## Stub Files

<!-- doc-ledger:stubs:start -->
<!-- doc-ledger:stubs:end -->

## Direct Folders

<!-- doc-ledger:folders:start -->
<!-- doc-ledger:folders:end -->