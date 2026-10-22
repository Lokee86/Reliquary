# Dream classifier v2 candidate design

Parent: [Dream](subsystems/dream.md)

## Purpose

This document records the measured Dream classifier-v2 prompt and the evidence supporting its production promotion.

## Overview

Classifier v1 over-links broad project context, while the rejected first v2 over-prunes useful same-workstream relations. The measured v2f candidate targets the middle boundary: semantic workstream continuity is sufficient, but project/domain proximity alone is not. It cleared both the exact-pair and population gates and is now production classifier contract v2.

## Status

The exact-pair boundary gate is **passed** with the frozen v2f prompt in [`../corpus/dream-classifier-v2-candidate.txt`](../corpus/dream-classifier-v2-candidate.txt). Three independent live `gpt-5.6-luna` / low-reasoning runs all cleared the advancement threshold. The 46-Memory Luna-low population gate also passed: 46/46 complete, zero failures/backlog, 96/98 whole-web agreement, all 10 audited false positives absent, and troubleshooting/step-by-step restored. The prompt is production contract v2.

## Why v1 and the rejected first v2 fail

The 40-case boundary fixture isolates the original tradeoff:

```text
contract        related        unrelated       overall
v1              29/30 96.7%     0/10  0.0%     29/40 72.5%
rejected v2      1/30  3.3%    10/10 100.0%    11/40 27.5%
```

v1 treats broad project/domain association as enough for `topical`. The rejected first v2 corrected those false positives but effectively required near-literal entity identity and destroyed legitimate relations across one semantic workstream.

The replacement abstraction is **semantic workstream/scope**: different entities and artifacts may be related when they participate in the same specific protocol, workflow, implementation concern, operational context, preference, or problem. Same project/domain membership by itself is not enough.

## Boundary rules

A pair is related when one of these is true:

- both Memories concern the same specific semantic workstream/problem and traversing between them is useful;
- one is a constraint, policy, design, schema, implementation state, or observed behavior of the workstream described by the other;
- one is a broad current implementation/status summary and the other is a concrete current capability, behavior, feature, defect, or state encompassed by it;
- multiple Memories describe operationally connected stages of one bounded session, transaction, lobby, workflow, or lifecycle;
- different facets jointly define the same durable runtime entity/subsystem's operation, state, representation, identity, configuration, or lifecycle;
- one is a general rule/preference and the other is a scoped application or specialization;
- one is an analogy or explanatory frame that directly informs the other concept.

A pair is unrelated when its only bridge is:

- membership in the same project/domain/codebase;
- chronology or nearby implementation stages;
- project identity/history versus an implementation detail;
- shared generic vocabulary across different entities/subsystems;
- vector similarity or existing Graph proximity without semantic support in the Memories.

The central distinction remains **same workstream, not same noun and not same project**.

## v2f prompt

The frozen reference prompt is [`corpus/dream-classifier-v2-candidate.txt`](../corpus/dream-classifier-v2-candidate.txt), and production `DREAM_CLASSIFIER_SYSTEM_PROMPT` matches it. The tracked file remains the reproducible acceptance artifact and a baseline for future prompt experiments.

The final tuning step made the bounded-session rule explicit: room/lobby admission or selection, readiness/match-start, and leave/exit handling are stages of the same session lifecycle. It simultaneously preserves the negative rule that gameplay/runtime subsystems such as spawning, rendering, collision, logging, or protocol implementation do not become session-lifecycle relations merely because they run in the same application.

The prompt also explicitly requires exact evidence copying, preferring the full Memory title when it is sufficient. This removed an observed non-verbatim evidence failure without relaxing semantic validation.

## Exact-pair gate

Fixture: `corpus/dream-classifier-regressions-v1.json`

```text
required:
related recall       >= 27/30
unrelated accuracy   >=  8/10
overall              >= 35/40
errors               =   0

v2f Luna-low run 1:  27/30 related   10/10 unrelated   37/40   0 errors
v2f Luna-low run 2:  29/30 related   10/10 unrelated   39/40   0 errors
v2f Luna-low run 3:  28/30 related   10/10 unrelated   38/40   0 errors
```

The 10 unrelated boundary cases were correct in all three runs. The only persistent miss was the medium-confidence `ship_runtime_identity` case; other misses moved probabilistically between runs. That is a better stopping point than widening the prompt further to chase one ambiguous fixture judgment.

Earlier Luna-low candidates progressed as follows:

```text
candidate            related   unrelated   overall   errors
workstream design     22/30      10/10      32/40      0
v2b tie-breaks        24/30      10/10      34/40      1
v2c flow/evidence     26/30      10/10      36/40      0
v2d strong defaults   26/30      10/10      36/40      0
v2e session examples  27/30      10/10      37/40      0  best first run
v2f lifecycle rule    27-29/30   10/10      37-39/40   0  three-run range
```

## Harness behavior

`examples/dream_classifier_tune/` accepts `--system-prompt-file PATH`. The override is tuning-only: `DreamClassifier::new` uses the production v2 prompt, while `DreamClassifier::with_system_prompt` lets the exact-pair harness inject the tracked candidate explicitly.

```text
cargo run --example dream_classifier_tune -- \
  <config> <baseline.cva> \
  corpus/dream-classifier-regressions-v1.json <report.json> \
  --concurrency 12 \
  --system-prompt-file corpus/dream-classifier-v2-candidate.txt
```

## Population gate

The durable 46-Memory population fixture at candidate limit 12 / pair concurrency 12 passed all required checks:

- 46/46 sources complete with zero recoverable failures;
- extracted backlog drains to zero;
- whole-web gold improves on classifier-v1's 87/98 baseline;
- the 10 audited false-positive pairs remain absent;
- troubleshooting/step-by-step relation is present;
- relation density is audited rather than optimized for being lower or higher by itself.

Retrieval was not tuned during this classifier experiment; it already found the tracked pairs at the intended population limit.

## Related docs

- [Dream](subsystems/dream.md)
- [Development](development.md)
- [Rust API](api.md)
- [Behavioral contracts](behavioral-contracts.md)

## Notes

Classifier v2 is shipped behavior; the frozen v2f prompt remains the reproducible acceptance artifact.
