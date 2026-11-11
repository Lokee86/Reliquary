# Engineering Standards Maintainer Map

Parent index: [Engineering Standards](INDEX.md)

## Purpose

This document routes maintainers to the canonical documentation and implementation boundary for common changes to shared engineering standards.

## Overview

Use this map when the owning standard or enforcement surface is unclear. It is a navigation aid, not a replacement for normative standard pages, checker implementation, Pitlord policy, templates, or repository-local decisions.

## Change-area routing

| Change area | Canonical documentation | Primary implementation boundary | Verification |
| --- | --- | --- | --- |
| Documentation taxonomy, required shapes, and ownership rules | [Documentation standard](documentation-standard.md) | `docs/documentation-standard.md` | Shared checker self-check |
| Repository-type documentation requirements and capabilities | [Repository profiles](profiles.md) | `docs/profiles.md`, `templates/docs-standard.json` | Shared checker self-check |
| Documentation workflow and agent behavior | [Documentation procedure](documentation-procedure.md) | `docs/documentation-procedure.md`, `skills/documentation/SKILL.md`, `templates/AGENTS-documentation.md` | Shared checker self-check |
| Documentation change-impact rules | [Change-impact rules](change-impact.md) | `docs/change-impact.md`, repository `docs-standard.json` mappings | Changed-from checker tests |
| Documentation completeness claims and legacy baselines | [Completeness and status claims](completeness.md) | `tools/docs_policy/baseline.py`, `tools/docs_policy/check.py` | `tools/docs_policy/tests/` |
| Documentation required paths, indexes, links, and coverage | [Adoption and enforcement](adoption.md) | `tools/docs_policy/`, `policies/pitlord/documentation-core.json` | `tools/docs_policy/tests/` and self-check |
| Vendored engineering-standard snapshots | [Adoption and enforcement](adoption.md) | `tools/sync_checker.py` | Isolated sync and repository check |
| Architecture principles and minimum evidence | [Architecture standard](architecture/architecture-standard.md) | `docs/architecture/architecture-standard.md` | Standards self-check and architectural review |
| Pitlord architecture enforcement, CI, and rollout | [Architectural enforcement with Pitlord](architecture/enforcement.md) | `policies/pitlord/architecture-core.json`, repository `tools/pitlord/` | Pitlord policy validation and repository gate |
| Responsibility ownership and dependency direction | [Ownership and dependency direction](architecture/ownership-and-dependency.md) | Repository Pitlord areas and dependency rules | Pitlord plus architectural review |
| Packages, seams, interfaces, helpers, and abstractions | [Seams and abstractions](architecture/seams-and-abstractions.md) | Canonical architecture docs and repository policy where statically detectable | Review, Pitlord, and focused tests |
| Mutable state, lifecycle, concurrency, and shutdown | [State, lifecycle, and concurrency](architecture/state-lifecycle-and-concurrency.md) | Owning runtime packages and focused tests | Behavioral and concurrency tests |
| Persistence, generated state, processes, protocols, and migration | [Data, processes, and protocols](architecture/data-processes-and-protocols.md) | Owning contracts, repository policy, and migration tests | Pitlord, contract, and migration review |
| Failures, degradation, observability, health, and recovery | [Resilience, observability, and operations](architecture/resilience-observability-and-operations.md) | Owning runtime and operations boundaries | Operational and failure tests |
| Package, service, monorepo, and umbrella-product boundaries | [Repository and component structure](architecture/repository-and-component-structure.md) | Repository Pitlord areas and component policies | Pitlord plus architectural review |
| Invariant tests, exceptions, and architectural debt | [Testing, evolution, and decisions](architecture/testing-evolution-and-decisions.md) | Repository tests, Pitlord policy, explicit exceptions, and baselines | Test, policy, and migration evidence |
| ADR structure, status, and supersession | [Architectural decision records](architecture/decision-records.md) | Repository `docs/decisions/` and `templates/adr.md` | Decision index, affected docs, policy, and protecting evidence |
| Architecture audit evidence and report shape | [Architecture audit reports](architecture/audit-reports.md) | `templates/architecture-audit-report.md` and repository-local reports | Revision, scope, evidence, remediation, and rerun verification |
| Architecture design and review workflow | [Architecture procedure](architecture/architecture-procedure.md) | Canonical docs, repository Pitlord policy, and focused gates | Review checklist and repository gate |

## Boundaries

- Normative rules belong in `docs/`; shared enforcement mechanics belong in `tools/` and `policies/`.
- Repository-specific architecture areas and rules belong in the repository that owns the architecture.
- Pitlord owns deterministic architectural enforcement; focused tests and runtime scenarios own non-static behavior.
- Templates illustrate compliant repository-local surfaces but do not override normative documents.
- Vendored `.standards/` directories are generated snapshots and are not edited directly.
- Repository-local policies and ADRs may specialize shared standards only through explicit documented decisions.
- Do not bulk-propagate repository-specific architecture policy without an explicitly approved rollout.

## Related docs

- [Documentation standard](documentation-standard.md)
- [Architecture standard](architecture/architecture-standard.md)
- [Architectural enforcement with Pitlord](architecture/enforcement.md)
- [Documentation procedure](documentation-procedure.md)
- [Architecture procedure](architecture/architecture-procedure.md)
- [Adoption and enforcement](adoption.md)

## Notes

Use this map to find the owner first. Use the owning standard, implementation, policy, tests, or repository-specific ADR for the detailed contract.