# Trial of a combined Ego and Perception audit call

Parent index: [Planning](./INDEX.md)

## Purpose

Find out whether one inference call can extract useful Ego subjects and Perception candidate Observations from the same Memory batch with less repeated work. Judge the outputs by reading and ranking them against the source material. A batch can support several good topics or claims, so a fixed list of expected answers would measure agreement with the list more than the usefulness of the audit.

## Overview

Both layouts use the same audit machinery and produce the same two candidate types. The trial freezes the Memory batches and compares the inference layouts directly. A regular shared REL audit can use whichever layout the trial supports. Scheduling, source selection and candidate verification or publication stay outside this comparison.

| Layout | Calls for one batch | Evidence offered |
| --- | --- | --- |
| Separate | One Ego extraction and one Perception extraction | Ego receives the Fresh passages. Perception receives the same Fresh passages plus any sampled Stale and Dormant passages. |
| Combined | One extraction returning an Ego list and a Perception list | Send the union once. Core labels each source with its grade and which list may cite it. The Ego list has the same Fresh evidence allowance as the separate Ego call. |

Ego returns a candidate subject and its source slots. Perception returns an ordinary proposition or a relational proposition with resolved participant slots, plus source slots. Choose the Perception contract per batch in advance, then keep that contract the same across both layouts. Allow up to four candidates per list and allow a valid empty list. These outputs are leads for later work. The trial makes no final summaries, Observation judgments or publication decisions.

## Trial cases and controls

Author eight compact batches with made-up Memories, source IDs, grade assignments and resolved Entity references where relevant. Freeze the text and packet ordering before either layout runs. Cover these situations:

1. Fresh Memories with one useful Ego subject and an ordinary claim to investigate.
2. Fresh Memories covering multiple subjects and a directly stated Relationship.
3. A compelling subject supported only by Stale or Dormant Memories, alongside a small current thread.
4. A Fresh thread where older passages supply material evidence for a Relationship.
5. A change over time where older material helps distinguish a historical claim from a current one.
6. Co-occurrence or a name collision that makes an unsupported Relationship tempting.
7. Several competing leads and repeated reporting from the same underlying source.
8. Sparse evidence where a short or empty candidate list is appropriate.

Use the same configured Switchboard General model, request limits, per-list output caps and sampling settings for both layouts. Give the combined reply enough output budget for both lists at their individual caps. Keep source order, Entity references and instructions for each contract fixed. Alternate which layout runs first across batches. Use the same fixed evidence packet in the first comparison. It can include passages a lookup would retrieve, so the comparison measures the call layout without variable search results. Start with 24 planned calls, three per batch. Repeat only a pair whose ranking could change the decision if its outcome looks unstable. Record transport retries separately from semantic results. [Previous configured trial pattern](../../crates/reliquary-entity-resolution/src/identity_trial/pilot.rs)

## Mechanical checks

Resolve every returned source and participant slot against the offered packet. Apply the same per-list validation to both layouts. For Ego, reject a candidate with any Stale, Dormant or missing source reference, including a mixture of Fresh and older references. For Perception, all offered grades are eligible and the relational participant slots must refer to distinct resolved Entities. Keep the raw invalid output for review. A parsable combined response can yield one valid list when the other fails. A response that cannot be parsed counts as a failure for both lists. Exercise both failure paths with scripted replies before the model trial.

## Manual review and ranking

Make one review sheet per batch and feature. Show the original source packet with Freshness grades, then render the two lists in the same format and shuffle their blind labels. Keep the mapping, raw response wrappers, token counts and latency out of the review sheet until the rankings are recorded. Review the sources first and note promising leads. During review, accept other defensible leads that turn up in either answer.

- Mark each candidate **keep**, **revise**, or **discard**. Give a short reason and point to the material source or omission. For Ego, check whether the subject is useful for a current summary and whether its Fresh citations actually support it. A Fresh citation attached to a detail found only in older text is a discard. For Perception, check whether the claim is specific and worth verifying, whether the citations lead to relevant evidence, and whether the claim keeps important uncertainty, counterevidence and time scope.
- Rank the paired lists separately for Ego and Perception as **A better**, **B better**, or **tie**. Favor meaningful coverage and useful abstention over candidate count. Flag a missing high-value lead, an unsupported claim, or a Freshness leak even if the rest of the list is good. Leave unresolved comparisons as ties with a reason.
- Unblind after the review. Report the rankings and material failures by feature, representative examples with source IDs, and actual calls, input and output tokens where available, and elapsed time. Keep the authored inputs, both raw outputs and the review sheet together so a later prompt change can be compared against this run.

## Decision

Prefer the combined layout if it saves meaningful model work and manual review finds comparable results for **both** features. A quality loss in one list cannot be offset by gains in the other. Look closely at the mixed-grade and ambiguous cases before changing the default. If the module stays split or the result is mixed, report which feature or batch type loses and why. Treat the trial as evidence for a call-layout choice on these cases.

## Related docs

- [Shared Memory Web audit specification](memory-web-audit-spec-2026-11-24.md)
- [Shared audit implementation plan](memory-web-audit-plan-2026-11-24.md)
- [Prior same-model comparison](../acceptance/cross-rel-entity-identity-2026-11-23.md#broader-same-model-comparison)

## Notes

Run this as a test-only comparison using authored inputs. Existing-Observation contribution, automated scheduling, interactive browsing and durable publication have their own seams in the main plan. Any later switch to a combined production call should carry forward the independently validated output lists and the demonstrated Fresh-only Ego rule.
