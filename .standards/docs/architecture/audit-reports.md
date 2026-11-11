# Architecture Audit Reports

Parent index: [Architecture Standards](INDEX.md)

## Purpose

This document defines a consistent evidence-based shape for repository architecture audits and remediation lists.

## Overview

An architecture audit records the exact evidence reviewed, distinguishes current implementation from plans and migration state, ranks findings by architectural consequence, and produces bounded remediation that can be rerun and verified.

## Audit rules

An audit must identify the exact revision or dirty working-tree state reviewed, the standard revision used, included and excluded scope, unavailable evidence, and whether any active migration makes the conclusion provisional.

Classify findings by architectural consequence rather than cosmetic preference. Prioritize:

1. ambiguous authority or multiple mutation owners;
2. unsafe dependency direction or bypasses;
3. generated state mixed with authored source;
4. missing lifecycle, cancellation, shutdown, recovery, or corruption behavior;
5. unversioned process, persistence, or cross-product contracts;
6. missing verification for critical invariants;
7. oversized owners only when evidence shows mixed responsibility.

Line count alone is not an architectural violation. It is a review signal that must be paired with evidence of unrelated ownership, state, lifecycle, policy, or dependency concerns.

## Evidence expectations

Review canonical architecture, operations, maintainer maps, behavioral-contract matrices, ADRs, migrations, limits, Pitlord policies, focused tests, CI, repository layout, and generated-state exclusions. Distinguish authored evidence from generated snapshots and stale migration copies.

Each finding should name a canonical owner, concrete evidence, risk, and bounded remediation. Broad findings such as “improve architecture” or “split large file” are not actionable without identifying the responsibilities that must separate.

## Output

Use the shared [architecture audit report template](../../templates/architecture-audit-report.md). Group repositories by alignment only after recording repository-specific evidence and caveats.

## Remediation follow-through

After changes, rerun the narrow verification associated with each finding and update the report or create a completion note. Do not claim a repository is aligned solely because documents were added; verify the implementation, policy, generated-state boundary, and tests that the documents describe.

## Related docs

- [Architecture standard](architecture-standard.md)
- [Architecture procedure](architecture-procedure.md)
- [Testing, evolution, and decisions](testing-evolution-and-decisions.md)
- [Completeness and status claims](../completeness.md)

## Notes

Audit reports are evidence snapshots, not permanent current-state architecture owners. Update or supersede them after remediation rather than treating an old classification as current truth.
