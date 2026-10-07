# P5-C canonical conversation metadata consumer API

## Purpose

Replace remaining Warlock direct legacy-host Archive metadata, Echo, and compaction calls with exact-instance gateway operations. This is a source patch, not a completed P5-C or P5-D migration.

## Overview

New GatewayRuntime methods:
- `conversation_branch_start_node_ids_for(instance, owner_id, conversation_id, leaf_node_id)` — explicit REL ViewRel, read-only.
- `conversation_compactions_for(instance, owner_id, conversation_id)` — explicit REL ViewRel, read-only.
- `set_conversation_title_for(instance, owner_id, conversation_id, title)` — explicit REL ManageRel.
- `put_conversation_compaction_for(instance, owner_id, conversation_id, through_message_id, summary, supersedes_through_message_id)` — explicit REL ManageRel.
- `put_echo_event_for(instance, owner_id, event)` — explicit REL ManageRel.

Mutations use existing P4 REL admission, committed CVA sync, and the new ConversationMetadata mutation classification. Auxiliary changes that are not reflected in Archive's source-version watermark still publish ArchiveChanged after sync. No additional queue or mutable owner runtime is exposed.

## Verification

Rust formatting passed. The Phase 4 source coverage checker passed with 42 positive/negative fixtures and classified 138 public APIs. Added a focused test for unauthorized title/Echo mutation and authorized persisted title read. The focused Rust test is **not yet executed**, so runtime acceptance remains open until that test passes.

## Integration

Warlock C2 should use these methods only through its authenticated InstanceRuntime and explicitly selected owner. No old-host forwarding is permitted. This patch does not provide a replacement for the accepted conversation submit/generation/stream protocol, which remains P3/P4 owner authority.

## Related docs

- [P5-C external consumer plan](multiplexing-phase5c-external-consumers-2026-10-06.md)
- [P5-A consumer matrix](multiplexing-p5a-consumer-matrix-2026-10-06.md)

## Notes

No storage schema, provider routing, CLI behavior, or Arcana/renderer code changed. Merge into the A/B integration branch only after source-guard reconciliation, then run the combined P5-A/P5-C focused Rust tests.