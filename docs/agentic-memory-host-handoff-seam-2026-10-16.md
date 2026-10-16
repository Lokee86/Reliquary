# Agentic Memory host handoff seam — 2026-10-16

## Purpose

Freeze the actual embedding-host source trace and the missing acceptance/replay primitive before wiring agentic Memory completion. The original Step 34 trace is retained below, followed by the implemented Step 36 receiver and gate.

## Overview

The integration target is Warlock's existing Rust chat/tool loop. Its planned recursive native-agent framework is not a prerequisite. `Warlock-p5c-final-cut` at `c960e42` is the integration base: it descends from Warlock main `4da92b3`, uses authenticated `ChatTaskContext` and the multiplexed `GatewayRuntime`, and pins the post-deletion Reliquary candidate `437edada819c60111bb2e176e5f7270aae12f643`. An isolated child worktree will preserve the dirty graph/UI main checkout and the existing migration worktree's untracked verification files. The migration candidate is not represented as a deployed release.

At the Step 34 snapshot, Reliquary AR8 was green at `12296de` and Warlock had no completion producer. Warlock `9633ab8` implements Step 35; `cf41c66` implements the real receiver/completion/recovery path and passes Step 36/F3. This is an isolated integration candidate, not a deployed release.

## Observed registration and execution path

Paths below are relative to the Warlock integration base.

1. `src-tauri/src/chat_tool_definitions.rs::available_tools` currently registers passive `memory_search` and Archive search. It can additionally unlock `memory_provenance`. The old Memory arguments include presentation limit and graph depth; those are not the new protocol.
2. `chat_compaction_runtime.rs` installs the tool definitions into `ProviderRequest`. `chat_commands.rs` calls `chat_tool_runtime.rs::execute_chat_with_tools` once for the requesting assistant turn.
3. The loop obtains `ProviderToolCall { id, name, arguments }`, validates availability, and invokes `chat_tools.rs::execute_tool_with_branch_access`.
4. The current Memory branch uses `ChatTaskRetrieval::search_memories_with_depth`; `chat_task_retrieval.rs` forwards that passive request to `GatewayRuntime::search_visible_memories`. The workspace passive/UI adapter remains independently valid.
5. `chat_tool_runtime.rs` records `stream.tool_result`, checks `stream.ensure_durable`, installs provider continuation state, unlocks provenance tools, and finally calls `request.tool_exchanges.push(execution.exchange)`.
6. `openai_compatible_body.rs` renders those exact exchanges as assistant tool calls plus tool messages. `openai_codex_body.rs` renders function calls plus function-call outputs. Provider wire details remain behind those adapters.

The current `seen_tool_calls` signature guard rejects identical name/argument pairs. That is incompatible with repeated stateful traversal of the same anchors and must not reject the three new Memory operations merely because their arguments repeat.

## Identity and authority already available

`chat_task_context.rs` captures the authenticated owner, conversation, branch, accepted submission and leaf into an independent operating attachment. It does not follow the desktop viewer's later selection. `DurableStream` owns the native operation identity and the assistant message/generation identity; `ProviderToolCall.id` and the model round identify the call within that turn. A versioned host delivery identity can therefore bind the immutable turn identity, round and call ID without accepting an identity from model arguments.

The controller belongs as a local value in `execute_chat_with_tools`, created once outside its model-round loop. Gateway/REL/PHY must not gain a controller registry.

## Missing host primitive

The existing context mutation is an in-memory push. There is no explicit receiver-owned success acknowledgment binding that acceptance to a prepared manifest, and no restart producer that restores/replays the same successful manifest. Existing Echo ToolResult records occur before the push and contain only ordinary result text. They cannot be retrospectively treated as accepted Memory consumption.

Before automatic completion can be enabled, the host needs one concrete tool-context acceptance operation with these properties:

- It receives the exact exchange and optional prepared Memory result at the actual requesting-context boundary, validates the result bytes against the canonical payload, and checks cancellation/failure before acceptance.
- It returns trusted success only when that exact result has transferred to the requesting context. Preparation, provider response/HTTP status, activity events, transcript admission and generic ConversationExecution receipts remain insufficient.
- It retains the exact original host identity, payload/manifest digest, delivered refs and owner cuts through the host's existing durable tool/Echo ownership seam, with explicit receiver-acceptance semantics. A generic old Echo record is never proof. No second delivery journal or durable controller snapshot may be added.
- Its restart path can recover that exact successful manifest under current authority, rather than rerunning retrieval, changing cuts or deriving a fresh use identity. Existing accepted-use receipts remain the sole durable reinforcement idempotence authority.
- Failure/cancellation before acceptance produces no completion. A receipt failure after successful acceptance retains SelectedIDs and the original manifest for retry.

The implementation must prove this receiver path in the actual host, including durable recovery, before marking Step 36/F3 complete. If existing durable ownership cannot support it, completion remains disabled; recording an intention to deliver is not a substitute.

## Verification and environment

The source trace is read-only. Warlock main retains its graph/UI edits, and the migration base remains unchanged. Warlock's documented `scripts/cargo-local-reliquary.ps1 -ReliquaryPath ...` supplies a transient local Cargo patch and restores the committed lockfile; no ambient sibling dependency is introduced.

Warlock development documentation identifies an automated target cleanup service that can delete active dependency archives. Reliquary verification encountered this exact failure. Its cache was moved inside the active worktree to ignored `bin/agentic-cargo-cache`, with `target` retained as a local junction. Logical build paths and worktree-local TEMP/TMP remain unchanged; machine cleanup policy was not modified.

## Implemented receiver seam (Step 36)

Warlock's `chat_memory_handoff::accept` now owns the exact requesting-context transfer. It validates cancellation and exact canonical payload bytes before transfer, pushes the exchange into the actual ProviderRequest, then synchronously publishes an explicit receiver acknowledgment through the existing Echo adjunct. Only successful publication returns the identity/digest confirmation used by completion. There is no fourth model tool or new delivery journal.

Echo has an additive `ToolContextAccepted` binary kind (tag 8). Ordinary ToolResult records remain insufficient, even if their text imitates an acknowledgment. The host reserves `warlock.memory_context_accepted.v1` for the acknowledgment envelope containing the original canonical manifest and digest. Library context rendering and host Echo/model/UI adapters exclude these private records.

Publication errors after transfer are explicitly uncertain, not rolled back: a failed sync/return may leave an acknowledgment recoverable, and the requesting context must retain the actual transfer that acknowledgment describes. Such errors return no positive confirmation and submit no immediate accepted-use receipt. Cancellation and validation failure before transfer leave the context untouched. No observer/provider execution is needed as an alternative consumption boundary.

Before constructing the next request in that conversation, authenticated ChatTaskContext reads explicit acknowledgments through `conversation_echo_events_for`, including interrupted generations with no assistant text. Strict `PreparedAgenticMemoryResult::restore_manifest` restores the exact original payload, identity, refs and cuts, checks canonical V1 shape and original digest, and completion revalidates current authority. Recovery uses disposable old-turn controller state and never alters the new turn's controller. Existing accepted-use receipts remain the sole reinforcement idempotence authority. Recovery does not rerun retrieval or append another Echo record.

The restart test must reuse the host's existing configured journal directory, as production startup does; replacing it with a new default directory loses native submission identity and is not a valid restart simulation. Focused host tests cover actual transfer, unsuccessful/uncertain acknowledgment, exact retry, full host/gateway replacement and recovery before the first receipt. All eight host Memory tests pass, including both full host/gateway restart cases and uncertain acknowledgment recovery. Five registry/live transcript and five task-context regressions pass. Library gates pass 79 agentic and four Echo tests. F3 is complete for the integration candidate; see execution evidence.

## Related docs

- [Linear implementation plan](agentic-memory-retrieval-linear-implementation-plan-2026-10-16.md)
- [Architecture specification](agentic-memory-retrieval-implementation-plan-2026-10-16.md)
- [Execution evidence](agentic-memory-retrieval-baseline-2026-10-16.md)
- [External consumption contract](freshness-external-consumption-contract.md)

## Notes

This document distinguishes observed source from the missing primitive's requirements. Neither source presence nor library simulator coverage proves real host handoff. Step 35 is committed in Warlock `9633ab8`. Step 36/F3 is complete in Warlock `cf41c66`; frozen-fixture and final production-disposition gates remain Steps 37–45.
