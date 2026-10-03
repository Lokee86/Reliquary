# Ego Cross-chat context plan

Parent index: [Documentation index](INDEX.md)

## Purpose

Record the future-only design for deterministic cross-chat context selection inside Ego without creating another summarization system.

## Overview

Cross-chat context should reuse REL conversation-compaction records that already contain bounded summaries of prior conversations. Ego should select among those existing summaries deterministically, favouring the most recently active conversations in the current REL and filling only the remaining Ego injection budget. Current-session compaction is not part of Ego and is budgeted with the current conversation instead.

## Status

The selection direction is settled at the policy level but is not yet implemented. Exact budget allocation and tail-handling details remain calibration work.

## Source of cross-chat context

Do not add a Cross-chat summarizer.

REL already persists conversation-compaction records keyed by conversation plus branch checkpoint (`through_message_id`) with a generation and UTF-8 summary. Cross-chat context should consume those records directly.

For a selected prior conversation/branch, the available context is conceptually:

```text
latest applicable conversation compaction
+ uncompacted tail after through_message_id, when needed
```

Cross-chat therefore selects existing conversation state; it does not reinterpret or resynthesize it.

## Deterministic selection

Cross-chat selection should be owner-local to the current REL.

Candidate prior conversations are ordered by **most recent activity**, not by conversation creation time. An old conversation resumed recently therefore outranks a newer conversation that has been inactive longer.

Conceptually:

```text
prior conversations in current REL
    -> order by latest activity descending
    -> select latest applicable branch compaction/tail
    -> append until Cross-chat budget is exhausted
```

No semantic importance score, model ranking, category weighting, or new summarization call is required.

Branch identity must be preserved. A compaction checkpoint belongs to its conversation/branch lineage; Cross-chat must not merge sibling branch summaries as if they were one conversation state.

## Budget boundary

The intended Ego injection ceiling is approximately 20% of the model's usable input context, subject to provider-specific output reservation and later calibration.

That Ego budget contains:

- Identity;
- Personality;
- Anchors;
- Memory-Web Summary; and
- Cross-chat context.

Cross-chat receives the remaining budget after the higher-priority Ego layers are placed. The 20% value is a ceiling, not a target; Ego should not pad context merely to consume the allowance.

Current-session state is outside this budget. In particular:

- current-session compaction; and
- the live current-conversation tail

belong to the current conversation context, not Ego.

## Relationship to other Ego layers

Cross-chat and the Memory-Web Summary solve different continuity problems.

- The Memory-Web Summary is durable, user-relative project orientation over shared REL Memory state.
- Cross-chat context preserves recent conversational state from other sessions in the same REL.
- Current-session compaction preserves continuity inside the current conversation.
- Retrieval remains available for deeper historical detail that is absent from default injection.

Cross-chat should not try to determine whether prior conversation material has become semantically obsolete. Recency of conversation activity and the fixed context budget are sufficient deterministic selection mechanics; semantic project state belongs to Memories/Dream/Web synthesis.

## Open calibration

Implementation still needs measured choices for:

- the exact usable-input-context calculation for providers whose output shares the context window;
- fixed/minimum allocations, if any, among Identity, Personality, Anchors, Memory-Web Summary, and Cross-chat;
- whether uncompacted tails are always included or only when a selected compaction does not reach the conversation tip;
- deterministic truncation when one compaction/tail exceeds the remaining Cross-chat budget; and
- treatment of conversations that are currently open elsewhere in the same REL.

## Related docs

- [Ego Memory-Web summary plan](ego-web-synthesis-plan.md)
- [Ego cross-REL summary inclusion plan](ego-cross-rel-summary-inclusion-plan.md)
- [Roadmap](roadmap.md)
- [Storage format](storage-format.md)
- [Current limitations](current-limitations.md)

## Notes

When Cross-chat selection ships, move implemented behaviour into the current-state architecture/API owners and retain only unresolved calibration work here.
