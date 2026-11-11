# Engineering Standards

Parent index: [Engineering Standards Repository](../README.md)

## Purpose

This index is the entry point for shared documentation and architectural standards.

## Overview

The documentation standard governs knowledge ownership, document types, structure, implementation coverage, change impact, agent behavior, compliance checks, and status claims.

The architecture standard governs responsibility ownership, dependency direction, seams, state, lifecycle, processes, protocols, failure handling, observability, repository structure, verification, migration, architectural decisions, and Pitlord enforcement.

## Documentation standards

- [Documentation standard](documentation-standard.md) — Normative rules for documentation ownership, taxonomy, shape, coverage, and lifecycle.
- [Documentation procedure](documentation-procedure.md) — Required workflow for creating, changing, moving, graduating, and removing documentation.
- [Maintainer map](maintainer-map.md) — Navigation from common standards changes to their canonical documents and implementation boundaries.
- [Repository profiles](profiles.md) — Required document surfaces for libraries, CLIs, services, applications, games, and umbrella products.
- [Change-impact rules](change-impact.md) — How implementation changes trigger documentation changes in the same work.
- [Completeness and status claims](completeness.md) — Evidence required before calling documentation complete, current, or compliant.
- [Adoption and enforcement](adoption.md) — Repository configuration, Demon Docs, Pitlord, CI, and rollout responsibilities.

## Architecture standards

- [Architecture standards](architecture/INDEX.md) — Core architectural rules and focused standards for ownership, seams, state, protocols, operations, structure, verification, evolution, and Pitlord enforcement.

## Related docs

Repository-specific architecture and documentation remain owned by each product repository. This repository defines shared judgment, procedure, and expected enforcement rather than centralizing every product's current design.

## Notes

The documentation standard has shared deterministic enforcement. Pitlord is the expected deterministic architecture-enforcement mechanism, with repository-specific semantic policy adopted through explicit repository rollout.