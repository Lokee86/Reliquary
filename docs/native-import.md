# Native historical import

Parent index: [Documentation index](INDEX.md)

## Historical Freshness post-generation operation

New imports remain fenced after receipt publication until the explicit Historical Freshness operation and exclusive completion coordinator both accept their evidence. The coordinator derives attributed Insomnia, local Memory, import-born Dream and active non-superseded output Memory mention-resolution readiness from owner stores. Routed PHY outputs return a typed unsupported blocker because this REL-only API cannot validate the other owner. Completed local epochs support ordered divergent replay through their concrete owners; mounted completion remains unavailable.

Historical reconstruction cannot reuse an earlier settlement while a different epoch is open. Internal attribution derives exact receipt source Nodes and complete Episode paths, keeps born-Memory identity separate from existing recipients, and bounds provisional events by physical start/freeze prefixes. Same-activity-cut pre-start receipts remain outside the import interval. Accepted scoped replay is implemented; completion additionally validates exact import Episode outputs and the downstream work those outputs actually cause.

Legacy initial reconstruction also refuses retained accepted-use receipts: without an authenticated import base, rebuilding from birth could replace legitimate live history. An already accepted settlement's exact retry continues to preserve later live use.

Historical reconstruction is a separate explicit post-generation step, not an automatic import side effect. Complete queued Import Insomnia work and real first-cycle Dream settlement, retain a compatible embedding profile, then call ConfiguredRuntime::reconstruct_historical_freshness on the exclusive offline Cva. The default options do not fill embeddings. Requested bounded fill uses the configured embedding route for missing genuine source-fragment rows, then native reconstruction refreezes and commits a sparse epoch-bound CVAFRS05 settlement. Legacy CVAFRS03 remains readable without rewriting its bytes. Exact accepted retries use no provider and preserve live growth.

C8 verifies a schema-1 import fixture through actual import_native, Insomnia publication, first-cycle settlement, configured local HTTP embedding, native checkpoint, reopen/retry and intrinsic post-checkpoint live F8. This is executable synthetic importer evidence, not retained production-corpus acceptance (C10). Noncanonical first acceptance rejects instead of rewriting the Archive clock. Gateway mounted historical management remains unavailable.

## Purpose

This document owns the current versioned native import contract, supported normalization profiles, source identity, durability and recovery. Archive owns imported evidence. This is distinct from the future reference-only external-derived index.

## Overview

The public `imports` module supplies `NativeImport`, conversation/message/branch/artifact DTOs, `ImportReceipt`, epoch status/completion types, `ImportReadinessBlocker`, `ImportReadinessReport`, `ImportVectorReadiness` and four byte-only normalizers. `Cva::native_import_readiness(import_id)` is a read-only report for the current open epoch, returning the import ID, optional typed blocker and vector status; unknown, completed and noncurrent IDs reject. `Cva::import_native(input, now_ns)` is the exclusive/offline construction path. It opens or resumes one deterministic durable epoch before Archive publication and Import-priority Insomnia scheduling. The import construction call does not itself write PHY state; later routed Insomnia output may reference a PHY Memory, which this REL-only readiness API currently reports as `Memory/Unsupported`.

Mounted F5 management must add exact REL authorization, canonical P4 mutation admission and coordinator inventory refresh around this same owner operation. No mounted import primitive is claimed by this branch.

## Divergent reconciliation boundary

The replay path copies the left CVA as the output prefix and consumes finite right-side intervals in physical order. Archive records and attachments, grouped Insomnia Memory publications, routing metadata, Dream journals, Perception resolutions, processing checkpoints, graph/community records and accepted Freshness effects pass through their concrete owners. Grouped publications preserve semantic fields, body bytes and birth evidence while rebasing destination coordinates. Fresh destination `ImportStart`, `CVAFRS05` settlement and `ImportComplete` evidence uses destination watermarks and authenticated physical prefixes. Same-ID duplicates require matching normalized accepted owner evidence and original timestamps; conflicting evidence returns a typed conflict before output creation.

Open epochs, external PHY publications, unsupported owner payloads and outstanding source Insomnia work remain refused. Retained local work/attempt journals are preserved. Destination owner validation may reject incompatible graph/community populations rather than inventing source evidence. Reopen, repack and conservative reclamation preserve accepted native and legacy epoch authority.

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

Preparation validates the complete batch before writes. The commit path synchronizes `ImportStart`, enters the private import-attributed physical-write scope, validates known source revision immutability, ingests deterministic Nodes/attachments, publishes branch heads/title metadata, materializes imported Episodes, repairs Import-priority queue admission, publishes an Archive receipt, then synchronizes before returning it. Returning the receipt does not imply epoch completion. Exact receipt retry is idempotent. Any unrelated ordinary write while the epoch is open fails at generic physical admission before version or file mutation.

`Cva::complete_native_import(import_id, now_ns) -> Result<ImportCompletionOutcome, CvaError>` is the public exclusive coordinator. Its public outcomes are `Pending(ImportCompletionPending)` or `Completed { complete, committed }`; callers cannot supply digests, evidence, or a readiness boolean. It derives the exact start, Archive receipt, and accepted scoped `CVAFRS05` settlement. Production marker append requires an opaque authority constructed only inside this coordinator; the direct storage seam exists only in test builds. The completion marker binds the exact receipt and the SHA-256 of the raw settlement payload. Reopen validates the same binding and requires the settlement frame to precede the completion frame physically.

The completion coordinator first requires the exact ImportStart, Archive receipt and matching authenticated scoped settlement. It then derives readiness from authoritative owner evidence. Every source-attributed Import Episode must have durable Insomnia `Complete` work and a matching completion output. Missing, Pending/Processing, Failed/Terminal, conflicting or unsupported evidence leaves the epoch open and fenced with an `ImportReadinessBlocker { owner, identity, status }`. For every nonzero local output ID it requires an exact retained same-epoch InsomniaCompletion record, a revision-1 Memory record with the same Episode source attribution, and a matching retained first mutation; IDs alone or caller-supplied Memory provenance do not prove publication. Missing records or mismatched identity are fail-closed `Memory/ConflictingEvidence`. Current, non-archived imported first-revision births must have accepted initial Dream settlement, including superseded births that remain non-archived. Archived and terminal Memories without an outstanding pass follow the Dream owner's eligibility exclusion; an existing import-owned pending pass still blocks completion. Active, non-superseded Dream-processed output Memory IDs contribute their persisted entity-mention keys; each key must have durable terminal `Resolved` or `Rejected` resolution. Dormant/unresolved keys block. Only inactive, archived or superseded Memories have no mention obligation. This obligation set is derived from exact import outputs, so unrelated queued Insomnia/Dream/Perception work does not block completion. A zero-output completion proves no import-created Memory or mention work for that Episode; this requires a successful completion receipt, not an aggregate counter or unchanged watermark.

Any `InsomniaCompletion.external_memory_refs` currently returns a typed `Memory/Unsupported` blocker: the REL completion API has no Phylactery handle and cannot authenticate routed user-owned Memory or its downstream state. The import path schedules no Ego mutations; Ego version changes or missing Ego documents are therefore not completion blockers. Vector status is `NotRequired` only when there are no source Nodes and no Memory outputs; otherwise it is `SupportedUnavailable` and does not certify complete vector coverage. Historical Freshness remains required as the authenticated semantic settlement. A zero-source, zero-output receipt is ready only with an authenticated true no-op settlement: source Nodes, births, cohort, replacement records and superseded IDs are all empty and `base_cut == settlement_cut`. A Dream pass already pending for an import-owned birth remains a blocker even if that Memory becomes archived or terminal; the Dream owner does not start new passes for archived births. Missing, pending, failed and conflicting evidence leaves the epoch open and ordinary writes fenced. Focused readiness coverage includes nonzero import births, affected existing recipients, unrelated owner work, PHY unsupported output, pending and terminal-failed Insomnia work, and pending Dream work. Completion append is coordinator-only in production; the direct store seam is compiled only for tests. No automatic Freshness reconstruction is performed by completion. The marker is synchronized before ordinary write admission opens. After uncertain append/sync failure, a retry rescans physical markers and reuses an already written marker instead of appending another. Storage format and semantic clocks are unchanged. Mounted completion is unsupported; mounted callers must not bypass the existing REL mutation gate. Full-suite acceptance is reported separately.

Import Episode coverage belongs to Archive and is per immutable source Node across branches. Already covered shared prefixes are skipped; contiguous uncovered ranges use the existing response-cycle and byte packing policy. Native continuation of an imported conversation reuses that branch-aware coverage with Live origin/priority; imported alternative paths cannot block its finalization. Conversations without import history retain their sequential scheduling policy. Queue identity and completed/leased work remain Insomnia-owned.

Pending reasons are `MissingReceipt`, `MissingFreshnessSettlement` and `Readiness(ImportReadinessBlocker)`. Readiness blockers name an `ImportReadinessOwner`, an exact owner-local identity and an `ImportReadinessStatus` (`Missing`, `Pending`, `Failed`, `ConflictingEvidence` or `Unsupported`). PHY references currently produce `Memory/Unsupported`. An accepted completion retry synchronizes and returns the original marker with `committed: false`, preserving its time and post cuts even after later activity. Retrying an older completed import cannot release a newer open epoch's fence. Unknown IDs and pre-start completion times reject without appending. A partial or corrupt physical frame remains an error rather than authorizing another append.

The operation is resumable, not an atomic whole-batch transaction. Failure before receipt acknowledgement may leave durable source/branch/Episode/queue progress. Retry the identical normalized input after reopen. Existing immutable IDs and Node coverage recover every tested handoff cut without duplicate work. A synchronized receipt acknowledges the full batch; exact accepted retry returns that receipt without advancing clocks, restoring old titles or rewinding later native branches.

`CVAIMPT1` is an Archive-versioned receipt record. Cold rebuild, semantic reconciliation and physical reclamation preserve it and validate native Node/branch/file references. Import/CVA separately owns the epoch metadata in ImportEpochStore. There is no second work queue, semantic clock or container-wide manifest.

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
- [P5-G specification](multiplexing-implementation-plan.md)
- [External conversation index plan](conversation-index-integration-plan.md)

## Notes

The supported native import contract does not imply mounted F5 integration, provider-private reconstruction, current-source synchronization, arbitrary Codex rollout support or external-derived indexing.

## Historical production scope

The [C12 production disposition](freshness-historical-production-disposition-2026-10-13.md) marks H1–H8 complete and merge-ready as supported explicit, offline/exclusive single-owner REL reconstruction. Historical reconstruction intentionally remains an offline/backfill operation rather than automatic or mounted migration. Live F8 is intrinsic REL behavior with no activation switch. PHY Freshness remains outside this release.
