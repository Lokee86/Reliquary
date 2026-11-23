# First cross-REL Entity identity comparison

Parent index: [Acceptance evidence](./INDEX.md)

## Purpose

Verify that Core can leave two owner-qualified Entities unresolved, reconsider them after a relevant change and retain the reasons for its earlier decisions.

## Overview

Core implementation `fbfbe353` is the revision covered here. The public fixture creates two native REL 2.1 files. Both contain an Entity named Alex Chen with the same local ID bytes. The two sources were authored independently. The test drives the configured Entity route with a scripted endpoint and uses the real owner writers, grants, worker and reopen path. [Public fixture](../../tests/entity_identity.rs)

## Observed decision history

| Revision | Change to the evidence | Decision and observed calls |
| --- | --- | --- |
| 1 | A says Alex signed the Meridian contract. B says Alex attended a briefing about it. | `Unresolved / Ambiguity`. Repeating the comparison returned revision 1. One endpoint call in total. |
| 2 | B adds an independent source identifying Alex as the signatory. | The running worker recorded `SameIdentity`. Both citations returned the original sentence. An unrelated Memory and an explicit retry kept revision 2. Two calls in total. |
| 3 | With the worker stopped, A corrects its source to name Morgan Lee as signatory. | Reopening A showed revision 2 as `Pending`. Restarting the worker recorded `Unresolved` without another endpoint call. Inspecting revision 2 returned its historical verdict and both retained quotes. |

## Authorization and publication

The public tests passed with a writable recording REL and a read-only peer. The peer file stayed byte-identical. A caller with both read grants could inspect the receipt. Writing with view-only access and inspecting without the peer grant both returned authorization errors.

Pausing the endpoint allowed a peer source correction and a credential revocation to be tested before publication. After each case a fresh authorized reader reopened both files and found no receipt. An unrelated peer write during evaluation preserved the judgment and produced one receipt from one call. Two equivalent comparisons evaluated concurrently and both returned the same first revision. [Public race fixtures](../../tests/entity_identity.rs)

## Evaluator checks

The 15 Entity resolution tests passed. They covered invalid citations, repeated provenance, input overflow, generic roles and two different contracts that the scripted model tried to use to distinguish the people. Independent city-of-birth evidence supported a distinction. Conflicting birthplaces also prevented a shared contract from establishing a match. [Evaluator tests](../../crates/reliquary-entity-resolution/src/identity_evaluator_tests.rs)

## Storage and recovery

The owner-session checks restored receipts by cold replay, exact checkpoint and full replay of a receipt-bearing suffix. On Windows, releasing the previous checkpoint's file handle before replacing it allowed the refreshed checkpoint to cover the successor receipt. Reopening the old prefix, copying it forward, pruning descriptions and reclaiming storage preserved the expected chains. An interrupted write recovered the complete prefix. Replay rejected a bad predecessor and an impossible local revision. Unsupported rewrites refused before creating a destination. [Receipt tests](../../crates/reliquary-owner-session/src/session/identity_receipt/tests.rs) · [Checkpoint test](../../crates/reliquary-owner-session/src/session/checkpoint/tests.rs)

## Integration checks

The locked offline workspace build passed. The full test run passed 737 tests. The two existing opt-in tests for the Archive vector corpus and offline migration candidates were skipped by default. Formatting, the 23-package dependency check and the documentation checks passed.

## Related docs

- [Public comparison and inspection API](../public-product-api.md)
- [Comparison contract](../planning/cross-rel-entity-identity-spec-2026-11-23.md)
- [Implementation plan and acceptance cases](../planning/cross-rel-entity-identity-plan-2026-11-23.md)

## Notes

These fixtures establish Core's decision, authorization and persistence behavior. Accuracy on a retained corpus with a live model is a separate milestone.
