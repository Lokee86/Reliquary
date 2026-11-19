# Offline REL and PHY history migration execution plan

Parent index: [Planning](./INDEX.md)

## Purpose

Implement the [offline history migration specification](owner-history-migration-spec-2026-11-18.md) from Core `184857a5`. The first full proof uses verified copies of the original 28-day REL and PHY.

## Overview

Use the archived handoff at `bc9ec7f0` for provenance and recover the needed F03, D01, and D02 code by path. Use the existing tool entry points and owner-session boundaries. The migration window runs from recovering the archived tools through the paired acceptance run. Tool builds may be incomplete while the current native encoders and observation paths are wired. Verify each completed seam once it is available.

## Work packages

| Work package | Depends on | Deliverable and exit check |
| --- | --- | --- |
| 1. Freeze the source evidence | Baseline | Recover the F03 reader and custody checks from `3b7cab0b`. Verify both copies against the fixture pins and freeze the expected owner, frame, family, reference, and historical-cut inventory for those two files. Recover the independent receipts for those hashes or regenerate observations from the verified copies. Bind the witness to those two specimens and choose the transcript, REL Memory history, and PHY history cases before conversion. This step exits with a complete comparison input for the pair. |
| 2. Establish native replay | 1 | Use the [Container publication primitives](../../crates/reliquary-container/src/lib.rs) and [owner-session](../architecture/owner-session.md) to preserve clocked Archive bindings and Memory history. Exercise an earlier reservation and a retained PHY revision using the archived Archive-binding and PHY Memory fixtures. Reopen the results as 2.1 and compare their exact values and clocks. Finish the canonical read and write mappings for the remaining observed fields that need native replay changes. This establishes the shared 2.1 boundary for the writer and verifier. |
| 3. Complete the D01 writer | 2 | Port the composer and staged writer from `5f2b1a7c`. For every observed carrier, record its source decoder, native encoder, and independent check. Finish Archive publication bindings, Memory and work groups, Freshness evidence, and the applicable Entity Resolution, Community, and vector history. Resolve backing references before publication, extend the existing vector relocation path for the observed references, and retain the stage-and-reopen checks. Exit with one create-only REL candidate and one PHY candidate, both pinned to their own transfer and source receipts. |
| 4. Complete the D02 comparison | 2 | Port the independent reader to the current authority and replay surfaces. Reuse the Memory, Archive, Graph, and Freshness comparators and close the missing Memory external-evidence coverage. Complete comparisons for any remaining observed values, cut membership, and relocated references across the pair. A targeted fixture proves that a changed intermediate value fails even when the head agrees. The verifier consumes the witness frozen in step 1. |
| 5. Accept the original pair | 3 and 4 | Run the converter and independent comparison on copies of the original `project.prj.rel` and `user.phy`. Verify both owners with the current CLI in separate processes, run the focused reads below, and check custody and file hashes. Publish one pair verdict with the per-owner results and the exact code and artifact hashes. Close the migration window after the integrated repository checks pass. |

## Focused verification

- **Small replay and refusal checks.** Reuse the archived binding and PHY history fixtures for exact chronology, grouped publication, and a reference relocation. Exercise an unresolved reference and destination collision through the actual writer. Use the existing independent comparator fixture for the changed intermediate value.
- **One full paired migration.** In fresh processes run `rel verify` and `phy verify`, then compare the retained values, typed unavailable states, and reference closure against the fixed witness. Prove coverage from the ordered record and cut streams. Confirm the original and copied-input hashes and the stable candidate hashes after verification.
- **Representative native reads.** Open a source-backed REL Memory from the witness and follow its provenance to the exact archived turn. Compare an earlier REL history cut and an earlier private PHY state against the source. Read an earlier PHY Memory revision if the pair contains one. The archived PHY fixture covers an explicit revision transition. Use normal read grants for the intended pair and verify rejection of ungranted PHY access.

## Integration and evidence

Run formatting, locked offline workspace build and tests, the relevant detached tool checks, and the docs and package-architecture checks once at integration. Use the [development workflow](../development/workflow.md) for commands. Check that production opens the native candidates through its ordinary admission and that the standalone source reader stays outside the production package graph. Update the affected storage, owner-session, contract, and coverage docs with any accepted native replay changes.

Record the paired acceptance under [acceptance evidence](../acceptance/INDEX.md). Include source and output hashes, exact tool revisions, covered families and cuts, reference checks, and the results of the focused native reads. Keep the actual history and private transfer receipts in the run directory. An unresolved mapping or comparison is an incomplete run.

## Related docs

- [Migration specification](owner-history-migration-spec-2026-11-18.md)
- [Current planning roadmap](roadmap.md)
- [CLI verification and inspection](../reference/cli.md)
- [Development workflow](../development/workflow.md)

## Notes

The pair is accepted as a standalone offline result. Keep the parity cases bound to the source observations chosen before conversion.
