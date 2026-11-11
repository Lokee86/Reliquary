# Documentation change procedure

Parent index: [Documentation](./INDEX.md)

## Purpose

Keep implementation, public contracts, maintainer routes and verification evidence in agreement.

## Overview

Use this procedure for code, API, CLI, config, architecture, state, operations and verification changes. The [policy](documentation-policy.md) defines local specialization; the synchronized [procedure](../.standards/docs/documentation-procedure.md) is normative.

## Workflow

1. Route the change intent through the [maintainer map](architecture/maintainer-map.md). Read its canonical owner and inspect source/tests before changing claims.
2. Classify each fact as current guide/reference/architecture/operations/development, future planning, current limitation or historical evidence. Reuse the current owner; create a document only for an independently discoverable responsibility.
3. Update exact contracts and workflow consequences in the same change. For an ownership, package, flow or test-boundary change, update maintainer, coverage and behavioral-contract routes too.
4. Update every affected direct-file and direct-folder index and inbound link. Preserve dated acceptance evidence and classify it in [disposition](development/document-disposition.md).
5. If changing a static architecture invariant, resolve the approved decision before changing the DAG or policy. Update the architecture owner and Pitlord rules together. Add behavioral verification for claims a graph cannot prove.
6. Run the commands in [development workflow](development/workflow.md), including full docs, committed change impact, index/link, architecture and applicable Rust/CLI/consumer checks. PR comparisons use the merge base of the actual PR target. Uncommitted work is not accepted production history.
7. Record actual outcomes, exit codes, source revision and tool/snapshot identities. A baseline is debt, never compliance. Missing tools and unsupported evidence remain explicit limits.

## Failure modes

An owner-map row without substantive owning prose does not satisfy coverage. A passing structural checker does not show that instructions work. If docs disagree with code, inspect the behavior and tests; document the verified current reality and track implementation defects separately. Never run real providers or mutate the user's owners/corpus merely to verify docs.

## Report contract

Every implementation report states documentation impact: inspected owners, updated files, unaffected surfaces with reason, check commands/results and known gaps. Architecture changes additionally report ownership/boundary, Pitlord and behavioral verification impact. Only claim complete/current/compliant at the strength supported by those results.

## Related docs

- [Documentation policy](documentation-policy.md)
- [Limits](limits/current-limitations.md)
- [Acceptance evidence](acceptance/INDEX.md)

## Notes

Keep temporary outputs in repository `work/`; pytest uses `python -m pytest --basetemp .pytest\_tmp`. Never use `C:\tmp`. A custom Go cache uses repository `.go-cache`.
