# Architecture Standards

Parent index: [Engineering Standards](../INDEX.md)

## Purpose

This index is the entry point for shared architectural standards.

## Overview

These standards define how responsibilities, dependencies, state, seams, processes, failure handling, repository structure, and architectural evolution should be designed and enforced across Laughing Skull projects.

Pitlord is the expected deterministic architecture-enforcement mechanism. Repository-local policies encode actual ownership areas and dependency invariants; focused tests and runtime scenarios protect behavior Pitlord cannot infer from static evidence.

## Direct files

- [Architecture standard](architecture-standard.md) — Core architectural rules and the minimum evidence required for a sound design.
- [Architectural enforcement with Pitlord](enforcement.md) — Expected enforcement mechanism, policy ownership, rule coverage, CI, and rollout.
- [Ownership and dependency direction](ownership-and-dependency.md) — Responsibility ownership, non-ownership, public boundaries, and dependency flow.
- [Seams and abstractions](seams-and-abstractions.md) — When to add seams, packages, interfaces, helpers, wrappers, and extension boundaries.
- [State, lifecycle, and concurrency](state-lifecycle-and-concurrency.md) — State ownership, transitions, mutation, scheduling, shutdown, and recovery.
- [Data, processes, and protocols](data-processes-and-protocols.md) — Sources of truth, generated state, process boundaries, contracts, compatibility, and migration.
- [Resilience, observability, and operations](resilience-observability-and-operations.md) — Failure behavior, diagnostics, health, recovery, and operational visibility.
- [Repository and component structure](repository-and-component-structure.md) — Package, service, application, monorepo, and independently usable component boundaries.
- [Testing, evolution, and decisions](testing-evolution-and-decisions.md) — Architectural invariants, Pitlord policy, contract tests, migrations, ADRs, exceptions, and debt.
- [Architectural decision records](decision-records.md) — Repository-local ADR structure, status, supersession, required content, and completion rules.
- [Architecture audit reports](audit-reports.md) — Evidence, severity, caveats, remediation ordering, and the standard audit report shape.
- [Architecture procedure](architecture-procedure.md) — Required reasoning, enforcement, and review workflow for architectural changes.

## Related docs

- [Engineering Standards Maintainer Map](../maintainer-map.md)
- [Adoption and enforcement](../adoption.md)
- [Documentation standard](../documentation-standard.md)
- [Documentation procedure](../documentation-procedure.md)

## Notes

Repository-specific architecture remains owned by each repository. These documents define how that architecture is judged and enforced, not what every product's area graph must contain.