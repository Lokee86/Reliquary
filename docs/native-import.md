# Native historical import

Parent index: [Documentation index](INDEX.md)

## Purpose

This document owns the current versioned native import contract, supported normalization profiles, source identity, durability and recovery. Archive owns imported evidence. This is distinct from the future reference-only external-derived index.

## Overview

The public `imports` module supplies `NativeImport`, conversation/message/branch/artifact DTOs, `ImportReceipt`, and four byte-only normalizers. `Cva::import_native(input, now_ns)` is the single exclusive/offline commit path. It uses existing Archive turn/file/branch publication and Import-priority Insomnia scheduling. It never mounts another runtime, follows an artifact URL/path, reads credentials, selects an instance conversation, or writes PHY state.

Mounted F5 management must add exact REL authorization, canonical P4 mutation admission and coordinator inventory refresh around this same owner operation. No mounted import primitive is claimed by this branch.

## Exact contract

Schema 1 is serialized using Serde with unknown fields rejected. Serialization emits all DTO fields; optional values use null. Omitted optional input fields normalize to null. A caller supplies a stable provider/account or database namespace, independent of filename, transport and display title.

| DTO | Required source facts |
| --- | --- |
| NativeImport | schema_version = 1, source_namespace, source_format, conversations |
| ImportConversation | id, optional title, messages, branches |
| ImportMessage | id, optional revision, optional parent_id/replaces, deleted, role, timestamp_ns, content, artifacts |
| ImportBranch | id, leaf_message_id, canonical |
| ImportArtifact | id, source_ref, filename, optional mime_type, bytes |

Roles are user, assistant, system, developer and tool. Unknown roles, missing ancestry, cycles, duplicate IDs, uncovered non-deleted messages, malformed tombstones and duplicate native attachments fail before publication. Keys are nonempty, at most 4,096 UTF-8 bytes and contain no NUL. Bounds are 10,000 conversations, 100,000 messages, 64 MiB combined body/artifact bytes. Normalizer input is capped at 64 MiB. These are admission bounds, not a streaming or bounded-allocation claim; preparation clones supplied values.

Omission is not deletion. A deleted message has empty content/artifacts and no native Node in that snapshot. Its source revision and deletion marker remain in Archive evidence; children and leaves resolve through its retained source ancestry to the nearest visible ancestor. An all-deleted branch can have no native head. `replaces` is explicit source provenance, not inferred from similar content; it can identify a prior source message absent from the supplied snapshot. In-place edits use the same source ID and a different revision. Absent revision derives from the complete normalized message fingerprint. Reusing an explicit source revision for different facts is rejected before import writes, during receipt publication/reconciliation and on cold rebuild.

Historical Nodes, Files, Episodes, receipts and branches are preserved. Deletion markers do not physically erase old Archive evidence or automatically retire previously extracted Memories. Historical rollback/retention and external-derived invalidation remain separate contracts.

## Identity and retries

SHA-256 over deterministic Serde JSON owns identity. Conversation/message/branch/artifact lists are sorted by source ID before snapshot hashing; parent resolution uses deterministic topological order. The import key includes schema, namespace, profile and complete normalized snapshot. Conversation identity hashes namespace plus source conversation ID. Node identity hashes native conversation, source message ID, effective revision, fingerprint and resolved native parent. An ancestor edit can therefore require a new immutable descendant Node while preserving the descendant's source identity in the receipt.

Native branch identity hashes conversation, source branch ID, resolved leaf and source canonical flag. Each distinct source path has a stable native branch; history is never rewound. Receipts retain source-to-native mappings. Multiple imported snapshots can retain historical canonical paths; no latest source revision, active selection or private provider state is inferred. Later native continuation can advance a branch without invalidating its historical receipt. A subsequent new import snapshot retains existing native branch heads instead of trying to rewind them.

Artifacts use Archive's existing native File identity. Receipts retain source artifact ID/reference, SHA-256 of supplied bytes and native FileId. The ordinary turn attachment record owns the bytes and source association; receipt records contain no second transcript/artifact body copy.

## Publication and recovery

Preparation validates the complete batch before writes. The commit path validates known source revision immutability, ingests deterministic Nodes/attachments, publishes branch heads/title metadata, materializes imported Episodes, repairs Import-priority queue admission, publishes an Archive receipt, then synchronizes before returning it.

Import Episode coverage belongs to Archive and is per immutable source Node across branches. Already covered shared prefixes are skipped; contiguous uncovered ranges use the existing response-cycle and byte packing policy. Native continuation of an imported conversation reuses that branch-aware coverage with Live origin/priority; imported alternative paths cannot block its finalization. Conversations without import history retain their sequential scheduling policy. Queue identity and completed/leased work remain Insomnia-owned.

The operation is resumable, not an atomic whole-batch transaction. Failure before receipt acknowledgement may leave durable source/branch/Episode/queue progress. Retry the identical normalized input after reopen. Existing immutable IDs and Node coverage recover every tested handoff cut without duplicate work. A synchronized receipt acknowledges the full batch; exact accepted retry returns that receipt without advancing clocks, restoring old titles or rewinding later native branches.

`CVAIMPT1` is an Archive-versioned receipt record. Cold rebuild, semantic reconciliation and physical reclamation preserve it and validate native Node/branch/file references. There is no independent import store, queue, semantic clock or container-wide manifest.

## Supported profiles and source audit

| Normalizer | Supported profile | Explicit tails |
| --- | --- | --- |
| normalize_chatgpt(namespace, bytes) | conversations.json array, id/conversation_id, mapping, message IDs, parent links, visible text parts, numeric create_time, current visible leaf | Multimodal parts, missing attachment bytes, tool recipients/roles, unknown channels and non-leaf/hidden selection are rejected |
| normalize_claude(namespace, bytes) | conversations.json array, uuid/name, chat_messages uuid/sender, RFC3339 created_at, text or text blocks, optional parent_message_uuid | Non-text/tool/artifact blocks and missing attachment bytes are rejected; explicit thinking/redacted_thinking blocks are excluded |
| normalize_hermes_jsonl(namespace, bytes) | full-session JSONL text exports with stable exported DB row IDs, timestamps and ordered active rows | Prompt-only exports, missing IDs, compression/fork lineage, inactive/compacted rows, tool correlation and non-text bodies require explicit normalized conversion |
| normalize_agent(bytes) | strict provider-neutral NativeImport schema 1, including branch/edit/replacement/delete/artifact facts | This is an interchange contract, not automatic support for arbitrary raw agent logs |

The maintained synthetic golden profiles live in `tests/fixtures/native_import/`. Profile suffix v1 versions the supported Reliquary parser shape; it does not assert a provider-maintained schema version or real-account export certification. No customer transcript was opened.

The 2026-10-08 UTC audit verified the official [ChatGPT export instructions](https://help.openai.com/en/articles/7260999-exporting-your-chatgpt-history-and-data) and [Claude export instructions](https://support.claude.com/en/articles/9450526-export-your-claude-data). They confirm export availability but do not freeze the parsed JSON field schema. No maintained real-account ChatGPT/Claude export fixture is available in this checkout; production profile coverage is therefore bounded by the synthetic fixtures and explicit shape validation. A new export shape requires a reviewed profile/fixture rather than silent coercion.

Hermes source is available at `7798df0a83721df7ff44b9ba023d56b85b351d1e`. `hermes_cli/session_export.py` emits full session objects per JSONL line; `hermes_state_portability.py` exports active message rows and `hermes_state_messages.py` exposes stable exported row IDs and separate reasoning fields. Namespace must identify the originating logical DB/history; rebuilt or copied-and-diverged source histories need an explicit namespace decision. No live DB/hydration/feed adapter is implemented.

ACP source is available in Hermes `acp_adapter/session.py`; it restores shared SessionDB history and has its own live cancellation/session mechanics. G3 consumes E's final runtime contract and remains separate. [Codex rollout source](https://github.com/openai/codex/blob/main/codex-rs/rollout/src/policy.rs) is publicly available and contains heterogeneous persisted item families; arbitrary raw rollout support is not claimed. Codex must supply schema-1 normalized facts until a pinned rollout adapter and maintained fixtures are accepted. G-external/G3/G5 are explicit unfinished tracks, not unavailable-source waivers.

## Verification

`src/imports/tests.rs` covers re-import/reopen, ordering independence, branch coverage, revision/delete/replacement history, artifact provenance, invalid input before publication, source/Episode/queue/receipt-publication interruption cuts, native continuation, reclamation and divergent reconciliation. `tests/native_import.rs` exercises all four public profiles using versioned golden files, unsupported formats and absence of hidden reasoning bytes in the REL.

## Related docs

- [Architecture](architecture.md)
- [Storage format](storage-format.md)
- [Rust API](api.md)
- [P5-G specification](multiplexing-phase5g-interoperability-2026-10-06.md)
- [External conversation index plan](conversation-index-integration-plan.md)

## Notes

The supported native import contract does not imply mounted F5 integration, provider-private reconstruction, current-source synchronization, arbitrary Codex rollout support or external-derived indexing.
