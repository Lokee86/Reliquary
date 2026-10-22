# Context Engine compaction refinement plan

Parent index: [Documentation index](INDEX.md)

Current implementation: [Context Engine](subsystems/context-engine.md)

## Purpose

Refine current-session compaction without changing the established ownership contract. This is an implementation/policy overhaul beneath the existing Context Engine boundary, not a transfer of transcript authority or model transport into Reliquary.

## Overview

This plan refines existing current-session compaction policy under the established ownership boundary.

## Fixed architectural contract

The following remain unchanged:

- canonical conversation history remains authoritative outside the compacted model-facing projection;
- Reliquary owns deterministic compaction policy, planning, validation, checkpoint construction, and checkpoint persistence;
- the host executes provider/model inference requested by Reliquary;
- compaction does not delete authoritative turns or Echo evidence;
- compacted older material remains retrievable from authoritative history; and
- current-session compaction remains separate from future Context Engine cross-chat selection (which reuses existing compaction records and introduces no new cross-chat summarizer).

The work below may strengthen the internal Reliquary/host generation contract, but it must not require a new external compaction ownership model.

The broader accepted Context Engine target (2026-10-21) also owns final model-facing context selection, budgeting and assembly, consuming Ego-produced Identity/Personality/Anchors/Summaries and other authorized sources. **This compaction-refinement plan remains current-session-only**; broad assembly and Ego synthesis are not implemented here. See [Roadmap](roadmap.md) and [Dependency Convergence Architecture Refactor](dependency-convergence-architecture-refactor-2026-10-21.md).

## Problems in the current implementation

The current Context Engine:

- carries fixed trigger/target/raw-tail percentages whose target and tail allocations are too large for the intended operating model;
- treats compaction primarily as summary generation instead of first reclaiming deterministic execution-trace overhead;
- emits a free-form continuation map whose source references are not structurally validated;
- token-fits an oversized summary by prefix truncation;
- has no explicit consolidation path for exceptional compaction inputs that exceed one inference request; and
- does not define monotonic same-turn compaction behavior or current-turn pressure cleanup.

## Planned compaction sequence

### C1 — Trigger and inspect

Keep a token-pressure trigger. The current 50% trigger may remain unless measurement shows a reason to change it.

At trigger time, inspect the actual model-facing context and establish:

- the current active turn;
- the protected conversational tail;
- already summarized/checkpointed history;
- eligible reasoning/tool execution evidence;
- the post-compaction target; and
- whether the target can be reached without applying pressure to the current active turn.

The current 40% post-compaction target and 20% raw-tail allocation are rejected as defaults and must not be carried forward unchanged.

### C2 — Protected conversational tail

Preserve a recent raw conversational continuity tail independently of execution traces.

Initial policy shape:

- protect no more than the newest approximately 10 complete turns;
- also cap that tail by a percentage of the model context window;
- select whole turns from newest to oldest until either the turn-count or token cap is reached; and
- treat the active current turn separately from the completed-turn tail.

The exact percentage cap and post-compaction target remain calibration parameters. The count and token cap are both ceilings: the system must not fill unused budget artificially.

A single unusually large recent turn must not make the tail permanently unbounded; deterministic execution-trace thinning remains available after the continuity surface has been established.

### C3 — Deterministic pre-tail trace stripping

Before semantic summarization, remove eligible reasoning and completed tool execution traces from model-facing history older than the protected tail.

Requirements:

- operate only on the model-facing projection, never authoritative history;
- evict only evidence already durably stored and retrievable through Echo;
- preserve conversational user/assistant surface needed for semantic compaction;
- do not split unsafe in-flight execution state; and
- recalculate token usage immediately afterward.

This stage has a kill switch: if deterministic stripping reaches the post-compaction target, stop. Do not invoke semantic summarization unnecessarily.

### C4 — Semantic compaction of older conversation

If the target is still exceeded, compact completed conversational history older than the protected tail.

The model output is not an unconstrained summary. It must provide structured fields for:

- current working state;
- active decisions and constraints;
- blockers;
- next actions; and
- retrieval descriptors for older topics/outcomes.

Retrieval descriptors must cite concrete turn and/or fragment identities supplied in the compaction input.

Reliquary validates all cited identities and ranges against the current conversation/branch before accepting output. References that merely exist elsewhere in the REL are insufficient.

Reliquary then assembles or normalizes the final retrieval map deterministically from the validated structured result.

### C5 — Reject, repair, and reduce; never truncate

Remove prefix token truncation as an accepted way to fit a generated summary.

If generated structured output is invalid or over budget:

1. reject it;
2. issue a repair/reduction inference request containing the candidate and the exact validation/budget failure;
3. revalidate the replacement; and
4. repeat only within an explicit bounded retry policy.

A failed reduction must leave the last accepted checkpoint intact. No partial/truncated candidate may be published.

### C6 — Post-summary deterministic thinning

After an accepted summary/checkpoint is assembled, recalculate context usage.

If still above target, remove additional completed execution traces/reasoning from oldest eligible live-context history first, continuing until the target is reached or no safe eligible evidence remains.

The summarized region and protected conversational tail are not reopened merely because the same active turn triggers compaction again.

### C7 — Same-turn monotonicity

Further compaction during the same active turn must be monotonic:

- do not repeatedly re-summarize already accepted summarized turns;
- do not rewrite the protected completed-turn tail during the same-turn pressure cycle;
- continue reclaiming eligible completed execution traces/reasoning around those fixed regions;
- reuse the accepted checkpoint; and
- advance only when genuinely new completed conversational history becomes eligible.

This prevents repeated lossy transformations of the same history during a long-running task.

### C8 — Current-turn pressure

The current active turn is normally protected from semantic compaction.

If the target cannot be reached using older summarized/history regions and completed execution evidence outside the active turn, enter an explicit current-turn pressure mode.

Current-turn pressure may reclaim only completed, durably recorded execution evidence that is no longer required as live in-flight state. If subdivision inside the turn is required, completed model-call boundaries are the preferred safe boundary; do not invent a finer first-class abstraction unless implementation requires it.

When a pressure-affected turn completes, and authoritative conversation plus Echo storage has succeeded, automatically strip all reasoning traces from that completed turn's live-context payload. Do not carry a huge pressure-turn reasoning trace into the next turn merely because it was temporarily required during execution.

### C9 — Echo durability/search prerequisite

Trace eviction depends on Echo being usable as authoritative execution-history backing.

Before enabling eviction, prove that the relevant streamed reasoning/tool evidence:

- is written durably soon enough for eviction;
- has stable conversation/message/model-round or equivalent retrieval identity;
- is searchable/readable while the conversation remains active;
- survives restart/reopen; and
- is never considered evictable until the required storage acknowledgment succeeds.

If current Echo behavior does not satisfy these requirements, fix that substrate before relying on trace eviction.

## Compaction input capacity and consolidation

Retain a hard compaction-input capacity limit for exceptional cases such as:

- an oversized restored conversation;
- switching to a smaller-context model;
- a compaction model with less context than the active conversation model; or
- imported/legacy state that bypassed ordinary trigger timing.

This limit is a capacity guard, not a normal-path 80%-of-window policy requirement.

The generation contract/instructions are not charged against the conversation-content budget. Physical request construction must still reserve real model context for instructions and output capacity.

If eligible compaction content exceeds one inference request:

1. partition it into contiguous bounded source ranges;
2. generate structured intermediate working-state/retrieval results;
3. validate every source identity/range;
4. consolidate the intermediate results;
5. repeat consolidation hierarchically if required; and
6. publish only the final validated checkpoint atomically.

Consolidation may merge or broaden retrieval entries but must not silently lose source coverage.

## Budget calibration

Do not freeze replacement percentages before measurement.

Required calibration must separately measure:

- context reclaimed by pre-tail trace stripping;
- normal completed-turn tail sizes across representative conversations;
- long-running agent-turn behavior;
- summary/retrieval-map sizes;
- frequency of same-turn retriggers;
- frequency of current-turn pressure; and
- model-switch/restore edge cases.

The intended policy shape is a materially lower post-compaction target than the current 40%, with a protected tail bounded by both approximately 10 complete turns and a percentage cap substantially below the current 20% raw-tail allocation.

## Acceptance

This refinement is complete when tests prove:

- deterministic trace stripping runs before semantic summarization and can terminate compaction by itself;
- the protected tail obeys both complete-turn and token-cap rules;
- structured generation references are rejected unless they resolve to the current conversation/branch;
- retrieval maps are assembled from validated structured data;
- oversized output is reduced by inference rather than truncated;
- exceptional oversized input consolidates recursively without silent source loss;
- repeated compaction in one active turn does not recompact accepted summary/tail state;
- current-turn pressure has an explicit safe path;
- pressure-affected completed turns shed reasoning after durable storage;
- authoritative transcript/Echo bytes remain unchanged by model-facing projection compaction; and
- existing external ownership/host-inference contracts remain compatible.

## Likely implementation surface

- `src/context_engine/compaction.rs`
- `src/context_engine/compaction_budget.rs`
- `src/context_engine/model.rs`
- `src/context_engine/evidence.rs`
- `src/context_engine/tests.rs`
- Echo persistence/retrieval surfaces only where C9 verification finds a real gap
- host adapter/request wiring only where the structured generation/reduction contract requires it

## Non-goals

- moving canonical transcript authority into Context Engine;
- turning Echo into a second conversation transcript;
- redesigning Ego or Cross-chat context;
- changing host ownership of provider transport;
- repeatedly summarizing already accepted history during one active turn; or
- selecting final replacement budget percentages without measurement.

## Related docs

- [Context Engine](subsystems/context-engine.md)
- [Roadmap](roadmap.md)

## Notes

This product follow-up remains outside Phase 1 physical storage convergence.
