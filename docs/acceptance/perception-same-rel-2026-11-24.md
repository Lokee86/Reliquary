# Same-REL Perception acceptance

Parent index: [Acceptance evidence](./INDEX.md)

## Purpose

Verify the complete Perception lifecycle through an authorized mounted REL and preserve the evidence for the tested native decision contracts.

## Overview

The mounted fixture passed in 9.54 seconds with scripted general, embedding and native decision endpoints. It used a disposable native REL with accepted Archive activity, direct Memories and settled Dream work. Gateway ran the managed worker and reopened the same owner file between processing windows. [Mounted test](../../crates/reliquary-runtime-gateway/src/instance_perception/tests.rs)

## Observed lifecycle

| Operation | Observed result |
| --- | --- |
| Discover and publish | The combined audit produced an ordinary Observation and a relational Observation. Each verified draft received one native Verify call. The relational revision retained two resolved participant anchors. |
| Use published Observations as evidence | B cited the exact verified revision of A, and C cited the exact verified revision of B. Both retained the original Memory lineage. |
| Sample older evidence | A recurring discovery result had zero proposals. The older evidence still reached existing Observations. An identical source revision and span was compared once per target revision, and the sample earned no Freshness credit. |
| Respond to new evidence | One newly settled qualification reached the original Observation before the next 25-Memory discovery window. The original wording entered review, and the verified successor kept the Observation ID. |
| Reverify dependent claims | The worker verified successors for B and C. Each kept its ID, cited the new verified upstream revision, and inherited the revised original Memory lineage. |
| Recover and inspect | Native reopen preserved cited revisions, the comparison receipt and the +25 total award for the qualifying activity. |

## Decision and retrieval checks

The shipped contribution template matched all 30 recorded live requests and 450 questions. The Verify template matched two recorded runs of 24 drafts each, with 110 questions across both runs. The native adapter tests checked one request per batch or draft, later-only and separate-scope gates, the per-hit veto, and rejection of malformed replies. [Contribution contract](../planning/observation-reconsideration-spec-2026-11-24.md#tested-question-templates) · [Verify contract](../planning/observation-creation-spec-2026-11-24.md#tested-jev-question-templates) · [Native adapter tests](../../crates/reliquary-perception/tests/native_contracts.rs)

The shared audit tests fit the 45-input routine and 125-Memory maximum in one combined request under the fixture's route budget. They verified that one invalid discovery list preserves its valid sibling and that derived Observation sources receive only Perception exposure. The Ego tests exercised Fresh-only evidence and selective retirement when evidence changes. [Audit tests](../../crates/reliquary-memory-audit/src/tests.rs) · [Ego tests](../../crates/reliquary-ego/src/tests.rs)

The Gateway retrieval suite passed 13 tests. A pending upstream claim and its dependent claims left current retrieval while a separately supported claim remained eligible. A standing change blocked a new handoff at unchanged Memory and Graph versions. Confirmed historical delivery recovered exact retained evidence, including an earlier manifest created before typed Observation support. [Gateway tests](../../crates/reliquary-runtime-gateway/src/instance_retrieval/observation_checks/tests.rs)

The Application fixture allowed one resynthesis after Verify requested a revision. It verified a qualification successor, rejected the result under a changed route binding, and reaffirmed unchanged support after a triggering Observation was withdrawn. Its reaffirmation retained the historical trigger reference. Five focused owner tests passed. They covered propagation and release through a 70-Observation chain, exact checkpoint and suffix recovery, verified copy, successor replay after reopen, and idempotent +25 and +50 awards. [Application tests](../../crates/reliquary-application/src/observation_drafts/tests.rs) · [Owner tests](../../crates/reliquary-owner-session/src/session/checkpoint/tests_observations.rs)

## Related docs

- [Implementation plan](../planning/perception-implementation-plan-2026-11-24.md)
- [Same-REL Observations](../public-product-api.md#same-rel-observations)
- [Native publication and recovery](../architecture/owner-session.md)
- [Mounted verification contract](../development/behavioral-contracts.md)

## Notes

The contract comparison used recorded live trial requests. The mounted, Application, Gateway and owner recovery fixtures used offline endpoints and disposable owner files.
