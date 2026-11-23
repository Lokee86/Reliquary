# First cross-REL Entity identity comparison

Parent index: [Acceptance evidence](./INDEX.md)

## Purpose

Verify that Core can leave two owner-qualified Entities unresolved, reconsider them after a relevant change and retain the reasons for its earlier decisions.

## Overview

The initial acceptance run used Core `fbfbe353` and comparison policy 1. The policy 2 results below cover the follow-up correction. The public fixture creates two native REL 2.1 files. Both contain an Entity named Alex Chen with the same local ID bytes. The two sources were authored independently. The test drives the configured Entity route with a scripted endpoint and uses the real owner writers, grants, worker and reopen path. [Public fixture](../../tests/entity_identity.rs)

## Observed decision history

| Revision | Change to the evidence | Decision and observed calls |
| --- | --- | --- |
| 1 | A says Alex signed the Meridian contract. B says Alex attended a briefing about it. | `Unresolved / Ambiguity`. Repeating the comparison returned revision 1. One endpoint call in total. |
| 2 | B adds an independent source identifying Alex as the signatory. | The running worker recorded `SameIdentity`. Both citations returned the original sentence. An unrelated Memory and an explicit retry kept revision 2. Two calls in total. |
| 3 | With the worker stopped, A corrects its source to name Morgan Lee as signatory. | Reopening A showed revision 2 as `Pending`. Restarting the worker recorded `Unresolved` without another endpoint call. Inspecting revision 2 returned its historical verdict and both retained quotes. |

## Authorization and publication

The public tests passed with a writable recording REL and a read-only peer. The peer file stayed byte-identical. A caller with both read grants could inspect the receipt. Writing with view-only access and inspecting without the peer grant both returned authorization errors.

Pausing the endpoint allowed a peer source correction and a credential revocation to be tested before publication. After each case a fresh authorized reader reopened both files and found no receipt. An unrelated peer write during evaluation preserved the judgment and produced one receipt from one call. Two equivalent comparisons evaluated concurrently and both returned the same first revision. [Public race fixtures](../../tests/entity_identity.rs)

## Initial evaluator checks (policy 1)

The 15 Entity resolution tests passed. They covered invalid citations, repeated provenance, input overflow, generic roles and two different contracts that the scripted model tried to use to distinguish the people. Policy 1 used a special case for conflicting birthplaces. Policy 2 retires that behavior. [Evaluator tests](../../crates/reliquary-entity-resolution/src/identity_evaluator_tests.rs)

## Storage and recovery

The owner-session checks restored receipts by cold replay, exact checkpoint and full replay of a receipt-bearing suffix. On Windows, releasing the previous checkpoint's file handle before replacing it allowed the refreshed checkpoint to cover the successor receipt. Reopening the old prefix, copying it forward, pruning descriptions and reclaiming storage preserved the expected chains. An interrupted write recovered the complete prefix. Replay rejected a bad predecessor and an impossible local revision. Unsupported rewrites refused before creating a destination. [Receipt tests](../../crates/reliquary-owner-session/src/session/identity_receipt/tests.rs) · [Checkpoint test](../../crates/reliquary-owner-session/src/session/checkpoint/tests.rs)

## Integration checks

The locked offline workspace build passed. The full test run passed 737 tests. The two existing opt-in tests for the Archive vector corpus and offline migration candidates were skipped by default. Formatting, the 23-package dependency check and the documentation checks passed.

## Follow-up verification for comparison policy 2

At Core `ec6a86f5` the evaluator applies the same evidence criteria to all Entity kinds. Its structured response distinguishes an ordinary description, corroboration of identity, evidence of separate referents and conflicting accounts. Core verifies exact quotes, indexed citations and independent source lineage. Every automatic result remains open to reconsideration.

| Fixture | Observation |
| --- | --- |
| Two programs named Relay, public Core API | Version 1.4 versus 2.0 yielded `Unresolved`. Independently authored inventories then identified one record as the editor and the other as the backup program. Comparing again appended `RelatedDistinct` with the two exact source quotes. |
| Two places named Eagle Point, evaluator | One source identifying a separate site was insufficient. With one source identifying the northern ridge and the other identifying the southern shore, and both documenting two sites, the evaluator accepted a distinction. In a second case, two names for a summit matched when independent sources placed it at the same distinctive survey marker. |
| Conflicting accounts about Alex Chen, evaluator | Different reported birthplaces supplied conflicting accounts. The pair stayed unresolved even when the scripted endpoint proposed a distinction. |
| Stored policy 1 receipt, public Core API | Inspection reported pending. On unchanged evidence and the same bound route, processing appended a policy 2 successor and retained the old receipt as history. |

These checks use a scripted endpoint and exercise Core's judgment validation and persistence. The locked offline workspace build passed, with 740 tests passing and the two opt-in corpus tests skipped. Formatting, the package dependency check and the documentation checks passed. [Public fixtures](../../tests/entity_identity.rs) · [Evaluator fixtures](../../crates/reliquary-entity-resolution/src/identity_evaluator_tests.rs)

## Automatic reconsideration during normal operation

A fresh Gateway reopened both RELs with the peer mounted read-only. The initial receipt had been unresolved with only one side grounded and zero model calls. After the peer gained an independent witness, Core used the session authorized for both owners and the configured Entity route to append revision 2. The scripted endpoint received one call. Revision 1 remained available as history. A separate fixture upgraded an old-policy receipt using only an authorized session, with no runtime configured. Replacing an inference route during a paused comparison discarded its late response and left the REL without a receipt for that pair. [Public fixtures](../../tests/entity_identity.rs)

## Related docs

- [Public comparison and inspection API](../public-product-api.md)
- [Comparison contract](../planning/cross-rel-entity-identity-spec-2026-11-23.md)
- [Implementation plan and acceptance cases](../planning/cross-rel-entity-identity-plan-2026-11-23.md)

## Notes

These fixtures establish Core's decision, authorization and persistence behavior. Accuracy on a retained corpus with a live model is a separate milestone.
