# Architectural Decision Records

Parent index: [Architecture Standards](INDEX.md)

## Purpose

This document defines the repository-local ADR surface expected for consequential architectural decisions.

## Overview

Repositories keep a small indexed set of immutable decision records for choices that are expensive to reverse or cross durable ownership boundaries. ADRs explain why; current architecture documents remain authoritative for what is true now.

## Location and naming

Use a dedicated directory such as:

```text
docs/decisions/
  README.md
  0001-short-decision-title.md
  0002-next-decision.md
```

The index owns numbering, current status, supersession links, and navigation to affected canonical architecture documents. Do not place active architecture only in ADRs; ADRs explain why a decision was made, while canonical documents state what is currently true.

## Required content

Use the shared [ADR template](../../templates/adr.md). Every accepted ADR must identify:

- the responsibility owner and explicit non-owners;
- dependency direction and bypasses that remain forbidden;
- state, lifecycle, failure, recovery, and operational consequences;
- compatibility, migration, and removal conditions;
- alternatives considered;
- protecting evidence, including Pitlord impact when static detection is reliable;
- known risks, debt, and review triggers.

## Status and supersession

Use one of these states:

- **Proposed** — under review and not yet authoritative;
- **Accepted** — governs the current architecture;
- **Superseded** — replaced by a later ADR, with both directions linked;
- **Rejected** — considered but not adopted.

Do not silently rewrite the decision in an accepted ADR. Correct minor factual errors in place, but record a materially different decision in a new ADR and supersede the old one.

## When an ADR is required

Create one for expensive-to-reverse, cross-repository, persistence, protocol, process-boundary, compatibility, authority, migration, or explicit standards-exception decisions. Routine local implementation choices do not require ADRs unless they establish a durable ownership rule.

## Completion

An ADR is not complete until affected canonical docs, maintainer maps, behavioral-contract matrices, Pitlord policies, tests, and migration owners are updated. A decision without protecting evidence remains an unverified intent.

## Related docs

- [Testing, evolution, and decisions](testing-evolution-and-decisions.md)
- [Architecture procedure](architecture-procedure.md)
- [Architectural enforcement with Pitlord](enforcement.md)

## Notes

A repository may use a different decision-directory name when its documentation taxonomy requires it, but it must preserve one canonical index, stable numbering or identity, explicit status, and supersession links.
