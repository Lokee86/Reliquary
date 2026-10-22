# Review-clock primitives

Parent index: [Subsystem documentation](./INDEX.md)

## Purpose

Describe the implemented typed clock and pure review-threshold arithmetic retained for owner-specific scheduling.

## Overview

The staleness module provides owner-qualified clock types and deterministic due-threshold math. It does not own persistence, semantic eligibility, queueing, inference, or Memory Freshness.

Supported clock sources are REL activity, owner Memory version, and explicitly sampled Unix nanoseconds.

## Clock typing

`ClockReading` binds one `ClockRef` to a compatible value. Sequence clocks cannot be compared with Unix time, and readings from different owners/sources cannot be compared as if they were one clock.

The module never reads a hidden wall clock; callers provide Unix samples explicitly.

## Review intervals

`ReviewInterval` supports fixed deltas and proportional-capped sequence intervals. Proportional intervals are not supported for wall-clock time.

Threshold calculations are checked for overflow and reject future anchors.

`is_due`, `threshold_from_anchor`, and `due_now` are pure evaluation helpers. They do not mutate the consumer or imply that a semantic review occurred.

## Ownership boundary

Memory Freshness does not use this generic review schedule as its score clock; Freshness has its own accepted-turn decay contract.

Ego/Observation or other consumers may use these primitives only under their own explicit scheduling policy.

## Code Map

- `src/staleness/**`
- `src/facade/review_clocks.rs`
- `src/facade/review_clocks_tests.rs`

## Related docs

- [Architecture](../architecture.md)
- [Freshness](freshness.md)
- [Ego future work](../ego-web-synthesis-plan.md)
- [Perception future work](../perception-subsystem-plan.md)

## Notes

These primitives survived the retirement of the old generic per-Memory staleness design because the arithmetic is independently useful. They are not a second Freshness owner.
