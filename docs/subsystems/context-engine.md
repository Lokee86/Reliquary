# Context Engine

Parent index: [Subsystem documentation](./INDEX.md)

## Purpose

Describe the implemented deterministic conversation-context and compaction planning layer.

## Overview

Context Engine constructs bounded model-facing conversation requests from current turns, optional compaction checkpoints, and retained assistant evidence. It owns deterministic compaction budgets and selection mechanics; it does not own canonical conversation history or Ego cross-chat synthesis.

## Context model

A context view contains ordered user/assistant turns. A request contains system/instruction text plus model-facing messages.

Assistant evidence has explicit kinds such as reasoning summary, commentary, reasoning trace, tool call/result, and activity. Rendering rules choose what is eligible for a model-facing context without mutating the canonical source history.

## Compaction

Compaction is deterministic machinery under the host's conversation-history authority.

Current planning computes:

- trigger and target token budgets;
- protected raw-tail budget;
- summary budget;
- compaction input budget;
- applicable prior checkpoint; and
- exact through-message boundary for the next summary.

A checkpoint contains `through_message_id` plus summary text. It is applied only to the matching conversation lineage/boundary.

## Budget contract

The engine exposes configured percentages and hard bounds, including a minimum raw tail and maximum summary size. Token accounting is supplied through an explicit `ContextTokenCounter`; the engine does not hide provider tokenization assumptions.

Compaction stops when the requested target is satisfied and does not invent content to fill unused budget.

## Ownership boundaries

Archive/host conversation history remains canonical. Context Engine produces derived views/plans; it does not delete source turns or become a second transcript store.

Ego cross-chat and Memory-Web synthesis are separate future context layers. Current-session compaction belongs to the current conversation context, not Ego.

## Failure behavior

- invalid/insufficient budgets return typed errors;
- a checkpoint that does not apply is ignored rather than forced;
- unknown token counts cannot be silently approximated by hidden constants; and
- compaction output is bounded/normalized before it can become a checkpoint.

## Code Map

- `src/context_engine.rs`
- `src/context_engine/**`

## Related docs

- [Architecture](../architecture.md)
- [Rust API](../api.md)
- [Ego cross-chat plan](../ego-cross-chat-context-plan.md)
- [Behavioral contracts](../behavioral-contracts.md)

## Notes

This is the current deterministic compaction/context layer. Broader Ego context assembly remains planned.
