# Architecture Procedure

Parent index: [Architecture Standards](INDEX.md)

## Purpose

This document defines the required reasoning and review procedure for creating or changing architecture.

## Overview

Use this procedure for new durable responsibilities, significant refactors, cross-component integrations, new persistent state, process boundaries, protocols, concurrency systems, migrations, and consequential architectural exceptions.

The procedure is proportional. A small package seam may need a short design note; a cross-repository storage migration may require several focused documents, an ADR, contract tests, and Pitlord policy changes.

## Procedure

1. **State the problem in behavioral terms.** Describe the responsibility, pressure, failure, or change being addressed without assuming a specific implementation.
2. **Name the owner.** Identify which component owns policy, mutation, lifecycle, validation, failure, recovery, and documentation.
3. **Name the non-owners.** State which callers, coordinators, UIs, caches, adapters, and shared tools must not absorb the responsibility.
4. **Draw dependency direction.** Identify compile-time, runtime, data, and operational dependencies before and after the change.
5. **Define the seam.** Choose the smallest concrete package, interface, protocol, process, schema, or state boundary that expresses the ownership.
6. **Map state and source of truth.** Identify authoritative state, derived state, readers, writers, consistency, publication, and reconstruction.
7. **Describe lifecycle and concurrency.** Cover initialization, readiness, activation, scheduling, cancellation, shutdown, replacement, and recovery as applicable.
8. **Define contracts.** Specify validation, bounds, errors, timeouts, compatibility, versioning, and trust assumptions for public or cross-process boundaries.
9. **Design failure and degradation.** Explain partial failure, retry, idempotency, unknown completion, rollback, repair, and operator action.
10. **Add observability.** Identify correlation, logs, metrics, traces, health, diagnostics, and ownership-specific failure signals.
11. **Plan evolution.** Define migration stages, authority at each stage, compatibility paths, cutover evidence, and removal criteria.
12. **Protect invariants.** Add focused tests, contract fixtures, Pitlord rules, runtime scenarios, or release gates. Statically detectable ownership, dependency, cycle, bypass, and repository-policy invariants should be encoded in Pitlord when evidence is reliable.
13. **Record consequential decisions.** Add an ADR for expensive, cross-cutting, surprising, or exception-bearing choices.
14. **Update canonical documentation.** Update architecture, protocol, data, operations, maintainer maps, code maps, coverage, limits, planning, and Pitlord operations owners as affected.
15. **Run enforcement.** Validate repository Pitlord policy, prepare current Lexicon and Arcana evidence, run Pitlord with a bounded timeout, and run the focused behavioral and operational checks for non-static invariants.
16. **Review against the standard.** Challenge duplicate ownership, vague abstractions, hidden cycles, missing recovery, unbounded retries, indefinite migration state, and unenforced repeated violations.

## Seam selection test

Before introducing a boundary, answer:

```text
What responsibility does it own?
What changes independently behind it?
What invariant does it enforce?
What state or lifecycle does it control?
Who consumes it?
What coupling does it remove?
How is it tested or enforced?
How could it be removed if the boundary proves false?
```

If most answers are empty, the seam is probably naming indirection rather than architecture.

## Process-boundary test

Before creating a service or daemon, answer:

```text
Why must this have an independent lifecycle?
What failure or resource domain is isolated?
How is it discovered and authenticated?
What protocol is stable?
Who owns retries and deadlines?
How is readiness established?
How is state recovered after restart?
Can a package boundary satisfy the same need more cheaply?
```

## Enforcement selection test

For every critical invariant, answer:

```text
Can reliable repository or Lexicon/Arcana evidence detect this violation?
Which Pitlord area and rule owns that detection?
Which relations are reliable for the active adapters?
What composition-root or adapter exception is intentional?
What behavior remains for focused tests or runtime scenarios?
What evidence would justify adding a future rule?
```

Do not force runtime semantics into unreliable static rules. Do not leave reliable static violations to repeated manual review.

## Review checklist

```text
One responsibility owner is clear.
One authoritative mutation boundary is clear.
Non-ownership is explicit.
Dependency direction has no accidental cycle.
The seam is concrete and proportionate.
State, lifecycle, concurrency, shutdown, and recovery are covered.
Public and persistent contracts are versioned and validated.
Failure and degraded behavior preserve uncertainty.
Observability identifies the owning boundary.
Migration stages preserve explicit authority.
Critical invariants have protecting evidence.
Statically detectable invariants have Pitlord coverage or a stated evidence gap.
Documentation and ADR impact are complete.
Any exception is explicit, bounded, and owned.
```

## Completion report

Architectural work should report the following. Repository-wide audits should instead use the [architecture audit report shape](audit-reports.md).

Architectural work should report:

```text
Architecture impact:
- Responsibility owner:
- New or changed boundaries:
- State and lifecycle impact:
- Dependency direction:
- Failure and recovery impact:
- Compatibility or migration impact:
- Pitlord rules added or changed:
- Other protecting tests or gates:
- Documentation and ADRs:
- Known architectural debt:
```

## Related docs

- [Architecture standard](architecture-standard.md)
- [Architectural enforcement with Pitlord](enforcement.md)
- [Seams and abstractions](seams-and-abstractions.md)
- [Testing, evolution, and decisions](testing-evolution-and-decisions.md)
- [Documentation procedure](../documentation-procedure.md)

## Notes

Do not turn this procedure into a mandatory large design document for trivial edits. The required depth follows the durability and blast radius of the architectural decision.